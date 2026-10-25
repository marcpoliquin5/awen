"""Peak resident memory of one kron3 training step, with and without ``rank_chunk``.

Each configuration runs in a fresh subprocess (so ``ru_maxrss`` is per configuration)
under an address-space cap, so an out-of-memory configuration fails alone instead of
taking the machine down. Output: one JSON line per configuration.

    PYTHONPATH=awen-models python awen-models/examples/kron3_step_memory.py \
        --batch 4 --chunks none 17 --cap-gb 9
"""
from __future__ import annotations

import argparse
import json
import platform
import resource
import subprocess
import sys


def child(batch: int, chunk: int | None, threads: int) -> dict:
    import torch
    from awen_models import efficiency as ef

    torch.set_num_threads(threads)
    torch.manual_seed(0)
    model = ef.build("kron3", 65)
    ef.set_kron3_rank_chunk(model, chunk)
    block = model.config.block_size
    x = torch.randint(0, 65, (batch, block))
    y = torch.randint(0, 65, (batch, block))
    _, loss = model(x, y)
    loss.backward()
    return {"batch": batch, "block": block, "rank_chunk": chunk, "loss": round(float(loss), 6),
            "max_rss_mb": round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024, 1)}


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--batch", type=int, nargs="+", default=[4])
    ap.add_argument("--chunks", nargs="+", default=["none", "17"])
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--cap-gb", type=float, default=9.0)
    ap.add_argument("--child", nargs=2, metavar=("BATCH", "CHUNK"))
    a = ap.parse_args(argv)
    if a.child:
        cap = int(a.cap_gb * 2**30)
        resource.setrlimit(resource.RLIMIT_AS, (cap, cap))
        b, c = int(a.child[0]), (None if a.child[1] == "none" else int(a.child[1]))
        print(json.dumps(child(b, c, a.threads)))
        return 0
    for b in a.batch:
        for c in a.chunks:
            p = subprocess.run([sys.executable, __file__, "--child", str(b), c, "--threads", str(a.threads),
                                "--cap-gb", str(a.cap_gb)], capture_output=True, text=True)
            if p.returncode == 0:
                rec = json.loads(p.stdout.strip().splitlines()[-1])
            else:
                rec = {"batch": b, "rank_chunk": None if c == "none" else int(c), "failed": True,
                       "returncode": p.returncode, "error": (p.stderr.strip().splitlines() or [""])[-1][:200]}
            rec.update(cap_gb=a.cap_gb, threads=a.threads, python=platform.python_version())
            print(json.dumps(rec), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
