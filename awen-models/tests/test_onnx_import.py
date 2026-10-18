"""ONNX import against onnx.reference.ReferenceEvaluator (independent oracle)."""

import json

import numpy as np
import pytest

onnx = pytest.importorskip("onnx")
from onnx import TensorProto, helper, numpy_helper  # noqa: E402
from onnx.reference import ReferenceEvaluator  # noqa: E402

import awen_native  # noqa: E402
from awen_models import onnx_import  # noqa: E402

RNG = np.random.default_rng(11)


def init(name, *shape):
    return numpy_helper.from_array(RNG.uniform(-0.5, 0.5, shape).astype(np.float32), name)


def mlp_model():
    nodes = [
        helper.make_node("MatMul", ["x", "w1"], ["h"]),
        helper.make_node("Add", ["h", "b1"], ["hb"]),
        helper.make_node("Relu", ["hb"], ["r"]),
        helper.make_node("Gemm", ["r", "w2", "b2"], ["z"], transB=1, alpha=0.5),
        helper.make_node("Softmax", ["z"], ["y"], axis=-1),
    ]
    g = helper.make_graph(
        nodes, "mlp",
        [helper.make_tensor_value_info("x", TensorProto.FLOAT, [4, 12])],
        [helper.make_tensor_value_info("y", TensorProto.FLOAT, [4, 5])],
        [init("w1", 12, 16), init("b1", 16), init("w2", 5, 16), init("b2", 5)],
    )
    return helper.make_model(g, opset_imports=[helper.make_opsetid("", 17)])


def conv_model():
    nodes = [
        helper.make_node("Conv", ["x", "k", "kb"], ["c"], strides=[2, 2], pads=[1, 1, 1, 1]),
        helper.make_node("Tanh", ["c"], ["t"]),
        helper.make_node("Flatten", ["t"], ["f"], axis=1),
        helper.make_node("Gemm", ["f", "w", "b"], ["y"]),
    ]
    g = helper.make_graph(
        nodes, "convnet",
        [helper.make_tensor_value_info("x", TensorProto.FLOAT, [2, 3, 8, 8])],
        [helper.make_tensor_value_info("y", TensorProto.FLOAT, [2, 6])],
        [init("k", 4, 3, 3, 3), init("kb", 4), init("w", 64, 6), init("b", 6)],
    )
    return helper.make_model(g, opset_imports=[helper.make_opsetid("", 17)])


def oracle(model, feeds):
    return ReferenceEvaluator(model).run(None, {k: v.astype(np.float32) for k, v in feeds.items()})[0]


@pytest.mark.parametrize("build,shape", [(mlp_model, (4, 12)), (conv_model, (2, 3, 8, 8))])
def test_reference_run_matches_onnx_reference_evaluator(build, shape):
    model = build()
    x = RNG.uniform(-1, 1, shape)
    out, rep = onnx_import.run(model, {"x": x})
    assert rep["fidelity"].startswith("exact")
    np.testing.assert_allclose(out["y"], oracle(model, {"x": x}), rtol=1e-4, atol=1e-5)


def test_report_placements_and_macs():
    rep = onnx_import.analyze(conv_model(), name="convnet")
    d = rep.to_dict()
    assert d["version"] == "awen.onnx-report.v1"
    placements = [(n["op_type"], n["placement"], n["awen_op"], n["level"]) for n in d["nodes"]]
    assert placements == [
        ("Conv", "photonic", "conv2d", "photonic_sim"),
        ("Tanh", "electronic", None, None),
        ("Flatten", "electronic", None, None),
        ("Gemm", "photonic", "gemm", "hardware_lowerable"),
    ]
    conv_macs = 2 * 4 * 4 * 4 * 27  # N*O*HO*WO * C*KH*KW
    gemm_macs = 2 * 64 * 6
    assert d["nodes"][0]["macs"] == conv_macs and d["nodes"][3]["macs"] == gemm_macs
    assert d["summary"]["photonic_mac_fraction"] == 1.0
    assert d["summary"]["executable"]
    json.dumps(d)


def test_activation_activation_matmul_is_electronic():
    g = helper.make_graph(
        [helper.make_node("MatMul", ["a", "b"], ["y"])], "aa",
        [helper.make_tensor_value_info("a", TensorProto.FLOAT, [3, 4]),
         helper.make_tensor_value_info("b", TensorProto.FLOAT, [4, 2])],
        [helper.make_tensor_value_info("y", TensorProto.FLOAT, [3, 2])],
    )
    model = helper.make_model(g, opset_imports=[helper.make_opsetid("", 17)])
    rep = onnx_import.analyze(model)
    assert rep.nodes[0].placement == "electronic" and rep.nodes[0].macs == 24
    a, b = RNG.uniform(size=(3, 4)), RNG.uniform(size=(4, 2))
    out, r = onnx_import.run(model, {"a": a, "b": b})
    np.testing.assert_allclose(out["y"], a @ b, atol=1e-12)
    assert r["photonic_macs"] == 0


def test_unsupported_ops_are_reported_and_refused():
    g = helper.make_graph(
        [helper.make_node("Conv", ["x", "k"], ["c"], group=2),
         helper.make_node("LpNormalization", ["c"], ["y"])], "bad",
        [helper.make_tensor_value_info("x", TensorProto.FLOAT, [1, 2, 5, 5])],
        [helper.make_tensor_value_info("y", TensorProto.FLOAT, [1, 2, 3, 3])],
        [init("k", 2, 1, 3, 3)],
    )
    model = helper.make_model(g, opset_imports=[helper.make_opsetid("", 17)])
    rep = onnx_import.analyze(model)
    assert [n.placement for n in rep.nodes] == ["unsupported", "unsupported"]
    assert "grouped" in rep.nodes[0].reason
    assert rep.summary()["unsupported_ops"] == ["Conv", "LpNormalization"]
    with pytest.raises(ValueError, match="unsupported"):
        onnx_import.run(model, {"x": np.zeros((1, 2, 5, 5))})


def test_photonic_sim_ideal_equals_reference_and_noise_is_labeled(tmp_path):
    model = conv_model()
    path = tmp_path / "convnet.onnx"
    onnx.save(model, path)
    x = RNG.uniform(-1, 1, (2, 3, 8, 8))
    ref, _ = onnx_import.run(path, {"x": x})
    ideal, r_ideal = onnx_import.run(path, {"x": x}, contract=awen_native.ideal_contract())
    np.testing.assert_allclose(ideal["y"], ref["y"], atol=1e-10)
    noisy, r_noisy = onnx_import.run(path, {"x": x}, contract=awen_native.default_contract(), seed=3)
    assert r_noisy["fidelity"].startswith("simulated")
    assert r_noisy["photonic_macs"] == 2 * 4 * 4 * 4 * 27 + 2 * 64 * 6
    assert not np.array_equal(noisy["y"], ref["y"])
    again, _ = onnx_import.run(path, {"x": x}, contract=awen_native.default_contract(), seed=3)
    np.testing.assert_array_equal(again["y"], noisy["y"])


def test_cli_writes_json(tmp_path, capsys):
    path = tmp_path / "mlp.onnx"
    onnx.save(mlp_model(), path)
    onnx_import.main([str(path), "--json", str(tmp_path / "r.json")])
    assert "photonic MAC fraction" in capsys.readouterr().out
    d = json.loads((tmp_path / "r.json").read_text())
    assert d["summary"]["photonic_nodes"] == 2
