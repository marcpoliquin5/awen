"""AWEN-Photon accuracy versus recalibration interval (E-023, registered, not run).

Question: how much weight drift between calibrations can the E-005 MNIST MLP take before its accuracy under the
independent noise model B (``awen_models.sim2real``, sourced primary point) drops by more than one percentage point,
and does noise-aware training change that?

Drift model (no checked source for any drift rate, so every drift value here is a labelled free parameter):

* after a calibration each programmed weight (normalized units, [-1, 1] per weight tile) drifts as an independent
  Gaussian random walk; ``budget`` ``s`` is the rms offset accumulated over one full recalibration interval;
* a recalibration resets the offset to zero, so the time-averaged accuracy over an interval is estimated with the
  midpoint rule at ``K`` evenly spaced times ``t/T = (j + 0.5) / K`` along one random-walk path;
* for a drift rate ``D`` (rms per square root of unit time) the interval is ``T = (s / D) ** 2``
  (``interval_for_budget``). No sourced ``D`` exists, so results are reported against ``s``, never in hours.

The registration (grid, seeds, verdict rule) is in
``research/experiments/awen-photon-recalibration/PREREGISTRATION.md``. Everything is a simulation (L1). Example::

    python -m awen_models.recalibration --seeds 1 2 3 --out results/photon-recalibration
"""

from __future__ import annotations

import argparse
import json
import math
from dataclasses import asdict
from pathlib import Path
from typing import Any, Optional, Sequence

import numpy as np
import torch

from .sim2real import EVAL_SEED_OFFSET, REGISTERED_SEEDS, T_975, NoiseModelB, find_assumptions, gemm_b, load_model_b, paired_verdict

REGISTERED_BUDGETS = [0.0, 0.01, 0.02, 0.05, 0.1, 0.2]  # rms drift per interval, normalized weight units (free)
REGISTERED_TIMES = 4  # midpoint-rule evaluation times per interval
TOLERANCE = 0.01  # registered: one percentage point of test accuracy
WALK_SEED_OFFSET = 30_000


def interval_for_budget(budget: float, rate: float) -> float:
    """Recalibration interval (in the units of ``rate``'s time base) at which a random walk reaches rms ``budget``."""
    if rate <= 0:
        raise ValueError("drift rate must be positive")
    return (budget / rate) ** 2


def drift_path(shape: tuple, budget: float, times: int, rng: np.random.Generator) -> list[np.ndarray]:
    """Offsets at ``t/T = (j + 0.5) / times`` along one random walk whose variance at ``t = T`` is ``budget**2``."""
    fractions = [(j + 0.5) / times for j in range(times)]
    steps = np.diff([0.0, *fractions])
    offset, path = np.zeros(shape), []
    for dt in steps:
        offset = offset + budget * math.sqrt(dt) * rng.standard_normal(shape)
        path.append(offset.copy())
    return path


@torch.no_grad()
def accuracy_with_drift(mlp: Any, x: torch.Tensor, y: torch.Tensor, model: NoiseModelB, drifts: Sequence[Optional[np.ndarray]],
                        noise_seed: int, tile: int = 64) -> float:
    """Model-B accuracy with per-layer weight offsets ``drifts`` (fc1, fc2); biases and ReLU stay electronic."""
    rng = np.random.default_rng(noise_seed)

    def layer(inp: np.ndarray, lin: Any, d: Optional[np.ndarray]) -> np.ndarray:
        w = lin.weight.detach().double().numpy()
        return gemm_b(inp, w, model, rng, tile, drift=d) + lin.bias.detach().double().numpy()

    h = np.maximum(layer(x.double().numpy(), mlp.fc1, drifts[0]), 0.0)
    logits = layer(h, mlp.fc2, drifts[1])
    return float((torch.from_numpy(logits).argmax(dim=1) == y).double().mean())


def interval_accuracy(mlp: Any, x: torch.Tensor, y: torch.Tensor, model: NoiseModelB, budget: float, seed: int,
                      budget_index: int, times: int = REGISTERED_TIMES, tile: int = 64) -> dict[str, Any]:
    """Time-averaged accuracy over one interval; the same noise seed at every time, so only the drift differs."""
    noise_seed = EVAL_SEED_OFFSET + seed
    if budget == 0.0:
        acc = accuracy_with_drift(mlp, x, y, model, [None, None], noise_seed, tile)
        return {"budget": budget, "accuracy_at_times": [acc] * times, "accuracy": acc}
    rng = np.random.default_rng(WALK_SEED_OFFSET + 100 * seed + budget_index)
    paths = [drift_path(tuple(lin.weight.shape), budget, times, rng) for lin in (mlp.fc1, mlp.fc2)]
    accs = [accuracy_with_drift(mlp, x, y, model, [paths[0][j], paths[1][j]], noise_seed, tile) for j in range(times)]
    return {"budget": budget, "accuracy_at_times": accs, "accuracy": float(np.mean(accs))}


