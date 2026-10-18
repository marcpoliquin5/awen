# awen-models

Small models trained for real, plus the `awen` runtime facade that runs them.
Free to use under the repository's MIT license.

```bash
pip install ./awen-models            # torch, numpy, safetensors
python awen-models/examples/generate.py --prompt "ROMEO:" --tokens 300
```

```python
from awen import Runtime

model = Runtime(device="auto").load_model("awen-mini")   # bundled checkpoint
print(model.generate("ROMEO:", max_new_tokens=200, seed=0))
```

## AWEN-Mini

A 1,016,320-parameter character-level decoder-only transformer:

| Setting | Value |
| --- | --- |
| Layers / width / heads | 5 / 128 / 4 |
| Context | 128 characters |
| Feed-forward | GELU, 512 hidden |
| Head | tied to the token embedding |
| Tokenizer | 65 characters (`awen.char-tokenizer.v1`) |

**Data.** Tiny Shakespeare: 1,115,394 characters. The text is public domain;
the file comes from the MIT-licensed `karpathy/char-rnn` repository and is
checked by SHA-256 `86c4e6aa…dc565ed`. The split is contiguous: the first 90%
trains and the last 10% (111,540 characters) is held out for validation.

**Training.** CPU only, 6 threads, seed 1337. 6,000 steps of batch 64 × 128
characters (49.2 M training tokens). AdamW at learning rate 1e-3 with cosine
decay to 1e-4 after 100 warmup steps; weight decay 0.1, dropout 0.1, gradient
clip 1.0. The command:

```bash
python -m awen_models.train --steps 6000 --batch-size 64 --eval-interval 250 \
  --dropout 0.1 --threads 6 --out runs/awen-mini
```

**Results.** Measured on this repository's build machine: an 8-vCPU Intel Xeon
running Linux, PyTorch 2.14.1+cpu, Python 3.13.5. Training wall time was
2,167 s.

| Metric | Value |
| --- | --- |
| Selected checkpoint | step 5,750 (lowest validation loss) |
| Train / validation loss during training | 1.199 / 1.479 nats |
| Validation loss, fixed evaluation set (float32) | 1.4717 nats (2.123 bits/char) |
| Validation loss, per-channel int8 weights | 1.4716 nats |
| Uniform-guess baseline | 4.174 nats |

The full loss curve is in `awen_models/checkpoints/awen-mini/loss_history.json`.
The evaluation report is `results/awen-mini-eval.json`; reproduce it with
`python -m awen_models.evaluate`. A 1M-parameter character model learns
spelling, play formatting and speaker names. It does not produce coherent
meaning.

The checkpoint is `awen_models/checkpoints/awen-mini/`: safetensors weights
(4.07 MB, SHA-256 `3a4e2158…a04b003`), config, tokenizer and metadata. Loading
it never executes code from the file.

## Execution targets

| `device=` | What runs | Label |
| --- | --- | --- |
| `auto`, `cpu`, `cuda` | PyTorch on the host | measured |
| `photonic-sim` | Every linear-layer GEMM runs on the AWEN Rust photonic simulator with its noise model (`awen_native`) | simulated |

`photonic-sim` needs the native extension (`pip install ./awen-native`).

```python
import awen_native
from awen import Runtime

contract = awen_native.default_contract()        # illustrative, not a measured device
contract["photons_per_mac"] = 1e5
sim = Runtime(device="photonic-sim", noise=contract, seed=0).load_model("awen-mini")
print(sim.generate("ROMEO:", max_new_tokens=100))   # simulated
```

### Photonic simulation vs the CPU oracle (simulated)

`python -m awen_models.photonic_demo --out results/photonic-sim-demo.json`
compares two things against the CPU:

- one AWEN-Mini layer (`blocks.0.ffn.fc`) run on a real validation activation;
- the whole model's validation loss when all 20 transformer linear layers run
  on the Rust photonic model. The tied output head stays on the host.

The base contract is the default: 8-bit DAC/ADC, 100 e⁻ thermal noise,
6 dB insertion loss, 2 mrad phase error and −35 dB crosstalk. Only
`photons_per_mac` is swept. All photonic rows are **simulated**
(compact model). The CPU rows are measured.

| Photons per MAC | Layer relative RMS error | Layer effective bits | Model val loss (CPU 1.4548) |
| --- | --- | --- | --- |
| ideal contract | 5e-17 | exact | 1.4548 |
| 10 | 4.8 | < 0 | 5.006 |
| 100 | 0.62 | < 0 | 4.839 |
| 1,000 | 0.090 | 2.7 | 4.239 |
| 10,000 | 0.027 | 4.4 | 1.706 |
| 100,000 | 0.018 | 5.0 | 1.527 |

