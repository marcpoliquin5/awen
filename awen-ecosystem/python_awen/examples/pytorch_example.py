"""PyTorch example using AWEN Python wrapper.

This script demonstrates using the `awen_py` helpers to run an IR and compute gradients via the
`awenctl` runtime. It is a thin integration demo; for production use native bindings or RPC.
"""
import torch
from awen_py import compute_gradients, run_ir
from awen_py.torch_wrapper import awen_forward, awen_backward


def main():
    ir_path = "../../awen-runtime/example_ir.json"
    param_names = ["mzi_0:phase", "mzi_1:phase"]

    # initial params
    params = torch.tensor([0.1, 0.2], dtype=torch.float64, requires_grad=True)

    # Run forward (shells out to awenctl run)
    print("Running forward...")
    out = awen_forward(ir_path, param_names, params, seed=42)
    print("Scalar cost:", out.item())

    # Compute gradients by calling awenctl gradient (manual backward)
    print("Computing gradients via awenctl gradient...")
    grads = awen_backward(out)
    print("Gradients:", grads)


if __name__ == "__main__":
    main()
