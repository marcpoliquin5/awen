# Reference Simulator v0.1: Realistic Photonic Simulation

**Version:** 0.1  
**Status:** Specification Draft  
**Scope:** Phase 2.4 (Reference Simulator Expansion)  
**Predecessors:** Phase 2.3 (HAL v0.2), Phase 1.4 (Device Model)  
**Constitutional Directive:** Full-scope, frontier-first, measurement-conditioned

---

## Executive Summary

The Reference Simulator provides a high-fidelity, feature-complete simulation of photonic quantum computers. It is the default implementation of `PhotonicBackend` from Phase 2.3 HAL v0.2, and serves three critical roles:

1. **Research Platform**: Enables algorithm development, testing, and validation without hardware
2. **Validation Tool**: Provides baseline behavior for real hardware backends
3. **Teaching Tool**: Reference implementation for understanding photonic control and measurement

This specification defines all noise models, measurement modes, calibration behaviors, and resource constraints that the simulator must enforce.

---

## 1. Core Simulation Model

### 1.1 Quantum State Representation

The simulator maintains a full density-matrix representation of the quantum state:

```
ρ = Σᵢ pᵢ |ψᵢ⟩⟨ψᵢ|
```

**Parameters:**
- **Photon Number Cutoff**: `max_photons` (default 3, configurable 1-8)
- **Mode Count**: `num_modes` (4-16 modes, configurable per device)
- **Precision**: IEEE 754 double precision complex numbers

**Memory Requirement:**
```
Memory = (2^(num_modes * max_photons))² × 16 bytes
Example: 4 modes, 3 photons = 4096² × 16 bytes = 256 MB
```

### 1.2 Unitary Evolution

Quantum gates evolve the state via unitary operations:

```
ρ(t+Δt) = U(Δt) ρ(t) U†(Δt)
```

**Supported Gates** (from Phase 2.1 Engine):
- Single-mode: Phase shift (φ), Squeezing (r, θ)
- Two-mode: Beam splitter (θ, φ), Linear interferometer
- Measurement: Homodyne, Heterodyne, Photon counting

**Gate Decomposition:**
- BS(θ, φ) = exp(-i θ (a†_in b_in e^{iφ} + a_in b†_in e^{-iφ}))
- PS(φ) = exp(-i φ a†a)
- SQ(r, θ) = exp((r/2)(e^{iθ} a² - e^{-iθ} a†²))

### 1.3 Time Evolution

The simulator supports three evolution modes:

| Mode | Algorithm | Use Case | Error |
|------|-----------|----------|-------|
| **Exact** | Full density matrix | Small circuits, validation | O(1) |
| **Gaussian** | Covariance matrix | Large circuits (linear optics) | O(ε) |
| **Approximate** | Variational ansatz | Hybrid classica/quantum | O(ε²) |

**Default**: Exact for ≤8 modes, Gaussian for >8 modes

---

## 2. Noise Models

### 2.1 Photon Loss (Dominant Noise)

**Physical Origin**: Absorption and scattering in waveguides, couplers, detectors

**Model**: Single-mode loss channel (Kraus operators)

```
L_loss(κ) = √(1-κ) ρ + κ tr(ρ) |0⟩⟨0|
```

**Parameters:**
- **Loss Rate** `κ` (default 0.01 per cm, 0.1-1% per mode)
- **Distance** (default 1 cm per mode, configurable)
- **Wavelength Dependence** (O(λ²) scattering)

**Implementation:**
```rust
fn apply_loss(state: &mut DensityMatrix, mode: usize, loss_rate: f64) {
    // Kraus operator: L = sqrt(1-κ) I + sqrt(κ) |0⟩⟨0| tr(state)
    let m0 = (1.0 - loss_rate).sqrt();
    let m1 = loss_rate.sqrt();
    state.scale(m0);
    state.add_thermal(m1 * state.trace());
}
```

### 2.2 Detector Dark Counts

**Physical Origin**: Thermal excitation in photodetectors

**Model**: Poisson arrival process

```
P(n_dark) = λ^n e^(-λ) / n!
```

