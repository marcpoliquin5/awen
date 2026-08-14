# AWEN Ecosystem

Community framework integrations, kernels, physical-design references,
marketplace metadata, and plugin templates live here. Implementations must keep
the compiler boundary versioned and must not turn AWEN into a layout, PDK,
foundry, circuit-solver, or electromagnetic-solver project.

## Physical-design reference

`pdks/example_silicon_pdk.json` is a synthetic open
`awen.physical-design.v1` binding. It demonstrates:

- an immutable example silicon PDK manifest and process-corner identity;
- a referenced gdsfactory component library;
- named, unit-bearing optical ports and a logical MZI topology;
- scalar layout constraints without polygons or GDS;
- a referenced Circulax model with public example parameters;
- typed gdsfactory and circuit-simulator adapter contracts; and
- immutable passed connectivity evidence.

The file contains no proprietary PDK data, foundry rules, mask geometry,
licensed model, credentials, or raw solver output. Its digests are deterministic
identities for the synthetic reference records; they are not foundry-qualified
signoff claims.

The corresponding exported compiler request is
`../awen-spec/fixtures/physical_design_mapping_request.v1.json`. It carries
logical operations, required ports, explicit units, mapping constraints, and a
candidate topology. The conformance suite round-trips the response without
losing ports, units, connections, or circuit-model parameters and rejects
identity tampering or constraint drift.

## Adapter responsibilities

A gdsfactory adapter owns PDK activation, cells, cross sections, layers, layer
stacks, ports, connectivity, geometry, routing, GDS/OASIS, DRC/LVS integration,
and foundry workflows. It returns only the closed AWEN logical metadata,
identities, and verification evidence.

A Circulax adapter may compile kfnetlist/SAX/recursive netlists and use JAX for
DC, transient, S-parameter, harmonic-balance, parameter-sweep, differentiable
calibration, or hardware-aware optimization. It returns model/result identities,
public parameters, settings fingerprints, and evidence. Equations, netlists,
JAX programs, gradients, solver state, and raw results remain external.

Electromagnetic adapters follow the same evidence boundary. Their meshes,
fields, licensed material models, convergence histories, and raw outputs remain
external.

Proprietary integrations must use `proprietary_reference`: no URI, process
parameter, component setting, model parameter, topology internals, geometry, or
foundry data may enter a public AWEN artifact.

See AEP-0021, `../awen-spec/specs/physical-design-boundary.md`, and
`../awen-spec/specs/plugin-contracts.md`.

## Python integration

`python_awen` provides the in-process NumPy runtime, PyTorch compiler backend,
portable JAX/StableHLO integration, profiling/replay contracts, and a legacy CLI
client. See `python_awen/README.md` for installation and examples.
