"""External model adapter: load an open Hugging Face causal LM safely and report
which of its layers could lower to AWEN GEMM.

Loading rules: ``trust_remote_code=False`` and ``use_safetensors=True`` are
always passed, so no repository-provided Python runs and no pickle is
deserialized.

The capability report is a static analysis against a declared AWEN matrix-core
capability (default: the synthetic ``reference-pace-like-128`` profile in
``awen-compiler/capabilities``). It states what *could* be lowered under that
profile, not measured photonic performance.

Example::

    python -m awen_models.hf_adapter HuggingFaceTB/SmolLM2-135M --prompt "The light"
"""

from __future__ import annotations

import argparse
import json
import math
from dataclasses import asdict, dataclass, field
from typing import Any, Optional

import torch
import torch.nn as nn

DEFAULT_CAPABILITY = {
    "backend_id": "reference-pace-like-128",
    "matrix_core": {"m": 128, "n": 128, "k": 128},
    "supported_dtypes": ["f16", "bf16", "int8", "int4"],
    "effective_bits": 8,
    "evidence": "synthetic reference capability; not a measured device",
}

_ROLE_HINTS = (
    ("q_proj", "attention query projection"),
    ("k_proj", "attention key projection"),
    ("v_proj", "attention value projection"),
    ("o_proj", "attention output projection"),
    ("gate_proj", "MLP gate projection"),
    ("up_proj", "MLP up projection"),
    ("down_proj", "MLP down projection"),
    ("lm_head", "output vocabulary projection"),
    ("c_attn", "fused attention QKV projection"),
    ("c_proj", "projection"),
    ("c_fc", "MLP up projection"),
)

_CPU_REASONS = {
    "Embedding": "table lookup (gather), not a matrix multiply",
    "LayerNorm": "per-token reduction and division; kept in electronic post-processing",
    "RMSNorm": "per-token reduction and division; kept in electronic post-processing",
    "Dropout": "identity at inference",
    "SiLU": "elementwise nonlinearity; applied electronically after detection",
    "GELU": "elementwise nonlinearity; applied electronically after detection",
    "Rotary": "position-dependent rotation of activations",
}


@dataclass
class LayerDecision:
    name: str
    module_type: str
    role: str
    target: str
    reason: str
    shape: Optional[list[int]] = None
    parameters: int = 0
    tiles: Optional[int] = None
    macs_per_token: int = 0


@dataclass
class CapabilityReport:
    model: str
    capability: dict[str, Any]
    layers: list[LayerDecision] = field(default_factory=list)
    attention_score_note: str = (
        "Attention scores (Q·Kᵀ) and score·V multiply two activations. They need the "
        "optical weights reprogrammed every token, so this report keeps them on the host."
    )

    @property
    def summary(self) -> dict[str, Any]:
        awen = [l for l in self.layers if l.target == "awen-gemm"]
        host = [l for l in self.layers if l.target != "awen-gemm"]
        total_linear_macs = sum(l.macs_per_token for l in self.layers)
        return {
            "layers_considered": len(self.layers),
            "awen_gemm_layers": len(awen),
            "host_layers": len(host),
            "awen_gemm_parameters": sum(l.parameters for l in awen),
            "host_parameters": sum(l.parameters for l in host),
            "awen_gemm_macs_per_token": sum(l.macs_per_token for l in awen),
            "awen_gemm_mac_fraction_of_dense_layers": (
                round(sum(l.macs_per_token for l in awen) / total_linear_macs, 4)
                if total_linear_macs
                else 0.0
            ),
            "matrix_core_tiles": sum(l.tiles or 0 for l in awen),
        }

    def to_dict(self) -> dict[str, Any]:
        return {
            "report": "awen.model-capability-report.v1",
            "model": self.model,
            "capability": self.capability,
            "summary": self.summary,
            "attention_score_note": self.attention_score_note,
            "layers": [asdict(l) for l in self.layers],
        }


def _role(name: str) -> str:
    leaf = name.rsplit(".", 1)[-1]
    for hint, role in _ROLE_HINTS:
        if leaf == hint:
            return role
    return "dense projection"


def _dense_weight(module: nn.Module) -> Optional[torch.Tensor]:
    """Return the weight as ``[out, in]`` for nn.Linear and GPT-2 style Conv1D."""
    if isinstance(module, nn.Linear):
        return module.weight
    if type(module).__name__ == "Conv1D" and hasattr(module, "weight"):
        return module.weight.T  # Conv1D stores [in, out]
    return None


