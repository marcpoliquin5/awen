"""Discrete-event interconnect fabric simulator (label: modeled).

Model
-----
* **Nodes** are endpoints (accelerators) or switches. **Links** are directed,
  FIFO, store-and-forward servers with a bandwidth, a propagation latency, an
  energy per bit, an optical loss and an optional failure time.
* **Switches** add a per-hop latency. An *optical circuit switch* additionally
  pays ``reconfig_delay_s`` whenever the set of circuits it must hold changes
  between collective phases (no per-packet buffering is modeled for it).
* **Contention** emerges from links serving messages one at a time in arrival
  order, so queueing delay is simulated, not assumed.
* **Loss/power:** a link whose received power ``tx_power_dbm - loss_db`` falls
  below ``rx_sensitivity_dbm`` is unusable. Energy = sum(bits x energy/bit) +
  static power x makespan.
* **Failures:** a link with ``fail_at_s`` stops accepting messages from that
  time; later messages are re-routed on the surviving graph (shortest path by
  hop count) or counted as dropped when no route exists.

All parameters are inputs. Nothing in this module is a measured property of
a real fabric; reported numbers are modeled under the stated parameters.
"""

from __future__ import annotations

import heapq
import itertools
import math
from collections import defaultdict, deque
from dataclasses import dataclass, field

import numpy as np


@dataclass
class Link:
    src: str
    dst: str
    bandwidth_bps: float
    latency_s: float = 0.0
    energy_j_per_bit: float = 0.0
    loss_db: float = 0.0
    fail_at_s: float = math.inf
    busy_until: float = 0.0
    bits: float = 0.0
    busy_s: float = 0.0


@dataclass
class FabricConfig:
    """Parameter set for one fabric technology (all values are user inputs)."""

    name: str
    link_bandwidth_bps: float
    link_latency_s: float
    switch_latency_s: float
    energy_j_per_bit: float
    static_power_w: float = 0.0
    reconfig_delay_s: float = 0.0
    circuit_switched: bool = False
    loss_db: float = 0.0
    tx_power_dbm: float = 0.0
    rx_sensitivity_dbm: float = -math.inf
    provenance: str = "assumed parameter"


@dataclass
class Message:
    src: str
    dst: str
    bits: float
    t_inject: float
    tag: object = None
    path: list = field(default_factory=list)
    hop: int = 0
    t_done: float = math.nan
    dropped: bool = False


class Fabric:
    def __init__(self, cfg: FabricConfig):
        self.cfg = cfg
        self.links: dict[tuple[str, str], Link] = {}
        self.switches: set[str] = set()
        self.endpoints: list[str] = []
        self.reconfigurations = 0
        self.dropped = 0

    # -- topology -----------------------------------------------------------
    def add_link(self, a: str, b: str, bidirectional: bool = True, **kw) -> None:
        c = self.cfg
        params = dict(bandwidth_bps=c.link_bandwidth_bps, latency_s=c.link_latency_s,
                      energy_j_per_bit=c.energy_j_per_bit, loss_db=c.loss_db)
        params.update(kw)
        self.links[(a, b)] = Link(a, b, **params)
        if bidirectional:
            self.links[(b, a)] = Link(b, a, **params)

    def usable(self, link: Link, t: float) -> bool:
        if t >= link.fail_at_s:
            return False
        return self.cfg.tx_power_dbm - link.loss_db >= self.cfg.rx_sensitivity_dbm

    def route(self, src: str, dst: str, t: float) -> list[tuple[str, str]] | None:
        """Shortest path (hops) over links usable at time t (BFS, deterministic order)."""
        adj = defaultdict(list)
        for (a, b), lk in sorted(self.links.items()):
            if self.usable(lk, t):
                adj[a].append(b)
        prev = {src: None}
        q = deque([src])
        while q:
            u = q.popleft()
            if u == dst:
                break
            for v in adj[u]:
                if v not in prev:
                    prev[v] = u
                    q.append(v)
        if dst not in prev:
            return None
        path, v = [], dst
        while prev[v] is not None:
            path.append((prev[v], v))
            v = prev[v]
        return path[::-1]

    # -- simulation ---------------------------------------------------------
    def run(self, messages: list[Message]) -> dict:
        """Simulate messages to completion. Returns modeled statistics."""
        seq = itertools.count()
        heap = [(m.t_inject, next(seq), m) for m in messages]
        heapq.heapify(heap)
        while heap:
            t, _, m = heapq.heappop(heap)
            if m.hop == 0 or m.hop >= len(m.path) or not self.usable(self.links[m.path[m.hop]], t):
                here = m.src if m.hop == 0 else m.path[m.hop - 1][1]
                if here == m.dst and m.hop > 0:
                    m.t_done = t
                    continue
                r = self.route(here, m.dst, t)
                if r is None:
                    m.dropped = True
                    self.dropped += 1
                    continue
                m.path = m.path[:m.hop] + r
            lk = self.links[m.path[m.hop]]
            start = max(t, lk.busy_until)
            tx = m.bits / lk.bandwidth_bps
            lk.busy_until = start + tx
            lk.bits += m.bits
            lk.busy_s += tx
            arrive = start + tx + lk.latency_s
            if lk.dst in self.switches:
                arrive += self.cfg.switch_latency_s
            m.hop += 1
            if m.hop == len(m.path):
                m.t_done = arrive
            else:
                heapq.heappush(heap, (arrive, next(seq), m))
        done = [m for m in messages if not m.dropped]
        makespan = max((m.t_done for m in done), default=0.0) - min((m.t_inject for m in messages), default=0.0)
        bits_moved = sum(lk.bits for lk in self.links.values())
        dyn = sum(lk.bits * lk.energy_j_per_bit for lk in self.links.values())
        return {"label": "modeled", "fabric": self.cfg.name, "messages": len(messages), "dropped": self.dropped,
                "makespan_s": makespan, "mean_latency_s": float(np.mean([m.t_done - m.t_inject for m in done]))
                if done else math.nan, "link_bits": bits_moved, "dynamic_energy_j": dyn,
                "static_energy_j": self.cfg.static_power_w * makespan,
                "energy_j": dyn + self.cfg.static_power_w * makespan}


