"""AWEN-Photon sim-to-real control: an independent second noise model ("model B").

E-005 trains the MNIST MLP against the AWEN compact noise model (model A: the
Rust ``awen_native.photonic_gemm`` and its torch surrogate). Master Prompt v2
section 7 asks for evaluation under a *different, independently parameterized*
noise model. Model B is that model. It shares no code, no random number
generator and no parameter with model A:

* parameters are read at run time from ``awen-spec/hardware_assumptions.yaml``
  (each with its source key and source type), never hard-coded here;
* system noise is one output-referred Gaussian term set by the measured
  whole-system effective bits (``system.pace_enob``), not by photon counts;
* weights are quantized to ``dac.weight_dac_bits`` with one scale per weight
  tile (model A scales per row) on a uniform grid of ``2**bits`` levels;
* inputs are intensities with finite extinction ratio
  (``modulator.tfln_extinction_ratio``): a zero input still leaks a fraction
  ``10**(-ER/10)`` of full scale; signed inputs use two passes that are
  subtracted; weights are signed by balanced differential detection;
* the RNG is NumPy PCG64 (``numpy.random.default_rng``).

Not modeled (no checked source in the assumption set): phase error, crosstalk,
drift, separate input-DAC and ADC quantization (the system ENOB is a lumped
whole-channel figure that already includes them). Everything here is a
simulation (L1); no number is a hardware measurement and none is an energy or
speed figure.

The registration (hypothesis, seeds, verdict rule) is in
``research/experiments/awen-photon-sim2real/PREREGISTRATION.md``. Example::

    python -m awen_models.sim2real --seeds 1 2 3 --out results/photon-sim2real
"""

from __future__ import annotations

import argparse
import json
import math
from dataclasses import asdict, dataclass, replace
from pathlib import Path
from typing import Any, Optional, Sequence

import numpy as np
import torch

# Where each model-B parameter lives in hardware_assumptions.yaml: (group, name, field).
ASSUMPTION_KEYS = {
    "system_enob": ("system", "pace_enob", "typical"),
    "weight_dac_bits": ("dac", "weight_dac_bits", "typical"),
    "extinction_ratio_db": ("modulator", "tfln_extinction_ratio", "typical"),
}
REGISTERED_ENOB_SWEEP = [3.0, 4.0, 5.0, 6.0, None, 10.0]  # None = the sourced value (primary point)
REGISTERED_SEEDS = [1, 2, 3]
EVAL_SEED_OFFSET = 20_000
T_975 = {2: 12.706, 3: 4.303, 4: 3.182, 5: 2.776}  # two-sided 95% Student t, by number of seeds


@dataclass(frozen=True)
class NoiseModelB:
    system_enob: float
    weight_dac_bits: int
    extinction_ratio_db: float
    provenance: tuple = ()

    @property
    def leakage(self) -> float:
        return 10.0 ** (-self.extinction_ratio_db / 10.0)

    @property
    def output_sigma(self) -> float:
        """Noise rms on the normalized tile output in [-1, 1]: an ideal b-bit quantizer over a range of 2."""
        return 2.0 ** (1.0 - self.system_enob) / math.sqrt(12.0)

    def with_enob(self, enob: Optional[float]) -> "NoiseModelB":
        if enob is None:
            return self
        return replace(self, system_enob=float(enob), provenance=self.provenance + (("system_enob", "free parameter (registered sweep)"),))


def find_assumptions(start: Optional[Path] = None) -> Path:
    for parent in [(start or Path.cwd()).resolve(), *(start or Path.cwd()).resolve().parents]:
        candidate = parent / "awen-spec" / "hardware_assumptions.yaml"
        if candidate.exists():
            return candidate
    raise FileNotFoundError("awen-spec/hardware_assumptions.yaml not found; pass --assumptions")


def load_model_b(path: str | Path) -> NoiseModelB:
    """Read model B's parameters from the assumption set; refuse null or unsourced values."""
    import yaml  # pyyaml, see the `test` / `assumptions` extras

    doc = yaml.safe_load(Path(path).read_text())
    values, provenance = {}, []
    for field, (group, name, key) in ASSUMPTION_KEYS.items():
        entry = doc["parameters"][group][name]
        value, source = entry.get(key), entry.get("source")
        if value is None or not source:
            raise ValueError(f"{group}.{name}.{key} has no sourced value; model B cannot use it")
        source_type = entry.get("source_type") or doc["sources"][source]["type"]
        values[field] = value
        provenance.append((field, f"{group}.{name}.{key}={value} {entry['unit']} ({source}, {source_type})"))
    provenance.append(("assumption_set", str(doc["version"])))
    return NoiseModelB(float(values["system_enob"]), int(values["weight_dac_bits"]), float(values["extinction_ratio_db"]), tuple(provenance))


def _quantize_uniform(w: np.ndarray, bits: int) -> np.ndarray:
    """Uniform grid of 2**bits levels spanning [-1, 1] (both ends included)."""
    step = 2.0 / ((1 << bits) - 1)
    return np.clip(np.round((w + 1.0) / step) * step - 1.0, -1.0, 1.0)