**Parameters:**
- **Dark Count Rate** `λ` (default 1000 Hz, configurable 100-10000 Hz)
- **Integration Time** (measurement duration)
- **Detector Type** (APD: high dark count, PD: low)

**Implementation:**
```rust
fn inject_dark_counts(readout: &mut PhotonCounts, dark_rate: f64, time_window: f64) {
    let lambda = dark_rate * time_window;
    let dark_photons = sample_poisson(lambda);
    readout.add_noise(dark_photons);
}
```

### 2.3 Phase Noise

**Physical Origin**: Laser frequency/phase instability

**Model**: Random walk on phase space

```
φ(t) = φ(0) + ∫ dW_t    (Wiener process)
```

**Parameters:**
- **Linewidth** `Δν` (default 1 kHz, 100 Hz - 100 kHz)
- **Timescale** (correlated on ~1/Δν)

**Effect on Measurements:**
- Homodyne: Local oscillator phase jitter → variance increase
- Heterodyne: Frequency offset uncertainty → SNR degradation
- Phase gates: Systematic error proportional to linewidth × gate time

**Implementation:**
```rust
fn apply_phase_noise(phase: &mut f64, linewidth: f64, time_step: f64) {
    let diffusion = (linewidth * std::f64::consts::PI * time_step).sqrt();
    *phase += gaussian_random() * diffusion;
}
```

### 2.4 Thermal Noise (Homodyne Only)

**Physical Origin**: Thermal photons in local oscillator / shot noise

**Model**: Additive Gaussian noise on quadratures

```
I_meas = I_true + N(0, σ²)
Q_meas = Q_true + N(0, σ²)
```

**Parameters:**
- **Relative Intensity Noise (RIN)** (default 0.001 = -30 dB)
- **Shot Noise Floor** (vacuum + LO intensity)

**Variance Scaling:**
```
σ² = (1 + RIN × P_LO) × (ℏω / 2)
```

where P_LO is local oscillator power

**Implementation:**
```rust
fn add_shot_noise(quadrature: &mut f64, rin: f64, lo_power: f64) {
    let shot_var = (1.0 + rin * lo_power) * HBAR_OMEGA / 2.0;
    *quadrature += gaussian_random() * shot_var.sqrt();
}
```

### 2.5 Kerr Nonlinearity

**Physical Origin**: Self-phase modulation, cross-phase modulation in waveguides

**Model**: Intensity-dependent phase shift

```
H_Kerr = χ a†² a²    (Kerr self-phase modulation)
H_XPM = χ a†_1 a_1 a†_2 a_2    (Cross-phase modulation)
```

**Parameters:**
- **Kerr Coefficient** χ (default 0.1 rad/(photon·cm), configurable)
- **Distance** (propagation length)
- **Mode Number** (affects nonlinearity strength)

**Hamiltonian Evolution:**
```
ρ(t) = exp(-i χ n² t) ρ(0) exp(i χ n² t)
```

where n = a†a (photon number operator)

**Implementation:**
```rust
fn apply_kerr_evolution(state: &mut DensityMatrix, chi: f64, time: f64) {
    for photon_num in 0..max_photons {
        let phase_shift = chi * (photon_num as f64)^2 * time;
        // Apply diagonal phase shifts based on photon number
        state.apply_diagonal_phase(photon_num, phase_shift);
    }
}
```

**Frontier Research Impact**:
- Kerr effect enables quantum gates (CZ gates via cross-phase modulation)
- Measurement-conditioned feedback can correct Kerr-induced errors
- Adaptive calibration must account for Kerr nonlinearity

### 2.6 Thermal Environment Coupling

**Physical Origin**: Thermal photons from environment, temperature fluctuations

**Model**: Thermal bath at temperature T

**Parameters:**
- **Temperature** T_env (default 300 K)
- **Bath Coupling** γ (thermal photon injection rate)
- **Thermal Photon Number** n_th = 1/(e^(ℏω/k_B T) - 1)

**Lindblad Operator** (thermalization channel):
```
L_thermal = √(γ n_th) a†  + √(γ(n_th+1)) a
```

