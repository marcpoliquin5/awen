"""AWEN-Mini: a small decoder-only transformer language model.

Every dense projection is an :class:`AwenLinear`. By default it is exactly
``torch.nn.functional.linear``; a runtime may attach a ``matmul_executor`` to
route the GEMM elsewhere (for example the AWEN photonic simulator) while the
rest of the model stays on the host.
"""

from __future__ import annotations

import math
from typing import Callable, Iterator, Optional

import torch
import torch.nn as nn
import torch.nn.functional as F

from .config import MiniConfig

MatmulExecutor = Callable[[torch.Tensor, torch.Tensor], torch.Tensor]


class AwenLinear(nn.Linear):
    """``nn.Linear`` with an optional external GEMM executor.

    The executor receives ``(x2d, weight)`` with ``x2d`` of shape ``[rows, in]``
    and ``weight`` of shape ``[out, in]`` and must return ``x2d @ weight.T``.
    Bias addition stays on the host.
    """

    matmul_executor: Optional[MatmulExecutor] = None

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        executor = self.matmul_executor
        if executor is None:
            return F.linear(x, self.weight, self.bias)
        leading = x.shape[:-1]
        y = executor(x.reshape(-1, x.shape[-1]), self.weight)
        y = y.reshape(*leading, self.out_features).to(x.dtype)
        if self.bias is not None:
            y = y + self.bias
        return y


class LowRankLinear(nn.Module):
    """``W ≈ U V`` with ``V: in→rank`` and ``U: rank→out``."""

    def __init__(self, in_features: int, out_features: int, rank: int, bias: bool):
        super().__init__()
        self.down = AwenLinear(in_features, rank, bias=False)
        self.up = AwenLinear(rank, out_features, bias=bias)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.up(self.down(x))


class FeedForward(nn.Module):
    def __init__(self, config: MiniConfig):
        super().__init__()
        d, h, b = config.n_embd, config.ffn_hidden, config.bias
        self.kind = config.ffn
        if config.ffn == "gelu":
            self.fc = AwenLinear(d, h, bias=b)
            self.proj = AwenLinear(h, d, bias=b)
        elif config.ffn == "gated":
            self.gate = AwenLinear(d, h, bias=b)
            self.value = AwenLinear(d, h, bias=b)
            self.proj = AwenLinear(h, d, bias=b)
        else:
            self.fc = LowRankLinear(d, h, config.ffn_rank, bias=b)
            self.proj = LowRankLinear(h, d, config.ffn_rank, bias=b)
        self.dropout = nn.Dropout(config.dropout)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        if self.kind == "gated":
            hidden = F.silu(self.gate(x)) * self.value(x)
        else:
            hidden = F.gelu(self.fc(x))
        return self.dropout(self.proj(hidden))


class CausalSelfAttention(nn.Module):
    def __init__(self, config: MiniConfig):
        super().__init__()
        self.n_head = config.n_head
        self.qkv = AwenLinear(config.n_embd, 3 * config.n_embd, bias=config.bias)
        self.proj = AwenLinear(config.n_embd, config.n_embd, bias=config.bias)
        self.dropout = config.dropout

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        batch, time, channels = x.shape
        q, k, v = self.qkv(x).split(channels, dim=2)
        shape = (batch, time, self.n_head, channels // self.n_head)
        q, k, v = (t.view(shape).transpose(1, 2) for t in (q, k, v))
        y = F.scaled_dot_product_attention(
            q, k, v, is_causal=True, dropout_p=self.dropout if self.training else 0.0
        )
        return self.proj(y.transpose(1, 2).contiguous().view(batch, time, channels))


class Block(nn.Module):
    def __init__(self, config: MiniConfig):
        super().__init__()
        self.ln1 = nn.LayerNorm(config.n_embd)
        self.attn = CausalSelfAttention(config)
        self.ln2 = nn.LayerNorm(config.n_embd)
        self.ffn = FeedForward(config)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = x + self.attn(self.ln1(x))
        return x + self.ffn(self.ln2(x))


class AwenMini(nn.Module):
    """Decoder-only transformer with learned positions and a tied output head."""

    def __init__(self, config: MiniConfig):
        super().__init__()
        self.config = config
        self.tok_emb = nn.Embedding(config.vocab_size, config.n_embd)
        self.pos_emb = nn.Embedding(config.block_size, config.n_embd)
        self.drop = nn.Dropout(config.dropout)
        count = 1 if config.shared_layers else config.n_layer
        self.blocks = nn.ModuleList(Block(config) for _ in range(count))
        self.ln_f = nn.LayerNorm(config.n_embd)
        self.apply(self._init_weights)

    @staticmethod
    def _init_weights(module: nn.Module) -> None:
        if isinstance(module, nn.Linear):
            nn.init.normal_(module.weight, mean=0.0, std=0.02)
            if module.bias is not None:
                nn.init.zeros_(module.bias)
        elif isinstance(module, nn.Embedding):
            nn.init.normal_(module.weight, mean=0.0, std=0.02)

    def num_parameters(self) -> int:
        return sum(p.numel() for p in self.parameters())

    def linear_layers(self) -> Iterator[tuple[str, AwenLinear]]:
        for name, module in self.named_modules():
            if isinstance(module, AwenLinear):
                yield name, module

    def _layer_sequence(self) -> Iterator[Block]:
        if self.config.shared_layers:
            for _ in range(self.config.n_layer):
                yield self.blocks[0]
        else:
            yield from self.blocks

    def forward(
        self, idx: torch.Tensor, targets: Optional[torch.Tensor] = None
    ) -> tuple[torch.Tensor, Optional[torch.Tensor]]:
        _, time = idx.shape
        if time > self.config.block_size:
            raise ValueError(f"sequence length {time} exceeds block_size {self.config.block_size}")
        positions = torch.arange(time, device=idx.device)
        x = self.drop(self.tok_emb(idx) + self.pos_emb(positions))
        for block in self._layer_sequence():
            x = block(x)
        x = self.ln_f(x)
        logits = x @ self.tok_emb.weight.T
        loss = None
        if targets is not None:
            loss = F.cross_entropy(logits.reshape(-1, logits.size(-1)), targets.reshape(-1))
        return logits, loss

    @torch.no_grad()
    def generate(
        self,
        idx: torch.Tensor,
        max_new_tokens: int,
        temperature: float = 1.0,
        top_k: Optional[int] = None,
        generator: Optional[torch.Generator] = None,
    ) -> torch.Tensor:
        """Autoregressively sample ``max_new_tokens`` tokens.

        ``temperature == 0`` selects greedy decoding.
        """
        was_training = self.training
        self.eval()
        try:
            for _ in range(max_new_tokens):
                window = idx[:, -self.config.block_size :]
                logits, _ = self(window)
                logits = logits[:, -1, :]
                if temperature == 0:
                    next_id = logits.argmax(dim=-1, keepdim=True)
                else:
                    logits = logits / temperature
                    if top_k is not None:
                        kth = torch.topk(logits, min(top_k, logits.size(-1))).values[:, [-1]]
                        logits = logits.masked_fill(logits < kth, -math.inf)
                    probs = F.softmax(logits, dim=-1)
                    next_id = torch.multinomial(probs, 1, generator=generator)
                idx = torch.cat([idx, next_id], dim=1)
        finally:
            self.train(was_training)
        return idx
