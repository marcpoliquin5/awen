"""Memory-technology parameter schema and a behavioral memory simulator.

Provenance rules (enforced by :func:`validate_technology`):

* ``literature-reported``: value copied from a cited source; ``citation`` must
  name a key in :data:`CITATIONS` and ``locator`` must say where in the source.
* ``physics``: derived from a stated textbook relation (e.g. delay = L n_g / c);
  the relation is recorded in ``note``.
* ``parameter``: no verified value is bundled. ``value`` is ``None`` and the
  caller must supply one through ``overrides`` before simulating.

The simulator is a behavioral model (quantization to a finite number of levels,
additive read noise, exponential decay for volatile storage, endurance counting,
energy/latency bookkeeping). Results are labeled ``modeled``.
"""

from __future__ import annotations

import copy
import math
from dataclasses import dataclass, field
from typing import Any

import numpy as np

PROVENANCES = ("literature-reported", "physics", "parameter")

#: Only sources whose bundled numbers were checked against the source text.
CITATIONS: dict[str, dict[str, str]] = {
    "rios2015": {
        "authors": "C. Rios, M. Stegmaier, P. Hosseini, D. Wang, T. Scherer, C. D. Wright, H. Bhaskaran, W. H. P. Pernice",
        "title": "Integrated all-photonic non-volatile multi-level memory",
        "venue": "Nature Photonics 9, 725-732 (2015)",
        "doi": "10.1038/nphoton.2015.182",
    },
    "oconnor2017": {
        "authors": "M. O'Connor, N. Chatterjee, D. Lee, J. Wilson, A. Agrawal, S. W. Keckler, W. J. Dally",
        "title": "Fine-Grained DRAM: Energy-Efficient DRAM for Extreme Bandwidth Systems",
        "venue": "Proc. 50th IEEE/ACM International Symposium on Microarchitecture (MICRO-50), 2017",
        "doi": "10.1145/3123939.3124545",
    },
}

#: Fields every technology must define (value may be None for ``parameter``).
FIELDS: dict[str, str] = {
    "levels": "count",
    "retention_s": "s",
    "read_latency_s": "s",
    "write_latency_s": "s",
    "read_energy_j_per_bit": "J/bit",
    "write_energy_j_per_cell": "J",
    "read_noise_sigma": "fraction of full scale",
    "endurance_cycles": "cycles",
    "bandwidth_bits_per_s": "bit/s",
}

C_LIGHT = 299_792_458.0


def P(value, unit, provenance="parameter", citation=None, locator=None, note=None) -> dict[str, Any]:
    return {"value": value, "unit": unit, "provenance": provenance, "citation": citation,
            "locator": locator, "note": note}


def _params(**given) -> dict[str, dict[str, Any]]:
    out = {k: P(None, u) for k, u in FIELDS.items()}
    out.update(given)
    return out


TECHNOLOGIES: dict[str, dict[str, Any]] = {
    "phase_change_photonic": {
        "description": "Non-volatile phase-change cell (e.g. GST) on a waveguide; state sets optical transmission.",
        "write_mechanism": "optical pulse heats the phase-change film (amorphize/crystallize)",
        "read_mechanism": "low-power probe transmission",
        "volatile": False,
        "params": _params(
            levels=P(8, "count", "literature-reported", "rios2015", "abstract",
                     "'bit storage of up to eight levels in a single device'"),
            write_energy_j_per_cell=P(13.4e-12, "J", "literature-reported", "rios2015", "abstract",
                                      "'switching energies as low as 13.4 pJ' (lowest reported, not typical); "
                                      "the abstract also reports speeds approaching 1 GHz, not bundled as a latency"),
        ),
    },
    "microring_fading": {
        "description": "Volatile storage of light in a high-Q microring; stored energy decays with photon lifetime.",
        "write_mechanism": "couple pulse into ring resonance",
        "read_mechanism": "out-couple / drop-port detection",
        "volatile": True,
        "params": _params(
            retention_s=P(None, "s", "parameter", note="physics: photon lifetime tau = Q * lambda / (2 pi c); "
                          "use photon_lifetime_s(Q, wavelength_m) and pass as override"),
        ),
    },
    "optical_rram": {
        "description": "Resistive-switching cell read or written through an optical interface (optoelectronic RRAM).",
        "write_mechanism": "electrical or optically assisted filament formation",
        "read_mechanism": "photocurrent or optical transmission",
        "volatile": False,
        "params": _params(),
    },
    "delay_line": {
        "description": "Recirculating optical delay line (fiber or waveguide loop); data lives only while circulating.",
        "write_mechanism": "inject modulated light into loop",
        "read_mechanism": "tap and detect after an integer number of round trips",
        "volatile": True,
        "params": _params(
            read_latency_s=P(None, "s", "parameter", note="physics: one round trip = L * n_g / c; "
                             "use delay_line_round_trip_s(length_m, n_group)"),
        ),
    },
    "dram_hbm2": {
        "description": "Electronic stacked DRAM (HBM2) baseline.",
        "write_mechanism": "electrical charge storage",
        "read_mechanism": "sense amplifier",
        "volatile": True,
        "params": _params(
            read_energy_j_per_bit=P(3.97e-12, "J/bit", "literature-reported", "oconnor2017", "Section 1 / Fig. 1b",
                                    "'The energy to access a bit in HBM2 is approximately 3.97 pJ/bit' "
                                    "(access energy; used for reads)"),
        ),
    },
}


def photon_lifetime_s(q_loaded: float, wavelength_m: float) -> float:
    """Photon (energy) lifetime of a resonator: tau = Q / omega = Q * lambda / (2 pi c)."""
    if q_loaded <= 0 or wavelength_m <= 0:
        raise ValueError("Q and wavelength must be positive")
    return q_loaded * wavelength_m / (2 * math.pi * C_LIGHT)