def analyze_model(model: nn.Module, name: str, capability: Optional[dict[str, Any]] = None) -> CapabilityReport:
    cap = capability or DEFAULT_CAPABILITY
    core = cap["matrix_core"]
    report = CapabilityReport(model=name, capability=cap)
    tied = set()
    get_out = getattr(model, "get_output_embeddings", None)
    get_in = getattr(model, "get_input_embeddings", None)
    if callable(get_out) and callable(get_in):
        out_emb, in_emb = get_out(), get_in()
        if out_emb is not None and in_emb is not None and out_emb.weight is in_emb.weight:
            tied.add(id(out_emb))
    for module_name, module in model.named_modules():
        if not module_name:
            continue
        weight = _dense_weight(module)
        type_name = type(module).__name__
        if weight is not None:
            out_features, in_features = int(weight.shape[0]), int(weight.shape[1])
            tiles = math.ceil(out_features / core["n"]) * math.ceil(in_features / core["k"])
            decision = LayerDecision(
                name=module_name,
                module_type=type_name,
                role=_role(module_name),
                target="awen-gemm",
                reason=(
                    f"static real weight [{out_features}x{in_features}] tiles onto "
                    f"{tiles} {core['n']}x{core['k']} matrix-core blocks; inputs are "
                    f"quantized to the profile's {cap.get('effective_bits', 8)} effective bits"
                ),
                shape=[out_features, in_features],
                parameters=out_features * in_features + (0 if getattr(module, "bias", None) is None else out_features),
                tiles=tiles,
                macs_per_token=out_features * in_features,
            )
            if not weight.dtype.is_floating_point or weight.is_complex():
                decision.target = "host"
                decision.reason = f"weight dtype {weight.dtype} is not a supported real floating type"
            if id(module) in tied:
                decision.reason += "; weight is tied to the input embedding (shared storage)"
            report.layers.append(decision)
            continue
        for key, reason in _CPU_REASONS.items():
            if key.lower() in type_name.lower() and not list(module.children()):
                parameters = sum(p.numel() for p in module.parameters(recurse=False))
                report.layers.append(
                    LayerDecision(
                        name=module_name,
                        module_type=type_name,
                        role=key.lower(),
                        target="host",
                        reason=reason,
                        parameters=parameters,
                    )
                )
                break
    return report


def _auto_classes():
    from transformers import AutoModelForCausalLM, AutoTokenizer

    return AutoModelForCausalLM, AutoTokenizer


def load_causal_lm(model_id: str, revision: Optional[str] = None, dtype: torch.dtype = torch.float32):
    """Load a causal LM and tokenizer without remote code or pickle."""
    AutoModelForCausalLM, AutoTokenizer = _auto_classes()
    tokenizer = AutoTokenizer.from_pretrained(model_id, revision=revision, trust_remote_code=False)
    model = AutoModelForCausalLM.from_pretrained(
        model_id,
        revision=revision,
        trust_remote_code=False,
        use_safetensors=True,
        dtype=dtype,
    )
    model.eval()
    return model, tokenizer


@torch.no_grad()
def generate_text(model, tokenizer, prompt: str, max_new_tokens: int = 32) -> str:
    inputs = tokenizer(prompt, return_tensors="pt")
    output = model.generate(
        **inputs,
        max_new_tokens=max_new_tokens,
        do_sample=False,
        pad_token_id=tokenizer.pad_token_id or tokenizer.eos_token_id,
    )
    return tokenizer.decode(output[0], skip_special_tokens=True)


def format_report(report: CapabilityReport) -> str:
    lines = [f"AWEN capability report for {report.model} (profile {report.capability['backend_id']})"]
    s = report.summary
    lines.append(
        f"  {s['awen_gemm_layers']} dense layers could lower to AWEN GEMM "
        f"({s['awen_gemm_parameters']:,} params, {s['matrix_core_tiles']} matrix-core tiles, "
        f"{s['awen_gemm_mac_fraction_of_dense_layers']:.1%} of dense-layer MACs); "
        f"{s['host_layers']} layers stay on the host"
    )
    lines.append(f"  note: {report.attention_score_note}")
    for layer in report.layers:
        lines.append(f"  [{layer.target:9}] {layer.name} ({layer.module_type}): {layer.reason}")
    return "\n".join(lines)


def main(argv: Optional[list[str]] = None) -> None:
    parser = argparse.ArgumentParser(description="Load an open causal LM and report AWEN GEMM lowering")
    parser.add_argument("model_id")
    parser.add_argument("--revision")
    parser.add_argument("--prompt", default="Light travels")
    parser.add_argument("--max-new-tokens", type=int, default=32)
    parser.add_argument("--json", help="write the full report as JSON to this path")
    args = parser.parse_args(argv)
    model, tokenizer = load_causal_lm(args.model_id, args.revision)
    print("generation:", generate_text(model, tokenizer, args.prompt, args.max_new_tokens))
    report = analyze_model(model, args.model_id)
    print(format_report(report))
    if args.json:
        with open(args.json, "w", encoding="utf-8") as handle:
            json.dump(report.to_dict(), handle, indent=2)


if __name__ == "__main__":
    main()
