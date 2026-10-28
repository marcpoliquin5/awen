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
    assert INVENTORY["scope"] == ["awen-compiler/src", "awen-runtime/src", "awen-native/src",
                                  "awen-mlir/include", "awen-mlir/lib", "awen-mlir/tools",
                                  "awen-models/awen", "awen-models/awen_models",
                                  "awen-physics/awen_physics", "awen-systems/awen_systems",
                                  "awen-qphotonic/awen_qphotonic", "awen-lab/awen_lab",
                                  "awen-studio/awen_studio", "awen-ecosystem/python_awen/awen_py"]
    assert not any(item.startswith(("awen-runtime", "awen-native", "awen-mlir")) for item in INVENTORY["not_yet_inventoried"])
    not_yet = " ".join(INVENTORY["not_yet_inventoried"])
    assert not any(name in not_yet for name in ("awen-models,", "awen-physics", "awen-systems", "awen-qphotonic",
                                                  "awen-lab", "awen-studio"))
    assert len(INVENTORY["entries"]) == 236
    assert Counter(e["status"] for e in INVENTORY["entries"]) == {
        "unsourced": 156,
        "numeric_guard": 27,
        "definitional": 48,
        "differs_from_assumption": 4,
        "matches_assumption": 1,
    }
    assert sum(e["field"].startswith("expr:") for e in INVENTORY["entries"]) == 38 + 26
    assert sum(b["file"].startswith("awen-runtime/") for b in INVENTORY["blocks"]) == 28


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


def test_widened_pattern_names():
    for name in ("dark_count_hz", "saturation_count_khz", "hysteresis_radians", "waveguide_loss_threshold_db",
                 "detector_dark_current_threshold_hz", "coherence_time_us", "phase_change_latency_ns",
                 "measurement_readout_latency_ns", "max_phase_voltage", "average_fidelity", "launch_latency_ns"):
        assert hd.is_device_field(name), name
    for name in ("latency_calibration_factor", "label", "photon_count", "t1_ns"):
        assert not hd.is_device_field(name), name


def test_hal_v0_device_values_are_inventoried():
    entries = {(e["block"], e["field"]): e["value"] for e in INVENTORY["entries"]}
    assert entries[("rt_hal_v0_detector_calibration_default", "dark_count_hz")] == 100.0
    assert entries[("rt_hal_v0_detector_calibration_default", "saturation_count_khz")] == 10.0
    assert entries[("rt_hal_v0_phase_calibration_default", "hysteresis_radians")] == 0.01
    assert entries[("rt_hal_v0_fault_detection_thresholds_default", "waveguide_loss_threshold_db")] == 0.5
    assert entries[("rt_hal_v0_fault_detection_thresholds_default", "detector_dark_current_threshold_hz")] == 1000.0
    assert entries[("rt_hal_v0_device_capabilities_default", "coherence_time_us")] == 10000
    assert entries[("rt_hal_v0_device_capabilities_default", "phase_change_latency_ns")] == 100
    assert entries[("rt_hal_v0_device_capabilities_default", "measurement_readout_latency_ns")] == 1000
    assert entries[("rt_hal_v0_device_capabilities_default", "max_phase_voltage")] == 10.0
    assert entries[("rt_hal_v0_device_metrics_default", "average_fidelity")] == 0.95


def test_widened_runtime_literal_change_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-runtime/src/hal_v0.rs", "dark_count_hz: 100.0,", "dark_count_hz: 50.0,")
    assert any("dark_count_hz" in p and "code has 50.0" in p for p in hd.check(root))


def test_cpp_named_defaults_form():
    text = (
        "namespace {\n"
        "constexpr int64_t kDialectMajor = 1;\n"
        "constexpr int64_t kDefaultEffectiveBits = 8;\n"
        "  static constexpr double kDefaultLaserPowerMw = 2.5e1;\n"
        "constexpr int64_t kDefaultTile = 1'024;\n"
        "// constexpr int64_t kDefaultCommented = 3;\n"
        "  if (!std::isfinite(x) || x <= 0.0)\n"
        "}\n"
    )
    assert hd.cpp_defaults(text) == {
        ("<file scope>", "kDefaultEffectiveBits"): 8.0,
        ("<file scope>", "kDefaultLaserPowerMw"): 25.0,
        ("<file scope>", "kDefaultTile"): 1024.0,
    }


