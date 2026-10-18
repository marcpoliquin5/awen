"""Execute a restricted StableHLO module (as printed by JAX) on the AWEN simulator.

This is the testable portion of the PJRT plugin item (C5, issue #61). It does
not implement the PJRT C API: JAX does not load it as a device. It takes the
StableHLO text that ``jax.jit(f).lower(...).as_text()`` produces, executes
every ``stablehlo.dot_general`` either on the f64 math reference or through
``awen_native.photonic_gemm`` (the Rust compact noise model, i.e. simulated),
runs the small set of supporting elementwise/shape ops electronically in
NumPy, and refuses anything else with a typed error that names the operation.
Nothing is executed silently wrong: unknown ops, calls, multi-result ops and
unsupported attributes raise ``UnsupportedGraphError``.

See ``docs/PJRT_BOUNDARY.md`` for what a real PJRT plugin still needs.
"""
from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple

import numpy as np

from .errors import ContractError, UnsupportedGraphError

STABLEHLO_EXEC_VERSION = "awen.stablehlo-exec.v1"
PHOTONIC_OPS = frozenset({"dot_general"})
ELECTRONIC_OPS = frozenset(
    {"add", "subtract", "multiply", "negate", "maximum", "transpose", "reshape", "broadcast_in_dim", "constant", "convert"}
)

_DTYPES = {"f16": np.float16, "bf16": np.float32, "f32": np.float32, "f64": np.float64, "i32": np.int32, "i64": np.int64}
_TENSOR = re.compile(r"tensor<([0-9x]*?)x?([a-z]+[0-9]*)>")
_FUNC = re.compile(r"func\.func\s+(public|private)?\s*@([\w.$-]+)\s*\((.*?)\)\s*->", re.S)
_ARG = re.compile(r"(%[\w]+)\s*:\s*(tensor<[^>]*>)")
_ASSIGN = re.compile(r"^\s*(%[\w#:]+)\s*=\s*(\"?)([\w.]+)\2\s*(.*)$")
_RETURN = re.compile(r"^\s*(?:func\.)?return\s+(.*?)\s*:\s*.*$")


@dataclass
class OpRecord:
    result: str
    op: str
    placement: str  # "photonic-sim", "reference", "electronic"
    fidelity: str
    shape: Tuple[int, ...]


@dataclass
class StableHLOReport:
    version: str = STABLEHLO_EXEC_VERSION
    target: str = "reference"
    ops: List[OpRecord] = field(default_factory=list)
    photonic_macs: int = 0
    electronic_ops: int = 0
    evidence: str = ""

    def to_dict(self) -> Dict[str, Any]:
        return {
            "version": self.version,
            "target": self.target,
            "photonic_macs": self.photonic_macs,
            "electronic_ops": self.electronic_ops,
            "evidence": self.evidence,
            "ops": [r.__dict__ | {"shape": list(r.shape)} for r in self.ops],
        }


def _parse_type(t: str) -> Tuple[Tuple[int, ...], np.dtype]:
    m = _TENSOR.search(t)
    if not m:
        raise UnsupportedGraphError(f"unsupported type {t!r}")
    dims, et = m.group(1), m.group(2)
    if et not in _DTYPES:
        raise UnsupportedGraphError(f"unsupported element type {et!r}")
    shape = tuple(int(d) for d in dims.split("x") if d) if dims else ()
    return shape, np.dtype(_DTYPES[et])


def _result_type(rest: str) -> str:
    return rest.rsplit("->", 1)[1] if "->" in rest else rest.rsplit(":", 1)[1]


def _int_list(text: str) -> List[int]:
    text = text.strip().strip("[]").strip()
    return [int(v) for v in text.split(",") if v.strip()] if text else []


def _operands(rest: str) -> List[str]:
    head = rest.split(":", 1)[0]
    head = head.split(", dims", 1)[0].split(", batching_dims", 1)[0].split(", contracting_dims", 1)[0]
    return re.findall(r"%[\w#]+", head)


