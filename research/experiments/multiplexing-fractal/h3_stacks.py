"""F2: Fibonacci (quasi-periodic) and triadic-Cantor multilayer stacks.

Layers A (high index) and B (low index) are quarter-wave at the reference
frequency ω0, so the phase of every layer is δ = (π/2)(ω/ω0). Each layer's
2×2 characteristic matrix (det = 1 for lossless layers) is the one used in
``awen_physics.tmm.transfer_matrix``; transmission here is cross-checked against
that function.

Established results reproduced:

* Kohmoto, Kadanoff & Tang, PRL 50, 1870 (1983), doi:10.1103/PhysRevLett.50.1870, and
  Kohmoto, Sutherland & Iguchi, PRL 58, 2436 (1987), doi:10.1103/PhysRevLett.58.2436:
  for S_{l+1} = S_l S_{l-1}, the half-traces x_l = Tr(M_l)/2 obey the trace map
  x_{l+1} = 2 x_l x_{l-1} − x_{l-2}, which conserves
  I = x_{l+1}² + x_l² + x_{l-1}² − 2 x_{l+1} x_l x_{l-1} − 1.
* Lavrinenko et al., PRE 65, 036621 (2002), doi:10.1103/PhysRevE.65.036621: scaling
  (self-similarity) of the transmission spectrum of an optical Cantor filter.
* Gellermann et al., PRL 72, 633 (1994), doi:10.1103/PhysRevLett.72.633: measured
  transmission of Fibonacci dielectric multilayers.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np

from awen_physics.tmm import transfer_matrix


def fibonacci_word(generation: int) -> str:
    """S_0 = 'B', S_1 = 'A', S_{l+1} = S_l + S_{l-1} (lengths are Fibonacci numbers)."""
    a, b = "B", "A"
    if generation == 0:
        return a
    for _ in range(generation - 1):
        a, b = b, b + a
    return b


def cantor_word(generation: int) -> str:
    """Triadic Cantor stack: C_0 = 'A', C_{k+1} = C_k + 'B'*3^k + C_k."""
    w = "A"
    for k in range(generation):
        w = w + "B" * 3 ** k + w
    return w


def layer_matrices(n: float, omega: np.ndarray) -> np.ndarray:
    """Quarter-wave-at-ω0 layer characteristic matrices ``[F, 2, 2]`` (normal incidence)."""
    d = 0.5 * np.pi * omega
    m = np.empty((omega.size, 2, 2), dtype=complex)
    m[:, 0, 0] = np.cos(d)
    m[:, 0, 1] = -1j * np.sin(d) / n
    m[:, 1, 0] = -1j * n * np.sin(d)
    m[:, 1, 1] = np.cos(d)
    return m


def stack_matrix(word: str, n_a: float, n_b: float, omega: np.ndarray) -> np.ndarray:
    ma, mb = layer_matrices(n_a, omega), layer_matrices(n_b, omega)
    m = np.broadcast_to(np.eye(2, dtype=complex), (omega.size, 2, 2)).copy()
    for ch in word:
        m = m @ (ma if ch == "A" else mb)
    return m


def transmission(m: np.ndarray, n0: float = 1.0, ns: float = 1.0) -> np.ndarray:
    b = m[:, 0, 0] + m[:, 0, 1] * ns
    c = m[:, 1, 0] + m[:, 1, 1] * ns
    t = 2 * n0 / (n0 * b + c)
    return (ns / n0) * np.abs(t) ** 2


def spectrum(word: str, n_a: float, n_b: float, omega: np.ndarray) -> np.ndarray:
    return transmission(stack_matrix(word, n_a, n_b, omega))


def tmm_crosscheck(word: str, n_a: float, n_b: float, omega: float) -> tuple[float, float]:
    """(this module, awen_physics.tmm) transmission at one frequency; λ0 = 1 µm."""
    idx = [1.0] + [n_a if ch == "A" else n_b for ch in word] + [1.0]
    th = [0.25 / (n_a if ch == "A" else n_b) for ch in word]
    ours = float(spectrum(word, n_a, n_b, np.array([omega]))[0])
    ref = transfer_matrix(idx, th, 1.0 / omega).T
    return ours, float(ref)


def trace_map(n_a: float, n_b: float, omega: np.ndarray, generations: int) -> np.ndarray:
    """Half-traces x_l of the Fibonacci stack matrices, computed by direct products ``[L, F]``."""
    return np.stack([0.5 * np.trace(stack_matrix(fibonacci_word(g), n_a, n_b, omega), axis1=1, axis2=2).real
                     for g in range(generations)])


def trace_invariant(x: np.ndarray) -> np.ndarray:
    """I_l for l = 2…L−1 from consecutive half-traces (should be constant in l)."""
    return x[2:] ** 2 + x[1:-1] ** 2 + x[:-2] ** 2 - 2 * x[2:] * x[1:-1] * x[:-2] - 1


def band_measure(t: np.ndarray, threshold: float = 0.5) -> float:
    """Fraction of the sampled frequency window with T above ``threshold``."""
    return float((t > threshold).mean())


def scaling_scan(n_a: float, n_b: float, generation: int, window: float, factors: np.ndarray,
                 samples: int = 4001) -> list[tuple[float, float]]:
    """Correlation of log T_{G+1}(1 + Δ/s) with log T_G(1 + Δ) over |Δ| ≤ window, for each s."""
    delta = np.linspace(-window, window, samples)
    base = np.log10(np.maximum(spectrum(cantor_word(generation), n_a, n_b, 1 + delta), 1e-12))
    out = []
    for s in factors:
        nxt = np.log10(np.maximum(spectrum(cantor_word(generation + 1), n_a, n_b, 1 + delta / s), 1e-12))
        out.append((float(s), float(np.corrcoef(base, nxt)[0, 1])))
    return out


def study(n_a: float = 2.3, n_b: float = 1.38) -> dict:
    omega = np.linspace(0.01, 1.99, 20001)
    x = trace_map(n_a, n_b, omega, 14)
    inv = trace_invariant(x)
    q = trace_map(n_a, n_b, np.array([1.0]), 20)[:, 0]
    t_qw = [float(spectrum(fibonacci_word(g), n_a, n_b, np.array([1.0]))[0]) for g in range(20)]
    fib_bands = [{"generation": g, "layers": len(fibonacci_word(g)),
                  "fraction_T_gt_0.5": round(band_measure(spectrum(fibonacci_word(g), n_a, n_b, omega)), 5)}
                 for g in range(3, 16)]
    periodic_bands = [{"layers": 2 * k, "fraction_T_gt_0.5": round(band_measure(spectrum("AB" * k, n_a, n_b, omega)), 5)}
                      for k in (2, 4, 8, 16, 32, 64, 128, 256)]
    factors = np.round(np.arange(1.5, 5.01, 0.1), 2)
    scans = {f"G{g}->G{g + 1}": scaling_scan(n_a, n_b, g, 0.5, factors) for g in (1, 2, 3, 4)}
    bounded = np.max(np.abs(x), axis=0) < 1e3  # in band gaps |x_l| grows exponentially and float64 loses I
    best = {k: max(v, key=lambda p: p[1]) for k, v in scans.items()}
    crosscheck = [tmm_crosscheck(fibonacci_word(7), n_a, n_b, w) for w in (0.37, 1.0, 1.61)]
    return {"study": "awen.fractal-stacks.v1", "n_a": n_a, "n_b": n_b, "ambient_and_substrate": 1.0,
            "note": "indices are free parameters of the study, not a specific fabricated material pair",
            "tmm_crosscheck_fibonacci_g7": crosscheck,
            "trace_invariant": {"max_abs_variation_over_generations_bounded_freqs":
                                float(np.max(np.abs(inv - inv[0])[:, bounded])),
                                "bounded_frequency_fraction": round(float(bounded.mean()), 4),
                                "closed_form_at_quarter_wave": float(0.25 * (n_a / n_b - n_b / n_a) ** 2),
                                "measured_at_quarter_wave": float(inv[0][np.argmin(abs(omega - 1.0))])},
            "quarter_wave_half_traces": [round(float(v), 10) for v in q],
            "quarter_wave_transmission_by_generation": [round(v, 10) for v in t_qw],
            "fibonacci_band_measure": fib_bands, "periodic_band_measure": periodic_bands,
            "cantor_scaling_scan_window": 0.5, "cantor_naive_prediction_factor": 3.0,
            "cantor_scaling_scan_best": {k: {"factor": v[0], "correlation": round(v[1], 5)} for k, v in best.items()},
            "cantor_scaling_scan": {k: [[s, round(c, 5)] for s, c in v] for k, v in scans.items()},
            "evidence": "exact transfer-matrix calculation (lossless, normal incidence)"}


# ---------------------------------------------------------------------------
# Registered full run (configs/h3.yaml, PREREGISTRATION.md section H3).
#
# Operationalization used by this entry point (stated here before any full run):
# * A channel is a maximal contiguous run of frequency samples with T >= passband_T_min.
# * The crosstalk between two adjacent channels is 10*log10(T_floor / T_peak), where T_floor is the
#   minimum transmission between them and T_peak the smaller of the two channel peaks (<= 0 dB).
# * A channel's isolation is the minimum of -crosstalk over its existing neighbours; a channel with no
#   neighbour has infinite isolation. A channel is usable if its isolation >= isolation_db_min.
# * Channel density = usable channels / width of the frequency grid (normalized frequency units).
# * Footprint = layer count and total physical thickness (in units of lambda0).
# * Uniform-grid comparator (one per channel spacing in the config): a symmetric Fabry-Perot comb
#   [spacer B] (AB)^p A [cavity B] A (BA)^p [spacer B]. The cavity is m = round(1/spacing) half-waves of B
#   at lambda0, so its resonances sit on a uniform grid with free spectral range ~spacing (ignoring mirror
#   penetration) and one of them at omega0. p is the fewest mirror pairs whose unperturbed comb has at
#   least two usable channels on the config's frequency grid (a design-to-spec rule: stronger mirrors give
#   channels narrower than the grid step); if no p that fits reaches that, the most pairs that fit. The leftover thickness is split equally between the two outer B spacers, and
#   the cavity is split into equal B sublayers until the layer count matches (adjacent equal-index layers act
#   as one layer, as the B runs in cantor_word do). Both footprints are then equal (thickness to float
#   rounding). A spacing whose cavity does not fit is recorded as "infeasible at equal footprint".
# * Comparison cell = (generation, sigma, spacing), seed means of each plan. Lower worst adjacent crosstalk
#   and higher usable-channel density are better. A cell is "cantor better on both", "cantor worse on both",
#   "mixed", or "not comparable" (a plan has fewer than two channels in some seed, so no crosstalk value).
# * How cells combine into keep/kill is not registered yet, so the verdict stays "not evaluated".

CONFIG_SCHEMA = "awen.fractal-prereg-config.v1"
PILOT_SEEDS = (0, 1, 2)
_REQUIRED = ("id", "schema", "free_parameters", "cantor_generations", "uniform_grid", "frequency_grid",
             "usable_channel", "seeds")


def load_config(path) -> dict:
    """Load and validate a registered H3 config (YAML)."""
    import yaml

    cfg = yaml.safe_load(Path(path).read_text())
    missing = [k for k in _REQUIRED if k not in cfg]
    if missing:
        raise ValueError(f"H3 config missing keys: {missing}")
    if cfg["id"] != "H3" or cfg["schema"] != CONFIG_SCHEMA:
        raise ValueError(f"not an H3 {CONFIG_SCHEMA} config: id={cfg['id']!r} schema={cfg['schema']!r}")
    reused = sorted(set(cfg["seeds"]) & set(PILOT_SEEDS))
    if reused:
        raise ValueError(f"confirmatory seeds must not reuse pilot seeds: {reused}")
    fg = cfg["frequency_grid"]
    if not (0 < fg["min"] < fg["max"]) or int(fg["points"]) < 2:
        raise ValueError(f"bad frequency_grid: {fg}")
    return cfg


def perturbed_stack_matrix(word: str, n_a: float, n_b: float, omega: np.ndarray,
                           factors: np.ndarray) -> np.ndarray:
    """Stack matrix with layer k's thickness scaled by ``factors[k]`` (1.0 = quarter-wave)."""
    m = np.broadcast_to(np.eye(2, dtype=complex), (omega.size, 2, 2)).copy()
    for ch, f in zip(word, factors):
        m = m @ layer_matrices(n_a if ch == "A" else n_b, omega * f)
    return m


