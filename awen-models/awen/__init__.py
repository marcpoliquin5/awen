"""AWEN: run AI models on simulated photonic matrix hardware."""

from .runtime import DEVICES, ExecutionInfo, LoadedModel, Runtime

__all__ = ["DEVICES", "ExecutionInfo", "LoadedModel", "Runtime"]
__version__ = "0.1.0"
