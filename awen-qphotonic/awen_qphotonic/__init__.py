"""AWEN quantum-photonic reference simulators (simulation only).

This package is the quantum-photonic domain (AEP-0020). It shares no
semantics with the classical photonic noise model in awen-native/awen-physics.
Everything here is an exact or analytic simulation; nothing is a hardware
measurement, and nothing here demonstrates quantum advantage.
"""
from . import detection, fock, gaussian, permanent

__all__ = ["detection", "fock", "gaussian", "permanent"]
