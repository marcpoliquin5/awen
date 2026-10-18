"""``awen-lab``: command line for the simulation-only Lab Agent.

    awen-lab --store .awen-lab quick experiment.json
    awen-lab --store .awen-lab truth experiment.json
    awen-lab --store .awen-lab repro main
    awen-lab --store .awen-lab compare a.json b.json
    awen-lab --store .awen-lab branch tuning main
    awen-lab --store .awen-lab diff main tuning
    awen-lab --store .awen-lab log main
    awen-lab --store .awen-lab publish main --out bundles --approver "Name" --reason "why"
    awen-lab --store .awen-lab hardware "turn on laser"     # always refused, exit code 3

Example experiment.json:
    {"kind": "photonic_gemm", "target": "photonic-sim", "shape_mkn": [64, 128, 32],
     "contract": {"photons_per_mac": 1000}, "seeds": [1, 2, 3]}
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Optional, Sequence

from .agent import LabAgent
from .policy import PolicyError


def _load(path: str) -> dict:
    return json.loads(Path(path).read_text())


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--store", default=".awen-lab")
    parser.add_argument("--branch", default="main")
    sub = parser.add_subparsers(dest="cmd", required=True)
    for name in ("quick", "truth"):
        sub.add_parser(name).add_argument("experiment")
    sub.add_parser("repro").add_argument("ref")
    c = sub.add_parser("compare")
    c.add_argument("a")
    c.add_argument("b")
    b = sub.add_parser("branch")
    b.add_argument("name")
    b.add_argument("ref")
    d = sub.add_parser("diff")
    d.add_argument("a")
    d.add_argument("b")
    sub.add_parser("log").add_argument("ref")
    sub.add_parser("show").add_argument("ref")
    p = sub.add_parser("publish")
    p.add_argument("ref")
    p.add_argument("--out", required=True)
    p.add_argument("--approver", default="")
    p.add_argument("--reason", default="")
    sub.add_parser("hardware").add_argument("description")
    args = parser.parse_args(argv)

    agent = LabAgent(args.store, branch=args.branch)
    try:
        if args.cmd == "quick":
            out = agent.quick(_load(args.experiment)).hash
        elif args.cmd == "truth":
            state = agent.truth(_load(args.experiment))
            out = json.dumps({"hash": state.hash, **state.metrics}, indent=2)
        elif args.cmd == "repro":
            state = agent.repro(args.ref)
            out = json.dumps({"hash": state.hash, "reproduced": state.metrics["reproduced"]})
        elif args.cmd == "compare":
            state = agent.compare(_load(args.a), _load(args.b))
            m = state.metrics
            out = json.dumps({"hash": state.hash, "metric": m["metric"], "a": m["a"], "b": m["b"],
                              "paired_delta": m["paired_delta"]}, indent=2)
        elif args.cmd == "branch":
            out = agent.branch_from(args.name, args.ref)
        elif args.cmd == "diff":
            out = json.dumps(agent.diff(args.a, args.b), indent=2)
        elif args.cmd == "log":
            out = "\n".join(f"{h} {agent.restore(h).workflow}" for h in agent.store.history(args.ref))
        elif args.cmd == "show":
            out = json.dumps(agent.restore(args.ref).to_dict(), indent=2)
        elif args.cmd == "publish":
            digest = agent.store.resolve(args.ref)
            if args.approver or args.reason:
                agent.policy.approve(digest, args.approver, args.reason)
            out = str(agent.publish(digest, args.out))
        else:
            agent.hardware(args.description)
            return 3
    except PolicyError as error:
        print(f"refused: {error}", file=sys.stderr)
        return 3
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