def test_mlir_defaults_are_inventoried():
    blocks = {b["id"]: b for b in INVENTORY["blocks"]}
    assert blocks["mlir_passes_file_scope"]["file"] == "awen-mlir/lib/Transforms/Passes.cpp"
    entries = {(e["block"], e["field"]): e for e in INVENTORY["entries"]}
    bits = entries[("mlir_passes_file_scope", "kDefaultEffectiveBits")]
    assert (bits["value"], bits["assumption"], bits["status"]) == (8, "system.pace_enob", "differs_from_assumption")
    tile = entries[("mlir_passes_file_scope", "kDefaultTile")]
    assert (tile["value"], tile["status"]) == (128, "unsourced")


def test_native_scanned_without_entries():
    assert not any(k[0].startswith("awen-native/") for k in hd.scan(hd.ROOT, ["awen-native/src"]))
    assert not any(b["file"].startswith("awen-native/") for b in INVENTORY["blocks"])


def test_mlir_default_change_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-mlir/lib/Transforms/Passes.cpp", "kDefaultTile = 128;", "kDefaultTile = 64;")
    assert any("kDefaultTile" in p and "code has 64.0" in p for p in hd.check(root))


def test_new_mlir_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-mlir/lib/Transforms/Passes.cpp", "constexpr int64_t kDefaultTile = 128;\n",
         "constexpr int64_t kDefaultTile = 128;\nconstexpr double kDefaultPhaseNoiseRad = 0.01;\n")
    assert any("new hard-coded device default" in p and "kDefaultPhaseNoiseRad" in p for p in hd.check(root))


def test_new_native_literal_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    path = root / "awen-native/src/lib.rs"
    path.write_text(path.read_text() + "\nfn reference() -> Contract {\n    Contract {\n        laser_power_mw: 5.0,\n    }\n}\n")
    assert any("awen-native/src/lib.rs" in p and "laser_power_mw" in p for p in hd.check(root))


# ---------------------------------------------------------------- Python (awen-models)


def test_python_entries_in_inventory():
    files = {b["id"]: b["file"] for b in INVENTORY["blocks"]}
    py = [e for e in INVENTORY["entries"] if files[e["block"]].startswith("awen-models/")]
    assert Counter(e["field"] for e in py) == {"tile_k": 7, "DEFAULT_WAVELENGTH_M": 1}
    assert all(e["status"] == "unsourced" for e in py)


def test_py_defaults_forms():
    text = (
        "import math\n"
        "PLANCK = 6.62607015e-34\n"
        "DEFAULT_WAVELENGTH_M = 1550e-9\n"
        "LASER_POWER_MW = -3\n"
        "class Settings:\n"
        "    tile_k: int = 64\n"
        "    lr: float = 0.002\n"
        "    def run(self, phase_sigma_rad: float = 0.01, *, adc_bits: int = 8, seed: int = 0):\n"
        "        def inner(tile_k=32):\n"
        "            return tile_k\n"
        "        return inner()\n"
        "def generate(prompt, temperature=0.8, temperature_k=300.0, gain_factor=2.0, tile_m=None):\n"
        "    return math.pi\n"
    )
    assert hd.py_defaults(text) == {
        ("<file scope>", "DEFAULT_WAVELENGTH_M"): 1550e-9,
        ("<file scope>", "LASER_POWER_MW"): -3.0,
        ("class Settings", "tile_k"): 64.0,
        ("Settings.run", "phase_sigma_rad"): 0.01,
        ("Settings.run", "adc_bits"): 8.0,
        ("generate", "temperature_k"): 300.0,
    }


def test_new_python_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-models/awen_models/photon_training.py", "DEFAULT_WAVELENGTH_M = 1550e-9\n",
         "DEFAULT_WAVELENGTH_M = 1550e-9\nMODULATOR_ENERGY_PJ = 1.5\n")
    assert any("new hard-coded device default" in p and "MODULATOR_ENERGY_PJ" in p for p in hd.check(root))


def test_changed_python_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-models/awen/photonic.py", "tile_k: int = 64):", "tile_k: int = 128):")
    assert any("PhotonicGemmExecutor.__init__" in p and "code has 128.0" in p for p in hd.check(root))


def test_python_tests_directory_is_not_scanned(tmp_path):
    root = copy_tree(tmp_path)
    tests = root / "awen-models/awen_models/tests"
    tests.mkdir()
    (tests / "test_fixture.py").write_text("ADC_BITS = 6\n")
    assert hd.check(root) == []


# ---------------------------------------------------------------- Python (awen-physics, awen-systems)


