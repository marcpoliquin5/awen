"""AWEN-Photon noise-injection training on MNIST.

Trains a small MLP whose GEMMs see the AWEN photonic noise model during
training (noise-aware) or not (standard), then evaluates every trained network
on the authoritative Rust simulator (``awen_native.photonic_gemm``, AEP-0022)
across a sweep of optical operating points. It writes accuracy-vs-ENOB and
accuracy-vs-optical-energy curves.

Fidelity: every photonic number here is **simulated** with the AWEN compact
noise model. Optical energy per MAC is **modeled** as
``photons_per_mac * h * c / wavelength`` at the modulator output; it excludes
laser wall-plug efficiency, modulator drive, DAC/ADC and memory energy. No
number here is a hardware measurement.

The training-time surrogate (:class:`PhotonicLinearFunction`) reproduces the
Rust model's tiling, per-vector DAC scaling and quantization, shot, thermal and
RIN noise and ADC quantization, with straight-through gradients for the
quantizers. Phase error and crosstalk are applied only at evaluation time (by
the Rust simulator). ``tests/test_photon_training.py`` checks that the
surrogate's error statistics agree with the Rust simulator.

Example::

    python -m awen_models.photon_training --seeds 1 2 --out results/photon-training

MNIST (LeCun, Cortes, Burges) is distributed under CC BY-SA 3.0; the files are
fetched from the PyTorch mirror and pinned by SHA-256.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import math
import time
import urllib.request
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any, Optional, Sequence

import numpy as np
import torch
from torch import nn

from .data import default_cache_dir
from .train import hardware_record

PLANCK = 6.62607015e-34
LIGHT_SPEED = 299_792_458.0
DEFAULT_WAVELENGTH_M = 1550e-9

MNIST_MIRROR = "https://ossci-datasets.s3.amazonaws.com/mnist/"
MNIST_FILES = {
    "train-images-idx3-ubyte.gz": "440fcabf73cc546fa21475e81ea370265605f56be210a4024d2ca8f203523609",
    "train-labels-idx1-ubyte.gz": "3552534a0a558bbed6aed32b30c495cca23d567ec52cac8be1a0730e8010255c",
    "t10k-images-idx3-ubyte.gz": "8d422c7b0a1c1c79245a5bcf07fe86e33eeafee792b84584aec276f5a2dbc4e6",
    "t10k-labels-idx1-ubyte.gz": "f7ae60f92e00ec6debd23a6088c31dbd2371eca3ffa0defaefb259924204aec6",
}
MNIST_LICENSE = "MNIST (LeCun, Cortes, Burges), CC BY-SA 3.0"


# --------------------------------------------------------------------------- data


def _fetch(name: str, cache: Path) -> bytes:
    path = cache / name
    if not path.exists():
        with urllib.request.urlopen(MNIST_MIRROR + name, timeout=60) as response:
            payload = response.read()
        if hashlib.sha256(payload).hexdigest() != MNIST_FILES[name]:
            raise ValueError(f"MNIST checksum mismatch for {name}")
        path.write_bytes(payload)
    payload = path.read_bytes()
    if hashlib.sha256(payload).hexdigest() != MNIST_FILES[name]:
        raise ValueError(f"cached {path} is corrupt; delete it and retry")
    return gzip.decompress(payload)


def _parse_idx(raw: bytes) -> np.ndarray:
    dims = raw[3]
    shape = tuple(int.from_bytes(raw[4 + 4 * i : 8 + 4 * i], "big") for i in range(dims))
    return np.frombuffer(raw, dtype=np.uint8, offset=4 + 4 * dims).reshape(shape)


def load_mnist(cache_dir: str | Path | None = None) -> dict[str, torch.Tensor]:
    """MNIST as float tensors in [0, 1] (flattened) and int64 labels."""
    cache = Path(cache_dir) if cache_dir else default_cache_dir() / "mnist"
    cache.mkdir(parents=True, exist_ok=True)
    arrays = {name: _parse_idx(_fetch(name, cache)) for name in MNIST_FILES}
    as_x = lambda a: torch.from_numpy(a.reshape(a.shape[0], -1).astype(np.float32) / 255.0)  # noqa: E731
    as_y = lambda a: torch.from_numpy(a.astype(np.int64))  # noqa: E731
    return {
        "train_x": as_x(arrays["train-images-idx3-ubyte.gz"]),
        "train_y": as_y(arrays["train-labels-idx1-ubyte.gz"]),
        "test_x": as_x(arrays["t10k-images-idx3-ubyte.gz"]),
        "test_y": as_y(arrays["t10k-labels-idx1-ubyte.gz"]),
    }


# ----------------------------------------------------------------- noise surrogate


def contract_with_photons(base: dict[str, Any], photons_per_mac: Optional[float]) -> dict[str, Any]:
    contract = dict(base)
    contract["photons_per_mac"] = photons_per_mac
    return contract


def optical_energy_per_mac_j(photons_per_mac: float, wavelength_m: float = DEFAULT_WAVELENGTH_M) -> float:
    """Modeled optical energy per MAC at the modulator output (photon energy only)."""
    return photons_per_mac * PLANCK * LIGHT_SPEED / wavelength_m


def _quantize_ste(x: torch.Tensor, bits: Optional[int]) -> torch.Tensor:
    if bits is None:
        return x
    levels = float((1 << (bits - 1)) - 1)
    clamped = x.clamp(-1.0, 1.0)
    quantized = torch.round(clamped * levels) / levels
    return clamped + (quantized - clamped).detach()


def surrogate_photonic_linear(
    x: torch.Tensor,
    weight: torch.Tensor,
    contract: dict[str, Any],
    tile_k: int = 64,
    generator: Optional[torch.Generator] = None,
) -> torch.Tensor:
    """Differentiable ``x @ weight.T`` with the AEP-0022 tile model (training surrogate).

    Matches ``awen_compiler::noise::photonic_gemm`` in tiling, per-vector DAC
    scaling, DAC/ADC quantization and shot/thermal/RIN noise. Quantizers use
    straight-through gradients; the per-vector scales are treated as constants.
    """
    rows, inner = x.shape
    transmission = 10.0 ** (-float(contract.get("insertion_loss_db", 0.0)) / 10.0)
    qe = float(contract.get("quantum_efficiency", 1.0))
    photons = contract.get("photons_per_mac")
    thermal_e = float(contract.get("thermal_noise_electrons", 0.0))
    rin_db = contract.get("rin_db_per_hz")
    rin_rel = math.sqrt(10.0 ** (rin_db / 10.0) * float(contract.get("detection_bandwidth_hz", 0.0))) if rin_db is not None else None
    dac, adc = contract.get("dac_bits"), contract.get("adc_bits")
    out = x.new_zeros(rows, weight.shape[0])
    for k0 in range(0, inner, tile_k):
        xt, wt = x[:, k0 : k0 + tile_k], weight[:, k0 : k0 + tile_k]
        k = xt.shape[1]
        sx = xt.detach().abs().amax(dim=1, keepdim=True)
        sw = wt.detach().abs().amax(dim=1, keepdim=True)
        xn = _quantize_ste(torch.where(sx > 0, xt / sx.clamp_min(1e-30), torch.zeros_like(xt)), dac)
        wn = _quantize_ste(torch.where(sw > 0, wt / sw.clamp_min(1e-30), torch.zeros_like(wt)), dac)
        z = xn @ wn.T / k
        noise_terms = []
        if rin_rel is not None:
            noise_terms.append(z.detach().abs() * rin_rel)
        if photons is not None:
            n_fs = float(photons) * k * transmission * qe
            sigma = math.sqrt(1.0 / n_fs + (thermal_e / n_fs) ** 2)
            noise_terms.append(torch.full_like(z, sigma))
        if noise_terms:
            std = torch.sqrt(sum(t * t for t in noise_terms))
            z = z + std * torch.randn(z.shape, generator=generator, dtype=z.dtype, device=z.device)
        z = _quantize_ste(z, adc)
        out = out + z * k * sx * sw.T
    return out


class PhotonicMLP(nn.Module):
    """784 -> hidden -> 10 MLP whose two GEMMs can run on the noise surrogate or the Rust simulator."""

    def __init__(self, hidden: int = 128):
        super().__init__()
        self.fc1 = nn.Linear(784, hidden)
        self.fc2 = nn.Linear(hidden, 10)
        self.hidden = hidden

    def forward(
        self,
        x: torch.Tensor,
        contract: Optional[dict[str, Any]] = None,
        generator: Optional[torch.Generator] = None,
        tile_k: int = 64,
    ) -> torch.Tensor:
        def gemm(inp: torch.Tensor, layer: nn.Linear) -> torch.Tensor:
            if contract is None:
                return inp @ layer.weight.T + layer.bias
            return surrogate_photonic_linear(inp, layer.weight, contract, tile_k, generator) + layer.bias

        return gemm(torch.relu(gemm(x, self.fc1)), self.fc2)

    @torch.no_grad()
    def forward_rust(self, x: torch.Tensor, contract: dict[str, Any], seed: int, tile_k: int = 64) -> torch.Tensor:
        """Inference with both GEMMs on ``awen_native.photonic_gemm`` (biases and ReLU stay electronic)."""
        import awen_native

        def gemm(inp: np.ndarray, layer: nn.Linear, s: int) -> np.ndarray:
            w = np.ascontiguousarray(layer.weight.detach().double().numpy())
            y, _ = awen_native.photonic_gemm(np.ascontiguousarray(inp), w, contract, tile_k=tile_k, seed=s)
            return y + layer.bias.detach().double().numpy()

        h = np.maximum(gemm(x.double().numpy(), self.fc1, seed), 0.0)
        return torch.from_numpy(gemm(h, self.fc2, seed + 1))


# ----------------------------------------------------------------------- training


@dataclass
class PhotonTrainSettings:
    hidden: int = 128
    epochs: int = 3
    batch_size: int = 128
    lr: float = 2e-3
    tile_k: int = 64
    train_limit: Optional[int] = None
    test_limit: Optional[int] = None


@dataclass
class TrainingRegime:
    """``photons_per_mac=None`` with ``noisy=False`` is standard (noise-free) training."""

    name: str
    noisy: bool
    photons_per_mac: Optional[float] = None


DEFAULT_REGIMES = [
    TrainingRegime("standard", noisy=False),
    TrainingRegime("noise-aware-100", noisy=True, photons_per_mac=100.0),
    TrainingRegime("noise-aware-1000", noisy=True, photons_per_mac=1000.0),
]
DEFAULT_SWEEP = [10.0, 30.0, 100.0, 300.0, 1000.0, 3000.0, 10000.0, 100000.0]


def train_mlp(
    data: dict[str, torch.Tensor],
    regime: TrainingRegime,
    base_contract: dict[str, Any],
    settings: PhotonTrainSettings,
    seed: int,
) -> tuple[PhotonicMLP, dict[str, Any]]:
    torch.manual_seed(seed)
    gen = torch.Generator().manual_seed(seed)
    model = PhotonicMLP(settings.hidden)
    opt = torch.optim.Adam(model.parameters(), lr=settings.lr)
    x, y = data["train_x"], data["train_y"]
    if settings.train_limit:
        x, y = x[: settings.train_limit], y[: settings.train_limit]
    contract = contract_with_photons(base_contract, regime.photons_per_mac) if regime.noisy else None
    losses = []
    start = time.perf_counter()
    for _ in range(settings.epochs):
        order = torch.randperm(len(x), generator=gen)
        for i in range(0, len(x), settings.batch_size):
            idx = order[i : i + settings.batch_size]
            logits = model(x[idx], contract, gen, settings.tile_k)
            loss = nn.functional.cross_entropy(logits, y[idx])
            opt.zero_grad()
            loss.backward()
            opt.step()
            losses.append(float(loss.detach()))
    return model, {"train_seconds": time.perf_counter() - start, "final_train_loss": float(np.mean(losses[-50:]))}


def evaluate_rust(
    model: PhotonicMLP,
    x: torch.Tensor,
    y: torch.Tensor,
    contract: dict[str, Any],
    seed: int,
    tile_k: int = 64,
    chunk: int = 2000,
) -> float:
    correct = 0
    for i in range(0, len(x), chunk):
        logits = model.forward_rust(x[i : i + chunk], contract, seed + 2 * (i // chunk), tile_k)
        correct += int((logits.argmax(dim=1) == y[i : i + chunk]).sum())
    return correct / len(x)


def wilson_interval(successes: int, n: int, z: float = 1.96) -> tuple[float, float]:
    """95% Wilson score interval for a binomial proportion."""
    if n == 0:
        return (0.0, 1.0)
    p = successes / n
    denom = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / denom
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / denom
    return (max(0.0, centre - half), min(1.0, centre + half))


def run(
    seeds: Sequence[int],
    out: str | Path,
    settings: Optional[PhotonTrainSettings] = None,
    regimes: Sequence[TrainingRegime] = DEFAULT_REGIMES,
    sweep: Sequence[float] = DEFAULT_SWEEP,
    data: Optional[dict[str, torch.Tensor]] = None,
    plot: bool = True,
) -> dict[str, Any]:
    import awen_native

    settings = settings or PhotonTrainSettings()
    data = data or load_mnist()
    base = awen_native.default_contract()
    tx, ty = data["test_x"], data["test_y"]
    if settings.test_limit:
        tx, ty = tx[: settings.test_limit], ty[: settings.test_limit]
    points = []
    for p in sweep:
        contract = contract_with_photons(base, p)
        budget = awen_native.predicted_noise(settings.tile_k, 1.0, contract)
        points.append({
            "photons_per_mac": p,
            "tile_effective_bits": budget["effective_bits"],
            "optical_energy_per_mac_j": optical_energy_per_mac_j(p),
        })
    runs = []
    for regime in regimes:
        for seed in seeds:
            model, info = train_mlp(data, regime, base, settings, seed)
            ideal = evaluate_rust(model, tx, ty, awen_native.ideal_contract(), seed, settings.tile_k)
            accuracies = []
            for point in points:
                acc = evaluate_rust(model, tx, ty, contract_with_photons(base, point["photons_per_mac"]), 10_000 + seed, settings.tile_k)
                lo, hi = wilson_interval(round(acc * len(tx)), len(tx))
                accuracies.append({**point, "accuracy": acc, "ci95": [lo, hi]})
            runs.append({"regime": asdict(regime), "seed": seed, **info, "ideal_accuracy": ideal, "sweep": accuracies})
    summary: dict[str, Any] = {}
    for regime in regimes:
        rs = [r for r in runs if r["regime"]["name"] == regime.name]
        summary[regime.name] = {
            "ideal_accuracy_mean": float(np.mean([r["ideal_accuracy"] for r in rs])),
            "sweep_accuracy_mean": [float(np.mean([r["sweep"][i]["accuracy"] for r in rs])) for i in range(len(points))],
            "sweep_accuracy_min": [float(min(r["sweep"][i]["accuracy"] for r in rs)) for i in range(len(points))],
            "sweep_accuracy_max": [float(max(r["sweep"][i]["accuracy"] for r in rs)) for i in range(len(points))],
        }
    report = {
        "version": "awen.photon-noise-training.v1",
        "fidelity": "simulated",
        "evidence": (
            "Accuracies are measured on the MNIST test set with GEMMs executed by the AWEN Rust compact noise model "
            "(simulated, not hardware). Optical energy per MAC is modeled photon energy only."
        ),
        "dataset": {"name": "mnist", "license": MNIST_LICENSE, "test_examples": len(tx), "files": MNIST_FILES},
        "settings": asdict(settings),
        "base_contract": base,
        "wavelength_m": DEFAULT_WAVELENGTH_M,
        "seeds": list(seeds),
        "points": points,
        "runs": runs,
        "summary": summary,
        "hardware": hardware_record(torch.device("cpu")),
    }
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    if plot:
        _plot(report, out)
    return report


def _plot(report: dict[str, Any], out: Path) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    points = report["points"]
    enob = [p["tile_effective_bits"] for p in points]
    energy = [p["optical_energy_per_mac_j"] * 1e18 for p in points]
    fig, axes = plt.subplots(1, 2, figsize=(11, 4.2))
    for name, s in report["summary"].items():
        mean = np.array(s["sweep_accuracy_mean"]) * 100
        lo = np.array(s["sweep_accuracy_min"]) * 100
        hi = np.array(s["sweep_accuracy_max"]) * 100
        for ax, xs in ((axes[0], enob), (axes[1], energy)):
            ax.plot(xs, mean, marker="o", label=name)
            ax.fill_between(xs, lo, hi, alpha=0.2)
    axes[0].set_xlabel("tile effective bits (ENOB, analytic, k=%d)" % report["settings"]["tile_k"])
    axes[1].set_xlabel("optical energy per MAC (aJ, modeled photon energy)")
    axes[1].set_xscale("log")
    for ax in axes:
        ax.set_ylabel("MNIST test accuracy (%)")
        ax.grid(alpha=0.3)
        ax.legend()
    fig.suptitle("AWEN-Photon MLP on the AWEN photonic noise model (simulated; seed min-max band)")
    fig.tight_layout()
    fig.savefig(out / "accuracy_curves.png", dpi=120)
    plt.close(fig)


def main(argv: Optional[Sequence[str]] = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--seeds", type=int, nargs="+", default=[1, 2])
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--hidden", type=int, default=128)
    parser.add_argument("--out", default="results/photon-training")
    args = parser.parse_args(argv)
    report = run(args.seeds, args.out, PhotonTrainSettings(hidden=args.hidden, epochs=args.epochs))
    for name, s in report["summary"].items():
        print(name, "ideal", round(s["ideal_accuracy_mean"], 4), "sweep", [round(a, 4) for a in s["sweep_accuracy_mean"]])


if __name__ == "__main__":
    main()
