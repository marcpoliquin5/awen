# kron3 `rank_chunk` step memory (measured by us)

One forward + backward training step of `efficiency.build("kron3", 65)` (block 128, 4 threads), each
configuration in a fresh subprocess capped at 9 GB address space; `max_rss_mb` is that subprocess's `ru_maxrss`.
Single run per configuration, no repeats, so treat small differences as noise. Step time was not measured.

Commands (repo root, at commit `7549936`, build box: see `env.txt`; Linux, 8 vCPU, no GPU):

    PYTHONPATH=awen-models python awen-models/examples/kron3_step_memory.py --batch 4 --chunks none 17 --cap-gb 9
    PYTHONPATH=awen-models python awen-models/examples/kron3_step_memory.py --batch 32 --chunks 17 --cap-gb 9

Output: `step_memory.jsonl`. Unchunked at batch 32 was not run here (an earlier run on this box was OOM-killed at
about 13.7 GB, see docs/sessions/2026-10-25-02.md).