def gemm_b(x: np.ndarray, w: np.ndarray, model: NoiseModelB, rng: np.random.Generator, tile: int = 64) -> np.ndarray:
    """``x @ w.T`` (x: rows x inner, w: out x inner) under model B."""
    x, w = np.asarray(x, dtype=np.float64), np.asarray(w, dtype=np.float64)
    eps, sigma = model.leakage, model.output_sigma
    out = np.zeros((x.shape[0], w.shape[0]))
    for k0 in range(0, x.shape[1], tile):
        xt, wt = x[:, k0 : k0 + tile], w[:, k0 : k0 + tile]
        k = xt.shape[1]
        sw = float(np.abs(wt).max())
        wn = _quantize_uniform(wt / sw, model.weight_dac_bits) if sw > 0 else np.zeros_like(wt)
        sx = np.abs(xt).max(axis=1, keepdims=True)
        xn = np.divide(xt, sx, out=np.zeros_like(xt), where=sx > 0)
        pos, neg = np.maximum(xn, 0.0), np.maximum(-xn, 0.0)
        leak = eps * wn.sum(axis=1)[None, :]
        dot = (1.0 - eps) * pos @ wn.T + leak
        signed = (neg > 0).any(axis=1, keepdims=True)
        dot = dot - np.where(signed, (1.0 - eps) * neg @ wn.T + leak, 0.0)
        z = dot / k + sigma * rng.standard_normal(dot.shape)
        out += z * k * sx * sw
    return out


@torch.no_grad()
def forward_b(mlp: Any, x: torch.Tensor, model: NoiseModelB, seed: int, tile: int = 64) -> torch.Tensor:
    """Inference of a ``PhotonicMLP`` with both GEMMs under model B (biases and ReLU electronic)."""
    rng = np.random.default_rng(seed)

    def layer(inp: np.ndarray, lin: Any) -> np.ndarray:
        return gemm_b(inp, lin.weight.detach().double().numpy(), model, rng, tile) + lin.bias.detach().double().numpy()

    h = np.maximum(layer(x.double().numpy(), mlp.fc1), 0.0)
    return torch.from_numpy(layer(h, mlp.fc2))


def accuracy_b(mlp: Any, x: torch.Tensor, y: torch.Tensor, model: NoiseModelB, seed: int, tile: int = 64) -> float:
    return float((forward_b(mlp, x, model, seed, tile).argmax(dim=1) == y).double().mean())


def paired_verdict(diffs: Sequence[float]) -> dict[str, Any]:
    """Registered rule: 95% t interval on per-seed paired differences (noise-aware minus standard)."""
    d = np.asarray(diffs, dtype=np.float64)
    half = T_975[len(d)] * float(d.std(ddof=1)) / math.sqrt(len(d))
    lo, hi = float(d.mean()) - half, float(d.mean()) + half
    verdict = "transfers" if lo > 0 else "reverses" if hi < 0 else "no detectable difference"
    return {"mean": float(d.mean()), "ci95": [lo, hi], "n": len(d), "verdict": verdict}


def run(assumptions: str | Path, seeds: Sequence[int], out: str | Path, settings: Any = None, data: Any = None,
        regimes: Any = None, enob_sweep: Sequence[Optional[float]] = REGISTERED_ENOB_SWEEP) -> dict[str, Any]:
    """Retrain the E-005 regimes (model A surrogate) and evaluate each under model A and model B."""
    import awen_native

    from .photon_training import DEFAULT_REGIMES, PhotonTrainSettings, evaluate_rust, load_mnist, train_mlp

    settings, regimes = settings or PhotonTrainSettings(), regimes or DEFAULT_REGIMES
    data = data or load_mnist()
    base = load_model_b(assumptions)
    tx, ty = data["test_x"], data["test_y"]
    contract_a = awen_native.default_contract()
    points = [base.with_enob(e) for e in enob_sweep]
    runs = []
    for regime in regimes:
        for seed in seeds:
            mlp, info = train_mlp(data, regime, contract_a, settings, seed)
            acc_a = evaluate_rust(mlp, tx, ty, contract_a, 10_000 + seed, settings.tile_k)
            acc_b = [accuracy_b(mlp, tx, ty, p, EVAL_SEED_OFFSET + seed, settings.tile_k) for p in points]
            runs.append({"regime": regime.name, "seed": seed, **info, "accuracy_model_a": acc_a, "accuracy_model_b": acc_b})
    by = {(r["regime"], r["seed"]): r for r in runs}
    std = regimes[0].name
    verdicts = {
        f"{reg.name} - {std}": [
            paired_verdict([by[(reg.name, s)]["accuracy_model_b"][i] - by[(std, s)]["accuracy_model_b"][i] for s in seeds])
            for i in range(len(points))
        ]
        for reg in regimes[1:]
    } if len(seeds) >= 2 else {}
    report = {
        "version": "awen.photon-sim2real.v1",
        "fidelity": "simulated",
        "model_a_contract": contract_a,
        "model_b_points": [{**asdict(p), "provenance": [list(t) for t in p.provenance], "output_sigma": p.output_sigma} for p in points],
        "primary_point_index": list(enob_sweep).index(None) if None in enob_sweep else None,
        "settings": asdict(settings),
        "seeds": list(seeds),
        "test_examples": len(tx),
        "runs": runs,
        "verdicts_model_b": verdicts,
    }
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def main(argv: Optional[Sequence[str]] = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--assumptions", default=None, help="path to awen-spec/hardware_assumptions.yaml")
    parser.add_argument("--seeds", type=int, nargs="+", default=REGISTERED_SEEDS)
    parser.add_argument("--out", default="results/photon-sim2real")
    args = parser.parse_args(argv)
    report = run(args.assumptions or find_assumptions(), args.seeds, args.out)
    for name, rows in report["verdicts_model_b"].items():
        print(name, [(round(v["mean"], 4), v["verdict"]) for v in rows])


if __name__ == "__main__":
    main()
