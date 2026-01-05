# Quantum Coherence & State Memory Model (v0.1)

This spec defines the quantum photonic state space, coherence window semantics, and state evolution model for AWEN. It provides the formal foundation for hybrid classical-quantum execution.

## Photonic State Space

### Classical Mode (Deterministic)
Represents a classical optical mode with amplitude and phase.

```json
{
  "mode_type": "classical",
  "amplitude": 1.0,
  "phase": 0.0
}
```

### Quantum Mode (Probabilistic)
Represents a quantum optical mode using Fock (photon number) basis. State is a probability amplitude over photon numbers.

```json
{
  "mode_type": "quantum_fock",
  "photon_numbers": [0, 1, 2],
  "amplitudes": [0.707, 0.5, 0.2],
  "phases": [0.0, 1.57, 3.14]
}
```

### Mixed State
Represents a density matrix state (coherence loss, entanglement with environment).

```json
{
  "mode_type": "mixed",
  "dims": [2, 2],
  "density_matrix_real": [...],
  "density_matrix_imag": [...]
}
```

## Coherence Window

Defines the temporal validity of a quantum state before decoherence invalidates computation.

```json
{
  "id": "coh_window_123",
  "start_ns": 1000,
  "end_ns": 2000,
  "decoherence_timescale_ns": 500,
  "cross_mode_decoherence_ns": 750,
  "idle_time_budget_ns": 200,
  "notes": "2-qubit gate window, photon lifetime ≈ 500 ns"
}
```

### Constraints
- If `current_time > end_ns`, state is invalid
- If `idle_time_used > idle_time_budget_ns`, coherence is lost
- Cross-mode decoherence limits multi-mode entanglement window

## Measurement

Measurement collapses a quantum state to a definite outcome and returns a classical result.

```json
{
  "outcome_index": 0,
  "photon_count": 2,
  "probability": 0.5,
  "collapsed_state": {...},
  "seed_used": 12345
}
```

Measurement is:
- **Destructive** (default): state collapses, quantum information is lost
- **Non-destructive** (heralded): post-selection on measurement outcome, state may be retained

## State Evolution

Unitary evolution of quantum states via photonic gates.

```json
{
  "gate": "BS",
  "params": {"theta": 1.57},
  "affected_modes": [0, 1],
  "duration_ns": 10
}
```

Standard gates:
- `BS` (beamsplitter): parametric, 2-mode
- `PS` (phase shifter): parametric, 1-mode
- `SQUEEZING` (parametric amplification): 1-mode
- `PD` (parametric down-conversion): 1→2 mode

## Measurement-Conditioned Feedback

Control flow that branches on quantum measurement outcomes.

```json
{
  "condition": "measurement_outcome:detector_0 == 1",
  "true_branch": {...},
  "false_branch": {...}
}
```

This enables:
- Adaptive algorithms (e.g., measurement-based cluster state computation)
- Shot-based feedback loops
- Error correction via syndrome measurement

## Reproducibility

All quantum state evolution and measurement outcomes are deterministically seeded via RNG.

```json
{
  "quantum_seed": 0xDEADBEEF,
  "evolution_trace": [
    {"gate": "BS", "state_before": {...}, "state_after": {...}},
    {"measurement": "det_0", "outcome": 1, "probability": 0.5}
  ]
}
```

## Schemas (JSON)

### QuantumState v0.1
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "AWEN QuantumState v0.1",
  "type": "object",
  "required": ["id", "modes", "coherence_window"],
  "properties": {
    "id": {"type": "string"},
    "modes": {
      "type": "array",
      "items": {"type": "object"}
    },
    "coherence_window": {"type": "object"},
    "provenance": {"type": "object"}
  }
}
```

### CoherenceWindow v0.1
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "AWEN CoherenceWindow v0.1",
  "type": "object",
  "required": ["id", "start_ns", "end_ns"],
  "properties": {
    "id": {"type": "string"},
    "start_ns": {"type": "integer"},
    "end_ns": {"type": "integer"},
    "decoherence_timescale_ns": {"type": "number"},
    "idle_time_budget_ns": {"type": "integer"}
  }
}
```

## Versioning

This spec is `quantum-coherence.v0.1`. Future versions (0.2+) will add support for:
- Continuous measurement and homodyne detection
- Advanced noise models (1/f noise, dephasing channels)
- Entanglement swapping and purification protocols
- Measurement-based cluster state computation

All changes follow AEP revision process.
