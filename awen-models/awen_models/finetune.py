"""Fine-tuning and distillation for AWEN-Mini and small open Hugging Face models.

Three entry points:

* :func:`lora_finetune`: LoRA fine-tuning of a Hugging Face causal LM
  (default ``HuggingFaceTB/SmolLM2-135M``) with a pure-PyTorch LoRA
  implementation (:class:`LoRALinear`). Runs on CPU or GPU.
* QLoRA (``--qlora``): the same LoRA adapters on a 4-bit NF4 base model loaded
  through ``transformers`` + ``bitsandbytes``. This needs a CUDA GPU (the plan
  in ``docs/COMPUTE_PLAN.md`` targets a 12 GB consumer card such as the user's
  RTX 5070 Ti with a cu128+ PyTorch build). It is not exercised in CI because
  no GPU is available there.
* :func:`distill`: knowledge distillation from a Hugging Face teacher into an
  AWEN-Mini student that shares the teacher's tokenizer, compared against the
  same student trained without the teacher at equal steps and seed.

All losses are measured on the machine that runs the script. Loading always
uses ``trust_remote_code=False`` and safetensors.

Examples::

    python -m awen_models.finetune lora --steps 200 --out results/finetune
    python -m awen_models.finetune distill --steps 300 --out results/distill
    python -m awen_models.finetune lora --qlora --model Qwen/Qwen3-0.6B   # CUDA only
"""

from __future__ import annotations

import argparse
import json
import math
import time
from pathlib import Path
from typing import Any, Iterable, Optional, Sequence

import torch
import torch.nn.functional as F
from torch import nn

from .config import MiniConfig
from .data import download_tiny_shakespeare
from .model import AwenMini
from .train import hardware_record

DEFAULT_MODEL = "HuggingFaceTB/SmolLM2-135M"
DEFAULT_TARGETS = ("q_proj", "v_proj")


# ---------------------------------------------------------------------------- LoRA


class LoRALinear(nn.Module):
    """``base(x) + (alpha / r) * B(A(x))`` with ``base`` frozen and ``B`` initialized to zero."""

    def __init__(self, base: nn.Module, r: int = 8, alpha: float = 16.0, dropout: float = 0.0):
        super().__init__()
        if r <= 0:
            raise ValueError("LoRA rank must be positive")
        in_features, out_features = base.in_features, base.out_features
        self.base = base
        for p in self.base.parameters():
            p.requires_grad_(False)
        ref = next(base.parameters())
        dtype = ref.dtype if ref.is_floating_point() else torch.float32
        self.lora_a = nn.Linear(in_features, r, bias=False, device=ref.device, dtype=dtype)
        self.lora_b = nn.Linear(r, out_features, bias=False, device=ref.device, dtype=dtype)
        nn.init.kaiming_uniform_(self.lora_a.weight, a=math.sqrt(5))
        nn.init.zeros_(self.lora_b.weight)
        self.scale = alpha / r
        self.dropout = nn.Dropout(dropout)
        self.in_features, self.out_features = in_features, out_features

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        update = self.lora_b(self.lora_a(self.dropout(x.to(self.lora_a.weight.dtype))))
        return self.base(x) + (self.scale * update).to(x.dtype)

    @torch.no_grad()
    def merged_weight(self) -> torch.Tensor:
        """Dense weight equal to the adapted layer (only for an unquantized ``nn.Linear`` base)."""
        if not isinstance(self.base, nn.Linear) or type(self.base) is not nn.Linear:
            raise TypeError("merging is supported only for a plain nn.Linear base")
        return self.base.weight + self.scale * (self.lora_b.weight @ self.lora_a.weight).to(self.base.weight.dtype)


def apply_lora(model: nn.Module, targets: Iterable[str] = DEFAULT_TARGETS, r: int = 8, alpha: float = 16.0) -> list[str]:
    """Freeze ``model`` and wrap every submodule whose name ends with a target in :class:`LoRALinear`."""
    targets = tuple(targets)
    for p in model.parameters():
        p.requires_grad_(False)
    replaced = []
    for name, module in list(model.named_modules()):
        for child_name, child in list(module.named_children()):
            full = f"{name}.{child_name}" if name else child_name
            if child_name in targets and hasattr(child, "in_features") and not isinstance(child, LoRALinear):
                setattr(module, child_name, LoRALinear(child, r=r, alpha=alpha))
                replaced.append(full)
    if not replaced:
        raise ValueError(f"no modules named {targets} found")
    return replaced


def trainable_parameters(model: nn.Module) -> int:
    return sum(p.numel() for p in model.parameters() if p.requires_grad)


# --------------------------------------------------------------------------- data


def token_splits(tokenizer, text: str, val_fraction: float = 0.1) -> tuple[torch.Tensor, torch.Tensor]:
    ids = torch.tensor(tokenizer(text, add_special_tokens=False)["input_ids"], dtype=torch.long)
    cut = int(len(ids) * (1 - val_fraction))
    return ids[:cut], ids[cut:]


