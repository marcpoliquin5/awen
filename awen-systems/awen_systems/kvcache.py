"""KV-cache workload study on modeled memory technologies (label: modeled).

Two parts:

1. **Traffic and energy.** For autoregressive decoding with a KV cache, step t
   reads ``2 * L * H_kv * d * t`` stored values and writes ``2 * L * H_kv * d``.
   Energy and time follow from per-bit/per-cell parameters. Only parameters with
   a bundled ``literature-reported`` value or an explicit user override are
   used; anything else raises instead of being guessed.
2. **Fidelity.** Keys/values are stored in a finite-level, noisy, possibly
   decaying memory and attention is recomputed. The error of the attention
   output against exact float64 attention is reported as a function of
   dimensionless parameters (levels, read-noise sigma, age/retention). This is
   a parameter sweep and makes no claim about any specific device.
"""

from __future__ import annotations

import argparse
import json
import math
from dataclasses import dataclass

import numpy as np

from .memory import TECHNOLOGIES


@dataclass(frozen=True)
class DecoderShape:
    name: str
    n_layers: int
    n_kv_heads: int
    head_dim: int
    source: str

    def kv_values_per_token(self) -> int:
        return 2 * self.n_layers * self.n_kv_heads * self.head_dim


#: Shapes taken from configs (exact, not estimates).
SHAPES = {
    "awen-mini": DecoderShape("AWEN-Mini", 5, 4, 32,
                              "awen-models/awen_models/checkpoints/awen-mini/config.json: n_layer 5, n_head 4, n_embd 128"),
    "smollm2-135m": DecoderShape("SmolLM2-135M", 30, 3, 64,
                                 "HuggingFaceTB/SmolLM2-135M config.json: 30 layers, 3 KV heads, 576/9 head_dim"),
}


def decode_traffic(shape: DecoderShape, context: int, bits_per_value: int) -> dict:
    """Total bits read / values written to decode tokens 1..context with a KV cache."""
    per_tok = shape.kv_values_per_token()
    values_read = per_tok * context * (context - 1) // 2  # step t reads t-1 cached tokens
    values_written = per_tok * context
    return {"values_read": values_read, "values_written": values_written,
            "bits_read": values_read * bits_per_value, "bits_written": values_written * bits_per_value,
            "kv_bytes_at_end": per_tok * context * bits_per_value / 8}


def decode_energy(technology: str, shape: DecoderShape, context: int, bits_per_value: int,
                  overrides: dict | None = None) -> dict:
    """Modeled read energy (and write energy if available) for a decode of ``context`` tokens."""
    overrides = overrides or {}
    params = TECHNOLOGIES[technology]["params"]
    tr = decode_traffic(shape, context, bits_per_value)

    def get(name):
        if name in overrides:
            return float(overrides[name]), "user-override"
        p = params[name]
        if p["value"] is None:
            return None, "parameter (unset)"
        return float(p["value"]), p["provenance"] + (f" [{p['citation']}]" if p["citation"] else "")

    e_bit, e_bit_src = get("read_energy_j_per_bit")
    e_cell, e_cell_src = get("write_energy_j_per_cell")
    levels, _ = get("levels")
    # A b-bit value needs ceil(b / log2(levels)) multi-level cells; one cell per value if levels unknown.
    cells_per_value = 1 if levels is None else math.ceil(bits_per_value / math.floor(math.log2(levels)))
    out = {"label": "modeled", "technology": technology, "shape": shape.name, "context": context,
           "bits_per_value": bits_per_value, **tr,
           "read_energy_j": None if e_bit is None else e_bit * tr["bits_read"],
           "read_energy_source": e_bit_src,
           "cells_per_value": cells_per_value,
           "write_energy_j": None if e_cell is None else e_cell * tr["values_written"] * cells_per_value,
           "write_energy_source": e_cell_src}
    return out


def _attention(q, k, v):
    s = q @ k.T / math.sqrt(q.shape[-1])
    s = s - s.max(axis=-1, keepdims=True)
    w = np.exp(s)
    w /= w.sum(axis=-1, keepdims=True)
    return w @ v


def _store(x, levels, sigma, decay, rng, scale):
    """Quantize x/scale to `levels` in [-1,1], apply amplitude decay and read noise, rescale."""
    y = np.clip(x / scale, -1, 1)
    step = 2.0 / (levels - 1)
    y = np.round((y + 1) / step) * step - 1
    y = y * decay + rng.normal(0.0, sigma, size=y.shape)
    return y * scale


def fidelity_sweep(levels_list=(4, 8, 16, 64, 256), sigmas=(0.0, 0.01, 0.03),
                   age_over_tau=(0.0, 0.1), seq=128, d=64, n_queries=16, seeds=(0, 1),
                   renormalize_decay=False) -> list[dict]:
    """Relative L2 error of attention output with K/V held in modeled memory."""
    rows = []
    for levels in levels_list:
        for sigma in sigmas:
            for a in age_over_tau:
                errs = []
                for seed in seeds:
                    rng = np.random.default_rng(seed)
                    k = rng.standard_normal((seq, d))
                    v = rng.standard_normal((seq, d))
                    q = rng.standard_normal((n_queries, d))
                    exact = _attention(q, k, v)
                    scale = 4.0  # clip range in units of std; fixed, stated
                    decay = math.exp(-a)
                    ks = _store(k, levels, sigma, decay, rng, scale)
                    vs = _store(v, levels, sigma, decay, rng, scale)
                    if renormalize_decay and decay > 0:
                        ks, vs = ks / decay, vs / decay
                    approx = _attention(q, ks, vs)
                    errs.append(float(np.linalg.norm(approx - exact) / np.linalg.norm(exact)))
                rows.append({"label": "modeled (parameter sweep)", "levels": levels, "read_noise_sigma": sigma,
                             "age_over_retention": a, "rel_l2_error_mean": float(np.mean(errs)),
                             "rel_l2_error_per_seed": errs, "seeds": list(seeds), "seq": seq, "head_dim": d,
                             "clip_scale_std": 4.0})
    return rows


def main(argv=None):
    ap = argparse.ArgumentParser(description="KV-cache workload study on modeled memories (all outputs modeled).")
    ap.add_argument("--context", type=int, default=2048)
    ap.add_argument("--bits", type=int, default=16)
    ap.add_argument("--overrides", type=str, default=None,
                    help='JSON {"technology": {"param": value}} of user-supplied parameters')
    ap.add_argument("--out", type=str, default=None)
    args = ap.parse_args(argv)
    ov = json.loads(open(args.overrides).read()) if args.overrides else {}
    report = {"label": "modeled; not a hardware measurement",
              "energy": [decode_energy(t, shape, args.context, args.bits, ov.get(t))
                         for shape in SHAPES.values() for t in TECHNOLOGIES],
              "fidelity": fidelity_sweep()}
    txt = json.dumps(report, indent=2)
    if args.out:
        open(args.out, "w").write(txt + "\n")
    print(txt)


if __name__ == "__main__":
    main()
