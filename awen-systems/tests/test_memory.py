import json
import math

import numpy as np
import pytest

from awen_systems import memory as m
from awen_systems import kvcache as kv


def test_every_technology_passes_provenance_rules():
    for name, tech in m.TECHNOLOGIES.items():
        assert m.validate_technology(name, tech) == [], name


def test_literature_values_are_the_cited_ones():
    pcm = m.TECHNOLOGIES["phase_change_photonic"]["params"]
    assert pcm["levels"]["value"] == 8 and pcm["levels"]["citation"] == "rios2015"
    assert pcm["write_energy_j_per_cell"]["value"] == pytest.approx(13.4e-12)
    hbm = m.TECHNOLOGIES["dram_hbm2"]["params"]["read_energy_j_per_bit"]
    assert hbm["value"] == pytest.approx(3.97e-12) and hbm["citation"] == "oconnor2017"
    for c in m.CITATIONS.values():
        assert c["doi"].startswith("10.")


def test_validator_rejects_uncited_literature_value():
    bad = json.loads(json.dumps(m.TECHNOLOGIES["optical_rram"]))
    bad["params"]["levels"] = m.P(16, "count", "literature-reported", citation="made-up", locator="p1")
    assert any("known citation" in e for e in m.validate_technology("bad", bad))


def test_unset_parameters_refuse_to_simulate():
    sim = m.MemorySimulator("optical_rram", 8)
    with pytest.raises(ValueError, match="no bundled verified value"):
        sim.write(np.arange(4), np.zeros(4))


def test_schema_document_is_json_and_matches_shipped_file():
    import importlib.resources as ir
    shipped = json.loads(ir.files("awen_systems").joinpath("memory_technologies.v1.json").read_text())
    assert shipped == json.loads(json.dumps(m.schema_document()))


def test_physics_helpers():
    # Q = 1e6 at 1550 nm -> tau = Q*lambda/(2 pi c) ~ 0.823 ns
    assert m.photon_lifetime_s(1e6, 1550e-9) == pytest.approx(1e6 * 1550e-9 / (2 * math.pi * 299792458.0))
    assert m.delay_line_round_trip_s(1.0, 1.0) == pytest.approx(1 / 299792458.0)
    with pytest.raises(ValueError):
        m.photon_lifetime_s(-1, 1e-6)


OV = {"levels": 16, "read_energy_j_per_bit": 1e-12, "write_energy_j_per_cell": 2e-12,
      "read_latency_s": 1e-9, "write_latency_s": 5e-9, "read_noise_sigma": 0.0,
      "endurance_cycles": 3, "retention_s": 1e-6}


def test_quantization_energy_and_latency_bookkeeping():
    sim = m.MemorySimulator("optical_rram", 10, overrides=OV)
    x = np.linspace(-1, 1, 10)
    sim.write(np.arange(10), x)
    y = sim.read(np.arange(10))
    assert np.max(np.abs(y - x)) <= (2 / 15) / 2 + 1e-12  # half an LSB
    assert sim.stats.energy_j == pytest.approx(10 * 2e-12 + 10 * 4 * 1e-12)  # 16 levels -> 4 bits
    assert sim.stats.busy_s == pytest.approx(6e-9)
    assert sim.provenance()["levels"] == "user-override"


def test_endurance_limit():
    sim = m.MemorySimulator("optical_rram", 2, overrides=OV)
    for _ in range(3):
        sim.write(np.arange(2), np.zeros(2))
    with pytest.raises(RuntimeError, match="endurance"):
        sim.write(np.arange(2), np.zeros(2))


def test_volatile_decay_is_exponential():
    sim = m.MemorySimulator("microring_fading", 1, overrides=OV)
    sim.write(np.array([0]), np.array([1.0]))
    sim.advance(1e-6 - OV["write_latency_s"])  # age at read == retention (read latency counted after read)
    y = sim.read(np.array([0]))
    assert y[0] == pytest.approx(math.exp(-1), rel=1e-9)


def test_read_noise_statistics():
    ov = dict(OV, read_noise_sigma=0.05, endurance_cycles=10)
    sim = m.MemorySimulator("optical_rram", 20000, overrides=ov, seed=3)
    sim.write(np.arange(20000), np.zeros(20000))
    y = sim.read(np.arange(20000))
    assert np.std(y) == pytest.approx(0.05, rel=0.03)


def test_kv_traffic_closed_form():
    s = kv.SHAPES["smollm2-135m"]
    assert s.kv_values_per_token() == 2 * 30 * 3 * 64
    tr = kv.decode_traffic(s, 4, 16)
    assert tr["values_read"] == s.kv_values_per_token() * (0 + 1 + 2 + 3)
    assert tr["bits_written"] == s.kv_values_per_token() * 4 * 16


def test_kv_energy_uses_only_cited_or_overridden_values():
    s = kv.SHAPES["awen-mini"]
    r = kv.decode_energy("dram_hbm2", s, 128, 16)
    assert r["read_energy_j"] == pytest.approx(3.97e-12 * r["bits_read"])
    assert "oconnor2017" in r["read_energy_source"] and r["write_energy_j"] is None
    r2 = kv.decode_energy("optical_rram", s, 128, 16)
    assert r2["read_energy_j"] is None and r2["read_energy_source"] == "parameter (unset)"
    r3 = kv.decode_energy("optical_rram", s, 128, 16, {"read_energy_j_per_bit": 1e-13})
    assert r3["read_energy_source"] == "user-override"


def test_fidelity_sweep_monotone_in_levels_and_exact_limit():
    rows = kv.fidelity_sweep(levels_list=(4, 16, 256), sigmas=(0.0,), age_over_tau=(0.0,), seq=32, d=16)
    errs = [r["rel_l2_error_mean"] for r in rows]
    assert errs[0] > errs[1] > errs[2]
    assert errs[2] < 0.02
    assert all(r["label"].startswith("modeled") for r in rows)


def test_pcm_write_energy_counts_multilevel_cells():
    s = kv.SHAPES["awen-mini"]
    r = kv.decode_energy("phase_change_photonic", s, 10, 16)
    assert r["cells_per_value"] == 6  # 8 levels = 3 bits/cell, 16-bit values
    assert r["write_energy_j"] == pytest.approx(13.4e-12 * r["values_written"] * 6)
