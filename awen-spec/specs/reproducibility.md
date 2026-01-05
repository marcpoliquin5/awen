# AWEN Artifact & Reproducibility Model v0.2

**Status:** Canonical specification for AWEN reproducibility system  
**Version:** v0.2  
**Last Updated:** 2026-01-05

## Overview

This document defines the complete artifact and reproducibility infrastructure for AWEN, enabling publication-grade provenance and deterministic replay of photonic computations. Every AWEN run produces a **hermetically sealed artifact bundle** containing everything needed to reproduce the computation exactly, cite it in papers, and audit results years later.

### Design Principles

1. **Hermetic bundles:** Self-contained artifacts with zero external dependencies
2. **Deterministic IDs:** Content-addressable identifiers for stable citations
3. **Complete provenance:** Capture every input that affects output (IR, params, seeds, calibration, environment)
4. **Publication-ready:** Citation format compatible with academic standards
5. **Audit trail:** Immutable records suitable for regulatory compliance
6. **Time-machine replay:** Bit-exact reproduction on demand

---

## Architecture

### Artifact Bundle Structure

```
awen_artifact_<deterministic_id>/
├── manifest.json                    # Bundle metadata and content index
├── ir/
│   ├── original.json                # Original IR graph
│   ├── lowered.json                 # Post-compilation IR
│   └── validation.json              # IR validation results
├── parameters/
│   ├── initial.json                 # Initial parameter values
│   └── final.json                   # Final parameter values (if optimization)
├── calibration/
│   ├── state_v0.json                # Initial calibration state
│   ├── state_vN.json                # Final calibration state
│   ├── drift_log.jsonl              # Drift detection events
│   └── recalibration_log.jsonl      # Recalibration operations
├── environment/
│   ├── runtime.json                 # Runtime version, build info, features
│   ├── system.json                  # OS, arch, CPU, memory
│   ├── device.json                  # Device capabilities, limits, firmware
│   └── dependencies.json            # Library versions, plugin versions
├── execution/
│   ├── schedule.json                # Execution schedule with timing
│   ├── traces.jsonl                 # Observability spans (from AEP-0005)
│   ├── timeline.json                # Timeline visualization data
│   ├── metrics.json                 # Performance metrics
│   ├── events.jsonl                 # Runtime events
│   └── observability_metadata.json  # Observability conformance info
├── results/
│   ├── outputs.json                 # Computation outputs
│   ├── measurements.json            # Quantum measurement outcomes
│   ├── quantum_states.json          # Quantum state evolution
│   └── gradients.json               # Gradients (if gradient run)
├── provenance/
│   ├── deterministic_id.txt         # Content-addressable artifact ID
│   ├── creation_timestamp.txt       # ISO8601 creation time
│   ├── creator.json                 # User/org/machine info
│   ├── parent_artifacts.json        # Lineage (if derived from other runs)
│   └── citation.txt                 # Ready-to-paste citation text
└── checksums.json                   # SHA256 checksums of all files
```

### Bundle Lifecycle

```
┌─────────────────────────────────────────────────┐
│ 1. EXECUTION                                    │
│   Engine runs IR, emits data to bundle builder │
└─────────────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────────────┐
│ 2. CAPTURE                                      │
│   Bundle builder collects all inputs/outputs   │
│   - IR snapshot                                 │
│   - Parameters                                  │
│   - Calibration state                           │
│   - Environment                                 │
│   - Execution traces                            │
│   - Results                                     │
└─────────────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────────────┐
│ 3. SEAL                                         │
│   Compute deterministic ID from content hash   │
│   Generate checksums for all files             │
│   Write manifest with provenance               │
└─────────────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────────────┐
│ 4. EXPORT                                       │
│   Bundle → tar.gz or local directory           │
│   Upload to cloud storage (optional)           │
│   Register in artifact registry (optional)     │
└─────────────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────────────┐
│ 5. REPLAY (deterministic)                       │
│   Import bundle                                 │
│   Validate checksums                            │
│   Restore environment constraints               │
│   Re-execute IR with identical inputs           │
│   Verify bit-exact output match                 │
└─────────────────────────────────────────────────┘
```

