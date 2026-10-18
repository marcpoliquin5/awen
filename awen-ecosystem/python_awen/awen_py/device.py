"""PyTorch ``awen`` device (PrivateUse1) backed by the AWEN photonic simulator.

    import torch, awen_py.device as awen_device
    awen_device.register()                 # once per process
    x = torch.randn(4, 8).to("awen")
    layer = torch.nn.Linear(8, 3).to("awen")
    with torch.no_grad():
        y = torch.relu(layer(x)).cpu()

Tensors on ``awen`` keep their values in host memory. Matrix products (``mm``,
``addmm``, ``bmm`` and therefore ``matmul``/``linear``) run in the AWEN Rust photonic
GEMM model (``awen_native``) with the configured noise contract. That is
simulated, compact-model fidelity, not hardware. ``add`` and ``relu`` model the
electronic stage after photodetection: they run exactly on the detected values.
View and copy operations are supported so tensors can move and reshape. Every
other operator raises ``NotImplementedError`` and asks for ``.cpu()``. The device is
meant for inference: autograd records the graph, but reductions such as
``sum`` that backward passes need are not implemented, so train on the CPU.
"""

from __future__ import annotations

import threading
from typing import Any, Mapping, Optional

import torch
from torch.utils._pytree import tree_map

from .errors import DeviceError

DEVICE_NAME = "awen"
_LOCK = threading.Lock()
_REGISTERED = False
_LIBRARY: Optional[torch.library.Library] = None
_STATE: dict[str, Any] = {"noise": None, "tile_k": 64, "seed": 0, "calls": 0}

GEMM_OPS = {"aten.mm.default", "aten.addmm.default", "aten.bmm.default"}
ELECTRONIC_OPS = {"aten.add.Tensor", "aten.relu.default", "aten.add_.Tensor", "aten.relu_.default"}
LAYOUT_OPS = {
    "aten.detach.default", "aten.alias.default", "aten.view.default", "aten._unsafe_view.default",
    "aten.t.default", "aten.transpose.int", "aten.permute.default", "aten.expand.default",
    "aten.clone.default", "aten.contiguous.default", "aten.reshape.default", "aten.unsqueeze.default",
    "aten.squeeze.dim", "aten.slice.Tensor", "aten.select.int", "aten.split.Tensor",
    "aten.empty_like.default", "aten.zeros_like.default", "aten.fill_.Scalar", "aten.zero_.default",
    "aten.is_same_size.default",
}


def configure(noise: Optional[Mapping[str, Any]] = None, tile_k: int = 64, seed: int = 0) -> None:
    """Set the noise contract (None = simulator default), contraction tile and base seed."""
    import awen_native

    if tile_k <= 0:
        raise ValueError("tile_k must be positive")
    _STATE.update(
        noise=None if noise is None else awen_native.validate_contract(noise),
        tile_k=int(tile_k),
        seed=int(seed),
        calls=0,
    )


def gemm_calls() -> int:
    """Number of GEMMs executed on the photonic simulator since the last ``configure``."""
    return _STATE["calls"]


class AwenTensor(torch.Tensor):
    """A tensor on the ``awen`` device; values live in ``_host`` (CPU memory)."""

    _host: torch.Tensor
    __torch_function__ = torch._C._disabled_torch_function_impl

    @staticmethod
    def __new__(cls, host: torch.Tensor):
        return torch.Tensor._make_wrapper_subclass(
            cls,
            host.shape,
            strides=host.stride(),
            storage_offset=host.storage_offset(),
            dtype=host.dtype,
            device=torch.device(DEVICE_NAME, 0),
            requires_grad=host.requires_grad,
        )

    def __init__(self, host: torch.Tensor):
        self._host = host

    def __repr__(self) -> str:  # printing must not dispatch more operators
        return f"tensor({self._host.tolist()}, device='{DEVICE_NAME}:0', dtype={self.dtype})"

    @classmethod
    def __torch_dispatch__(cls, func, types, args=(), kwargs=None):
        kwargs = kwargs or {}
        name = str(func)
        if name == "aten._to_copy.default":
            return _to_copy(args[0], **kwargs)
        if name in ("aten.copy_.default", "aten._copy_from.default"):
            src, dst = (args[1], args[0]) if name == "aten.copy_.default" else (args[0], args[1])
            source = src._host if isinstance(src, AwenTensor) else src
            target = dst._host if isinstance(dst, AwenTensor) else dst
            target.copy_(source)
            return dst
        if name in GEMM_OPS:
            return _gemm(name, *map(_unwrap_checked, args), **kwargs)
        if name in ELECTRONIC_OPS or name in LAYOUT_OPS:
            hosts = tree_map(_unwrap_checked, args)
            host_kwargs = tree_map(_unwrap_checked, kwargs)
            result = func(*hosts, **host_kwargs)
            if name.endswith("_.default") or name.endswith("_.Tensor") or name.endswith("_.Scalar"):
                return args[0]
            return tree_map(_wrap, result)
        raise NotImplementedError(
            f"{name} is not implemented on the '{DEVICE_NAME}' device; supported: matrix products "
            "(photonic simulator), add and relu (electronic), views and copies. Move the tensor with .cpu()."
        )


