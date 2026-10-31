"""Tests for scripts/phase1_devices.py, the Phase 1 device check (run: python -m pytest scripts/tests).

The unit tests use a fake runtime, so they need no torch. The end-to-end tests run the real Runtime and are
skipped when awen, torch or awen_native is not installed.
"""

import importlib.util
import json
import re
from pathlib import Path
from types import SimpleNamespace

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("phase1_devices", ROOT / "scripts" / "phase1_devices.py")
p1 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(p1)

from test_fresh_clone_install import DOC, STEP, WORKFLOW, commands, doc_section_commands, script_commands  # noqa: E402

PROMPT = "AB"
CAPABILITY = {
    "report": "awen.runtime-capability-report.v1",
    "execution": {"evidence_level": "L1", "physical": False, "fidelity": "simulated (compact model)"},
    "lowered": [{"name": "w"}, {"name": "v"}],
    "fallback": [{"op": "ln"}],
    "mac_split": {"photonic_sim_fraction": 0.9},
    "conversions": {"dac_input": 1, "dac_weight": 2, "adc": 3, "counting_rule": "x"},
    "deviation_vs_cpu": {"top1_agreement": 1.0},
    "energy": "not estimated",
}


class FakeLoaded:
    def __init__(self, device, noise):
        resolved = {"auto": "cpu"}.get(device, device)
        fidelity = "simulated" if device == "photonic-sim" else "measured"
        self.execution = SimpleNamespace(device=resolved, fidelity=fidelity, gemm_backend=f"fake-{resolved}")
        self.noise = noise

    def generate(self, prompt, max_new_tokens, seed):
        fill = "s" if self.execution.device == "photonic-sim" and self.noise is None else "c"
        return prompt + fill * max_new_tokens

    def capability_report(self, text):
        return CAPABILITY


def fake_runtime(device, seed, noise=None):
    return SimpleNamespace(load_model=lambda model: FakeLoaded(device, noise))


def run(devices, cuda=False, require_cuda=False, noise=None, factory=fake_runtime):
    return p1.run_all(devices, "awen-mini", PROMPT, 4, 0, require_cuda, cuda, factory, noise)


def test_all_devices_pass_and_cuda_skips_without_gpu():
    results = run(["cpu", "auto", "photonic-sim", "cuda"])
    assert [results[d]["status"] for d in ("cpu", "auto", "photonic-sim")] == ["passed"] * 3
    assert results["cuda"] == {"requested": "cuda", "status": "skipped", "error": "torch.cuda.is_available() is False"}
    assert results["auto"]["device"] == "cpu"
    cap = results["photonic-sim"]["capability"]
    assert cap["lowered_gemms"] == 2 and cap["host_fallbacks"] == 1
    assert cap["conversions"] == {"dac_input": 1, "dac_weight": 2, "adc": 3}


def test_require_cuda_turns_the_skip_into_a_failure():
    assert run(["cuda"], require_cuda=True)["cuda"]["status"] == "failed"


def test_auto_must_resolve_to_cuda_when_present():
    results = run(["cpu", "auto"], cuda=True)
    assert results["auto"]["status"] == "failed"
    assert "expected 'cuda'" in results["auto"]["error"]


def test_auto_on_cpu_must_reproduce_the_cpu_text():
    def factory(device, seed, noise=None):
        loaded = FakeLoaded(device, noise)
        if device == "auto":
            loaded.generate = lambda prompt, max_new_tokens, seed: prompt + "x" * max_new_tokens
        return SimpleNamespace(load_model=lambda model: loaded)

    results = run(["cpu", "auto"], factory=factory)
    assert results["auto"]["status"] == "failed"
    assert "differs from the cpu text" in results["auto"]["error"]


def test_ideal_contract_photonic_text_must_equal_cpu_text():
    assert run(["cpu", "photonic-sim"], noise={"ideal": True})["photonic-sim"]["status"] == "passed"

    def noisy(device, seed, noise=None):  # ignores the contract, so the text differs from cpu
        return fake_runtime(device, seed, None)

    results = run(["cpu", "photonic-sim"], noise={"ideal": True}, factory=noisy)
    assert results["photonic-sim"]["status"] == "failed"
    assert "ideal contract" in results["photonic-sim"]["error"]