---

## Manifest Schema

### manifest.json

```json
{
  "schema_version": "awen_artifact.v0.2",
  "artifact_id": "awen_0123456789abcdef",
  "artifact_type": "run",
  "created_at": "2026-01-05T10:23:45.123456Z",
  "awen_runtime_version": "0.5.0",
  "conformance_level": "full",
  "determinism_guarantee": "bit-exact",
  "contents": {
    "ir": ["original.json", "lowered.json", "validation.json"],
    "parameters": ["initial.json", "final.json"],
    "calibration": ["state_v0.json", "drift_log.jsonl"],
    "environment": ["runtime.json", "system.json", "device.json"],
    "execution": ["traces.jsonl", "timeline.json", "metrics.json", "events.jsonl"],
    "results": ["outputs.json", "measurements.json"],
    "provenance": ["deterministic_id.txt", "citation.txt"]
  },
  "inputs": {
    "ir_hash": "sha256:abc123...",
    "parameters_hash": "sha256:def456...",
    "calibration_hash": "sha256:ghi789...",
    "seed": 42
  },
  "outputs": {
    "results_hash": "sha256:jkl012...",
    "success": true,
    "error": null
  },
  "provenance": {
    "creator": {
      "user": "researcher@university.edu",
      "organization": "Quantum Photonics Lab",
      "machine": "lab-server-03"
    },
    "parent_artifacts": ["awen_fedcba9876543210"],
    "tags": ["experiment_2026_01", "mzi_mesh_optimization"],
    "notes": "Initial MZI mesh characterization run"
  }
}
```

### Artifact Types

- **`run`**: Standard execution producing outputs
- **`gradient`**: Gradient computation for optimization
- **`calibration`**: Standalone calibration operation
- **`replay`**: Reproduction of existing artifact
- **`validation`**: Conformance test or benchmark

---

## Deterministic ID Generation

### Algorithm

```
deterministic_id = sha256(
    ir_canonical_json ||
    parameters_sorted_json ||
    calibration_state_json ||
    seed_bytes ||
    runtime_version ||
    device_capabilities_sorted
)
```

### Properties

- **Content-addressable:** Same inputs → same ID
- **Collision-resistant:** SHA256 provides 2^256 namespace
- **URL-safe:** Hex encoding (lowercase alphanumeric)
- **Human-readable prefix:** `awen_` for namespace clarity

### Example

```
awen_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
```

Shortened for citations (first 16 hex chars):
```
awen_0123456789abcdef
```

---

## Citation Format

### Academic Citation (IEEE)

```
[Author], "[Title]," AWEN Artifact awen_0123456789abcdef, [Organization], [Year]. 
Available: https://artifacts.awen.dev/awen_0123456789abcdef
```

### Example

```
J. Smith and A. Doe, "Optimization of 8×8 MZI Mesh for Quantum Sampling," 
AWEN Artifact awen_7a3f9c2e81d6b054, Quantum Photonics Lab, 2026. 
Available: https://artifacts.awen.dev/awen_7a3f9c2e81d6b054
```

### BibTeX

```bibtex
@misc{awen_7a3f9c2e81d6b054,
  author = {Smith, Jane and Doe, Alex},
  title = {Optimization of 8×8 MZI Mesh for Quantum Sampling},
  howpublished = {AWEN Artifact},
  note = {awen\_7a3f9c2e81d6b054},
  year = {2026},
  url = {https://artifacts.awen.dev/awen_7a3f9c2e81d6b054}
}
```

### citation.txt (Auto-generated)

