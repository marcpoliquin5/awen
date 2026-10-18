"""Lab Agent: workflows, immutable states, branches, provenance and safety gates."""

import json

import pytest

pytest.importorskip("awen_native")
pytest.importorskip("awen_qphotonic")

from awen_lab import (ApprovalRequired, BoundsViolation, HardwareActionRefused, LabAgent, LabStore, OperatingBounds,
                      Policy, StateIntegrityError)
from awen_lab.agent import verify_bundle
from awen_lab.cli import main as cli

GEMM = {"kind": "photonic_gemm", "target": "photonic-sim", "shape_mkn": [32, 128, 16],
        "contract": {"photons_per_mac": 1000.0}, "seeds": [1, 2, 3]}
BOSON = {"kind": "boson_sampling", "target": "qphotonic-sim", "modes": 4, "input": [1, 1, 0, 0],
         "shots": 4000, "unitary_seed": 3, "seeds": [5, 6]}


@pytest.fixture
def agent(tmp_path):
    return LabAgent(tmp_path / "lab")


def test_quick_creates_content_addressed_state_on_branch(agent):
    state = agent.quick(GEMM)
    assert state.hash.startswith("sha256:")
    assert agent.store.branches()["main"] == state.hash
    assert len(state.metrics["runs"]) == 1
    prov = state.to_dict()["provenance"]
    assert prov["evidence"].startswith("simulation only")
    assert prov["packages"]["numpy"]
    with pytest.raises(TypeError):
        state.document["metrics"] = {}  # read-only view


def test_states_are_immutable_on_disk(agent):
    state = agent.quick(GEMM)
    path = agent.store.root / "objects" / (state.hash.split(":")[1] + ".json")
    doc = json.loads(path.read_text())
    doc["metrics"]["runs"][0]["rms_error"] = 0.0
    path.write_text(json.dumps(doc))
    with pytest.raises(StateIntegrityError):
        agent.restore(state.hash)


def test_truth_gemm_passes_oracle_and_noise_budget(agent):
    state = agent.truth(GEMM)
    checks = {c["check"]: c for c in state.metrics["checks"]}
    assert checks["ideal contract equals float64 oracle"]["passed"]
    assert checks["measured / analytic RMS error (noise budget)"]["passed"]
    assert state.metrics["passed"]


def test_truth_boson_sampling(agent):
    state = agent.truth(BOSON)
    assert state.metrics["passed"], state.metrics


def test_repro_is_bit_identical_and_links_parent(agent):
    first = agent.quick(BOSON)
    again = agent.repro(first.hash)
    assert again.metrics["reproduced"] is True
    assert again.parents == [first.hash]


def test_compare_photon_budgets_reports_paired_delta(agent):
    low = {**GEMM, "contract": {"photons_per_mac": 100.0}}
    high = {**GEMM, "contract": {"photons_per_mac": 100000.0}}
    head = agent.quick(GEMM)
    state = agent.compare(low, high)
    m = state.metrics
    assert m["metric"] == "relative_rms_error"
    assert m["paired_delta"]["mean"] < 0  # more photons, less error
    assert m["paired_delta"]["ci95"][1] < 0
    assert "contract.photons_per_mac" in m["states"]["setting_differences"]
    assert len(state.parents) == 3 and state.parents[0] == head.hash


def test_branch_restore_diff_history(agent):
    base = agent.quick(GEMM)
    agent.branch_from("tuning", "main")
    tuned = agent.quick({**GEMM, "contract": {"photons_per_mac": 50000.0}}, branch="tuning")
    assert agent.store.branches()["main"] == base.hash
    assert agent.restore("tuning").hash == tuned.hash
    assert agent.store.history("tuning") == [tuned.hash, base.hash]
    diff = agent.diff("main", "tuning")
    assert diff["setting_differences"]["contract.photons_per_mac"] == {"a": 1000.0, "b": 50000.0}
    assert agent.restore(base.hash.split(":")[1][:10]).hash == base.hash
    with pytest.raises(ValueError):
        agent.branch_from("tuning", "main")


def test_hardware_actions_always_refused_even_with_approval(agent):
    with pytest.raises(HardwareActionRefused):
        agent.quick({**GEMM, "target": "hardware"})
    with pytest.raises(HardwareActionRefused):
        agent.quick({**GEMM, "target": "vendor-chip-0"})
    state = agent.quick(GEMM)
    agent.policy.approve(state.hash, "Reviewer", "approved for publication")
    with pytest.raises(HardwareActionRefused):
        agent.hardware("enable laser at 10 mW")
    assert any(a["action"] == "hardware" and a["decision"] == "refused" for a in agent.policy.audit)


def test_operating_bounds(tmp_path):
    agent = LabAgent(tmp_path / "lab", Policy(OperatingBounds(max_matrix_elements=1000, max_photons_quantum=2)))
    with pytest.raises(BoundsViolation):
        agent.quick(GEMM)
    with pytest.raises(BoundsViolation):
        agent.quick({**BOSON, "input": [1, 1, 1, 0]})
    with pytest.raises(BoundsViolation):
        agent.quick({**GEMM, "shape_mkn": [4, 4, 4], "contract": {"photons_per_mac": 0.01}})
    with pytest.raises(BoundsViolation):
        agent.quick({"kind": "mystery", "target": "photonic-sim"})


def test_publish_requires_approval_and_bundle_verifies(agent, tmp_path):
    state = agent.truth(GEMM)
    with pytest.raises(ApprovalRequired):
        agent.publish(state.hash, tmp_path / "out")
    with pytest.raises(ApprovalRequired):
        agent.policy.approve(state.hash, "", "")
    agent.policy.approve(state.hash, "Reviewer", "truth checks passed")
    path = agent.publish(state.hash, tmp_path / "out")
    assert verify_bundle(path)
    bundle = json.loads(path.read_text())
    assert bundle["approval"]["approver"] == "Reviewer"
    assert "none are hardware measurements" in bundle["statement"]
    with pytest.raises(FileExistsError):
        agent.publish(state.hash, tmp_path / "out")
    assert agent.store.verify_all() >= 2


def test_cli_end_to_end(tmp_path, capsys):
    exp = tmp_path / "exp.json"
    exp.write_text(json.dumps(GEMM))
    store = str(tmp_path / "lab")
    assert cli(["--store", store, "quick", str(exp)]) == 0
    assert cli(["--store", store, "repro", "main"]) == 0
    assert json.loads(capsys.readouterr().out.strip().splitlines()[-1])["reproduced"] is True
    assert cli(["--store", store, "hardware", "power on"]) == 3
    assert cli(["--store", store, "publish", "main", "--out", str(tmp_path / "pub")]) == 3
    assert cli(["--store", store, "publish", "main", "--out", str(tmp_path / "pub"),
                "--approver", "Reviewer", "--reason", "ok"]) == 0
    assert LabStore(store).verify_all() >= 3