def _wrap(value: Any) -> Any:
    return AwenTensor(value) if isinstance(value, torch.Tensor) and not isinstance(value, AwenTensor) else value


def _unwrap_checked(value: Any) -> Any:
    if isinstance(value, AwenTensor):
        return value._host
    if isinstance(value, torch.Tensor) and value.dim() > 0:
        raise DeviceError(
            f"expected all tensors on '{DEVICE_NAME}', found one on {value.device}; move it with .to('{DEVICE_NAME}')"
        )
    return value


def _to_copy(tensor: "AwenTensor", dtype=None, device=None, **_: Any):
    host = tensor._host.to(dtype=dtype) if dtype is not None else tensor._host.clone()
    target = torch.device(device) if device is not None else tensor.device
    if target.type == DEVICE_NAME:
        return AwenTensor(host.clone() if host is tensor._host else host)
    return host.to(target)


def _photonic_mm(lhs: torch.Tensor, rhs: torch.Tensor) -> torch.Tensor:
    import awen_native
    import numpy as np

    x = np.ascontiguousarray(lhs.detach().to(torch.float64).numpy())
    w = np.ascontiguousarray(rhs.detach().to(torch.float64).numpy().T)
    seed = _STATE["seed"] + _STATE["calls"]
    _STATE["calls"] += 1
    y, _ = awen_native.photonic_gemm(x, w, _STATE["noise"], tile_k=_STATE["tile_k"], seed=seed)
    return torch.from_numpy(y).to(lhs.dtype)


def _gemm(name: str, *args: Any, beta: float = 1, alpha: float = 1) -> "AwenTensor":
    if name == "aten.mm.default":
        return AwenTensor(_photonic_mm(args[0], args[1]))
    if name == "aten.bmm.default":
        return AwenTensor(torch.stack([_photonic_mm(a, b) for a, b in zip(args[0], args[1])]))
    bias, lhs, rhs = args
    product = _photonic_mm(lhs, rhs)
    return AwenTensor(beta * bias + alpha * product)  # bias added electronically after detection


class _DeviceModule:
    """Minimal device module so ``torch.awen`` and device-generic helpers work."""

    @staticmethod
    def is_available() -> bool:
        try:
            import awen_native  # noqa: F401
        except ImportError:
            return False
        return True

    @staticmethod
    def device_count() -> int:
        return 1

    @staticmethod
    def current_device() -> int:
        return 0

    @staticmethod
    def _is_in_bad_fork() -> bool:
        return False

    @staticmethod
    def manual_seed_all(seed: int) -> None:
        _STATE["seed"] = int(seed)

    @staticmethod
    def get_amp_supported_dtype():
        return [torch.float32]


def _empty(size, dtype=None, layout=None, device=None, pin_memory=None, memory_format=None):
    return AwenTensor(torch.empty(size, dtype=dtype or torch.get_default_dtype()))


def _empty_strided(size, stride, dtype=None, layout=None, device=None, pin_memory=None):
    return AwenTensor(torch.empty_strided(size, stride, dtype=dtype or torch.get_default_dtype()))


def register() -> None:
    """Register the ``awen`` device with PyTorch (idempotent within a process)."""
    global _REGISTERED, _LIBRARY
    with _LOCK:
        if _REGISTERED:
            return
        if not _DeviceModule.is_available():
            raise DeviceError("the awen device needs the native extension: pip install ./awen-native")
        current = torch._C._get_privateuse1_backend_name()
        if current != "privateuseone":
            raise DeviceError(f"PrivateUse1 is already claimed by '{current}' in this process")
        # Experimental PyTorch API for Python-implemented PrivateUse1 backends: it
        # registers the device guard, hooks and device module a C++ backend would.
        try:
            from torch.utils.backend_registration import _setup_privateuseone_for_python_backend
        except ImportError as error:
            raise DeviceError(
                "this PyTorch build lacks Python PrivateUse1 backend support; tested with PyTorch 2.14"
            ) from error

        _setup_privateuseone_for_python_backend(rename=DEVICE_NAME, backend_module=_DeviceModule())
        _LIBRARY = torch.library.Library("aten", "IMPL")
        _LIBRARY.impl("empty.memory_format", _empty, "PrivateUse1")
        _LIBRARY.impl("empty_strided", _empty_strided, "PrivateUse1")
        _REGISTERED = True
