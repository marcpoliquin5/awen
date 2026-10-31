"""Capability report for torch.compile(backend=awen) runs on the photonic simulator."""

import math

import pytest

awen_native = pytest.importorskip("awen_native")
torch = pytest.importorskip("torch")

from awen_py.compile_report import CompileCapture, build_compile_capability_report, fallback_reason  # noqa: E402


class Mlp(torch.nn.Module):
    def __init__(self):
        super().__init__()
        self.fc1 = torch.nn.Linear(100, 16)
        self.fc2 = torch.nn.Linear(16, 4)

    def forward(self, x):
        return self.fc2(torch.relu(self.fc1(x)))


def _run(noise, seed=0, options=None):
    torch._dynamo.reset()
    torch.manual_seed(3)
    model = Mlp().eval()
    x = torch.randn(5, 100)
    capture = CompileCapture({"target": "photonic", "photonic_noise": noise, "seed": seed, **(options or {})})
    compiled = torch.compile(model, backend=capture)
    with torch.no_grad():
        reference, output = model(x), compiled(x)
    return capture, output, reference


def test_ideal_contract_report_lists_lowered_fallback_and_conversions():
    capture, output, reference = _run(awen_native.ideal_contract())
    report = build_compile_capability_report(capture, output, reference)
    assert report["execution"]["physical"] is False and report["execution"]["evidence_level"] == "L1"
    assert [row["operation"] for row in report["lowered"]] == ["linear", "linear"]
    first, second = report["lowered"]
    assert (first["rows"], first["inner"], first["outputs"]) == (5, 100, 16)
    assert first["macs"] == 5 * 100 * 16 and second["macs"] == 5 * 16 * 4
    tiles = math.ceil(100 / 64)
    assert first["adc_conversions"] == 5 * 16 * tiles
    assert report["conversions"]["dac_input"] == 5 * 100 + 5 * 16
    assert report["conversions"]["dac_weight"] == 100 * 16 + 16 * 4
    assert report["conversions"]["adc"] == 5 * 16 * tiles + 5 * 4 * 1
    assert [row["reason"] for row in report["fallback"] if "relu" in row["target"]]
    assert report["counts"]["lowered_nodes"] == 2 and report["counts"]["fallback_nodes"] >= 1
    assert report["deviation_vs_reference"]["max_abs_diff"] < 1e-5
    assert report["deviation_vs_reference"]["top1_agreement"] == 1.0
    assert report["energy"].startswith("not estimated")


def test_default_contract_is_reproducible_and_deviates():
    first = build_compile_capability_report(*_run(None, seed=4))
    second = build_compile_capability_report(*_run(None, seed=4))
    assert first["deviation_vs_reference"] == second["deviation_vs_reference"]
    assert first["deviation_vs_reference"]["max_abs_diff"] > 0
    assert first["precision_and_calibration"]["dac_bits"] == awen_native.default_contract()["dac_bits"]


def test_report_refuses_non_photonic_targets_and_missing_traces():
    capture, output, reference = _run(None, options={"target": "cpu"})
    with pytest.raises(ValueError, match="photonic"):
        build_compile_capability_report(capture, output, reference)
    capture, output, reference = _run(awen_native.ideal_contract())
    with pytest.raises(ValueError, match="trace"):
        build_compile_capability_report(capture, output, reference, traces=())
    capture.compiled.append(capture.compiled[0])
    with pytest.raises(ValueError, match="one compiled graph"):
        build_compile_capability_report(capture, output, reference)


def test_fallback_reasons_cover_known_kinds():
    assert "activations" in fallback_reason("torch._c._nn.scaled_dot_product_attention")
    assert fallback_reason("view") == "view or layout change; no arithmetic"
    assert "no AWEN lowering" in fallback_reason("torch.special.erfinv")


class TwinHeads(torch.nn.Module):
    """Two linear layers with identical weights applied to the same input."""

    def __init__(self):
        super().__init__()
        self.a = torch.nn.Linear(32, 8, bias=False)
        self.b = torch.nn.Linear(32, 8, bias=False)
        self.b.weight.data.copy_(self.a.weight.data)

    def forward(self, x):
        return torch.stack((self.a(x), self.b(x)))


def _twins(seed_policy):
    torch._dynamo.reset()
    torch.manual_seed(5)
    model = TwinHeads().eval()
    x = torch.randn(4, 32)
    capture = CompileCapture({"target": "photonic", "seed": 9, "seed_policy": seed_policy})
    with torch.no_grad():
        output = torch.compile(model, backend=capture)(x)
        reference = model(x)
    return capture, output, reference


def test_shared_seed_correlates_layer_noise_and_per_gemm_seeds_decorrelate_it():
    from awen_py.torch_backend import derive_gemm_seed

    capture, shared, reference = _twins("shared")
    report = build_compile_capability_report(capture, shared, reference)
    assert report["execution"]["seed_policy"] == "shared"
    assert [row["seed"] for row in report["lowered"]] == [9, 9]
    assert torch.equal(shared[0], shared[1])  # same weights, input and seed: identical noise draws
    assert not torch.equal(shared[0], reference[0])

    capture, independent, reference = _twins("per_gemm")
    report = build_compile_capability_report(capture, independent, reference)
    assert report["execution"]["seed_policy"] == "per_gemm"
    assert [row["seed"] for row in report["lowered"]] == [derive_gemm_seed(9, 0), derive_gemm_seed(9, 1)]
    assert not torch.equal(independent[0], independent[1])
    _, again, _ = _twins("per_gemm")
    assert torch.equal(independent, again)  # deterministic


def test_per_gemm_policy_keeps_the_ideal_contract_exact():
    capture, output, reference = _run(awen_native.ideal_contract(), options={"seed_policy": "per_gemm"})
    report = build_compile_capability_report(capture, output, reference)
    assert report["deviation_vs_reference"]["max_abs_diff"] < 1e-5
    assert len({row["seed"] for row in report["lowered"]}) == 2