**At room temperature (300 K)**:
```
For λ = 1550 nm:  n_th ≈ 10^(-30)   (negligible)
For λ = 10 µm:    n_th ≈ 10^(-3)    (small)
```

Thermal noise is negligible for infrared photonics at room temperature.

---

## 3. Measurement Mode Implementation

### 3.1 Homodyne Measurement

**Physical Operation**: Beam splitter + two photodiodes (I and Q quadratures)

**Ideal Measurement**:
```
I = ⟨a + a†⟩
Q = ⟨-i(a - a†)⟩
```

**Realistic Model** (with noise):
1. Apply phase noise to LO
2. Detect with shot noise
3. Add thermal noise (RIN)
4. ADC quantization (if specified)

**Variance** (per measurement):
```
Var(I) = 1/2 + shot_noise + RIN_noise
```

**Implementation**:
```rust
fn homodyne_measurement(state: &DensityMatrix, mode: usize, config: &HomodyneConfig) 
    -> HomodyneResult 
{
    let lo_phase = sample_phase_noise(config.lo_linewidth);
    let ideal_i = state.quadrature(mode, 0) * lo_phase.cos();
    let ideal_q = state.quadrature(mode, PI/2) * lo_phase.sin();
    
    let measured_i = ideal_i + sample_shot_noise(config.lo_power);
    let measured_q = ideal_q + sample_shot_noise(config.lo_power);
    
    HomodyneResult {
        quadrature_i: measured_i,
        quadrature_q: measured_q,
        variance_i_sq: config.rin + SHOT_NOISE,
        timestamp: get_timestamp(),
    }
}
```

### 3.2 Heterodyne Measurement

**Physical Operation**: Beam splitter with frequency-shifted LO + single photodiode

**Ideal Detection**:
```
I_het = ⟨(a + a†) cos(ω_IF t)⟩
Q_het = ⟨(a + a†) sin(ω_IF t)⟩
```

where ω_IF is the intermediate frequency

**Frequency Jitter Effect**:
- LO linewidth → uncertainty in detected frequency
- Phase noise → SNR degradation proportional to (Δν × measurement_time)²

**Implementation**:
```rust
fn heterodyne_measurement(state: &DensityMatrix, mode: usize, config: &HeterodyneConfig) 
    -> HeterodyneResult 
{
    let lo_freq = config.lo_freq + sample_frequency_noise(config.lo_linewidth);
    let if_freq = lo_freq - config.signal_freq;
    
    // Downconvert to IF band and measure
    let ideal_i = state.quadrature(mode, 0) * if_freq.cos();
    let ideal_q = state.quadrature(mode, PI/2) * if_freq.sin();
    
    let snr_degradation = (config.lo_linewidth * config.measurement_time).powi(2);
    let measured_mag = (ideal_i.powi(2) + ideal_q.powi(2)).sqrt() / (1.0 + snr_degradation);
    let measured_phase = ideal_q.atan2(ideal_i);
    
    HeterodyneResult {
        magnitude: measured_mag,
        phase: measured_phase,
        snr: estimate_snr(measured_mag, snr_degradation),
        timestamp: get_timestamp(),
    }
}
```

### 3.3 Photon Counting (Direct Detection)

**Physical Operation**: Single photodiode detecting individual photons

**Photon Counting Statistics**:
```
P(n | ρ) = ⟨Π_n | ρ | Π_n⟩    where Π_n = |n⟩⟨n|
```

**Dark Count Injection**:
```
Total = signal_photons + dark_photons
```

**Efficiency**:
- Quantum efficiency η (default 0.95, 0.8-0.99)
- Dark count rate λ_dark (default 1000 Hz, 100-10000 Hz)

**Implementation**:
```rust
fn photon_counting_measurement(state: &DensityMatrix, mode: usize, config: &DirectDetectionConfig) 
    -> DirectDetectionResult 
{
    // Sample photon number from state
    let signal_photons = state.sample_photon_number(mode);
    
    // Apply quantum efficiency
    let detected = if random() < config.quantum_efficiency {
        signal_photons
    } else {
        0  // Missed detection
    };
    
    // Add dark counts
    let dark = poisson_sample(config.dark_count_rate * config.integration_time);
    let total = detected + dark;
    
    DirectDetectionResult {
        photon_count: total,
        click_probability: 1.0 - (-0.5).exp(),  // Coherent state
        dark_count_subtracted: total - dark,
        timestamp: get_timestamp(),
    }
}
```

