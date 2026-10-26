"""Wall time and peak memory of kron3 training steps: ``rank_chunk`` contraction vs ``dense_weight``.

Each configuration runs in a fresh subprocess (so ``ru_maxrss`` is per configuration). A configuration
times ``--steps`` full training steps (forward, backward, AdamW step) after ``--warmup`` untimed steps on
random token batches; one JSON line per configuration. Factor shapes are arguments, so the H2 amendment A1
shape can be timed without the H2 script:

    PYTHONPATH=awen-models python awen-models/examples/kron3_step_time.py \\
        --ins 8 2 8 --hidden 32 2 32 --rank 126 --batch 32 --modes chunk:18 dense
"""
from __future__ import annotations

import argparse
import json
import platform
import resource
import subprocess
import sys
import time


def child(mode: str, a) -> dict:
    import torch
    from awen_models import efficiency as ef

    torch.set_num_threads(a.threads)
    torch.manual_seed(0)
    chunk = int(mode.split(":")[1]) if mode.startswith("chunk:") else None
    dense = mode == "dense"
    model = ef._replace_ffn(ef._mini(65), lambda: ef.Kron3FeedForward(
        tuple(a.ins), tuple(a.hidden), a.rank, rank_chunk=chunk, dense_weight=dense))
    block = model.config.block_size
    opt = torch.optim.AdamW(model.parameters(), lr=1e-3)
    g = torch.Generator().manual_seed(0)
    times, loss = [], None
    for i in range(a.warmup + a.steps):
        x = torch.randint(0, 65, (a.batch, block), generator=g)
        y = torch.randint(0, 65, (a.batch, block), generator=g)
        t = time.perf_counter()
        _, loss = model(x, y)
        opt.zero_grad(set_to_none=True)
        loss.backward()
        opt.step()
        if i >= a.warmup:
            times.append(time.perf_counter() - t)
    times.sort()
    return {"mode": mode, "ins": a.ins, "hidden": a.hidden, "rank": a.rank, "batch": a.batch, "block": block,
            "warmup": a.warmup, "steps": a.steps, "step_s_min": round(times[0], 4),
            "step_s_median": round(times[len(times) // 2], 4), "step_s_max": round(times[-1], 4),
            "last_loss": round(float(loss), 6),
            "max_rss_mb": round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024, 1)}


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--ins", type=int, nargs=3, default=[8, 2, 8])
    ap.add_argument("--hidden", type=int, nargs=3, default=[32, 2, 32])
    ap.add_argument("--rank", type=int, default=126)
    ap.add_argument("--batch", type=int, default=32)
    ap.add_argument("--modes", nargs="+", default=["chunk:18", "dense"])
    ap.add_argument("--warmup", type=int, default=1)
    ap.add_argument("--steps", type=int, default=5)
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--child")
    a = ap.parse_args(argv)
    if a.child:
        print(json.dumps(child(a.child, a)))
        return 0
    for m in a.modes:
        cmd = [sys.executable, __file__, "--child", m, "--ins", *map(str, a.ins), "--hidden", *map(str, a.hidden),
               "--rank", str(a.rank), "--batch", str(a.batch), "--warmup", str(a.warmup), "--steps", str(a.steps),
               "--threads", str(a.threads)]
        p = subprocess.run(cmd, capture_output=True, text=True)
        if p.returncode == 0:
            rec = json.loads(p.stdout.strip().splitlines()[-1])
        else:
            rec = {"mode": m, "failed": True, "returncode": p.returncode,
                   "error": (p.stderr.strip().splitlines() or [""])[-1][:200]}
        rec.update(threads=a.threads, python=platform.python_version())
        print(json.dumps(rec), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
