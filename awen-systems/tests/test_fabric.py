import math

import pytest

from awen_systems import fabric as fb

CFG = fb.FabricConfig("eps", link_bandwidth_bps=100e9, link_latency_s=50e-9, switch_latency_s=300e-9,
                      energy_j_per_bit=1e-12)
OCS = fb.FabricConfig("ocs", link_bandwidth_bps=100e9, link_latency_s=50e-9, switch_latency_s=0.0,
                      energy_j_per_bit=1e-12, reconfig_delay_s=10e-6, circuit_switched=True)


@pytest.mark.parametrize("lam,mu,n,tol", [(0.5, 1.0, 60000, 0.05), (0.8, 1.0, 200000, 0.08)])
def test_mm1_mean_sojourn_matches_analytic(lam, mu, n, tol):
    r = fb.mm1_experiment(lam, mu, n=n, seed=1)
    assert r["simulated_mean_sojourn"] == pytest.approx(r["analytic_mean_sojourn"], rel=tol)


@pytest.mark.parametrize("n", [2, 4, 8])
def test_ring_allreduce_matches_alpha_beta(n):
    data = 8e9
    r = fb.ring_allreduce(fb.ring(CFG, n), data)
    assert r["time_s"] == pytest.approx(fb.ring_allreduce_analytic(n, data, 100e9, 50e-9), rel=1e-12)
    # each of 2(n-1) phases moves n chunks of data/n bits over one link
    assert r["dynamic_energy_j"] == pytest.approx(2 * (n - 1) * data * 1e-12)


def test_shift_all_to_all_on_star_closed_form():
    n, data = 4, 4e9
    chunk = data / n
    per_phase = 2 * (chunk / 100e9 + 50e-9) + 300e-9
    r = fb.all_to_all(fb.star(CFG, n), data)
    assert r["time_s"] == pytest.approx((n - 1) * per_phase, rel=1e-12)


def test_direct_all_to_all_contention_lower_bound():
    n, data = 4, 4e9
    r = fb.all_to_all(fb.star(CFG, n), data, schedule="direct")
    assert r["time_s"] >= (n - 1) * (data / n) / 100e9  # each uplink serializes n-1 chunks


def test_circuit_switch_pays_reconfiguration_per_new_pattern():
    n, data = 4, 4e9
    a2a = fb.all_to_all(fb.star(OCS, n), data)
    assert a2a["reconfigurations"] == n - 1
    chunk = data / n
    assert a2a["time_s"] == pytest.approx((n - 1) * (10e-6 + 2 * (chunk / 100e9 + 50e-9)), rel=1e-12)
    ar = fb.ring_allreduce(fb.star(OCS, n), data)
    assert ar["reconfigurations"] == 1  # same permutation every phase
    with pytest.raises(ValueError, match="permutation"):
        fb.all_to_all(fb.star(OCS, n), data, schedule="direct")


def test_link_failure_reroutes_and_slows_down():
    n, data = 6, 6e9
    healthy = fb.ring_allreduce(fb.ring(CFG, n), data)
    f = fb.ring(CFG, n)
    f.links[("n0", "n1")].fail_at_s = 0.0
    hurt = fb.ring_allreduce(f, data)
    assert hurt["dropped"] == 0 and hurt["time_s"] > healthy["time_s"]


def test_partition_drops_messages():
    f = fb.ring(CFG, 4)
    for a, b in [("n0", "n1"), ("n1", "n0"), ("n2", "n3"), ("n3", "n2")]:
        f.links[(a, b)].fail_at_s = 0.0
    r = f.run([fb.Message("n0", "n2", 1e3, 0.0)])
    assert r["dropped"] == 1


def test_loss_budget_makes_link_unusable():
    cfg = fb.FabricConfig("lossy", 1e9, 0.0, 0.0, 0.0, loss_db=12.0, tx_power_dbm=0.0, rx_sensitivity_dbm=-10.0)
    f = fb.ring(cfg, 3)
    assert f.run([fb.Message("n0", "n1", 1e3, 0.0)])["dropped"] == 1


def test_static_power_accounting_and_compare_rows():
    cfg = fb.FabricConfig("p", 1e9, 0.0, 0.0, 0.0, static_power_w=2.0)
    r = fb.ring_allreduce(fb.ring(cfg, 2), 1e9)
    assert r["static_energy_j"] == pytest.approx(2.0 * r["time_s"])
    rows = fb.compare([CFG, OCS], 4, 4e9)
    assert {x["fabric"] for x in rows} == {"eps", "ocs"} and all(x["label"] == "modeled" for x in rows)
    assert math.isfinite(rows[0]["time_s"])