def parse_main(text: str) -> Tuple[List[Tuple[str, Tuple[int, ...], np.dtype]], List[str], List[str]]:
    """Return (arguments, body lines, returned names) of the single public main function."""
    funcs = _FUNC.findall(text)
    if not funcs:
        raise UnsupportedGraphError("no func.func found in StableHLO module")
    if len(funcs) != 1:
        names = [f[1] for f in funcs]
        raise UnsupportedGraphError(f"multiple functions {names}; calls between functions are not supported")
    args = [(n, *_parse_type(t)) for n, t in _ARG.findall(funcs[0][2])]
    body_start = text.index("\n", text.index("func.func")) + 1
    lines = text[body_start:].splitlines()
    body, returned = [], []
    for line in lines:
        s = line.strip()
        if not s or s == "}":
            continue
        r = _RETURN.match(s)
        if r:
            returned = re.findall(r"%[\w#]+", r.group(1))
            break
        body.append(s)
    if not returned:
        raise UnsupportedGraphError("main has no return")
    return args, body, returned


def _parse_dense(rest: str, shape: Tuple[int, ...], dtype: np.dtype) -> np.ndarray:
    m = re.search(r"dense<(.*?)>\s*:", rest)
    if not m:
        raise UnsupportedGraphError(f"unsupported constant {rest!r}")
    body = m.group(1).strip()
    if body.startswith('"0x'):
        raise UnsupportedGraphError("hex-encoded dense constants are not supported")
    vals = [float(v) for v in re.findall(r"[-+]?(?:\d+\.?\d*(?:[eE][-+]?\d+)?|inf|nan)", body)]
    arr = np.array(vals, dtype=np.float64)
    if arr.size == 1:
        return np.full(shape, arr[0]).astype(dtype)
    return arr.reshape(shape).astype(dtype)


def _dot_general(lhs, rhs, rest, target, contract, seed, report, name):
    bm = re.search(r"batching_dims\s*=\s*(\[[^\]]*\])\s*x\s*(\[[^\]]*\])", rest)
    cm = re.search(r"contracting_dims\s*=\s*(\[[^\]]*\])\s*x\s*(\[[^\]]*\])", rest)
    if not cm:
        raise UnsupportedGraphError(f"{name}: dot_general without contracting_dims")
    lb, rb = (_int_list(bm.group(1)), _int_list(bm.group(2))) if bm else ([], [])
    lc, rc = _int_list(cm.group(1)), _int_list(cm.group(2))
    if "algorithm" in rest:
        raise UnsupportedGraphError(f"{name}: dot_general algorithm attribute is not supported")
    lf = [d for d in range(lhs.ndim) if d not in lb + lc]
    rf = [d for d in range(rhs.ndim) if d not in rb + rc]
    bshape = [lhs.shape[d] for d in lb]
    m = int(np.prod([lhs.shape[d] for d in lf])) if lf else 1
    n = int(np.prod([rhs.shape[d] for d in rf])) if rf else 1
    k = int(np.prod([lhs.shape[d] for d in lc])) if lc else 1
    nb = int(np.prod(bshape)) if bshape else 1
    a = np.transpose(lhs, lb + lf + lc).reshape(nb, m, k).astype(np.float64)
    b = np.transpose(rhs, rb + rc + rf).reshape(nb, k, n).astype(np.float64)
    out = np.empty((nb, m, n))
    for i in range(nb):
        if target == "photonic-sim":
            import awen_native

            out[i], _ = awen_native.photonic_gemm(a[i], b[i].T, contract, seed=seed + i)
        else:
            out[i] = a[i] @ b[i]
    report.photonic_macs += nb * m * n * k
    shape = tuple(bshape + [lhs.shape[d] for d in lf] + [rhs.shape[d] for d in rf])
    return out.reshape(shape)