# -- topologies --------------------------------------------------------------
def ring(cfg: FabricConfig, n: int) -> Fabric:
    f = Fabric(cfg)
    f.endpoints = [f"n{i}" for i in range(n)]
    for i in range(n):
        f.add_link(f"n{i}", f"n{(i + 1) % n}", bidirectional=True)
    return f


def star(cfg: FabricConfig, n: int) -> Fabric:
    """n endpoints each with one bidirectional link to a central switch."""
    f = Fabric(cfg)
    f.endpoints = [f"n{i}" for i in range(n)]
    f.switches.add("sw")
    for e in f.endpoints:
        f.add_link(e, "sw")
    return f


# -- collectives -------------------------------------------------------------
def _phased(f: Fabric, phases: list[list[tuple[str, str, float]]]) -> dict:
    """Run collective phases back to back; each phase starts when the previous finishes.

    For a circuit-switched fabric, every phase whose (src,dst) pattern differs
    from the previous one pays ``reconfig_delay_s`` before it starts.
    """
    t = 0.0
    prev_pattern = None
    total = {"dynamic_energy_j": 0.0}
    msgs_all = []
    for ph in phases:
        pattern = frozenset((s, d) for s, d, _ in ph)
        if f.cfg.circuit_switched:
            srcs = [s for s, _, _ in ph]
            dsts = [d for _, d, _ in ph]
            if len(set(srcs)) != len(srcs) or len(set(dsts)) != len(dsts):
                raise ValueError("a circuit switch with one port per node can only hold a permutation per phase")
        if f.cfg.circuit_switched and pattern != prev_pattern:
            t += f.cfg.reconfig_delay_s
            f.reconfigurations += 1
        prev_pattern = pattern
        msgs = [Message(s, d, b, t) for s, d, b in ph]
        r = f.run(msgs)
        msgs_all += msgs
        done = [m.t_done for m in msgs if not m.dropped]
        t = max(done) if done else t
        total["dynamic_energy_j"] = r["dynamic_energy_j"]
    dyn = total["dynamic_energy_j"]
    return {"label": "modeled", "fabric": f.cfg.name, "time_s": t, "phases": len(phases),
            "reconfigurations": f.reconfigurations, "dropped": f.dropped,
            "dynamic_energy_j": dyn, "static_energy_j": f.cfg.static_power_w * t,
            "energy_j": dyn + f.cfg.static_power_w * t}


def ring_allreduce(f: Fabric, data_bits: float) -> dict:
    """Bandwidth-optimal ring all-reduce: 2(n-1) phases, each node sends data/n to its successor."""
    n = len(f.endpoints)
    chunk = data_bits / n
    phase = [(f.endpoints[i], f.endpoints[(i + 1) % n], chunk) for i in range(n)]
    return _phased(f, [phase] * (2 * (n - 1)))