Reading:

- The ideal contract reproduces the CPU exactly.
- Under the default contract the model only approaches CPU quality at
  10⁴–10⁵ photons per MAC.
- The layer error levels off near 5 effective bits. With 8-bit converters
  and per-vector scaling, typical activations use only a small part of the
  converter range: the model reproduces this dynamic-range loss of analog
  accumulation. It is a model result, not a hardware measurement.

## AWEN-Nonlinear ablation

`python -m awen_models.nonlinear --steps 1500 --seeds 1 2 --out runs/nonlinear`
trains feed-forward variants at an equal parameter budget (±3%), on identical
data and steps, with at least two seeds each. It reports validation loss,
parameters, latency, a memory estimate and a Pareto plot. The committed run
(`results/nonlinear/results.json` and `pareto.png`) used 4-layer models and
1,500 steps of batch 32 on the same CPU as above:

| Variant | Params | Val loss (mean ± sd, seeds 1, 2) | Forward latency, batch 1 | Train-step memory (estimate) |
| --- | --- | --- | --- | --- |
| Baseline GELU MLP (hidden 512) | 818,048 | 1.718 ± 0.004 | 1.13 ms | 158 MiB |
| Gated multiplicative (SiLU·linear, hidden 341) | 818,216 | **1.625 ± 0.007** | 1.16 ms | 187 MiB |
| Low-rank wide MLP (hidden 1024, rank 56) | 811,904 | 1.974 ± 0.004 | 1.42 ms | 229 MiB |
| Recursive shared block (d=256, 4 passes) | 839,680 | **1.611 ± 0.006** | 2.79 ms | 308 MiB |

The gated multiplicative FFN beats the matched baseline by 0.09 nats at the
same parameter count and nearly the same latency, so it sits on the Pareto
front. The recursive shared block reaches the lowest loss, but it spends
about 2.5× the latency and 2× the memory: it buys quality with compute, not
with parameters. **Negative result:** the low-rank wide MLP is clearly worse
than the baseline at this budget.

Caveats:

- These are short runs (1,500 steps) on one dataset, and the ranking may
  change with longer training.
- The comparison holds parameters fixed, not FLOPs.
- Latency was measured while other jobs shared the CPU, so treat it as a
  relative comparison only.

## Fine-tuning, QLoRA and distillation

`python -m awen_models.finetune lora|distill` (`awen-finetune`):

- **LoRA**: pure-PyTorch `LoRALinear` adapters on any Hugging Face causal LM.
  Loading always uses `trust_remote_code=False` and safetensors.
- **QLoRA** (`--qlora`): the same adapters on an NF4 4-bit base through
  `bitsandbytes` (`pip install ./awen-models[qlora]`). It needs a CUDA GPU
  and refuses to start without one.
- **Distillation**: a Hugging Face teacher into an AWEN-Mini student that uses
  the teacher's tokenizer (Hinton et al. 2015 loss, T = 2, α = 0.5). It is
  compared against the same student trained without the teacher, at equal
  steps and seed.

Committed CPU smoke results (`results/finetune/`, measured on an 8-vCPU Xeon):

| Run | Result |
| --- | --- |
| LoRA r=8 on q/v of SmolLM2-135M, 200 steps | held-out loss 3.592 → 3.353 nats/token (121 s) |
| KD into a 7.10M AWEN-Mini student, 300 steps, seed 0 / 1 | 5.209 / 5.331 with KD vs 5.426 / 5.637 CE-only |

Memory and compute sizing for larger runs on the user's 12 GB GPU is in
`docs/COMPUTE_PLAN.md` (`python -m awen_models.compute_plan`). Its numbers are
estimates.
## AWEN-Photon noise-injection training

`python -m awen_models.photon_training --seeds 1 2` (`awen-photon-train`) trains
a 784-128-10 MLP on MNIST (CC BY-SA 3.0, SHA-256 pinned) three ways: standard,
and noise-aware with the photonic noise contract at 100 and 1,000 photons per
MAC injected during training through a differentiable surrogate of the Rust
tile model. Every trained network is then evaluated with both GEMMs executed by
`awen_native.photonic_gemm` across 10 to 100,000 photons per MAC. Needs
`awen-native`.

Committed results (`results/photon-training/`, 3 epochs, seeds 1 and 2, CPU).
Accuracy is on the 10,000-image test set and is the mean of the two seeds.
Photonic accuracies are **simulated** (compact noise model, default contract with
8-bit DAC/ADC, k = 64 tiles). Energy is **modeled** photon energy at 1550 nm
only. It excludes laser, modulator, converter and memory energy.

