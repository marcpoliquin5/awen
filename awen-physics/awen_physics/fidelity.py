"""Fidelity labels shared with the Rust crates (``awen_compiler::noise::Fidelity``)."""

from enum import Enum


class Fidelity(str, Enum):
    ANALYTIC = "analytic"
    COMPACT_MODEL = "compact_model"
    NUMERICAL_PROPAGATION = "numerical_propagation"
    FULL_WAVE = "full_wave"
    DIGITAL_TWIN = "digital_twin"
    MEASURED = "measured"