def ring_allreduce_analytic(n: int, data_bits: float, bw: float, alpha: float) -> float:
    """Classic alpha-beta cost of ring all-reduce: 2(n-1) (alpha + data/(n*bw))."""
    return 2 * (n - 1) * (alpha + data_bits / (n * bw))


def all_to_all(f: Fabric, data_bits_per_node: float, schedule: str = "shift") -> dict:
    """All-to-all personalized exchange; ``shift`` uses n-1 phases (i -> i+k), ``direct`` one phase."""
    n = len(f.endpoints)
    chunk = data_bits_per_node / n
    E = f.endpoints
    if schedule == "direct":
        return _phased(f, [[(E[i], E[j], chunk) for i in range(n) for j in range(n) if i != j]])
    return _phased(f, [[(E[i], E[(i + k) % n], chunk) for i in range(n)] for k in range(1, n)])


# -- queueing ----------------------------------------------------------------
def mm1_experiment(lam: float, mu: float, n: int = 20000, seed: int = 0) -> dict:
    """Poisson arrivals (rate lam), exponential service (rate mu) on one link; compare with 1/(mu-lam)."""
    rng = np.random.default_rng(seed)
    cfg = FabricConfig("mm1", link_bandwidth_bps=1.0, link_latency_s=0.0, switch_latency_s=0.0, energy_j_per_bit=0.0)
    f = Fabric(cfg)
    f.add_link("a", "b", bidirectional=False)
    t = np.cumsum(rng.exponential(1 / lam, n))
    sizes = rng.exponential(1 / mu, n)  # bits at 1 bit/s -> service time
    msgs = [Message("a", "b", float(s), float(ti)) for s, ti in zip(sizes, t)]
    f.run(msgs)
    soj = np.array([m.t_done - m.t_inject for m in msgs])
    soj = soj[n // 10:]  # drop warm-up
    return {"label": "modeled", "lambda": lam, "mu": mu, "simulated_mean_sojourn": float(soj.mean()),
            "analytic_mean_sojourn": 1 / (mu - lam), "n": n, "seed": seed}


def compare(configs: list[FabricConfig], n: int, data_bits: float) -> list[dict]:
    """Run identical workloads (ring all-reduce, shift all-to-all) on a star of each config."""
    rows = []
    for cfg in configs:
        rows.append({"workload": "ring_allreduce", "n": n, "data_bits": data_bits, **ring_allreduce(star(cfg, n), data_bits)})
        rows.append({"workload": "all_to_all_shift", "n": n, "data_bits": data_bits, **all_to_all(star(cfg, n), data_bits)})
    return rows


def study(n: int = 8) -> dict:
    """Optical circuit switch vs electrical packet switch on identical workloads (modeled).

    Both use the same assumed link bandwidth and propagation latency so that
    only switching behavior differs. The electrical switch has a per-hop
    latency; the optical circuit switch has none but pays a reconfiguration
    delay per new circuit pattern. Results are swept over that delay and the
    message size. All parameters are illustrative assumptions, not
    literature values; energy is not compared because no cited per-bit
    energies are bundled.
    """
    assumptions = {"link_bandwidth_bps": 400e9, "link_latency_s": 25e-9, "eps_switch_latency_s": 500e-9,
                   "provenance": "assumed parameter (illustrative; not literature, not measured)"}
    rows = []
    for data in (1e6, 1e8, 1e10):
        for reconf in (1e-6, 1e-5, 1e-4, 1e-3):
            eps = FabricConfig("electrical-packet", assumptions["link_bandwidth_bps"], assumptions["link_latency_s"],
                               assumptions["eps_switch_latency_s"], 0.0)
            ocs = FabricConfig("optical-circuit", assumptions["link_bandwidth_bps"], assumptions["link_latency_s"],
                               0.0, 0.0, reconfig_delay_s=reconf, circuit_switched=True)
            for r in compare([eps, ocs], n, data):
                rows.append({"reconfig_delay_s": reconf, **r})
    return {"label": "modeled; assumed parameters", "n_endpoints": n, "assumptions": assumptions, "rows": rows}


def main(argv=None):
    import argparse
    import json
    ap = argparse.ArgumentParser(description="Optical vs electrical fabric study (modeled).")
    ap.add_argument("--n", type=int, default=8)
    ap.add_argument("--out", default=None)
    a = ap.parse_args(argv)
    txt = json.dumps(study(a.n), indent=2)
    if a.out:
        open(a.out, "w").write(txt + "\n")
    print(txt)


if __name__ == "__main__":
    main()
