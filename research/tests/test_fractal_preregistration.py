"""H1-H4 pre-registration: configs are complete before any full run (Master Prompt v2, section 10)."""

from pathlib import Path

import pytest
import yaml  # required: CI pins pyyaml

HERE = Path(__file__).resolve().parents[1] / "experiments" / "multiplexing-fractal"
IDS = ["H1", "H2", "H3", "H4"]
# Seeds used by the pilots in draft #128 @ 0974f38 (meshes seed 0, H2 seeds 1-2, boxcount 0-2).
PILOT_SEEDS = {0, 1, 2}


def load(hid: str) -> dict:
    return yaml.safe_load((HERE / "configs" / f"{hid.lower()}.yaml").read_text())


@pytest.mark.parametrize("hid", IDS)
def test_config_is_complete(hid):
    cfg = load(hid)
    assert cfg["id"] == hid and cfg["schema"] == "awen.fractal-prereg-config.v1"
    assert cfg["status"] == "registered"
    assert len(cfg["seeds"]) >= 3 and len(set(cfg["seeds"])) == len(cfg["seeds"])
    assert cfg["hypothesis"].strip() and cfg["falsification"] and cfg["command"].strip()


def test_h1_has_three_tasks_and_nested_seeds():
    cfg = load("H1")
    assert len(cfg["tasks"]) >= 3 and len(cfg["fabrication_noise_seeds"]) >= 5
    assert len(cfg["seeds"]) == cfg["analysis"]["n"] == 5 and cfg["analysis"]["df"] == 4


@pytest.mark.parametrize("hid", IDS)
def test_confirmatory_seeds_never_reuse_pilot_seeds(hid):
    cfg = load(hid)
    seeds = set(cfg["seeds"]) | set(cfg.get("fabrication_noise_seeds", []))
    assert not seeds & PILOT_SEEDS
    assert not set(cfg["seeds"]) & set(cfg.get("fabrication_noise_seeds", []))


def test_h2_candidates_follow_v2():
    assert load("H2")["variants"] == ["baseline", "kron3", "tensor-train"]


def test_pilots_are_disclosed_as_exploratory():
    text = (HERE / "PREREGISTRATION.md").read_text()
    assert "0974f38" in text and "#128" in text and "not confirmatory" in text
    for hid in ["H1", "H2", "H3"]:
        assert load(hid)["pilot"] == {**load(hid)["pilot"], "pr": 128, "commit": "0974f38", "role": "exploratory"}
    assert load("H4")["pilot"] is None

