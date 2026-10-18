"""Symmetric per-output-channel int8 weight quantization for inference.

Weights are stored as int8 with one float32 scale per output row,
``w ≈ scale[o] * q[o, i]`` with ``q ∈ [-127, 127]``. Activations stay in
floating point (weight-only quantization), so the quantized GEMM is
``x @ (q * scale).T``. This mirrors how a fixed-precision analog weight bank
is programmed and keeps the arithmetic exact apart from weight rounding.
"""

from __future__ import annotations

import copy

import torch
import torch.nn as nn
import torch.nn.functional as F

from .model import AwenLinear


def quantize_tensor_int8(weight: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
    max_abs = weight.detach().abs().amax(dim=1, keepdim=True).clamp_min(1e-12)
    scale = max_abs / 127.0
    q = torch.clamp(torch.round(weight.detach() / scale), -127, 127).to(torch.int8)
    return q, scale.squeeze(1).to(torch.float32)


class Int8Linear(nn.Module):
    """Weight-only int8 linear layer produced from an ``nn.Linear``."""

    def __init__(self, linear: nn.Linear):
        super().__init__()
        q, scale = quantize_tensor_int8(linear.weight)
        self.in_features = linear.in_features
        self.out_features = linear.out_features
        self.register_buffer("qweight", q)
        self.register_buffer("scale", scale)
        if linear.bias is not None:
            self.register_buffer("bias", linear.bias.detach().clone())
        else:
            self.bias = None
        self.matmul_executor = getattr(linear, "matmul_executor", None)

    def dequantized_weight(self) -> torch.Tensor:
        return self.qweight.to(torch.float32) * self.scale[:, None]

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        weight = self.dequantized_weight().to(x.dtype)
        return F.linear(x, weight, self.bias)


def quantize_model_int8(model: nn.Module) -> nn.Module:
    """Return a copy of ``model`` with every :class:`AwenLinear` replaced by :class:`Int8Linear`."""
    quantized = copy.deepcopy(model).eval()
    for name, module in list(quantized.named_modules()):
        for child_name, child in list(module.named_children()):
            if isinstance(child, AwenLinear):
                setattr(module, child_name, Int8Linear(child))
    return quantized


def int8_weight_bytes(model: nn.Module) -> int:
    total = 0
    for module in model.modules():
        if isinstance(module, Int8Linear):
            total += module.qweight.numel() + 4 * module.scale.numel()
    return total
