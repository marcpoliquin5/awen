# Compute and cost plan

How AWEN's model work is sized for the hardware actually available. Every
number in the tables comes from `awen_models.compute_plan` and is an
**estimate** from closed-form formulas. Rows marked *measured* are
measurements. Nothing here requires paid compute.

## Available compute

| Resource | Use | Status |
| --- | --- | --- |
| CI runners and the development box (8-vCPU Xeon, no GPU) | AWEN-Mini training, noise-aware training, LoRA and distillation smoke runs on SmolLM2-135M | used; all committed results come from here |
| The user's RTX 5070 Ti (directive: 12 GB, CUDA 12.8+ PyTorch build) | QLoRA/LoRA fine-tuning of Qwen3-0.6B/1.7B and SmolLM2-360M/1.7B | not yet run; needs the user to run the commands below |
| Cloud GPUs | — | not used; no spend is planned. Any future cloud use needs the user's approval and a budget first |

The 12 GB figure comes from the directive and matches the laptop RTX 5070 Ti.
NVIDIA lists the desktop RTX 5070 Ti with 16 GB GDDR7. The plan below
budgets for 12 GB, so it also fits the 16 GB card.

## Measured CPU smoke runs (committed in `awen-models/results/finetune/`)

| Run | Setting | Result (measured on the development box) |
| --- | --- | --- |
| LoRA, SmolLM2-135M | r = 8 on q/v (460,800 trainable of 135.0M), 200 steps × 4 × 128 tokens, lr 2e-4 | held-out loss 3.592 → 3.353 nats/token in 121 s |
| Distillation into AWEN-Mini | 7.10M-parameter student with the SmolLM2 tokenizer, teacher SmolLM2-135M, T = 2, α = 0.5, 300 steps × 8 × 128 | see `distill-*.json`: KD vs CE-only at equal steps and seed |

These runs show that the scripts work end to end and that the losses move
the right way. They are not quality claims about the resulting models.

## Memory estimate for one training step (batch 1, sequence 1,024, LoRA rank 16 on all seven projections)

`python -m awen_models.compute_plan --config <config.json> --budget-gb 12 --seq 1024 --rank 16`

| Model | Parameters (exact, from config) | LoRA params | Full fine-tune bf16 + AdamW | LoRA, bf16 base | QLoRA, NF4 base |
| --- | ---: | ---: | ---: | ---: | ---: |
| SmolLM2-135M | 134,515,008 | 4,884,480 | 3.7 GiB | 1.7 GiB | 1.6 GiB |
| SmolLM2-360M | 361,821,120 | 8,683,520 | 7.5 GiB | 2.3 GiB | 1.8 GiB |
| Qwen3-0.6B | 596,049,920 | 10,092,544 | 12.2 GiB (does not fit 12 GB) | 3.5 GiB | 2.9 GiB |
| SmolLM2-1.7B | 1,711,376,384 | 18,087,936 | 30.2 GiB (does not fit) | 5.0 GiB | 2.8 GiB |
| Qwen3-1.7B | 1,720,574,976 | 17,432,576 | 31.2 GiB (does not fit) | 5.8 GiB | 3.9 GiB |

Each total includes 1 GiB assumed for the CUDA context and allocator
fragmentation. Qwen3's 151,936-token vocabulary makes the fp32 logits and
their gradient large. At batch 2 × 2,048 tokens they alone are 4.6 GiB, which
puts Qwen3-1.7B QLoRA at about 7.9 GiB, still inside 12 GB. Chunked
cross-entropy would remove most of this term. A parameter count is checked
in CI against instantiated Llama and Qwen3 models. The memory model is not
checked against a GPU. The first GPU run should record
`torch.cuda.max_memory_allocated()` so the estimate can be corrected.

## Compute estimate

Training compute is ≈ 6 · N · D FLOPs (Kaplan et al., 2020). LoRA with
gradient checkpointing costs about the same: forward, recompute and
input-gradient backward. For D = 20M tokens:

| Model | FLOPs | Hours at 10 TFLOP/s sustained |
| --- | ---: | ---: |
| SmolLM2-135M | 1.6e16 | 0.45 |
| SmolLM2-360M | 4.3e16 | 1.2 |
| Qwen3-0.6B | 7.2e16 | 2.0 |
| SmolLM2-1.7B | 2.1e17 | 5.7 |
| Qwen3-1.7B | 2.1e17 | 5.7 |

To scale the table, measure the sustained rate on the target GPU:
`python -m awen_models.compute_plan --config qwen3.json --measure --utilization 0.3`.
That command times a bf16 matmul on the local device and uses it as the
peak. Utilization is an assumption. Report the measured tokens per second
from the first real run instead.

## Commands for the user's GPU

```bash
pip install "torch>=2.10" --index-url https://download.pytorch.org/whl/cu128
pip install ./awen-models[hf] bitsandbytes
python -m awen_models.finetune lora --qlora --model Qwen/Qwen3-0.6B --steps 1000 --rank 16 --out results/finetune
python -m awen_models.finetune lora --qlora --model Qwen/Qwen3-1.7B --steps 1000 --rank 16 --out results/finetune
python -m awen_models.finetune distill --teacher HuggingFaceTB/SmolLM2-360M --steps 3000 --out results/distill
```

Each run writes a JSON report with the measured loss before and after,
wall time, and hardware. QLoRA refuses to start without CUDA. On CPU, use
plain LoRA.

## Budget rule

No paid compute is used. Downloads are limited to small public checkpoints
(SmolLM2-135M, about 270 MB) and datasets pinned by SHA-256 (Tiny
Shakespeare, MNIST). Larger checkpoints are downloaded only on the user's
machine, by the user.