def execute_stablehlo(
    text: str,
    *inputs: Any,
    target: str = "reference",
    contract: Optional[Mapping[str, Any]] = None,
    seed: int = 0,
) -> Tuple[List[np.ndarray], StableHLOReport]:
    """Run the module's main function. ``target`` is "reference" (f64) or "photonic-sim"."""
    if target not in ("reference", "photonic-sim"):
        raise ContractError(f"unknown target {target!r}")
    if target == "photonic-sim" and contract is None:
        import awen_native

        contract = awen_native.default_contract()
    args, body, returned = parse_main(text)
    if len(inputs) != len(args):
        raise ContractError(f"expected {len(args)} inputs, got {len(inputs)}")
    env: Dict[str, np.ndarray] = {}
    dtypes: Dict[str, np.dtype] = {}
    for (name, shape, dtype), value in zip(args, inputs):
        arr = np.asarray(value)
        if tuple(arr.shape) != shape:
            raise ContractError(f"{name}: shape {arr.shape} != {shape}")
        env[name] = arr.astype(np.float64)
        dtypes[name] = dtype
    report = StableHLOReport(target=target)
    report.evidence = (
        "f64 math reference" if target == "reference"
        else "dot_general simulated with the AWEN compact noise model; not a hardware measurement"
    )
    for line in body:
        m = _ASSIGN.match(line)
        if not m:
            raise UnsupportedGraphError(f"cannot parse StableHLO line: {line!r}")
        res, _, op, rest = m.groups()
        if ":" in res or "#" in res:
            raise UnsupportedGraphError(f"multi-result operation {op} is not supported")
        if not op.startswith("stablehlo."):
            raise UnsupportedGraphError(f"operation {op} is not supported (only stablehlo.*)")
        short = op.split(".", 1)[1]
        if short not in PHOTONIC_OPS | ELECTRONIC_OPS:
            raise UnsupportedGraphError(
                f"stablehlo.{short} is not supported by the AWEN StableHLO executor; "
                "run it on the host framework or extend awen_py.stablehlo_exec"
            )
        shape, dtype = _parse_type(_result_type(rest))
        ops = [env[o] for o in _operands(rest)] if short != "constant" else []
        if short == "dot_general":
            val = _dot_general(ops[0], ops[1], rest, target, contract, seed, report, res)
            placement = "photonic-sim" if target == "photonic-sim" else "reference"
            fidelity = "compact_model (simulated)" if target == "photonic-sim" else "math_reference"
        else:
            placement, fidelity = "electronic", "math_reference"
            report.electronic_ops += 1
            if short == "constant":
                val = _parse_dense(rest, shape, dtype).astype(np.float64)
            elif short == "add":
                val = ops[0] + ops[1]
            elif short == "subtract":
                val = ops[0] - ops[1]
            elif short == "multiply":
                val = ops[0] * ops[1]
            elif short == "maximum":
                val = np.maximum(ops[0], ops[1])
            elif short == "negate":
                val = -ops[0]
            elif short == "transpose":
                val = np.transpose(ops[0], _int_list(re.search(r"dims\s*=\s*(\[[^\]]*\])", rest).group(1)))
            elif short == "reshape":
                val = ops[0].reshape(shape)
            elif short == "convert":
                val = ops[0].astype(dtype).astype(np.float64)
            else:  # broadcast_in_dim
                dims = _int_list(re.search(r"dims\s*=\s*(\[[^\]]*\])", rest).group(1))
                src = ops[0]
                view = [1] * len(shape)
                for i, d in enumerate(dims):
                    view[d] = src.shape[i]
                val = np.broadcast_to(src.reshape(view), shape)
        if tuple(np.shape(val)) != shape:
            raise UnsupportedGraphError(f"{res}: computed shape {np.shape(val)} != declared {shape}")
        if np.issubdtype(dtype, np.floating):
            val = np.asarray(val, dtype=np.float64).astype(dtype).astype(np.float64)
        env[res] = np.asarray(val)
        dtypes[res] = dtype
        report.ops.append(OpRecord(res, short, placement, fidelity, shape))
    outs = [env[r].astype(dtypes[r]) for r in returned]
    return outs, report


def run_jax_function(fn, *args: Any, target: str = "reference", contract: Optional[Mapping[str, Any]] = None, seed: int = 0):
    """Lower ``fn`` with jax.jit and execute the StableHLO on AWEN. Requires jax."""
    import jax

    text = jax.jit(fn).lower(*args).as_text()
    return execute_stablehlo(text, *[np.asarray(a) for a in args], target=target, contract=contract, seed=seed)
