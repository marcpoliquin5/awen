"""Model conversion utility: AWEN-Mini -> ONNX (verified) and ONNX -> awen.tensor.v1."""

import json
import os
import subprocess
from pathlib import Path

import numpy as np
import pytest

onnx = pytest.importorskip("onnx")
from onnx import TensorProto, helper, numpy_helper  # noqa: E402

from awen_models import convert  # noqa: E402
from awen_models.checkpoint import bundled_checkpoint_dir  # noqa: E402

REPO = Path(__file__).resolve().parents[2]


@pytest.fixture(scope="module")
def exported(tmp_path_factory):
    out = tmp_path_factory.mktemp("convert") / "mini.onnx"
    report = convert.mini_to_onnx(bundled_checkpoint_dir(), out, seq_len=16, seed=3)
    return out, report


def test_mini_to_onnx_matches_pytorch_and_reports_compatibility(exported):
    path, report = exported
    assert path.exists()
    assert report["version"] == convert.REPORT_VERSION
    verification = report["verification"]
    assert verification["logits_shape"][:2] == [1, 16]
    assert verification["max_abs_logit_diff"] < 1e-4
    compat = report["compatibility"]
    matmuls = report["op_histogram"]["MatMul"]
    assert 0 < compat["photonic_nodes"] < matmuls  # attention score products stay electronic
    assert 0.0 < compat["photonic_mac_fraction"] <= 1.0
    with pytest.raises(ValueError):
        convert.mini_to_onnx(bundled_checkpoint_dir(), path.parent / "bad.onnx", seq_len=10_000)


def _schema_validator():
    jsonschema = pytest.importorskip("jsonschema")
    from referencing import Registry, Resource

    schemas = REPO / "awen-spec" / "schemas"
    registry = Registry()
    for name in ("awen_tensor_ir.v1.json", "awen_precision.v1.json"):
        resource = Resource.from_contents(json.loads((schemas / name).read_text()))
        registry = registry.with_resource(name, resource)
        registry = registry.with_resource(f"https://awen.dev/schemas/{name}", resource)
    schema = json.loads((schemas / "awen_tensor_ir.v1.json").read_text())
    return jsonschema.Draft202012Validator(schema, registry=registry)


def test_onnx_to_program_is_schema_valid_and_shape_consistent(exported, tmp_path):
    path, report = exported
    result = convert.onnx_to_tensor_program(path)
    program = result["program"]
    _schema_validator().validate(program)
    assert len(program["ops"]) == report["compatibility"]["photonic_nodes"]
    assert len(result["skipped"]) == report["op_histogram"]["MatMul"] - len(program["ops"])
    assert all("activations" in item["reason"] for item in result["skipped"])
    tensors = {t["id"]: t for t in program["tensors"]}
    for op in program["ops"]:
        lhs, rhs, out = (tensors[op[k]]["shape"] for k in ("lhs", "rhs", "output"))
        k, n = (rhs[1], rhs[0]) if op["transpose_rhs"] else (rhs[0], rhs[1])
        assert lhs[1] == k and out == [lhs[0], n]
        assert lhs[0] == 16  # batch 1 x sequence 16

    awenctl = os.environ.get("AWENCTL") or str(REPO / "awen-runtime" / "target" / "debug" / "awenctl")
    if not Path(awenctl).exists():
        pytest.skip("awenctl binary not built")
    program_path = tmp_path / "program.json"
    program_path.write_text(json.dumps(program))
    artifact = tmp_path / "compiled.json"
    run = subprocess.run(
        [awenctl, "compile", str(program_path), "--output", str(artifact)],
        capture_output=True, text=True, cwd=tmp_path,
    )
    assert run.returncode == 0, run.stderr
    compiled = json.loads(artifact.read_text())
    assert len(compiled["placement"]) == len(program["ops"])


def test_gemm_transb_epilogue_and_dynamic_shapes():
    rng = np.random.default_rng(0)
    w = numpy_helper.from_array(rng.normal(size=(5, 12)).astype(np.float32), "w")
    b = numpy_helper.from_array(np.zeros(5, np.float32), "b")

    def model(batch):
        g = helper.make_graph(
            [helper.make_node("Gemm", ["x", "w", "b"], ["y"], transB=1, alpha=0.5)],
            "g",
            [helper.make_tensor_value_info("x", TensorProto.FLOAT, [batch, 12])],
            [helper.make_tensor_value_info("y", TensorProto.FLOAT, [batch, 5])],
            [w, b],
        )
        return helper.make_model(g, opset_imports=[helper.make_opsetid("", 17)])

    result = convert.onnx_to_tensor_program(model(4))
    (op,) = result["program"]["ops"]
    assert op["transpose_rhs"] is True
    shapes = {t["id"]: t["shape"] for t in result["program"]["tensors"]}
    assert shapes[op["output"]] == [4, 5]
    assert result["electronic_epilogues"] == [{"op": op["id"], "alpha": 0.5, "bias": "b"}]

    dynamic = convert.onnx_to_tensor_program(model("N"))
    assert dynamic["program"]["ops"] == []
    assert dynamic["skipped"][0]["reason"] == "activation shape is not static"


def test_cli_round_trip(tmp_path, exported):
    path, _ = exported
    out = tmp_path / "p.json"
    convert.main(["to-program", str(path), "--output", str(out), "--dtype", "int8"])
    program = json.loads(out.read_text())
    assert {t["dtype"] for t in program["tensors"]} == {"int8"}