def tolerable_budget(curves: Sequence[Sequence[float]], budgets: Sequence[float], tolerance: float = TOLERANCE) -> dict[str, Any]:
    """Registered rule. ``curves[s][i]`` = interval accuracy of seed s at budget i (budget 0 first).

    Paired drop ``acc(0) - acc(i)`` per seed, two-sided 95% t interval; a budget passes when the upper bound is at
    most ``tolerance``. The tolerable budget is the largest grid budget such that it and every smaller one pass.
    """
    a = np.asarray(curves, dtype=np.float64)
    n = a.shape[0]
    rows, best = [], budgets[0]
    still_ok = True
    for i, b in enumerate(budgets):
        d = a[:, 0] - a[:, i]
        half = T_975[n] * float(d.std(ddof=1)) / math.sqrt(n) if n > 1 else math.inf
        upper = float(d.mean()) + half
        ok = upper <= tolerance
        still_ok = still_ok and ok
        if still_ok:
            best = b
        rows.append({"budget": b, "mean_drop": float(d.mean()), "upper95": upper, "passes": ok})
    return {"tolerable_budget": best, "rows": rows, "tolerance": tolerance, "n": n}


def run(assumptions: str | Path, seeds: Sequence[int], out: str | Path, settings: Any = None, data: Any = None,
        regimes: Any = None, budgets: Sequence[float] = REGISTERED_BUDGETS, times: int = REGISTERED_TIMES) -> dict[str, Any]:
    """Retrain the E-005 regimes (E-022 code and seeds) and measure the interval-accuracy curve under model B."""
    import awen_native

    from .photon_training import DEFAULT_REGIMES, PhotonTrainSettings, load_mnist, train_mlp

    if list(budgets)[0] != 0.0:
        raise ValueError("the budget grid must start at 0 (the no-drift reference)")
    settings, regimes = settings or PhotonTrainSettings(), regimes or DEFAULT_REGIMES
    data = data or load_mnist()
    model = load_model_b(assumptions)
    tx, ty = data["test_x"], data["test_y"]
    contract_a = awen_native.default_contract()
    runs = []
    for regime in regimes:
        for seed in seeds:
            mlp, info = train_mlp(data, regime, contract_a, settings, seed)
            curve = [interval_accuracy(mlp, tx, ty, model, b, seed, i, times, settings.tile_k) for i, b in enumerate(budgets)]
            runs.append({"regime": regime.name, "seed": seed, **info, "curve": curve})
    by = {(r["regime"], r["seed"]): [p["accuracy"] for p in r["curve"]] for r in runs}
    tolerable = {reg.name: tolerable_budget([by[(reg.name, s)] for s in seeds], budgets) for reg in regimes} if len(seeds) >= 2 else {}
    std = regimes[0].name
    comparisons = {
        f"{reg.name} - {std}": [paired_verdict([by[(reg.name, s)][i] - by[(std, s)][i] for s in seeds]) for i in range(len(budgets))]
        for reg in regimes[1:]
    } if len(seeds) >= 2 else {}
    report = {
        "version": "awen.photon-recalibration.v1",
        "fidelity": "simulated",
        "drift_values": "free parameters (no checked source for any drift rate)",
        "model_b": {**asdict(model), "provenance": [list(t) for t in model.provenance], "output_sigma": model.output_sigma},
        "budgets": list(budgets),
        "times_per_interval": times,
        "settings": asdict(settings),
        "seeds": list(seeds),
        "test_examples": len(tx),
        "runs": runs,
        "tolerable_budget": tolerable,
        "exploratory_regime_comparisons": comparisons,
    }
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def main(argv: Optional[Sequence[str]] = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--assumptions", default=None, help="path to awen-spec/hardware_assumptions.yaml")
    parser.add_argument("--seeds", type=int, nargs="+", default=REGISTERED_SEEDS)
    parser.add_argument("--out", default="results/photon-recalibration")
    args = parser.parse_args(argv)
    report = run(args.assumptions or find_assumptions(), args.seeds, args.out)
    for name, row in report["tolerable_budget"].items():
        print(name, "tolerable budget", row["tolerable_budget"])


if __name__ == "__main__":
    main()