def thickness_factors(n_layers: int, sigma: float, seed: int) -> np.ndarray:
    """Independent Gaussian thickness errors, 1 + sigma * N(0, 1) per layer (seeded)."""
    if sigma == 0.0:
        return np.ones(n_layers)
    return 1.0 + sigma * np.random.default_rng(seed).standard_normal(n_layers)


def footprint(word: str, n_a: float, n_b: float, factors: np.ndarray) -> dict:
    th = [0.25 * f / (n_a if ch == "A" else n_b) for ch, f in zip(word, factors)]
    return {"layers": len(word), "total_thickness_lambda0": float(sum(th))}


def channels(t: np.ndarray, omega: np.ndarray, t_min: float, isolation_db_min: float) -> dict:
    """Channel metrics of one spectrum under the operationalization stated above."""
    above = t >= t_min
    edges = np.flatnonzero(np.diff(above.astype(np.int8)))
    starts = list(edges[~above[edges]] + 1)
    stops = list(edges[above[edges]] + 1)
    if above[0]:
        starts.insert(0, 0)
    if above[-1]:
        stops.append(above.size)
    bands = [(int(a), int(b)) for a, b in zip(starts, stops)]
    peaks = [float(t[a:b].max()) for a, b in bands]
    xtalk = []
    for i in range(len(bands) - 1):
        floor = float(t[bands[i][1]:bands[i + 1][0]].min())
        xtalk.append(10 * np.log10(max(floor, 1e-300) / min(peaks[i], peaks[i + 1])))
    iso = []
    for i in range(len(bands)):
        nb = [-xtalk[j] for j in (i - 1, i) if 0 <= j < len(xtalk)]
        iso.append(min(nb) if nb else float("inf"))
    usable = [i for i, v in enumerate(iso) if v >= isolation_db_min]
    width = float(omega[-1] - omega[0])
    worst = max(xtalk) if xtalk else None
    return {"channels": len(bands), "usable_channels": len(usable),
            "worst_adjacent_crosstalk_db": None if worst is None else round(float(worst), 6),
            "channels_per_unit_normalized_frequency": round(len(usable) / width, 6),
            "centers": [round(float(omega[(a + b - 1) // 2]), 6) for a, b in bands]}


def layers_matrix(ns, ds, omega: np.ndarray) -> np.ndarray:
    """Stack matrix for layers of index ``ns[k]`` and thickness ``ds[k]`` (units of lambda0)."""
    m = np.broadcast_to(np.eye(2, dtype=complex), (omega.size, 2, 2)).copy()
    for n, d in zip(ns, ds):
        # layer_matrices uses phase (pi/2) * x; a layer of optical thickness n*d has phase 2*pi*n*d*omega.
        m = m @ layer_matrices(float(n), omega * (4.0 * float(n) * float(d)))
    return m


def _comb(n_a: float, n_b: float, p: int, cavity: float, layers: int, thickness: float):
    """Fabry-Perot comb with p mirror pairs at the given footprint; None if it does not fit."""
    qa, qb = 0.25 / n_a, 0.25 / n_b
    left = thickness - cavity - 2 * ((p + 1) * qa + p * qb)
    spacers = 2 if left > 1e-12 else 0
    cav_layers = layers - 2 * (2 * p + 1) - spacers
    if left < -1e-12 or cav_layers < 1:
        return None
    mir_ns, mir_ds = [n_a] + [n_b, n_a] * p, [qa] + [qb, qa] * p  # outside to cavity: A (BA)^p
    sp_ns, sp_ds = [n_b] * (spacers // 2), [left / 2] * (spacers // 2)
    ns = sp_ns + mir_ns + [n_b] * cav_layers + mir_ns[::-1] + sp_ns
    ds = sp_ds + mir_ds + [cavity / cav_layers] * cav_layers + mir_ds[::-1] + sp_ds
    return np.array(ns), np.array(ds), {"mirror_pairs": p, "cavity_sublayers": cav_layers,
                                        "spacer_thickness_lambda0_each": left / 2 if spacers else 0.0}


def uniform_grid_stack(n_a: float, n_b: float, spacing: float, layers: int, thickness: float,
                       omega: np.ndarray | None = None, t_min: float = 0.5, isolation_db_min: float = 15.0):
    """Equal-footprint Fabry-Perot comb (see the operationalization above); None if it cannot fit.

    Mirror rule: the fewest mirror pairs whose unperturbed comb has at least two usable channels on
    ``omega`` (the design spec); if no pair count reaches that, or ``omega`` is None, the most pairs that fit.
    """
    cavity = round(1.0 / spacing) * 0.5 / n_b
    fits = [c for c in (_comb(n_a, n_b, p, cavity, layers, thickness) for p in range(layers // 4 + 1)) if c]
    if not fits:
        return None
    chosen, rule = fits[-1], "most pairs that fit (no pair count met the spec)"
    if omega is not None:
        for c in fits:
            t = transmission(layers_matrix(c[0], c[1], omega))
            if channels(t, omega, t_min, isolation_db_min)["usable_channels"] >= 2:
                chosen, rule = c, "fewest pairs with >= 2 usable channels unperturbed"
                break
    ns, ds, info = chosen
    return ns, ds, {**info, "cavity_half_waves": round(1.0 / spacing), "mirror_rule": rule,
                    "measured_spacing_near_omega0": measured_spacing(ns, ds)}


def measured_spacing(ns, ds, half_window: float = 0.2, points: int = 8001) -> float | None:
    """Mean gap between unperturbed channel centers within omega0 +/- half_window (T >= 0.5, any isolation).

    Mirror penetration makes this smaller than the nominal spacing, so the actual grid is recorded per stack.
    """
    omega = np.linspace(1.0 - half_window, 1.0 + half_window, points)
    centers = channels(transmission(layers_matrix(ns, ds, omega)), omega, 0.5, 0.0)["centers"]
    return round(float(np.mean(np.diff(centers))), 6) if len(centers) >= 2 else None


def compare_cell(cantor_runs: list, uniform_runs: list) -> dict:
    """Seed-mean comparison of one (generation, sigma, spacing) cell; see the operationalization above."""
    def mean(runs, key):
        vals = [r[key] for r in runs]
        return None if any(v is None for v in vals) else float(np.mean(vals))
    cd, ud = (mean(r, "channels_per_unit_normalized_frequency") for r in (cantor_runs, uniform_runs))
    cx, ux = (mean(r, "worst_adjacent_crosstalk_db") for r in (cantor_runs, uniform_runs))
    out = {"cantor_density": cd, "uniform_density": ud, "cantor_worst_xtalk_db": cx, "uniform_worst_xtalk_db": ux}
    if cx is None or ux is None:
        return {**out, "outcome": "not comparable"}
    better = (cx < ux, cd > ud)
    worse = (cx > ux, cd < ud)
    outcome = ("cantor better on both" if all(better) else "cantor worse on both" if all(worse) else "mixed")
    return {**out, "outcome": outcome}


def run_config(cfg: dict) -> dict:
    fp, fg, uc = cfg["free_parameters"], cfg["frequency_grid"], cfg["usable_channel"]
    n_a, n_b = float(fp["n_high"]), float(fp["n_low"])
    omega = np.linspace(float(fg["min"]), float(fg["max"]), int(fg["points"]))
    runs = []
    for g in cfg["cantor_generations"]:
        word = cantor_word(int(g))
        for sigma in fp["thickness_sigma_fraction"]:
            for seed in cfg["seeds"]:
                f = thickness_factors(len(word), float(sigma), int(seed))
                t = transmission(perturbed_stack_matrix(word, n_a, n_b, omega, f))
                runs.append({"plan": "cantor", "generation": int(g), "thickness_sigma_fraction": float(sigma),
                             "seed": int(seed), "footprint": footprint(word, n_a, n_b, f),
                             **channels(t, omega, float(uc["passband_T_min"]), float(uc["isolation_db_min"]))})
    uniform, cells, infeasible = [], [], []
    for g in cfg["cantor_generations"]:
        base = footprint(cantor_word(int(g)), n_a, n_b, np.ones(3 ** int(g)))
        for spacing in cfg["uniform_grid"]["channel_spacings_normalized"]:
            built = uniform_grid_stack(n_a, n_b, float(spacing), base["layers"], base["total_thickness_lambda0"],
                                       omega, float(uc["passband_T_min"]), float(uc["isolation_db_min"]))
            if built is None:
                infeasible.append({"generation": int(g), "spacing": float(spacing),
                                   "status": "infeasible at equal footprint"})
                continue
            ns, ds, info = built
            for sigma in fp["thickness_sigma_fraction"]:
                cell = []
                for seed in cfg["seeds"]:
                    f = thickness_factors(len(ns), float(sigma), int(seed))
                    t = transmission(layers_matrix(ns, ds * f, omega))
                    cell.append({"plan": "uniform", "generation": int(g), "spacing": float(spacing),
                                 "thickness_sigma_fraction": float(sigma), "seed": int(seed), **info,
                                 "footprint": {"layers": len(ns), "total_thickness_lambda0": float((ds * f).sum())},
                                 **channels(t, omega, float(uc["passband_T_min"]), float(uc["isolation_db_min"]))})
                uniform += cell
                cantor = [r for r in runs if r["generation"] == int(g) and r["thickness_sigma_fraction"] == float(sigma)]
                cells.append({"generation": int(g), "spacing": float(spacing), "thickness_sigma_fraction": float(sigma),
                              **compare_cell(cantor, cell)})
    return {"study": "awen.h3-channel-plans.v1", "config_id": cfg["id"], "n_high": n_a, "n_low": n_b,
            "frequency_grid": fg, "usable_channel": uc, "runs": runs,
            "uniform_grid": {"status": "run", "runs": uniform, "infeasible": infeasible},
            "comparison_cells": cells,
            "verdict": "not evaluated (rule combining comparison cells into keep/kill not registered yet)",
            "evidence": "exact transfer-matrix calculation (lossless, normal incidence)"}


def main(argv=None):
    ap = argparse.ArgumentParser(description="Fibonacci and Cantor multilayer study (pilot) or registered H3 run (--config)")
    ap.add_argument("--out", default=None)
    ap.add_argument("--config", default=None, help="registered H3 config (configs/h3.yaml)")
    a = ap.parse_args(argv)
    if a.config:
        rep = run_config(load_config(a.config))
        out = Path(a.out or Path(__file__).parent / "results" / "h3")
        out.mkdir(parents=True, exist_ok=True)
        (out / "h3_channel_plans.json").write_text(json.dumps(rep, indent=2) + "\n")
        print(json.dumps({"runs": len(rep["runs"]), "uniform_runs": len(rep["uniform_grid"]["runs"]),
                          "cells": len(rep["comparison_cells"]), "verdict": rep["verdict"]}))
        return
    out = Path(a.out or Path(__file__).parent / "results" / "pilot")
    out.mkdir(parents=True, exist_ok=True)
    rep = study()
    (out / "stacks.json").write_text(json.dumps(rep, indent=2) + "\n")
    print(json.dumps({k: v for k, v in rep.items() if k != "cantor_scaling_scan"}, indent=1))


if __name__ == "__main__":
    main()
