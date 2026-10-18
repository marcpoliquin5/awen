# Changelog

## Unreleased

- Release automation (X2): the manual `Verified release` workflow now builds
  real artifacts with `scripts/build_release_artifacts.sh` (the `awenctl`
  Linux x86_64 binary, built twice to record bit-for-bit reproducibility;
  the `awen_native` wheel via maturin; wheels for every Python package, with
  AWEN-Mini inside `awen_models` and the built frontend inside `awen_studio`;
  the AWEN-Mini checkpoint tarball; CycloneDX SBOMs; the source archive;
  `BUILD-INFO.json`; `SHA256SUMS`), verifies them in a fresh virtual
  environment (`scripts/verify_release_artifacts.py`), attaches Sigstore
  build-provenance attestations, and creates a draft release whose notes
  state plainly that everything runs on CPUs and nothing is a hardware
  measurement. `-rc.N` tags become pre-releases (closes #78).
- Added AWEN Studio (`awen-studio`, S1): a local web app (FastAPI backend +
  React/Vite frontend, blue/black interference design) wired to the real
  backends: projects, AWEN-Mini generation on CPU or the photonic simulator,
  Rust compiler placement/capability/diagnostics, the operator table, noise
  contract validation with analytic noise, photonic-sim vs CPU GEMM and
  likelihood comparison, Lab Agent experiment history, and Markdown/JSON
  report export. Localhost only; not hosted. 13 API tests and an HTTP
  end-to-end test run in CI after a frontend build (closes #75).
- Upgraded the runtime's `jsonschema` from 0.16 to 0.59 (closes #79). A new
  `awen_runtime::schema` module wraps it for every caller: formats are
  asserted under every draft (0.16 behaviour), `$ref`s resolve only against
  documents passed in (offline; nothing is fetched), and errors come back as
  a flat message list. 0.59 resolves references eagerly, so the plugin-manifest
  conformance test now registers the capability and calibration schemas it
  transitively references.
- Security (X1b): trust-pinned Ed25519 plugin signing (`TrustedKeys`,
  `sign_manifest`, `discover_trusted`; `awenctl plugin keygen|sign|verify|discover`)
  so self-signed manifests from unpinned keys are rejected; `cargo deny`
  license/source policy (`deny.toml`); CycloneDX SBOMs for the three Rust
  crates and the CI Python environment (`scripts/generate_sbom.py`, uploaded
  per CI run); an AST-based safe-model-loading check (no remote code, no
  pickle, `torch.load` only with `weights_only=True`); `docs/REPRODUCIBLE_BUILDS.md`
  and `docs/COMPATIBILITY.md` (refs #78).
- Added `docs/CLOUD_ARCHITECTURE.md`: a design-only architecture for a
  possible hosted service, mapped component by component onto existing
  modules and schemas, with what exists and what does not. No service exists
  (closes #116).
- Lab Agent (S2): new `awen-lab` package and CLI. It orchestrates Quick,
  Truth, Repro, Compare and Publish workflows over the existing simulators
  (the Rust photonic GEMM model and the quantum-photonic references). Each
  run produces an immutable, content-addressed lab state with provenance,
  and states support branch, restore, history and diff. Operating bounds
  cap every simulation. Publish writes a local, self-verifying bundle only
  after a recorded approval. Every hardware action is refused, with or
  without approval (refs #76).
- Research (R1–R3): `research/ledger/` adds the research ledger schema
  (`awen.research-ledger.v1`), an importer for CSV and Markdown tables
  (unchecked rows land as `citation pending`) and 15 seed directions, each
  with a citation checked against Crossref. The 72-paper historical ledger is
  not available and is not reconstructed. `research/frontier-physics/` adds
  hypotheses H1 (Casimir), H2 (toroidal/fractal optics) and H3 (information
  capacity), each with falsification conditions and runnable checks of the
  established results they build on. `docs/RESEARCH_PIPELINE.md` describes
  the research-to-code path. Kept separate from the core; no free-energy
  claims (refs #77).
- Benchmarks (B1): `awen-bench` compares CPU, CUDA (skipped with a reason
  when no GPU is visible), the AWEN software reference, the photonic
  simulator and a hybrid placement on GEMM and AWEN-Mini inference, with
  p50/p95/p99 latency, throughput, memory, compile time, time to first token,
  tokens/s and perplexity, bootstrap 95% CIs over multiple runs, and a
  write-once, self-hashed evidence record (`awen.benchmark-evidence.v1`,
  git SHA, hardware, environment). One measured run on the build host's CPU
  is committed; see `docs/BENCHMARKS.md` (refs #74).
- Added the efficiency program (`awen-efficiency`): low-rank, tensor-train,
  untied head, MoE top-1, 50% magnitude pruning, int4 QAT vs PTQ,
  distillation from AWEN-Mini and recursive reuse, all at a matched
  818k-parameter budget with two seeds on CPU, plus a Pareto plot of quality
  vs parameters, latency, memory and MACs (energy only estimated). Negative
  results (low-rank, sparsity, untied head) are reported (refs #72).
- Added `awen_systems.fabric`, a discrete-event interconnect simulator (links,
  electrical packet and optical circuit switches, contention, reconfiguration
  delay, loss budget, power, failures with rerouting, ring all-reduce and
  all-to-all) checked against M/M/1 and the alpha-beta all-reduce formula,
  plus an optical-vs-electrical study on identical workloads under stated
  assumed parameters (`awen-fabric-study`; modeled, refs #71).
- Added `awen-systems` with a memory-technology parameter schema
  (`awen.memory_technologies.v1`: phase-change photonic, microring fading,
  optical RRAM, delay line, HBM2 baseline). Every value carries a provenance;
  only values checked against a cited source are bundled, the rest must be
  supplied. Includes a behavioral memory simulator and a KV-cache workload
  study (traffic, energy, attention fidelity), all labeled modeled (refs #70).
- Added the model conversion utility `awen-convert` (`awen_models.convert`):
  `to-onnx` exports an AWEN-Mini checkpoint to ONNX and verifies it against
  PyTorch with `onnx.reference.ReferenceEvaluator` (max logit difference
  1.0e-05 on the bundled checkpoint at sequence length 32), with the ONNX
  compatibility analysis in the report; `to-program` turns constant-weight
  MatMul/Gemm nodes into an `awen.tensor.v1` program that `awenctl compile`
  plans, listing activation products and Gemm epilogues as electronic
  (refs #66).
- Added the vendor-neutral hardware description schema
  (`awen.hardware-description.v1`) for MZI meshes, microring banks, WDM,
  diffractive layers, detectors, ADCs, DACs, lasers, reservoirs, memory and
  quantum-photonic resources, with mandatory provenance
  (`reference_simulator`, `vendor_datasheet` or `measured`); a
  reference-simulator fixture covering every class; reference simulator
  drivers on the existing HAL `Device` trait (Clements MZI mesh, all-pass
  microring bank, detector + ADC) checked against closed-form physics; and
  `docs/VENDOR_INTEGRATION.md`. No vendor drivers are included (refs #67).
- C SDK: `awen-runtime/include/awen/awen.h` adds the awenBLAS kernel ABI
  (`awen.kernel-c.v1`) next to the existing dense GEMM ABI: kernel kind
  listing, interleaved complex GEMM (`awen_cgemm_f64`) and
  `awen_kernel_execute_json`, which runs any `awen.blas.v1` request on the
  CPU reference or the photonic noise-model simulator and returns the result
  JSON with size-query semantics. A C11 example
  (`examples/awen_c_example.c`) is compiled with `-Werror -pedantic` and run
  in CI (refs #66).
- awenBLAS now has 25 kernel kinds: `batched_complex_gemm`,
  `scaled_dot_product_attention` (two GEMMs plus an electronic softmax) and
  `feed_forward` (`W2 gelu(W1 x + b1) + b2`, tanh GELU). New
  `awen-compiler/tests/awenblas_oracles.rs` checks complex, batched and
  batched-complex GEMM, FFT, low-rank GEMM, the reservoir update, attention
  and the feed-forward block against independently written oracles
  (real block embedding of complex GEMM, naive DFT with Parseval and inverse
  round trip, explicit U·Vᵀ reconstruction, the op library's attention and
  FFN), plus simulator error versus injected noise. Reference and simulator
  only (refs #65).
- `awenctl caps` explores capability documents (device capabilities, plugin
  manifests, hardware descriptions, awenBLAS backend profiles) as text or
  JSON; `awenctl profile` times one awenBLAS request on the CPU reference or
  the simulator and writes a Chrome/Perfetto trace. Profile times are host
  wall-clock, not device timings (refs #66).
- `awen_runtime::devrt`: health-aware placement over simulated photonic
  devices (healthy / degraded / failed) that remaps past unavailable devices
  and falls back to the exact CPU reference, with the reason recorded;
  deterministic mode (SplitMix64 seed stream from one root seed); experiment
  records (`awen.experiment-record.v1`) holding SHA-256 digests of each
  request and of the output bits, with a bit-exact replay check; and a host
  wall-clock profiler with per-kernel summaries and Chrome trace export.
  Profiler times are host timings of the simulator or CPU, not device
  timings (refs #64).
- Added `awen_runtime::devrt`, runtime services layered on the existing HAL
  and scheduler: ordered asynchronous streams (one worker thread each) with
  events for cross-stream and cross-device dependencies and sticky error
  propagation; memory residency tracking with per-device capacity, LRU
  eviction, pinning and transfer byte accounting; and device leases
  (exclusive/shared, logical-clock expiry) with operation, energy and wall-time
  budgets. Budgets enforce caller-reported or simulator-estimated usage; they
  are not hardware measurements (refs #64).
- Added `awen_py.stablehlo_exec`: runs the StableHLO that JAX lowers
  (`dot_general` on the f64 reference or the photonic noise model,
  simulated; a few shape and elementwise ops electronically) and refuses
  other operations by name. Checked against JAX CPU. This is not yet a PJRT
  plugin; `docs/PJRT_BOUNDARY.md` records what remains (#100, refs #61).
- Added the quantum-photonic resource model (`awen_qphotonic.resources`) and
  a small quantum-kernel experiment (`awen_qphotonic.kernel`,
  `awen-qkernel`) compared with RBF and linear baselines on the same
  splits. Committed results (simulation, three seeds): the photonic kernel
  scores below RBF on both toy datasets; no advantage is claimed (#99,
  closes #69).
- Added Gaussian states to `awen-qphotonic` (`gaussian`, covariance
  formalism with hbar = 2): squeezing, two-mode squeezing, passive
  interferometers, displacement, pure loss, thermal states, purity,
  symplectic eigenvalues, vacuum probability, homodyne sampling and
  conditioning. Checked against closed forms and a truncated-Fock-space
  squeeze operator. Simulation only (#98, refs #69).
- Added `awen-qphotonic`, the quantum-photonic reference simulators, kept
  separate from the classical photonic domain (AEP-0020): permanents by
  Ryser's formula, Fock-space linear optics, exact boson sampling for small
  n, the Hong-Ou-Mandel dip with partial distinguishability, and photon
  counting with detection efficiency, uniform loss and dark counts. Checked
  against brute-force permutation sums, explicit creation-operator
  expansion, closed-form HOM coincidence and an environment-mode loss
  oracle. Simulation only (#97, refs #69).
- `awen-physics`: χ⁽²⁾ second-harmonic generation by integrating the
  coupled-wave equations (`shg`), checked against the undepleted sinc²,
  depleted tanh² and first-order quasi-phase-matching closed forms; and a
  1-D frequency-domain Helmholtz solver with adjoint permittivity gradients
  (`fdfd_adjoint`), checked against central finite differences and the
  transfer-matrix reflectance (#96, refs #68).
- `awen-physics`: 1-D Yee FDTD reflectance spectra (`fdtd1d`, full-wave 1-D)
  converging at second order to the transfer-matrix result, and a 2-D
  scalar paraxial Crank–Nicolson BPM with imaginary-distance mode solving
  (`bpm`), checked against the Gaussian-beam width and the analytic slab
  mode (#95, refs #68).
- `awen-physics`: planar multilayer optics by transfer matrices and an
  independent Redheffer S-matrix composition (`tmm`), and microring
  all-pass/add-drop transmission with explicit round-trip propagation, FSR
  and FWHM (`rings`). Checked against Fresnel, quarter-wave-mirror and
  Beer–Lambert closed forms (fidelity: analytic / numerical propagation;
  #94, refs #68).
- Added minimal ONNX import (`awen_models.onnx_import`, `awen-onnx-report`,
  `awen.onnx-report.v1`). It maps constant-weight MatMul/Gemm and 2-D Conv
  onto the AWEN `gemm`/`conv2d` operators, keeps activation-activation
  MatMul and elementwise/shape ops electronic, refuses unmapped ops, reports
  photonic MAC fraction (static analysis), and runs supported graphs on the
  f64 reference or the photonic noise model (simulated). Checked against
  `onnx.reference.ReferenceEvaluator` (#93, refs #63).
- Added the operator library (`awen-compiler::ops`, `awen.op-table.v1`,
  `awen-spec/specs/ops.md`): complex matmul, DFT, conv2d, 1-D correlation,
  low-rank linear, attention with projections, FFN, spectral convolution, RNN
  and reservoir steps, and a square-law detection layer. Each has a
  four-level fidelity label (math reference / photonic sim /
  hardware-lowerable / hardware-measured; none is measured), an f64
  reference checked against NumPy oracles, and a simulated path through the
  photonic noise model. Exposed as `awen_native.run_op` and
  `awen_native.op_table` (#92, refs #62).
- Added `awen_models.finetune`, covering LoRA, QLoRA (NF4, CUDA only) and
  knowledge distillation into AWEN-Mini, with committed CPU smoke results on
  SmolLM2-135M. Added `awen_models.compute_plan` and `docs/COMPUTE_PLAN.md`,
  which estimate memory and FLOPs for Qwen3-0.6B/1.7B and SmolLM2 on a
  12 GB GPU. Parameter counts are checked against instantiated models (#91).
- Added the Virtual Photonic ISA (VPISA v1, AEP-0024, `awen-spec/specs/isa.md`).
  It has 24 typed instructions across classical, quantum and interop domains,
  a JSON schema, a deterministic binary codec (`AWENISA\0`), a validator with
  typed errors, version negotiation and target checks, a GEMM lowering, and a
  reference interpreter that runs photonic instructions on the noise model
  (simulated). `awen-compiler` now parses JSON floats with exact round-trip
  (#89, closes #55).
- Added AWEN-Photon noise-injection training (`awen_models.photon_training`).
  An MNIST MLP is trained standard and noise-aware, then evaluated with its
  GEMMs on the Rust photonic noise model (simulated). It writes
  accuracy-vs-ENOB and accuracy-vs-modeled-optical-energy curves and commits
  results for two seeds (#88, refs #73).
- Added `awen_py.device`, a PyTorch PrivateUse1 device named `awen`. It runs
  `mm`, `addmm` and `bmm` (so `matmul` and `nn.Linear`) on the Rust photonic
  GEMM model, and `add`/`relu` as the electronic stage. Other ops raise a clear
  error. It is a simulated, inference-only device (#87).
- Added `awen-physics` with three validation notebooks that CI executes:
  slab-waveguide TE modes against the analytic dispersion relation and a
  finite-difference solver; Clements et al. (2016) MZI-mesh decomposition;
  and shot-noise-limited ENOB of the Rust GEMM model against IEEE 1241 theory
  (AEP-0023, #86).
- Added `awen-native`, a PyO3/maturin binding to the Rust compiler and the
  photonic GEMM model. `awen_py` operations that target `photonic`, including
  `torch.compile(backend=awen)` with `target="photonic"`, now run their matrix
  products in Rust instead of framework math labeled as simulated (#84).
- `Runtime(device="photonic-sim")` runs every AWEN-Mini transformer linear
  layer on the Rust photonic simulator. A committed demo compares one layer
  and the whole model's validation loss against the CPU oracle across optical
  power (simulated, compact model).
- Added `awen-models`: the AWEN-Mini character-level language model
  (1,016,320 parameters), trained on CPU on Tiny Shakespeare. It ships with a
  bundled safetensors checkpoint (validation loss 1.4795 nats, measured), int8
  inference, `Runtime(device="auto")`, an evaluation command, the
  AWEN-Nonlinear equal-parameter ablation with committed results, and a
  Hugging Face capability report (`trust_remote_code=False`) for SmolLM2-135M.
- `awenctl execute --inputs` runs AWENEXE GEMM dispatches on the photonic
  noise model and reports error and measured effective bits against an exact
  reference (#82).
- Added the physical-unit photonic noise contract
  (`awen.photonic-noise-contract.v1`) and a compact GEMM noise model covering
  shot, thermal, RIN, phase, drift, crosstalk, insertion loss and DAC/ADC
  terms (AEP-0022, #80).
- Added `docs/AUDIT.md` (implementation truth table at `020be48`),
  `docs/ROADMAP.md` (dependency-aware milestones with acceptance tests and
  hardware needs) and `docs/DIRECTIVE_TRACKER.md`; opened labeled issues
  #55–#79 for every missing module.
- Removed about 1,060 lines of never-compiled runtime code (`control_v0`, the
  old simulator module, a duplicate `awenctl`) and 65 tests that only exercised
  mocks; added real plugin-loader, HAL and `awenctl` tests. Runtime line
  coverage rose from 81.2% to 85.6% (#54).
- Updated Rust dependencies: `sha2` 0.11, `ed25519-dalek` 3.0, `serde_json`,
  `flate2`, `log`, `time`; `uuid` held at 1.24.0 because newer releases need
  Rust 1.89 (#53). Superseded Dependabot PRs were closed with explanations.
- Supported PyTorch 2.10 through 2.14 in `awen_py`; CI tests 2.10.0 and 2.14.1
  (#52, supersedes Dependabot #49).
- Pinned the Rust toolchain (1.99.1) in `rust-toolchain.toml`, declared and
  CI-checked a minimum supported Rust version of 1.88, added a weekly scheduled
  quality-gate run, Rust build caching, a prebuilt `cargo-audit`, and upgraded
  GitHub Actions off the deprecated Node 20 runtime.
- Added strict normalized equal-batch rank-three StableHLO `dot_general`
  lowering through typed MLIR dialects, deterministic AWENEXE emission, and
  direct Rust runtime preparation.
- Consolidated repository policy, security, CI, licensing, and release evidence.
- Removed obsolete phase sign-offs and nonfunctional product directories.

## Historical development snapshot

The tag `phase-2.5-gated-2026-01-05` is a historical source snapshot. It is not
a supported product release, does not establish hardware-performance evidence,
and predates the current verified-release policy.
