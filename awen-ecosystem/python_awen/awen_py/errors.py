"""Typed errors shared by all AWEN framework frontends."""


class AWENError(RuntimeError):
    """Base class for failures at the AWEN framework/runtime boundary."""


class ContractError(AWENError, ValueError):
    """A version, shape, dtype, precision, or serialization contract is invalid."""


class UnsupportedGraphError(AWENError):
    """A graph cannot be lowered and fallback was disabled."""


class ExecutionError(AWENError):
    """An in-process operation failed after its contract was accepted."""


class DeviceError(AWENError):
    """A requested device or stream cannot execute the operation."""


class SerializationError(AWENError):
    """A serialized framework plan or executable is incompatible."""