```
AWEN Artifact: awen_7a3f9c2e81d6b054
Title: Optimization of 8×8 MZI Mesh for Quantum Sampling
Authors: Jane Smith, Alex Doe
Organization: Quantum Photonics Lab
Created: 2026-01-05T10:23:45.123456Z
Runtime: awen-runtime v0.5.0

Citation (IEEE):
J. Smith and A. Doe, "Optimization of 8×8 MZI Mesh for Quantum Sampling," 
AWEN Artifact awen_7a3f9c2e81d6b054, Quantum Photonics Lab, 2026. 
Available: https://artifacts.awen.dev/awen_7a3f9c2e81d6b054

BibTeX:
@misc{awen_7a3f9c2e81d6b054,
  author = {Smith, Jane and Doe, Alex},
  title = {Optimization of 8×8 MZI Mesh for Quantum Sampling},
  howpublished = {AWEN Artifact},
  note = {awen\_7a3f9c2e81d6b054},
  year = {2026},
  url = {https://artifacts.awen.dev/awen_7a3f9c2e81d6b054}
}

Reproducibility Command:
awenctl replay --artifact awen_7a3f9c2e81d6b054 --verify
```

---

## Deterministic Replay Contract

### Replay Guarantees

**Bit-exact replay** is guaranteed when:

1. ✅ **Same runtime version:** Exact `awen-runtime` version match
2. ✅ **Same IR:** Identical IR graph structure and parameters
3. ✅ **Same calibration state:** Calibration state restored from bundle
4. ✅ **Same seed:** Deterministic random seed specified
5. ✅ **Compatible device:** Device meets capability constraints from original run

**Probabilistic replay** (statistically equivalent) when:

- Different device with same capabilities
- Stochastic operations with seed re-sampling
- Measurement outcomes re-sampled from same distributions

### Replay Process

```bash
# Extract artifact bundle
awenctl artifact extract awen_0123456789abcdef.tar.gz

# Validate bundle integrity
awenctl artifact validate awen_0123456789abcdef/
# Checks:
# - All checksums match
# - Manifest schema valid
# - Required files present
# - Runtime version compatible

# Replay execution
awenctl replay --artifact awen_0123456789abcdef/ --verify
# Process:
# 1. Load IR from bundle
# 2. Restore parameters
# 3. Restore calibration state
# 4. Apply seed
# 5. Re-execute
# 6. Compare outputs bit-exact

# Output:
# ✓ Replay successful
# ✓ Outputs match (bit-exact)
# ✓ New artifact: awen_fedcba9876543210 (replay)
#   Parent: awen_0123456789abcdef (original)
```

### Replay Failure Modes

| Failure | Cause | Resolution |
|---------|-------|------------|
| Checksum mismatch | Corrupted bundle | Re-download original |
| Version mismatch | Different runtime | Use Docker container with pinned version |
| Device incompatible | Missing capabilities | Use simulator or compatible device |
| Non-deterministic op | Uncontrolled randomness | Report bug (violation of contract) |

---

## Environment Capture

### runtime.json

```json
{
  "runtime_name": "awen-runtime",
  "runtime_version": "0.5.0",
  "build_timestamp": "2026-01-01T00:00:00Z",
  "build_profile": "release",
  "rust_version": "1.75.0",
  "features": ["observability", "gradients", "quantum"],
  "plugins": [
    {"name": "reference_sim", "version": "0.5.0"},
    {"name": "perceval_adapter", "version": "0.1.0"}
  ]
}
```

### system.json

```json
{
  "os": "Linux",
  "os_version": "6.5.0-ubuntu",
  "arch": "x86_64",
  "cpu_model": "Intel Xeon E5-2680 v4",
  "cpu_cores": 28,
  "memory_gb": 128,
  "hostname": "lab-server-03"
}
```

### device.json

```json
{
  "device_type": "simulated",
  "device_id": "sim_reference_0",
  "capabilities": {
    "channels": 64,
    "max_frequency_hz": 1e15,
    "wavelength_range_nm": [1530, 1570],
    "phase_resolution_rad": 0.001,
    "power_range_dbm": [-30, 10]
  },
  "firmware_version": null,
  "calibration_date": null
}
```

---

## Provenance Tracking

### Lineage Graph

Artifacts form a DAG (directed acyclic graph) of provenance:

```
awen_abc123 (initial run)
    │
    ├──> awen_def456 (optimization epoch 1)
    │       │
    │       └──> awen_ghi789 (optimization epoch 2)
    │
    └──> awen_jkl012 (replay verification)
```

### parent_artifacts.json

