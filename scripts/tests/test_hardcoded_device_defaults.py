"""awen-spec/hardcoded_device_defaults.yaml must list exactly the device literals in the code."""

import shutil
import sys
from collections import Counter
from pathlib import Path

import pytest
import yaml  # required: CI pins pyyaml==6.0.3

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import hardcoded_defaults as hd

INVENTORY = yaml.safe_load(hd.INVENTORY.read_text())


def copy_tree(tmp_path: Path) -> Path:
    for tree in INVENTORY["scope"]:
        shutil.copytree(ROOT / tree, tmp_path / tree)
    (tmp_path / "awen-spec").mkdir(exist_ok=True)
    for name in ("hardcoded_device_defaults.yaml", "hardware_assumptions.yaml"):
        shutil.copy(ROOT / "awen-spec" / name, tmp_path / "awen-spec" / name)
    return tmp_path


def edit(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    assert text.count(old) == 1, old
    path.write_text(text.replace(old, new))


def test_inventory_matches_code_and_assumptions():
    assert hd.check() == []


def test_inventory_summary():
    assert INVENTORY["assumption_set"] == "awen.hardware-assumptions.v0"
    assert INVENTORY["scope"] == ["awen-compiler/src", "awen-runtime/src"]
    assert not any(item.startswith("awen-runtime") for item in INVENTORY["not_yet_inventoried"])
    assert len(INVENTORY["entries"]) == 176
    assert Counter(e["status"] for e in INVENTORY["entries"]) == {
        "unsourced": 108,
        "numeric_guard": 27,
        "definitional": 37,
        "differs_from_assumption": 3,
        "matches_assumption": 1,
    }
    assert sum(e["field"].startswith("expr:") for e in INVENTORY["entries"]) == 38 + 26
    assert sum(b["file"].startswith("awen-runtime/") for b in INVENTORY["blocks"]) == 21


def test_new_hard_coded_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-compiler/src/noise.rs", "            photons_per_mac: Some(1000.0),\n",
         "            photons_per_mac: Some(1000.0),\n            laser_energy_pj_per_mac: 3.0,\n")
    assert any("new hard-coded device default" in p and "laser_energy_pj_per_mac" in p for p in hd.check(root))


def test_changed_value_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-compiler/src/noise.rs", "rin_db_per_hz: Some(-150.0),", "rin_db_per_hz: Some(-140.0),")
    assert any("rin_db_per_hz" in p and "code has -140.0" in p for p in hd.check(root))


def test_removed_literal_is_reported_stale(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-compiler/src/cost.rs", "            memory_energy_pj_per_byte: 0.5,\n",
         "            memory_energy_pj_per_byte: assumptions.memory_energy(),\n")
    assert any("stale entry" in p and "memory_energy_pj_per_byte" in p for p in hd.check(root))


def test_status_must_agree_with_assumption_values(tmp_path):
    root = copy_tree(tmp_path)
    inv = root / "awen-spec/hardcoded_device_defaults.yaml"
    edit(inv, "field: dac_bits, value: 8, unit: bits, assumption: dac.weight_dac_bits, status: matches_assumption",
         "field: dac_bits, value: 8, unit: bits, assumption: dac.weight_dac_bits, status: unsourced")
    assert any("marked unsourced" in p for p in hd.check(root))


def test_unknown_assumption_key_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    inv = root / "awen-spec/hardcoded_device_defaults.yaml"
    edit(inv, "assumption: laser.rin,", "assumption: laser.not_a_parameter,")
    assert any("not a parameter" in p for p in hd.check(root))


def test_literal_in_a_new_function_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    path = root / "awen-compiler/src/cost.rs"
    path.write_text(path.read_text().replace("#[cfg(test)]\nmod tests", "pub fn new_profile() -> Profile {\n    Profile {\n        laser_power_mw: 9.0,\n    }\n}\n\n#[cfg(test)]\nmod tests", 1))
    assert any("new_profile" in p and "laser_power_mw" in p for p in hd.check(root))


