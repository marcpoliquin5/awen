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
    for block in INVENTORY["blocks"]:
        dest = tmp_path / block["file"]
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(ROOT / block["file"], dest)
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
    assert len(INVENTORY["entries"]) == 40
    assert Counter(e["status"] for e in INVENTORY["entries"]) == {
        "unsourced": 36,
        "differs_from_assumption": 3,
        "matches_assumption": 1,
    }


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


def test_block_anchor_must_be_unique():
    with pytest.raises(ValueError, match="found 2 times"):
        hd.block_lines("fn a() {\n}\nfn a() {\n}\n", "fn a()")


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