---

## 4. Calibration Model

### 4.1 Phase Calibration in Simulator

**Phase Gate Errors**:
- Systematic error: Kerr + phase noise accumulation
- Random error: Phase noise jitter

**Calibration Strategy**:
```
φ_applied = φ_desired / (1 + f_Kerr(n_photons) + δφ_noise)
```

**Implementation**:
```rust
fn calibrate_phase_gate(desired_phase: f64, calib_state: &CalibrationState) 
    -> f64 
{
    let kerr_correction = 1.0 / (1.0 + calib_state.kerr_coefficient 
        * calib_state.avg_photon_number);
    let noise_correction = (1.0 - calib_state.phase_noise_var / 2.0);
    
    desired_phase * kerr_correction * noise_correction
}
```

### 4.2 Detector Calibration in Simulator

**Dark Count Compensation**:
```
Signal = Measured - Dark_baseline
```

**Efficiency Calibration**:
```
True_photons = Detected / quantum_efficiency
```

**Implementation**:
```rust
fn calibrate_detector(measured: u32, calib_state: &CalibrationState) 
    -> u32 
{
    let signal = (measured as f64 - calib_state.dark_count_rate 
        * calib_state.integration_time) as i32;
    let true_count = (signal as f64 / calib_state.quantum_efficiency) as u32;
    true_count.max(0)
}
```

### 4.3 Drift Simulation

**Phase Drift** (from thermal expansion, laser aging):
```
δφ(t) = Δφ_rate × t
```

**Dark Count Drift**:
```
λ_dark(t) = λ_dark(0) × (1 + temperature_coeff × ΔT(t))
```

**Calibration Lifetime**:
- Phase calibration valid for ~30 minutes (300 μrad drift)
- Dark count calibration valid for ~1 hour (10% drift)

---

## 5. Resource Constraints

### 5.1 Computation Cost

**Gate Time**:
| Operation | Latency | Notes |
|-----------|---------|-------|
| Phase shift | 1 ns | Deterministic |
| Beam splitter | 2 ns | Phase shifters + mode routing |
| Measurement | 100 ns | Detector + electronics |
| Readout | 1-10 ms | ADC conversion + network |

**Circuit Depth Overhead**:
- Heisenberg scaling: O(N) depth for N-mode interferometer
- Measurement overhead: +1 operation per measured mode

### 5.2 Memory Scaling

```
Simulator Memory = O(4^num_modes × max_photon^num_modes)
```

| Config | Memory | Realism |
|--------|--------|---------|
| 4 modes, 3 photons | 256 MB | Full quantum (realistic) |
| 6 modes, 3 photons | 4 GB | Full quantum (large) |
| 8 modes, 3 photons | 64 GB | Full quantum (GPU required) |
| Gaussian | O(modes²) | Linear optics only |

### 5.3 Sampling Overhead

Each measurement requires sampling from distribution:
- Photon counting: O(1)
- Homodyne: O(1)
- Full density matrix: O(1)

For parameter sweeps (adaptive experiments):
- 1000 parameter values × 100 shots = 100,000 measurements
- Estimate: ~100 seconds (exact mode)

---

## 6. Quantum-Photonics Integration (Future)

### 6.1 Qubit-Photon Entanglement

**Planned** (Phase 2.4+):
```rust
pub enum QubitPhotonInterface {
    EmbeddedQubit { mode: usize },  // Photon at specific mode
    ExternalQubit { address: String },  // Remote qubit (network)
}
```

**Measurement Model**:
- Photon number correlates with qubit state
- Measurement-conditioned feedback updates qubit

### 6.2 Atom-Photon Interaction

**Future Enhancement**:
- Cavity QED hamiltonian
- Atom-photon entanglement
- Atomic cooling via photon sideband

---

## 7. Conformance & Validation

### 7.1 Test Categories (25+)

