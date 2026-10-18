"""Generate text with the bundled AWEN-Mini checkpoint.

    python awen-models/examples/generate.py --prompt "ROMEO:" --tokens 300
    python awen-models/examples/generate.py --int8
"""

import argparse
import time

from awen import Runtime
from awen_models.quant import int8_weight_bytes, quantize_model_int8


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--prompt", default="ROMEO:")
    parser.add_argument("--tokens", type=int, default=300)
    parser.add_argument("--temperature", type=float, default=0.8)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--device", default="auto")
    parser.add_argument("--int8", action="store_true", help="per-channel int8 weights (CPU)")
    args = parser.parse_args()

    runtime = Runtime(device="cpu" if args.int8 else args.device)
    model = runtime.load_model("awen-mini")
    if args.int8:
        model.model = quantize_model_int8(model.model)
        print(f"int8 linear weights: {int8_weight_bytes(model.model)} bytes")
    started = time.perf_counter()
    text = model.generate(args.prompt, max_new_tokens=args.tokens, temperature=args.temperature, seed=args.seed)
    elapsed = time.perf_counter() - started
    print(text)
    print(f"\n[{model.execution.device}, {model.execution.fidelity}] {args.tokens} tokens in {elapsed:.2f} s")


if __name__ == "__main__":
    main()