def test_contexts_numbering_and_test_module_cut():
    text = (
        "impl Default for Thing {\n    fn default() -> Self {\n        Self {\n"
        "            gain: 0.9,\n            inner: Inner {\n                gain: 0.8,\n            },\n"
        "            label: 3.0,\n        }\n    }\n}\n\n"
        "fn free() {\n    let x = Y { energy_uj: 0.0, };\n    let y = Y {\n        energy_uj: 0.5,\n    };\n}\n\n"
        "#[cfg(test)]\nmod tests {\n    fn t() { Z {\n        laser_power_mw: 1.0,\n    }; }\n}\n"
    )
    assert hd.literals(text) == {
        ("impl Default for Thing::default", "gain"): 0.9,
        ("impl Default for Thing::default", "gain#2"): 0.8,
        ("free", "energy_uj"): 0.5,
    }


@pytest.mark.parametrize(
    "line,expected",
    [
        ("            laser_power_mw: 2_000.0,", ("laser_power_mw", 2000.0)),
        ("            rin_db_per_hz: Some(-150.0),", ("rin_db_per_hz", -150.0)),
        ("            detection_bandwidth_hz: 10e9,", ("detection_bandwidth_hz", 10e9)),
        ("            shot_noise_fraction: 0.000_05,", ("shot_noise_fraction", 5e-05)),
    ],
)
def test_literal_forms(line, expected):
    match = hd.LITERAL.match(line)
    assert match and (match.group(1), hd.parse_number(match.group(2))) == expected


def test_formulas_are_not_literals():
    assert hd.LITERAL.match("            signal_to_noise_ratio_db: 6.02 * f64::from(bits) + 1.76,") is None


def test_in_expression_fallbacks_are_found():
    text = (
        "impl CostModelInputs {\n    pub fn from_capabilities() -> Self {\n"
        "        let u = profile\n            .map_or(0.05, calibration_error_fraction)\n            .max(0.001);\n"
        "        let t = p.map_or(20.0, |p| p.t).min(1.0e2).clamp(-1.0, 1.0);\n"
        "        let n = count.max(1);\n        let s = xs.fold(0.0, f64::max);\n"
        "        // .max(0.5) in a comment\n    }\n}\n"
    )
    assert hd.literals(text) == {
        ("impl CostModelInputs::from_capabilities", "expr:map_or"): 0.05,
        ("impl CostModelInputs::from_capabilities", "expr:max"): 0.001,
        ("impl CostModelInputs::from_capabilities", "expr:map_or#2"): 20.0,
        ("impl CostModelInputs::from_capabilities", "expr:min"): 100.0,
        ("impl CostModelInputs::from_capabilities", "expr:clamp"): -1.0,
    }


def test_known_fallbacks_are_inventoried():
    entries = {(e["block"], e["field"]): e for e in INVENTORY["entries"]}
    assert entries[("cost_inputs_from_capabilities", "expr:map_or")]["value"] == 0.05
    assert entries[("cost_inputs_from_capabilities", "expr:max")]["value"] == 0.001
    assert entries[("cost_inputs_from_snapshot", "expr:map_or")]["value"] == 0.05
    assert entries[("offline_snapshot_health", "expr:map_or")]["value"] == 20.0
    assert all(entries[k]["status"] == "unsourced" for k in [
        ("cost_inputs_from_capabilities", "expr:map_or"), ("offline_snapshot_health", "expr:map_or")])


def test_changed_fallback_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-compiler/src/capability.rs", ".map_or(20.0, |profile| profile.environment.temperature_c)",
         ".map_or(25.0, |profile| profile.environment.temperature_c)")
    assert any("expr:map_or" in p and "code has 25.0" in p for p in hd.check(root))


def test_new_fallback_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-compiler/src/cost.rs", "let active_fraction = (1.0 - model.disabled_component_fraction).max(0.01);",
         "let active_fraction = (1.0 - model.disabled_component_fraction).max(0.01).min(0.95);")
    assert any("new hard-coded device default" in p and "expr:min" in p for p in hd.check(root))


def test_runtime_literal_change_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-runtime/src/hal_v0.rs", "insertion_loss_db: 1.5,", "insertion_loss_db: 1.25,")
    assert any("hal_v0.rs" in p and "insertion_loss_db" in p for p in hd.check(root))
