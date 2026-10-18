"""The Lab Agent: Quick / Truth / Repro / Compare / Publish over the AWEN simulators."""

from __future__ import annotations

import json
import platform
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Mapping, Optional, Sequence

import numpy as np

from . import sims
from .policy import Policy
from .store import LabState, LabStore, canonical, compare_states, content_hash, lineage

AGENT_VERSION = "awen.lab-agent.v1"
GEMM_RATIO_BAND = (0.75, 1.33)


def _git_sha() -> Optional[str]:
    try:
        return subprocess.run(["git", "rev-parse", "HEAD"], cwd=Path(__file__).resolve().parent,
                              capture_output=True, text=True, check=True).stdout.strip() or None
    except (OSError, subprocess.CalledProcessError):
        return None


def _bootstrap_mean_ci(values: Sequence[float], seed: int = 0, resamples: int = 2000) -> list[float]:
    data = np.asarray(values, dtype=float)
    if data.size < 2:
        return [float(data.mean()), float(data.mean())]
    idx = np.random.default_rng(seed).integers(0, data.size, (resamples, data.size))
    means = data[idx].mean(axis=1)
    return [float(np.quantile(means, 0.025)), float(np.quantile(means, 0.975))]


class LabAgent:
    def __init__(self, store: LabStore | str | Path, policy: Optional[Policy] = None, branch: str = "main"):
        self.store = store if isinstance(store, LabStore) else LabStore(store)
        self.policy = policy or Policy()
        self.branch = branch

    # -- helpers -------------------------------------------------------------
    def _head(self, branch: str) -> Optional[str]:
        return self.store.branches().get(branch)

    def _provenance(self) -> dict[str, Any]:
        return {
            "agent": AGENT_VERSION,
            "created_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "git_sha": _git_sha(),
            "python": platform.python_version(),
            "packages": sims.versions(),
            "operating_bounds": self.policy.bounds.to_dict(),
            "evidence": "simulation only; no hardware was involved",
        }

    def _commit(self, workflow: str, experiment: Optional[Mapping[str, Any]], metrics: Mapping[str, Any],
                parents: Sequence[str], branch: Optional[str], extra: Optional[Mapping[str, Any]] = None) -> LabState:
        doc: dict[str, Any] = {
            "version": AGENT_VERSION,
            "workflow": workflow,
            "parents": [p for p in parents if p],
            "experiment": dict(experiment) if experiment is not None else None,
            "metrics": dict(metrics),
            "provenance": self._provenance(),
            "policy_audit": list(self.policy.audit[-20:]),
        }
        if extra:
            doc.update(extra)
        state = LabState.create(doc)
        self.store.put(state)
        if branch:
            self.store.set_branch(branch, state.hash)
        return state

    def _runs(self, experiment: Mapping[str, Any]) -> list[dict[str, Any]]:
        self.policy.check_simulation(experiment)
        return [{"seed": int(s), **sims.run(experiment, int(s))} for s in experiment.get("seeds", [0])]

    @staticmethod
    def _summarize(runs: list[dict[str, Any]], key: str) -> dict[str, Any]:
        values = [r[key] for r in runs]
        return {"mean": float(np.mean(values)), "std": float(np.std(values, ddof=1)) if len(values) > 1 else 0.0,
                "ci95": _bootstrap_mean_ci(values), "n": len(values)}

    @staticmethod
    def _primary(kind: str) -> str:
        return "relative_rms_error" if kind == "photonic_gemm" else "total_variation_vs_exact"

    # -- workflows -----------------------------------------------------------
    def quick(self, experiment: Mapping[str, Any], branch: Optional[str] = None) -> LabState:
        """One seed, fast feedback."""
        exp = {**experiment, "seeds": list(experiment.get("seeds", [0]))[:1]}
        runs = self._runs(exp)
        branch = branch or self.branch
        return self._commit("quick", exp, {"runs": runs}, [self._head(branch)], branch)

    def truth(self, experiment: Mapping[str, Any], branch: Optional[str] = None) -> LabState:
        """Check the simulator against exact oracles and analytic predictions."""
        checks: list[dict[str, Any]] = []
        kind = experiment["kind"]
        if kind == "photonic_gemm":
            ideal = {**experiment, "target": "awen-reference", "contract": {}}
            ideal_runs = self._runs(ideal)
            worst = max(r["max_abs_error"] for r in ideal_runs)
            checks.append({"check": "ideal contract equals float64 oracle", "value": worst,
                           "threshold": 1e-9, "passed": worst < 1e-9})
            if experiment["target"] == "photonic-sim":
                runs = self._runs(experiment)
                ratio = float(np.mean([r["rms_error"] / r["predicted_rms_error"] for r in runs]))
                lo, hi = GEMM_RATIO_BAND
                checks.append({"check": "measured / analytic RMS error (noise budget)", "value": ratio,
                               "band": [lo, hi], "passed": lo <= ratio <= hi})
        elif kind == "boson_sampling":
            runs = self._runs(experiment)
            worst = max(r["total_variation_vs_exact"] for r in runs)
            bound = runs[0]["tv_bound"]
            checks.append({"check": "sampled vs exact distribution (total variation)", "value": worst,
                           "threshold": bound, "passed": worst <= bound})
            hom = sims.hom_visibility_check()
            checks.append({"check": "HOM coincidence at zero delay (50:50, indistinguishable)",
                           "value": hom["coincidence_at_zero_delay_50_50"], "expected": 0.0,
                           "passed": abs(hom["coincidence_at_zero_delay_50_50"]) < 1e-12})
            checks.append({"check": "distinguishable photons coincidence (50:50)",
                           "value": hom["coincidence_distinguishable_50_50"], "expected": 0.5,
                           "passed": abs(hom["coincidence_distinguishable_50_50"] - 0.5) < 1e-12})
        else:
            self.policy.check_simulation(experiment)  # raises for unknown kinds
        verdict = all(c["passed"] for c in checks)
        branch = branch or self.branch
        return self._commit("truth", experiment, {"checks": checks, "passed": verdict}, [self._head(branch)], branch)

    def repro(self, ref: str, branch: Optional[str] = None) -> LabState:
        """Re-run a stored state's experiment and require bit-identical outputs."""
        original = self.store.restore(ref)
        exp = original.to_dict()["experiment"]
        if exp is None or "runs" not in original.metrics:
            raise ValueError("repro needs a state produced by quick or compare-side runs")
        runs = self._runs(exp)
        before = {r["seed"]: r["output_hash"] for r in original.metrics["runs"]}
        after = {r["seed"]: r["output_hash"] for r in runs}
        matches = {str(s): before.get(s) == after.get(s) for s in after}
        metrics = {"reproduced": all(matches.values()) and set(before) == set(after), "per_seed": matches,
                   "runs": runs}
        branch = branch or self.branch
        return self._commit("repro", exp, metrics, [original.hash], branch, {"reproduces": original.hash})

    def compare(self, experiment_a: Mapping[str, Any], experiment_b: Mapping[str, Any],
                branch: Optional[str] = None) -> LabState:
        """Multi-seed comparison of two experiments on the same metric (b - a)."""
        if experiment_a["kind"] != experiment_b["kind"]:
            raise ValueError("compare needs two experiments of the same kind")
        key = self._primary(experiment_a["kind"])
        branch = branch or self.branch
        sa = self._commit("compare-side", experiment_a, {"runs": self._runs(experiment_a)}, [self._head(branch)], None)
        sb = self._commit("compare-side", experiment_b, {"runs": self._runs(experiment_b)}, [self._head(branch)], None)
        ra, rb = sa.metrics["runs"], sb.metrics["runs"]
        diffs = [b[key] - a[key] for a, b in zip(ra, rb)]
        metrics = {
            "metric": key,
            "a": self._summarize(ra, key),
            "b": self._summarize(rb, key),
            "paired_delta": {"mean": float(np.mean(diffs)), "ci95": _bootstrap_mean_ci(diffs), "n": len(diffs)},
            "states": compare_states(sa, sb),
        }
        return self._commit("compare", None, metrics, [self._head(branch), sa.hash, sb.hash], branch)

    def publish(self, ref: str, out_dir: str | Path) -> Path:
        """Write a self-verifying publication bundle locally. Needs a recorded approval."""
        state = self.store.restore(ref)
        approval = self.policy.check_publish(state.hash)
        chain = lineage(self.store, state.hash)
        bundle = {
            "version": "awen.lab-publication.v1",
            "state": state.to_dict(),
            "state_hash": state.hash,
            "lineage": chain,
            "lineage_states": {c["hash"]: self.store.get(c["hash"]).to_dict() for c in chain},
            "approval": approval.to_dict(),
            "statement": "All results are simulated by the AWEN reference simulators; none are hardware measurements.",
        }
        bundle["bundle_hash"] = content_hash(bundle)
        out = Path(out_dir)
        out.mkdir(parents=True, exist_ok=True)
        path = out / f"publication-{state.hash.split(':', 1)[1][:12]}.json"
        with open(path, "x", encoding="utf-8") as handle:
            handle.write(json.dumps(bundle, indent=2, sort_keys=True) + "\n")
        self._commit("publish", None, {"bundle": path.name, "bundle_hash": bundle["bundle_hash"]},
                     [state.hash], None)
        return path

    # -- version control -----------------------------------------------------
    def branch_from(self, name: str, ref: str) -> str:
        return self.store.branch(name, ref)

    def restore(self, ref: str) -> LabState:
        return self.store.restore(ref)

    def diff(self, ref_a: str, ref_b: str) -> dict[str, Any]:
        return compare_states(self.store.restore(ref_a), self.store.restore(ref_b))

    def hardware(self, description: str) -> None:
        """Explicit entry point for hardware requests; always refused."""
        self.policy.request_hardware(description)


def verify_bundle(path: str | Path) -> bool:
    bundle = json.loads(Path(path).read_text())
    body = {k: v for k, v in bundle.items() if k != "bundle_hash"}
    if content_hash(body) != bundle.get("bundle_hash"):
        return False
    return all(content_hash(doc) == h for h, doc in bundle["lineage_states"].items())


__all__ = ["LabAgent", "verify_bundle", "canonical"]