def test_physics_and_systems_entries_in_inventory():
    files = {b["id"]: b["file"] for b in INVENTORY["blocks"]}
    physics = [e for e in INVENTORY["entries"] if files[e["block"]].startswith("awen-physics/")]
    systems = {(files[e["block"]].rsplit("/", 1)[1], e["field"]): e["status"]
               for e in INVENTORY["entries"] if files[e["block"]].startswith("awen-systems/")}
    assert physics == []
    assert systems == {
        ("fabric.py", "latency_s"): "definitional",
        ("fabric.py", "energy_j_per_bit"): "definitional",
        ("fabric.py", "static_power_w"): "definitional",
        ("fabric.py", "tx_power_dbm"): "unsourced",
        ("memory.py", "energy_j"): "definitional",
    }


def test_new_physics_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    path = root / "awen-physics/awen_physics/rings.py"
    path.write_text(path.read_text() + "\n\ndef ring_heater(thermal_tuning_power_mw: float = 2.5):\n    return thermal_tuning_power_mw\n")
    assert any("new hard-coded device default" in p and "ring_heater" in p for p in hd.check(root))


def test_changed_systems_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-systems/awen_systems/fabric.py", "    tx_power_dbm: float = 0.0\n", "    tx_power_dbm: float = 3.0\n")
    assert any("class FabricConfig" in p and "code has 3.0" in p for p in hd.check(root))


def test_systems_tests_outside_package_are_not_in_scope():
    assert all(not tree.endswith("/tests") for tree in INVENTORY["scope"])
    assert (ROOT / "awen-systems/tests").is_dir() and not (ROOT / "awen-systems/awen_systems/tests").exists()


# ---------------------------------------------------------------- Python (awen-qphotonic)


def test_qphotonic_entries_in_inventory():
    files = {b["id"]: b["file"] for b in INVENTORY["blocks"]}
    qp = {(files[e["block"]].rsplit("/", 1)[1], e["field"]): (e["value"], e["status"])
          for e in INVENTORY["entries"] if files[e["block"]].startswith("awen-qphotonic/")}
    assert qp == {
        ("detection.py", "dark_count"): (0.0, "definitional"),
        ("kernel.py", "noise"): (0.15, "unsourced"),
        ("resources.py", "dark_count_per_gate"): (0.0, "definitional"),
    }


def test_new_qphotonic_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    path = root / "awen-qphotonic/awen_qphotonic/detection.py"
    path.write_text(path.read_text() + "\n\ndef pnr_detection(dark_count_rate: float = 0.01):\n    return dark_count_rate\n")
    assert any("new hard-coded device default" in p and "pnr_detection" in p for p in hd.check(root))


def test_changed_qphotonic_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-qphotonic/awen_qphotonic/resources.py", "    dark_count_per_gate: float = 0.0\n",
         "    dark_count_per_gate: float = 1e-5\n")
    assert any("class ExperimentSpec" in p and "code has 1e-05" in p for p in hd.check(root))


def test_qphotonic_tests_outside_package_are_not_in_scope():
    assert (ROOT / "awen-qphotonic/tests").is_dir() and not (ROOT / "awen-qphotonic/awen_qphotonic/tests").exists()


# ---------------------------------------------------------------- Python (awen-lab)


def test_lab_entries_in_inventory():
    files = {b["id"]: b["file"] for b in INVENTORY["blocks"]}
    lab = {(files[e["block"]].rsplit("/", 1)[1], e["field"]): (e["value"], e["status"])
           for e in INVENTORY["entries"] if files[e["block"]].startswith("awen-lab/")}
    assert lab == {
        ("policy.py", "min_photons_per_mac"): (1.0, "unsourced"),
        ("policy.py", "max_photons_per_mac"): (1e9, "unsourced"),
        ("policy.py", "max_photons_quantum"): (4.0, "unsourced"),
    }


def test_new_lab_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    path = root / "awen-lab/awen_lab/sims.py"
    path.write_text(path.read_text() + "\n\ndef lossy_gemm(insertion_loss_db: float = 3.0):\n    return insertion_loss_db\n")
    assert any("new hard-coded device default" in p and "lossy_gemm" in p for p in hd.check(root))


def test_changed_lab_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-lab/awen_lab/policy.py", "    max_photons_quantum: int = 4\n",
         "    max_photons_quantum: int = 6\n")
    assert any("class OperatingBounds" in p and "code has 6" in p for p in hd.check(root))


def test_lab_tests_outside_package_are_not_in_scope():
    assert (ROOT / "awen-lab/tests").is_dir() and not (ROOT / "awen-lab/awen_lab/tests").exists()