def _batch(data: torch.Tensor, batch: int, block: int, gen: torch.Generator) -> tuple[torch.Tensor, torch.Tensor]:
    starts = torch.randint(len(data) - block - 1, (batch,), generator=gen).tolist()
    x = torch.stack([data[s : s + block] for s in starts])
    y = torch.stack([data[s + 1 : s + 1 + block] for s in starts])
    return x, y


def _fixed_eval_windows(data: torch.Tensor, block: int, count: int) -> list[tuple[torch.Tensor, torch.Tensor]]:
    stride = max(1, (len(data) - block - 1) // count)
    return [
        (data[s : s + block].unsqueeze(0), data[s + 1 : s + 1 + block].unsqueeze(0))
        for s in range(0, stride * count, stride)
    ]


@torch.no_grad()
def hf_eval_loss(model, windows, device) -> float:
    model.eval()
    losses = [float(F.cross_entropy(model(input_ids=x.to(device)).logits.flatten(0, 1).float(), y.to(device).flatten()))
              for x, y in windows]
    return sum(losses) / len(losses)


@torch.no_grad()
def mini_eval_loss(model: AwenMini, windows) -> float:
    model.eval()
    return sum(float(model(x, y)[1]) for x, y in windows) / len(windows)


# ------------------------------------------------------------------------ loading


def load_base(model_id: str, qlora: bool = False, revision: Optional[str] = None):
    from transformers import AutoModelForCausalLM, AutoTokenizer

    tokenizer = AutoTokenizer.from_pretrained(model_id, revision=revision, trust_remote_code=False)
    kwargs: dict[str, Any] = {"revision": revision, "trust_remote_code": False, "use_safetensors": True}
    if qlora:
        kwargs.update(qlora_load_kwargs())
    else:
        kwargs["dtype"] = torch.float32
    model = AutoModelForCausalLM.from_pretrained(model_id, **kwargs)
    return model, tokenizer


def qlora_load_kwargs() -> dict[str, Any]:
    """``from_pretrained`` arguments for a 4-bit NF4 base with bf16 compute (QLoRA, Dettmers et al. 2023).

    Raises ``RuntimeError`` with the reason when CUDA or bitsandbytes is unavailable.
    """
    if not torch.cuda.is_available():
        raise RuntimeError("QLoRA needs a CUDA GPU (4-bit bitsandbytes kernels); use plain LoRA on CPU")
    try:
        import bitsandbytes  # noqa: F401
        from transformers import BitsAndBytesConfig
    except ImportError as error:
        raise RuntimeError("QLoRA needs `pip install bitsandbytes` (CUDA build)") from error
    return {
        "quantization_config": BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_use_double_quant=True,
            bnb_4bit_compute_dtype=torch.bfloat16,
        ),
        "device_map": {"": 0},
    }


# ----------------------------------------------------------------- LoRA training


def lora_finetune(
    model_id: str = DEFAULT_MODEL,
    text: Optional[str] = None,
    steps: int = 200,
    batch_size: int = 4,
    block_size: int = 128,
    lr: float = 2e-4,
    r: int = 8,
    alpha: float = 16.0,
    targets: Sequence[str] = DEFAULT_TARGETS,
    eval_windows: int = 32,
    seed: int = 0,
    qlora: bool = False,
    model=None,
    tokenizer=None,
) -> dict[str, Any]:
    torch.manual_seed(seed)
    gen = torch.Generator().manual_seed(seed)
    if model is None:
        model, tokenizer = load_base(model_id, qlora=qlora)
    device = next(model.parameters()).device
    text = text if text is not None else download_tiny_shakespeare()
    train_ids, val_ids = token_splits(tokenizer, text)
    windows = _fixed_eval_windows(val_ids, block_size, eval_windows)
    before = hf_eval_loss(model, windows, device)
    replaced = apply_lora(model, targets, r=r, alpha=alpha)
    params = [p for p in model.parameters() if p.requires_grad]
    opt = torch.optim.AdamW(params, lr=lr, weight_decay=0.0)
    start = time.perf_counter()
    model.train()
    for _ in range(steps):
        x, y = _batch(train_ids, batch_size, block_size, gen)
        logits = model(input_ids=x.to(device)).logits
        loss = F.cross_entropy(logits.flatten(0, 1).float(), y.to(device).flatten())
        opt.zero_grad()
        loss.backward()
        opt.step()
    seconds = time.perf_counter() - start
    after = hf_eval_loss(model, windows, device)
    return {
        "version": "awen.lora-finetune.v1",
        "model": model_id,
        "method": "qlora-nf4" if qlora else "lora",
        "lora": {"r": r, "alpha": alpha, "targets": list(targets), "wrapped_modules": len(replaced)},
        "trainable_parameters": trainable_parameters(model),
        "total_parameters": sum(p.numel() for p in model.parameters()),
        "steps": steps, "batch_size": batch_size, "block_size": block_size, "lr": lr, "seed": seed,
        "train_tokens": len(train_ids), "val_tokens": len(val_ids),
        "val_loss_before": before, "val_loss_after": after,
        "train_seconds": seconds,
        "evidence": "measured validation cross-entropy (nats/token) on fixed held-out windows",
        "hardware": hardware_record(device),
    }