def delay_line_round_trip_s(length_m: float, n_group: float) -> float:
    """Group delay of one pass through a loop of length L: L * n_g / c."""
    if length_m <= 0 or n_group <= 0:
        raise ValueError("length and group index must be positive")
    return length_m * n_group / C_LIGHT


def validate_technology(name: str, tech: dict[str, Any]) -> list[str]:
    """Return a list of provenance/schema violations (empty list == valid)."""
    errs = []
    for key in ("description", "write_mechanism", "read_mechanism", "volatile", "params"):
        if key not in tech:
            errs.append(f"{name}: missing {key}")
    params = tech.get("params", {})
    for f in FIELDS:
        if f not in params:
            errs.append(f"{name}.{f}: missing")
    for f, p in params.items():
        prov = p.get("provenance")
        if prov not in PROVENANCES:
            errs.append(f"{name}.{f}: bad provenance {prov!r}")
        if prov == "literature-reported":
            if p.get("citation") not in CITATIONS or not p.get("locator"):
                errs.append(f"{name}.{f}: literature-reported needs a known citation and locator")
            if p.get("value") is None:
                errs.append(f"{name}.{f}: literature-reported needs a value")
        if prov == "parameter" and p.get("value") is not None and p.get("citation"):
            errs.append(f"{name}.{f}: free parameter must not carry a citation")
    return errs


def schema_document() -> dict[str, Any]:
    """The full parameter table as a JSON-serializable document (awen.memory_technologies.v1)."""
    return {"schema": "awen.memory_technologies.v1", "label": "parameters (not measurements)",
            "fields": FIELDS, "citations": CITATIONS, "technologies": TECHNOLOGIES}


@dataclass
class MemoryStats:
    reads: int = 0
    writes: int = 0
    bits_read: int = 0
    cells_written: int = 0
    energy_j: float = 0.0
    busy_s: float = 0.0
    label: str = "modeled"


@dataclass
class MemorySimulator:
    """Behavioral model of an array of analog/multi-level cells holding values in [-1, 1]."""

    technology: str
    n_cells: int
    overrides: dict[str, float] = field(default_factory=dict)
    bits_per_value: int | None = None
    seed: int = 0

    def __post_init__(self):
        if self.technology not in TECHNOLOGIES:
            raise KeyError(f"unknown technology {self.technology!r}")
        self.spec = copy.deepcopy(TECHNOLOGIES[self.technology])
        self.p: dict[str, float | None] = {k: v["value"] for k, v in self.spec["params"].items()}
        for k, v in self.overrides.items():
            if k not in FIELDS:
                raise KeyError(f"unknown parameter {k!r}")
            self.p[k] = v
        self.rng = np.random.default_rng(self.seed)
        self.values = np.zeros(self.n_cells)
        self.written_at = np.zeros(self.n_cells)
        self.wear = np.zeros(self.n_cells, dtype=np.int64)
        self.now = 0.0
        self.stats = MemoryStats()

    def need(self, *names: str) -> list[float]:
        missing = [n for n in names if self.p.get(n) is None]
        if missing:
            raise ValueError(f"{self.technology}: parameters {missing} have no bundled verified value; "
                             "supply them via overrides (they will be reported as user parameters)")
        return [float(self.p[n]) for n in names]

    def provenance(self) -> dict[str, str]:
        return {k: ("user-override" if k in self.overrides else v["provenance"])
                for k, v in self.spec["params"].items()}

    def _bits(self) -> int:
        if self.bits_per_value is not None:
            return self.bits_per_value
        (levels,) = self.need("levels")
        return max(1, int(math.ceil(math.log2(levels))))

    def quantize(self, x: np.ndarray) -> np.ndarray:
        (levels,) = self.need("levels")
        levels = int(levels)
        x = np.clip(x, -1.0, 1.0)
        if levels < 2:
            raise ValueError("levels must be >= 2")
        step = 2.0 / (levels - 1)
        return np.round((x + 1.0) / step) * step - 1.0

    def advance(self, dt_s: float) -> None:
        self.now += dt_s

    def write(self, idx: np.ndarray, x: np.ndarray) -> None:
        e_cell, t_w, endurance = self.need("write_energy_j_per_cell", "write_latency_s", "endurance_cycles")
        idx = np.asarray(idx)
        self.values[idx] = self.quantize(np.asarray(x, dtype=float))
        self.written_at[idx] = self.now
        self.wear[idx] += 1
        self.stats.writes += 1
        self.stats.cells_written += idx.size
        self.stats.energy_j += e_cell * idx.size
        self.stats.busy_s += t_w
        self.now += t_w
        if np.any(self.wear[idx] > endurance):
            raise RuntimeError(f"{self.technology}: endurance exceeded ({int(endurance)} cycles)")

    def read(self, idx: np.ndarray) -> np.ndarray:
        e_bit, t_r, sigma = self.need("read_energy_j_per_bit", "read_latency_s", "read_noise_sigma")
        idx = np.asarray(idx)
        v = self.values[idx].copy()
        if self.spec["volatile"]:
            (tau,) = self.need("retention_s")
            age = self.now - self.written_at[idx]
            v = v * np.exp(-age / tau)
        v = v + self.rng.normal(0.0, sigma, size=v.shape)
        nbits = idx.size * self._bits()
        self.stats.reads += 1
        self.stats.bits_read += nbits
        self.stats.energy_j += e_bit * nbits
        self.stats.busy_s += t_r
        self.now += t_r
        return v
