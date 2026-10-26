# kron3 `dense_weight` step time (measured by us)

Training steps (forward, backward, AdamW) of a char-LM AWEN-Mini whose FFN is `Kron3FeedForward` with the H2
amendment A1 shape `(8, 2, 8) -> (32, 2, 32)`, rank 126, block 128, batch 32, 4 threads, random token batches,
1 untimed warm-up step then 5 timed steps, each configuration in a fresh subprocess (`max_rss_mb` = its `ru_maxrss`).
Single run per configuration. Modes: `chunk:18` = the per-token contraction with `rank_chunk` 18 (the A1 setting);
`dense` = `dense_weight=True` (dense-equivalent weight built once per forward, then one matrix product).

Command (repo root, at commit `f61a02e`, build box: see `env.txt`; Linux, 8 vCPU, no GPU):

    PYTHONPATH=awen-models python awen-models/examples/kron3_step_time.py --ins 8 2 8 --hidden 32 2 32 \
        --rank 126 --batch 32 --modes chunk:18 dense --warmup 1 --steps 5 --threads 4

Output: `step_time.jsonl`. Median step: `chunk:18` 32.8607 s, `dense` 0.2565 s; peak RSS 1239.7 MB vs 729.1 MB;
the loss after the 6 steps is the same to 6 decimals (4.184411) on both paths, as expected for the same model.
This is CPU training-step time on this box only; it says nothing about photonic latency or energy.