# ---------------------------------------------------------------- Python (awen-studio)


def test_studio_entries_in_inventory():
    files = {b["id"]: b["file"] for b in INVENTORY["blocks"]}
    studio = {(files[e["block"]].rsplit("/", 1)[1], e["field"]): (e["value"], e["status"])
              for e in INVENTORY["entries"] if files[e["block"]].startswith("awen-studio/")}
    assert studio == {("app.py", "tile_k"): (64.0, "unsourced")}


def test_pydantic_and_dataclass_field_defaults_are_read():
    found = hd.py_defaults(
        "from pydantic import BaseModel, Field\n"
        "import dataclasses\n"
        "class A(BaseModel):\n"
        "    tile_k: int = Field(32, ge=1)\n"
        "    phase_sigma_rad: float = Field(default=0.01)\n"
        "    m: int = Field(16, ge=1)\n"
        "    noise: dict = Field(default_factory=dict)\n"
        "@dataclasses.dataclass\n"
        "class B:\n"
        "    insertion_loss_db: float = dataclasses.field(default=3.0)\n"
        "TILE = Field(8)\n"
        "tile_n = Field(8)\n"
    )
    assert found == {("class A", "tile_k"): 32.0, ("class A", "phase_sigma_rad"): 0.01,
                     ("class B", "insertion_loss_db"): 3.0}


def test_changed_studio_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-studio/awen_studio/app.py", "    tile_k: int = Field(64, ge=1, le=MAX_GEMM_DIM)\n",
         "    tile_k: int = Field(32, ge=1, le=MAX_GEMM_DIM)\n")
    assert any("class CompareRequest" in p and "code has 32" in p for p in hd.check(root))


def test_new_studio_field_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    path = root / "awen-studio/awen_studio/app.py"
    path.write_text(path.read_text() + "\n\nclass LossyRequest(BaseModel):\n"
                    "    insertion_loss_db: float = Field(3.0, ge=0)\n")
    assert any("new hard-coded device default" in p and "LossyRequest" in p for p in hd.check(root))


def test_studio_tests_outside_package_are_not_in_scope():
    assert (ROOT / "awen-studio/tests").is_dir() and not (ROOT / "awen-studio/awen_studio/tests").exists()


# ---------------------------------------------------------------- Python (awen-ecosystem)


def test_ecosystem_entries_in_inventory():
    files = {b["id"]: b["file"] for b in INVENTORY["blocks"]}
    eco = {(files[e["block"]].rsplit("/", 1)[1], e["field"]): (e["value"], e["status"])
           for e in INVENTORY["entries"] if files[e["block"]].startswith("awen-ecosystem/")}
    assert eco == {("device.py", "tile_k"): (64.0, "unsourced"),
                   ("runtime.py", "photonic_tile_k"): (64.0, "unsourced"),
                   ("torch_backend.py", "photonic_tile_k"): (64.0, "unsourced")}


def test_prefixed_tile_name_is_matched():
    assert hd.is_py_device_field("photonic_tile_k") and hd.is_py_device_field("tile_k")
    assert not hd.is_py_device_field("tiles") and not hd.is_py_device_field("projectile_count")
    found = hd.py_defaults("class Opts:\n    photonic_tile_k: int = 32\n    max_abs_error: float = 1.0e-5\n")
    assert found == {("class Opts", "photonic_tile_k"): 32.0}


def test_changed_ecosystem_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    edit(root / "awen-ecosystem/python_awen/awen_py/runtime.py", "    photonic_tile_k: int = 64\n",
         "    photonic_tile_k: int = 16\n")
    assert any("class ExecutionOptions" in p and "code has 16" in p for p in hd.check(root))


def test_new_ecosystem_default_is_caught(tmp_path):
    root = copy_tree(tmp_path)
    path = root / "awen-ecosystem/python_awen/awen_py/device.py"
    path.write_text(path.read_text() + "\n\ndef lossy(insertion_loss_db: float = 3.0) -> float:\n"
                    "    return insertion_loss_db\n")
    assert any("new hard-coded device default" in p and "lossy" in p for p in hd.check(root))


def test_ecosystem_tests_and_examples_outside_package_are_not_in_scope():
    package = ROOT / "awen-ecosystem/python_awen"
    assert (package / "tests").is_dir() and (package / "examples").is_dir()
    assert not (package / "awen_py/tests").exists()
    assert "awen-ecosystem/python_awen/examples (scripts)" in INVENTORY["not_yet_inventoried"]
