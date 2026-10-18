"""Approval gates and operating bounds.

Action classes:

* ``simulate``: runs a simulator. Allowed when the request is inside the
  operating bounds.
* ``publish``: writes a publication bundle to a local directory. Requires an
  approval recorded for that exact state hash (who approved and why). Nothing
  is uploaded or sent anywhere.
* ``hardware``: anything that would touch a physical device (lasers,
  modulators, heaters, detectors, stages, vendor drivers). Always refused.
  An approval does not unlock it; the package has no hardware backend.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from typing import Any, Mapping, Optional

SIMULATED_TARGETS = ("photonic-sim", "awen-reference", "qphotonic-sim")
HARDWARE_WORDS = ("hardware", "device", "laser", "chip", "hil", "driver", "vendor", "stage", "heater")


class PolicyError(RuntimeError):
    """Base class for refused actions."""


class HardwareActionRefused(PolicyError):
    """Raised for every action that targets physical hardware."""


class ApprovalRequired(PolicyError):
    """Raised when an action needs a recorded human approval that is missing."""


class BoundsViolation(PolicyError):
    """Raised when a request exceeds the operating bounds."""


@dataclass(frozen=True)
class OperatingBounds:
    max_matrix_elements: int = 1_000_000
    max_inner_dim: int = 4096
    min_photons_per_mac: float = 1.0
    max_photons_per_mac: float = 1e9
    max_seeds: int = 16
    max_modes: int = 8
    max_photons_quantum: int = 4
    max_shots: int = 200_000

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class Approval:
    state_hash: str
    approver: str
    reason: str

    def to_dict(self) -> dict[str, str]:
        return asdict(self)


@dataclass
class Policy:
    bounds: OperatingBounds = field(default_factory=OperatingBounds)
    approvals: dict[str, Approval] = field(default_factory=dict)
    audit: list[dict[str, Any]] = field(default_factory=list)

    def _log(self, action: str, decision: str, detail: str) -> None:
        self.audit.append({"action": action, "decision": decision, "detail": detail})

    def approve(self, state_hash: str, approver: str, reason: str) -> Approval:
        if not approver.strip() or not reason.strip():
            raise ApprovalRequired("an approval needs a named approver and a reason")
        approval = Approval(state_hash, approver.strip(), reason.strip())
        self.approvals[state_hash] = approval
        self._log("approve", "recorded", f"{approver} approved {state_hash}")
        return approval

    def check_target(self, target: str) -> None:
        lowered = target.lower()
        if target not in SIMULATED_TARGETS or any(word in lowered for word in HARDWARE_WORDS):
            self._log("hardware", "refused", target)
            raise HardwareActionRefused(
                f"target {target!r} is not a simulator; the Lab Agent refuses all hardware actions "
                f"(allowed targets: {', '.join(SIMULATED_TARGETS)})")

    def check_simulation(self, experiment: Mapping[str, Any]) -> None:
        self.check_target(str(experiment.get("target", "")))
        b = self.bounds
        kind = experiment.get("kind")
        seeds = experiment.get("seeds", [0])
        if len(seeds) > b.max_seeds:
            self._deny(f"{len(seeds)} seeds > max_seeds {b.max_seeds}")
        if kind == "photonic_gemm":
            m, k, n = experiment["shape_mkn"]
            if min(m, k, n) <= 0:
                self._deny("shape dimensions must be positive")
            if max(m * k, k * n, m * n) > b.max_matrix_elements:
                self._deny(f"matrix of {max(m * k, k * n, m * n)} elements > {b.max_matrix_elements}")
            if k > b.max_inner_dim:
                self._deny(f"inner dimension {k} > {b.max_inner_dim}")
            photons = experiment.get("contract", {}).get("photons_per_mac")
            if photons is not None and not b.min_photons_per_mac <= photons <= b.max_photons_per_mac:
                self._deny(f"photons_per_mac {photons} outside [{b.min_photons_per_mac}, {b.max_photons_per_mac}]")
        elif kind == "boson_sampling":
            modes, inputs = experiment["modes"], experiment["input"]
            if modes > b.max_modes or len(inputs) != modes:
                self._deny(f"modes {modes} > {b.max_modes} or input length mismatch")
            if sum(inputs) > b.max_photons_quantum:
                self._deny(f"{sum(inputs)} photons > {b.max_photons_quantum}")
            if experiment.get("shots", 0) > b.max_shots:
                self._deny(f"shots {experiment['shots']} > {b.max_shots}")
        else:
            self._deny(f"unknown experiment kind {kind!r}")
        self._log("simulate", "allowed", f"{kind} on {experiment['target']}")

    def check_publish(self, state_hash: str) -> Approval:
        approval = self.approvals.get(state_hash)
        if approval is None:
            self._log("publish", "refused", f"no approval for {state_hash}")
            raise ApprovalRequired(f"publishing {state_hash} needs a recorded approval (Policy.approve)")
        self._log("publish", "allowed", f"approved by {approval.approver}")
        return approval

    def request_hardware(self, description: str, approval: Optional[Approval] = None) -> None:
        """Every hardware request ends here and is refused, approved or not."""
        self._log("hardware", "refused", description)
        raise HardwareActionRefused(
            f"hardware action refused: {description!r}. The Lab Agent is simulation-only; "
            "approvals cannot unlock hardware actions.")

    def _deny(self, why: str) -> None:
        self._log("simulate", "refused", why)
        raise BoundsViolation(why)