| photons/MAC | tile ENOB (analytic) | optical energy/MAC | standard | noise-aware-100 | noise-aware-1000 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| exact (ideal contract) | — | — | 97.06% | 87.00% | 95.53% |
| 10 | −0.44 | 1.3 aJ | 9.9% | 10.5% | 10.0% |
| 30 | 1.13 | 3.8 aJ | 10.2% | 14.7% | 10.4% |
| 100 | 2.80 | 12.8 aJ | 13.9% | 51.0% | 18.2% |
| 300 | 4.23 | 38.4 aJ | 35.7% | 80.9% | 57.0% |
| 1,000 | 5.54 | 128 aJ | 82.4% | 85.9% | 90.9% |
| 3,000 | 6.38 | 384 aJ | 94.5% | 86.7% | 94.6% |
| 10,000 | 6.90 | 1.28 fJ | 96.4% | 86.9% | 95.1% |
| 100,000 | 7.18 | 12.8 fJ | 96.8% | 86.9% | 95.4% |

Noise-aware training moves the usable operating point to fewer photons.
Training at 100 photons/MAC lifts accuracy at 100 photons from 13.9% to 51.0%
and at 300 photons from 35.7% to 80.9%, but caps clean accuracy at about 87%.
Training at 1,000 photons/MAC gains 8.5 points at 1,000 photons for a
1.4-point loss at 100,000 photons. ENOB saturates near 7.2 bits because of
the 8-bit ADC. With two seeds, the band in `accuracy_curves.png` is the seed
min–max. Per-point 95% Wilson intervals (test-set sampling only) are in
`results.json`.

The surrogate omits phase error and crosstalk during training. Evaluation
applies them through the Rust model. `tests/test_photon_training.py` checks
that the surrogate's error standard deviation is within 5% of the Rust
simulator's.

## Hugging Face adapter

`python -m awen_models.hf_adapter HuggingFaceTB/SmolLM2-135M` loads a causal
LM with `trust_remote_code=False` and safetensors only, generates text, and
prints a capability report. The report lists which linear layers can lower to
an AWEN GEMM and which stay on the CPU, and why. Needs `pip install
"./awen-models[hf]"`.

## ONNX import

`pip install "awen-models[onnx]"`, then:

```bash
awen-onnx-report model.onnx --json report.json
```

```python
from awen_models import onnx_import
report = onnx_import.analyze("model.onnx")          # per-node placement and level
outputs, info = onnx_import.run("model.onnx", {"x": x})                  # f64 reference
outputs, info = onnx_import.run("model.onnx", {"x": x}, contract=c)      # simulated
```

Constant-weight `MatMul`/`Gemm` and 2-D `Conv` (group 1, no dilation) map to
the AWEN `gemm` and `conv2d` operators and are placed `photonic`.
Activation-activation `MatMul`, elementwise ops (`Add`, `Sub`, `Mul`, `Div`,
`Relu`, `Sigmoid`, `Tanh`, `Gelu`, `Softmax`) and shape ops stay
`electronic`. Everything else is `unsupported`: the report lists it and `run`
refuses the graph. The photonic MAC fraction is static analysis, not a
measurement. Not mapped yet: `Gather`, `LayerNormalization`, pooling, grouped
and dilated convolution, and fused attention ops.

## Model conversion (`awen-convert`)

```bash
# AWEN-Mini checkpoint -> ONNX, checked against PyTorch with onnx's reference evaluator
awen-convert to-onnx --output mini.onnx --seq-len 32 --report mini.conversion.json
# ONNX -> awen.tensor.v1 program (constant-weight MatMul/Gemm only), then plan it
awen-convert to-program mini.onnx --output mini.program.json
awenctl compile mini.program.json --output mini.compiled.json
```

`to-onnx` uses the legacy TorchScript exporter at opset 17 with a fixed
sequence length. On the bundled checkpoint at sequence length 32 the largest
logit difference between PyTorch and `onnx.reference.ReferenceEvaluator` was
1.0e-05 (run on CPU here, 2026-10-18); the test requires < 1e-4. The report
also carries the ONNX compatibility analysis: 21 of 31 MatMul nodes have a
constant weight and are photonic candidates (96% of the MACs at that
sequence length); the 10 attention products multiply two activations and
stay electronic.

`to-program` keeps only the GEMMs. Gemm `alpha` and bias are listed as
electronic epilogues and are not part of the program. The default dtype is
`f32` because the model is fp32; the reference photonic capability accepts
`f16`/`int8` only, so `awenctl compile` places an fp32 program on the CPU and
says why. Pass `--dtype int8` to plan a quantized deployment.

## Tests

```bash
pip install "./awen-models[test]"
python -m pytest awen-models/tests
```