```json
[
  {
    "artifact_id": "awen_abc123",
    "relationship": "derived_from",
    "description": "Initial characterization run"
  },
  {
    "artifact_id": "awen_xyz999",
    "relationship": "calibration_source",
    "description": "Calibration state imported from prior run"
  }
]
```

### Provenance Queries

```bash
# Find all descendants of an artifact
awenctl artifact lineage awen_abc123 --direction descendants

# Find all ancestors
awenctl artifact lineage awen_ghi789 --direction ancestors

# Verify integrity of entire lineage
awenctl artifact verify-lineage awen_ghi789 --recursive
```

---

## Export & Import

### Export Formats

1. **Directory:** Uncompressed folder (development/inspection)
2. **tar.gz:** Compressed archive (sharing/archival)
3. **Cloud object:** S3/GCS/Azure blob (large-scale storage)

### Export Commands

```bash
# Export to directory (default)
awenctl run --ir example.json --export-artifact /tmp/my-run

# Export to tar.gz
awenctl run --ir example.json --export-artifact my-run.tar.gz

# Export to cloud (requires awen-cloud)
awenctl run --ir example.json --export-artifact s3://my-bucket/artifacts/
```

### Import Commands

```bash
# Import from directory
awenctl artifact import /path/to/awen_abc123/

# Import from tar.gz
awenctl artifact import my-run.tar.gz

# Import from cloud
awenctl artifact import s3://my-bucket/artifacts/awen_abc123.tar.gz

# Verify after import
awenctl artifact validate awen_abc123/
```

---

## Conformance Levels

### Level 1: Basic (v0.2 Requirement)

- [x] Artifact bundle structure with manifest
- [x] Deterministic ID generation
- [x] IR, parameters, results captured
- [x] Checksum validation
- [x] Citation format generation
- [x] Export/import to directory and tar.gz
- [x] Integration tests validate replay

### Level 2: Complete (v0.3 Target)

- [ ] Cloud storage backends (S3, GCS, Azure)
- [ ] Artifact registry with search/discovery
- [ ] Automatic versioning and tagging
- [ ] Advanced lineage visualization
- [ ] Compliance reporting (GxP, FDA 21 CFR Part 11)
- [ ] Multi-artifact comparisons

---

## Security & Compliance

### Integrity Protection

- **Checksums:** SHA256 for all files
- **Manifest signing:** PGP signatures (optional, v0.3)
- **Tamper detection:** Checksum mismatch = abort

### Access Control (Cloud only, v0.3)

- Public artifacts: Open access for citations
- Private artifacts: Organization/user ACLs
- Restricted artifacts: Compliance-mode auditing

### Audit Trail

Every artifact records:
- Who created it (user, org, machine)
- When (ISO8601 timestamp)
- Why (tags, notes, parent artifacts)
- How (runtime version, device, environment)

---

## Performance Considerations

### Bundle Size

Typical artifact sizes:
- **Small run** (simple IR, no quantum): ~1-10 MB
- **Medium run** (complex IR, observability): ~10-100 MB
- **Large run** (quantum states, full history): ~100 MB - 1 GB

### Optimization Strategies

- Compress with gzip (-9 for archival)
- Deduplicate common files (IR, calibration) via content-addressable storage
- Stream large files (quantum states) instead of in-memory buffering

### Scalability

- Local storage: Tested to 10,000 artifacts (~10 GB)
- Cloud storage: Unlimited with proper sharding
- Artifact registry: PostgreSQL backend supports millions

---

## Versioning

**Current Version:** `awen_artifact.v0.2`  
**Schema Evolution:** Backward-compatible additions only (new optional fields)  
**Breaking Changes:** Require major version bump and migration tools

---

## References

- AEP-0006: Reproducibility & Artifacts (this spec implements AEP-0006)
- AEP-0005: Observability (artifact bundles include observability data)
- FAIR Principles: https://www.go-fair.org/fair-principles/
- NIH Reproducibility: https://www.nih.gov/research-training/rigor-reproducibility
- FDA 21 CFR Part 11: https://www.fda.gov/regulatory-information/search-fda-guidance-documents/part-11-electronic-records-electronic-signatures-scope-and-application