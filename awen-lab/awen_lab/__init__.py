"""AWEN Lab Agent (simulation only).

Orchestrates experiments over the existing AWEN simulators (the Rust photonic
GEMM model via ``awen_native`` and the quantum-photonic reference simulators
in ``awen_qphotonic``) through five workflows: Quick, Truth, Repro, Compare
and Publish. Every run produces an immutable, content-addressed lab state
with full provenance. Branch, restore and compare work like a tiny version
control system over those states.

Safety model: every action passes an :class:`~awen_lab.policy.Policy`.
Simulation actions inside the operating bounds run automatically; Publish
needs a recorded human approval; any hardware action is refused
unconditionally, with or without approval. There is no code path in this
package that talks to hardware.
"""

from .policy import (ApprovalRequired, BoundsViolation, HardwareActionRefused, OperatingBounds, Policy,
                     PolicyError)
from .store import LabState, LabStore, StateIntegrityError
from .agent import LabAgent

__all__ = [
    "ApprovalRequired", "BoundsViolation", "HardwareActionRefused", "OperatingBounds", "Policy",
    "PolicyError", "LabState", "LabStore", "StateIntegrityError", "LabAgent",
]