def test_load_errors_are_recorded_as_failures():
    def broken(device, seed, noise=None):
        raise RuntimeError("no native extension")

    results = run(["photonic-sim"], factory=broken)
    assert results["photonic-sim"] == {"requested": "photonic-sim", "status": "failed",
                                       "error": "RuntimeError: no native extension"}


def test_check_result_rejects_wrong_length_fidelity_and_missing_lowering():
    base = {"requested": "cpu", "device": "cpu", "fidelity": "measured", "text": PROMPT + "cccc"}
    assert p1.check_result(base, PROMPT, 4) == []
    assert "expected the prompt plus 4" in p1.check_result({**base, "text": PROMPT + "c"}, PROMPT, 4)[0]
    assert "expected 'measured'" in p1.check_result({**base, "fidelity": "simulated"}, PROMPT, 4)[0]
    assert "expected 'cpu'" in p1.check_result({**base, "device": "cuda"}, PROMPT, 4)[0]
    sim = {"requested": "photonic-sim", "device": "photonic-sim", "fidelity": "simulated", "text": PROMPT + "ssss",
           "capability": {"evidence_level": "L1", "physical": False, "lowered_gemms": 0}}
    assert p1.check_result(sim, PROMPT, 4) == ["no GEMM was lowered to the photonic simulator"]
    sim["capability"].update(physical=True, lowered_gemms=3)
    assert p1.check_result(sim, PROMPT, 4) == ["capability report must say evidence level L1 and physical false"]


def test_parse_devices_orders_cpu_first_and_rejects_unknown_or_repeated():
    assert p1.parse_devices("photonic-sim,auto,cpu") == ["cpu", "auto", "photonic-sim"]
    for bad in ("tpu", "cpu,cpu", ""):
        with pytest.raises(Exception):
            p1.parse_devices(bad)


def test_doc_commands_run_in_the_fresh_clone_script_and_records_are_uploaded():
    def without_out(command):
        return re.sub(r"\s--out\s+\S+", "", command)

    doc = doc_section_commands("Run the three Phase 1 devices")
    assert [c.split()[1] for c in doc] == ["scripts/phase1_devices.py"] * 2
    script = [without_out(c) for c in script_commands()]
    start = script.index(without_out(doc[0]))
    assert script[start:start + 2] == [without_out(c) for c in doc]
    assert start > script.index("python scripts/examples_cpu.py --run")
    workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
    (job,) = workflow["jobs"].values()
    names = [s.get("name") for s in job["steps"]]
    step = job["steps"][names.index(STEP)]
    upload = job["steps"][names.index(STEP) + 1]
    assert upload["uses"].startswith("actions/upload-artifact@")
    assert upload["with"]["path"].rstrip("/") == step["env"]["PHASE1_RECORD_DIR"]


def test_laptop_cuda_command_is_documented():
    laptop = "python scripts/phase1_devices.py --require-cuda --out phase1-devices-cuda.json"
    assert laptop in commands(DOC.read_text(encoding="utf-8"))


NEEDS_RUNTIME = pytest.mark.skipif(
    any(importlib.util.find_spec(m) is None for m in ("torch", "awen", "awen_models", "awen_native")),
    reason="needs torch, awen, awen_models and awen_native installed",
)


@NEEDS_RUNTIME
def test_end_to_end_cpu_auto_and_photonic_sim(tmp_path):
    out = tmp_path / "record.json"
    assert p1.main(["--devices", "cpu,auto,photonic-sim", "--max-new-tokens", "8", "--out", str(out)]) == 0
    record = json.loads(out.read_text())
    assert record["schema"] == p1.SCHEMA and record["commit"]
    assert [d["status"] for d in record["devices"]] == ["passed"] * 3
    sim = record["devices"][2]
    assert sim["capability"]["evidence_level"] == "L1" and sim["capability"]["lowered_gemms"] > 0
    assert sim["capability"]["energy"].startswith("not estimated")


@NEEDS_RUNTIME
def test_end_to_end_ideal_contract_matches_cpu(tmp_path):
    out = tmp_path / "ideal.json"
    assert p1.main(["--devices", "cpu,photonic-sim", "--ideal", "--max-new-tokens", "8", "--out", str(out)]) == 0
    cpu, sim = json.loads(out.read_text())["devices"]
    assert sim["text"] == cpu["text"]
    assert sim["capability"]["deviation_vs_cpu"]["top1_agreement"] == 1.0
