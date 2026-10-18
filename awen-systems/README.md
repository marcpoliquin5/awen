# awen-systems

System-level models for AWEN. **Every output is modeled, not measured.**

## Memory subsystem (`awen_systems.memory`)

A parameter schema (`awen.memory_technologies.v1`, shipped as
`awen_systems/memory_technologies.v1.json`) for five technologies:
phase-change photonic cells, microring (volatile, fading) storage, optical
RRAM, recirculating delay lines and an electronic HBM2 DRAM baseline. Each
technology records its write/read mechanism and volatility, and these
parameters: levels, retention, read/write latency, read energy per bit, write
energy per cell, read noise, endurance and bandwidth.

Every parameter carries a provenance:

| provenance | meaning |
|---|---|
| `literature-reported` | copied from a cited source, with a locator (where in the source) |
| `physics` / note | derived from a textbook relation (photon lifetime τ = Qλ/2πc, loop delay L·n_g/c) via helper functions |
| `parameter` | no verified value bundled; `value` is `null` and you must supply it |

Bundled literature values (all checked against the source text):

| technology | parameter | value | source |
|---|---|---|---|
| phase-change photonic | levels | 8 | Ríos et al., *Nature Photonics* 9, 725 (2015), doi:10.1038/nphoton.2015.182, abstract ("up to eight levels") |
| phase-change photonic | write energy per cell | 13.4 pJ | same, abstract ("switching energies as low as 13.4 pJ"; lowest reported, not typical) |
| HBM2 DRAM | access energy | 3.97 pJ/bit | O'Connor et al., MICRO-50 (2017), doi:10.1145/3123939.3124545, Section 1 |

Everything else is a free parameter. The simulator (`MemorySimulator`) refuses
to run while a needed parameter is unset rather than guessing. It models
quantization to a finite number of levels, additive read noise, exponential
decay for volatile storage, endurance limits, and energy/latency bookkeeping.

## KV-cache workload study (`awen_systems.kvcache`, `awen-kvcache-study`)

* **Traffic/energy:** exact KV traffic for decoding with a KV cache (shapes
  from the AWEN-Mini and SmolLM2-135M configs), priced only with cited or
  user-supplied parameters. In `results/kvcache_study.json` (2048-token
  context, 16-bit values) the HBM2 read energy is 0.170 J (AWEN-Mini) and
  1.53 J (SmolLM2-135M) of modeled DRAM access energy. The phase-change
  number in the same file is *write* energy for the newly written values only
  (lowest reported switching energy, 6 cells per 16-bit value); it is not
  comparable to the DRAM read figure, and no read energy is bundled for it.
* **Fidelity sweep:** K/V stored with L levels (clip at ±4σ), read noise σ
  and decay age/τ; relative L2 error of attention output vs exact float64,
  2 seeds. Example (σ = 0, no decay): 4 levels 1.61, 8 levels 0.47,
  16 levels 0.22, 64 levels 0.053, 256 levels 0.013. Read noise of 0.03 full
  scale floors the error near 0.16 even at 256 levels; 10% amplitude decay
  (age/τ = 0.1) without renormalization floors it near 0.17. These are
  dimensionless parameter sweeps, not device claims.

```bash
pip install ./awen-systems[test]
python -m pytest awen-systems/tests -q
awen-kvcache-study --context 2048 --bits 16 --overrides my_params.json
```