# --------------------------------------------------------------------- distillation


def kd_loss(student_logits: torch.Tensor, teacher_logits: torch.Tensor, targets: torch.Tensor,
            temperature: float = 2.0, alpha: float = 0.5) -> torch.Tensor:
    """``alpha * T^2 * KL(teacher_T || student_T) + (1 - alpha) * CE(student, targets)`` (Hinton et al. 2015)."""
    s = student_logits.flatten(0, 1).float()
    t = teacher_logits.flatten(0, 1).float()
    kl = F.kl_div(F.log_softmax(s / temperature, -1), F.log_softmax(t / temperature, -1),
                  log_target=True, reduction="batchmean")
    return alpha * temperature**2 * kl + (1 - alpha) * F.cross_entropy(s, targets.flatten())


def distill(
    teacher_id: str = DEFAULT_MODEL,
    text: Optional[str] = None,
    steps: int = 300,
    batch_size: int = 8,
    block_size: int = 128,
    lr: float = 1e-3,
    temperature: float = 2.0,
    alpha: float = 0.5,
    student: Optional[dict[str, Any]] = None,
    eval_windows: int = 32,
    seed: int = 0,
    teacher=None,
    tokenizer=None,
) -> dict[str, Any]:
    if teacher is None:
        teacher, tokenizer = load_base(teacher_id)
    teacher.eval()
    for p in teacher.parameters():
        p.requires_grad_(False)
    text = text if text is not None else download_tiny_shakespeare()
    train_ids, val_ids = token_splits(tokenizer, text)
    windows = _fixed_eval_windows(val_ids, block_size, eval_windows)
    vocab = teacher.get_output_embeddings().weight.shape[0]
    student_cfg = MiniConfig(vocab_size=vocab, block_size=block_size, **(student or {"n_layer": 4, "n_embd": 128, "n_head": 4, "ffn_hidden": 512}))
    teacher_loss = hf_eval_loss(teacher, windows, "cpu")
    results = {}
    for mode in ("kd", "ce-only"):
        torch.manual_seed(seed)
        gen = torch.Generator().manual_seed(seed)
        model = AwenMini(student_cfg)
        opt = torch.optim.AdamW(model.parameters(), lr=lr, weight_decay=0.01)
        start = time.perf_counter()
        for _ in range(steps):
            model.train()
            x, y = _batch(train_ids, batch_size, block_size, gen)
            logits, ce = model(x, y)
            if mode == "kd":
                with torch.no_grad():
                    t_logits = teacher(input_ids=x).logits[..., :vocab]
                loss = kd_loss(logits, t_logits, y, temperature, alpha)
            else:
                loss = ce
            opt.zero_grad()
            loss.backward()
            opt.step()
        results[mode] = {"val_loss": mini_eval_loss(model, windows), "train_seconds": time.perf_counter() - start}
    return {
        "version": "awen.distill.v1",
        "teacher": teacher_id,
        "teacher_val_loss": teacher_loss,
        "student_config": student_cfg.to_dict(),
        "student_parameters": AwenMini(student_cfg).num_parameters(),
        "steps": steps, "batch_size": batch_size, "block_size": block_size, "lr": lr,
        "temperature": temperature, "alpha": alpha, "seed": seed,
        "results": results,
        "evidence": "measured validation cross-entropy (nats/token, teacher tokenizer) on fixed held-out windows",
        "hardware": hardware_record(torch.device("cpu")),
    }


def main(argv: Optional[Sequence[str]] = None) -> None:
    parser = argparse.ArgumentParser(description="AWEN fine-tuning and distillation")
    sub = parser.add_subparsers(dest="command", required=True)
    lora = sub.add_parser("lora")
    lora.add_argument("--model", default=DEFAULT_MODEL)
    lora.add_argument("--steps", type=int, default=200)
    lora.add_argument("--rank", type=int, default=8)
    lora.add_argument("--qlora", action="store_true")
    lora.add_argument("--seed", type=int, default=0)
    lora.add_argument("--out", default="results/finetune")
    dist = sub.add_parser("distill")
    dist.add_argument("--teacher", default=DEFAULT_MODEL)
    dist.add_argument("--steps", type=int, default=300)
    dist.add_argument("--seed", type=int, default=0)
    dist.add_argument("--out", default="results/distill")
    args = parser.parse_args(argv)
    if args.command == "lora":
        report = lora_finetune(args.model, steps=args.steps, r=args.rank, qlora=args.qlora, seed=args.seed)
    else:
        report = distill(args.teacher, steps=args.steps, seed=args.seed)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    name = f"{args.command}-{(args.model if args.command == 'lora' else args.teacher).split('/')[-1]}.json"
    (out / name).write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "hardware"}, indent=2))


if __name__ == "__main__":
    main()
