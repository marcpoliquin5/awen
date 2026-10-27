# H4 registered full run

Verdict: **kept** under the registered rule (`../../configs/h4.yaml`, `../../PREREGISTRATION.md`, "H4 definitions",
D-021). On every task at least one candidate beats the dense baseline at matched FLOPs per sample (95% paired-t
interval over 3 seeds lies above 0). The candidate that does so on all three tasks is `fno`; `haar_wavelet` beats
dense only on the multi-scale denoising task. Evidence level L0 (CPU training) plus analytic FLOP counts; no
device, energy or speedup claim. Source label for every number below: **measured by us**, stored in
`h4_registered.json` (SHA-256 in `SHA256SUMS`), except FLOPs per sample, which are **analytic** (FLOP convention
in PREREGISTRATION.md).

Command (commit `e9cf511`; environment in `env.txt`: build box Linux 6.12.94+ x86_64, 8 vCPU, no GPU,
Python 3.13.5, torch 2.14.1+cpu, numpy 2.5.4, 8 torch threads):

```
python research/experiments/multiplexing-fractal/h4_multiscale.py \
    --config research/experiments/multiplexing-fractal/configs/h4.yaml
```

Run in a detached worktree at `e9cf511`. Started 2026-10-26 12:38:13 ET, `EXIT 0` at 2026-10-27 01:00:47 ET
(about 12 h 22 min of wall time, against a pre-run estimate of about 1.5 h; the cause was not measured, and wall
time is not a registered metric). Run once; not repeated. Seeds 101, 102, 103; 20 epochs, batch 64, Adam lr 1e-3.

## Results (means over 3 seeds; per-seed values in the JSON and `run.log`)

Lower is better for relative L2 error; higher is better for accuracy. Improvement is dense error minus candidate
error, or candidate accuracy minus dense accuracy.

| Task (score) | Dense | FNO | Haar | FNO improvement, 95% paired-t CI | Haar improvement, 95% paired-t CI |
| --- | --- | --- | --- | --- | --- |
| Burgers 1D (rel. L2) | 0.1779 | 0.0565 | 0.4479 | 0.1214 [0.1032, 0.1396] | -0.2700 [-0.2755, -0.2644] |
| Multi-scale denoise 1D (rel. L2) | 0.3898 | 0.2530 | 0.2769 | 0.1367 [0.1253, 0.1482] | 0.1129 [0.1089, 0.1169] |
| MNIST 28x28 (test accuracy) | 0.9827 | 0.9871 | 0.4126 | 0.0044 [0.0013, 0.0076] | -0.5700 [-0.6120, -0.5281] |

Matched FLOPs per sample (analytic, within the registered 5%): Burgers dense 3,145,728, FNO +1.17%, Haar +2.39%;
denoise dense 4,194,304, FNO +3.52%, Haar +3.93%; MNIST dense 3,723,264, FNO +0.25%, Haar +0.41%.

## What this does and does not show

- At matched FLOPs, this FNO beats this dense MLP on all three registered tasks at 3 seeds. The MNIST margin is
  small (0.44 points, CI 0.13 to 0.76 points).
- This Haar-wavelet model beats dense only on denoising. Its MNIST accuracy (0.41 mean) and Burgers error are far
  worse than dense. That gap is large enough that a defect in the Haar layer or its 2D use is possible; it was not
  investigated before this report, and the verdict is recorded as registered either way. Follow-up check
  (2026-10-27, after the run, see below): no defect found in the Haar transform itself.
- FLOPs per sample are an analytic count, not a measured latency or energy figure. Nothing here says anything
  about photonic hardware; mapping Fourier or wavelet layers to optics needs the full system accounting in
  `awen-spec/hardware_assumptions.yaml` before any speed or energy statement.
- One model size per task, 3 seeds, CPU only; not tested at other sizes, budgets or tasks.

## Follow-up: Haar transform defect check (2026-10-27, after the run; verdict unchanged)

`research/tests/test_fractal_h4_haar_reference.py` checks `h4_multiscale.haar` and `haar_bands` against an
independent NumPy reference built from explicit orthonormal level matrices (no shared code). At the registered
shapes and levels (1-D 256 at 4 levels, 512 at 5; 2-D 28x28 at 2) and on small cases, the forward and inverse
transforms match the reference to 1e-12, energy is preserved, the gradient equals the reference transpose (and
`torch.autograd.gradcheck` passes in 2-D), every band label matches the level its basis function came from, and a
Haar block with identity band mixing reconstructs its input exactly. Result (measured by us, commit of this
change, build box Linux 6.12.94+, Python 3.13.5, torch 2.14.1+cpu, numpy 2.5.4):

```
cd research && python -m pytest -q tests/test_fractal_h4_haar_reference.py   # 11 passed
```

So the MNIST and Burgers gaps are not explained by a wrong transform, inverse, gradient or band layout. Not
tested, and stated only as a hypothesis: the registered Haar layer has one `width x width` mixing matrix per band
(3 matrices at 2 levels on MNIST), shared by every position in that band, so unlike the FNO layer's per-mode
weights it has no position- or scale-position-dependent weights. Any change to the layer would be a new,
separately registered variant; the registered verdict above stays as run.