| Category | Tests | Coverage |
|----------|-------|----------|
| Noise models | 6 | Loss, dark counts, phase noise, Kerr |
| Measurements | 9 | Homodyne, heterodyne, photon counting + noise |
| Calibration | 4 | Phase, detector, drift simulation |
| Integration | 4 | With HAL v0.2, Engine v0.1, Scheduler v0.1 |
| Performance | 2 | Scaling, memory usage |

### 7.2 Correctness Criteria

All measurements must satisfy:
- ✅ Variance ≥ 0.5 (shot noise limit)
- ✅ Dark counts ∝ λ × t (Poisson)
- ✅ Phase error ∝ (Δν × gate_time)²
- ✅ Kerr phase ∝ n² × distance

---

## 8. Implementation Plan

### 8.1 Files to Create/Modify

**New Files**:
- `awen-runtime/src/simulator/mod.rs` (2000+ lines)
- `awen-runtime/src/simulator/noise.rs` (1000+ lines)
- `awen-runtime/src/simulator/measurement.rs` (800+ lines)
- `awen-runtime/tests/simulator_integration.rs` (1200+ lines)

**Modified Files**:
- `awen-runtime/src/hal_v0.rs` (add SimulatorBackend impl)
- `.github/workflows/simulator-conformance.yml` (new CI job)

### 8.2 Schedule

| Task | Est. Time | Dependencies |
|------|-----------|--------------|
| Noise models (loss, dark count, phase) | 4h | spec |
| Measurement implementations | 3h | noise models |
| Kerr effect | 2h | measurement |
| Calibration simulation | 2h | Kerr |
| Integration tests (20+) | 4h | all above |
| Performance optimization | 3h | tests passing |
| CI/CD setup | 2h | final code |

**Total Estimated**: 20 hours (can parallelize)

---

## 9. Success Criteria

Phase 2.4 is COMPLETE when:

- ✅ All noise models implemented (loss, dark counts, phase, Kerr, thermal)
- ✅ All measurement modes functional (homodyne, heterodyne, photon counting)
- ✅ Calibration drift simulation working
- ✅ 25+ integration tests passing
- ✅ >90% code coverage
- ✅ Performance: 1000-shot experiment in <1 second
- ✅ Zero scope reduction (all noise models present)
- ✅ Non-bypassable: SimulatorBackend only via PhotonicBackend trait
- ✅ Frontier-ready: Measurement-conditioned feedback functional, coherence limits enforced

---

## 10. Next Phases

**Phase 2.5**: Control + Calibration Integration
- Measurement-driven phase calibration (closed loop)
- Resource-aware scheduling of calibration

**Phase 2.6**: Artifacts + Storage Integration
- Measurement result capture for reproducibility
- Deterministic replay from artifacts

**Phase 3.x**: Production Hardening
- Real hardware backends (Broadcom, Intel, Xanadu)
- Distributed simulation (multi-GPU)
- Cloud deployment

---

## Appendix: Noise Model Parameters

### Default Configuration

```rust
pub struct SimulatorConfig {
    // Quantum state
    pub max_photons: usize = 3,
    pub num_modes: usize = 4,
    
    // Noise
    pub loss_rate: f64 = 0.01,  // per cm
    pub dark_count_rate: f64 = 1000.0,  // Hz
    pub lo_linewidth: f64 = 1000.0,  // Hz
    pub kerr_coefficient: f64 = 0.1,  // rad/(photon·cm)
    pub rin: f64 = 0.001,  // -30 dB
    pub temperature: f64 = 300.0,  // K
    
    // Measurement
    pub quantum_efficiency: f64 = 0.95,
    pub integration_time: f64 = 1e-6,  // 1 microsecond
    
    // Calibration
    pub phase_drift_rate: f64 = 1e-5,  // rad/s
    pub dark_count_drift: f64 = 0.0001,  // per second
}
```

---

**Specification Status**: ✅ Complete  
**Ready for Implementation**: Yes  
**Constitutional Directive**: ✅ Full-scope (all noise models), non-bypassable (trait-based), frontier-first (measurement-conditioned)

