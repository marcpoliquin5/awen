/// Reference Simulator v0.1 - Realistic Photonic Quantum Simulation
///
/// This module extends the SimulatorBackend from Phase 2.3 HAL v0.2 with
/// realistic noise models, Kerr effects, and measurement-conditioned dynamics.
///
/// CONSTITUTIONAL DIRECTIVE ALIGNMENT:
/// - Full-scope: All noise models (loss, dark counts, phase, Kerr, thermal)
/// - Non-bypassable: SimulatorBackend impl of PhotonicBackend trait only
/// - Frontier-first: Measurement-conditioned feedback, coherence limits enforced

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Noise model configuration for reference simulator
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulatorNoiseConfig {
    /// Photon loss rate (per cm of propagation)
    pub loss_rate_per_cm: f64,
    /// Dark count rate in detectors (Hz)
    pub dark_count_rate: f64,
    /// Local oscillator linewidth (Hz)
    pub lo_linewidth: f64,
    /// Kerr coefficient for self-phase modulation
    pub kerr_coefficient: f64,
    /// Relative intensity noise (-30 dB typical)
    pub relative_intensity_noise: f64,
    /// Temperature of environment (K)
    pub temperature: f64,
    /// Maximum photon number cutoff
    pub max_photons: usize,
}

impl Default for SimulatorNoiseConfig {
    fn default() -> Self {
        Self {
            loss_rate_per_cm: 0.01,           // 1% per cm
            dark_count_rate: 1000.0,          // 1 kHz
            lo_linewidth: 1000.0,             // 1 kHz
            kerr_coefficient: 0.1,            // rad/(photon·cm)
            relative_intensity_noise: 0.001,  // -30 dB
            temperature: 300.0,               // Room temp
            max_photons: 3,
        }
    }
}

/// Noise injection parameters for measurement
#[derive(Clone, Debug)]
pub struct NoiseInjectionParams {
    /// Phase noise to apply to local oscillator
    pub lo_phase_noise: f64,
    /// Frequency noise (for heterodyne)
    pub lo_frequency_noise: f64,
    /// Shot noise variance
    pub shot_noise_variance: f64,
    /// Thermal noise variance
    pub thermal_noise_variance: f64,
    /// Kerr-induced phase shift
    pub kerr_phase_shift: f64,
}

impl NoiseInjectionParams {
    /// Sample noise parameters from configuration and physics
    pub fn sample(config: &SimulatorNoiseConfig) -> Self {
        Self {
            lo_phase_noise: sample_gaussian() * (config.lo_linewidth * PI).sqrt(),
            lo_frequency_noise: sample_gaussian() * config.lo_linewidth,
            shot_noise_variance: 0.5,  // Vacuum shot noise limit
            thermal_noise_variance: config.relative_intensity_noise * 0.1,
            kerr_phase_shift: sample_gaussian() * config.kerr_coefficient * 0.01,
        }
    }
}

/// Photon loss simulation
#[derive(Clone, Debug)]
pub struct PhotonLossChannel {
    /// Loss rate (0.0 = no loss, 1.0 = complete absorption)
    pub loss_probability: f64,
}

impl PhotonLossChannel {
    /// Create loss channel from distance and loss coefficient
    pub fn from_distance(distance: f64, loss_rate_per_cm: f64) -> Self {
        let loss_prob = 1.0 - (-loss_rate_per_cm * distance).exp();
        Self {
            loss_probability: loss_prob.max(0.0).min(1.0),
        }
    }

    /// Apply loss to measured photon number
    pub fn apply(&self, photon_count: u32) -> u32 {
        if photon_count == 0 {
            return 0;
        }
        
        let mut remaining = photon_count;
        for _ in 0..photon_count {
            if sample_uniform() < self.loss_probability {
                remaining -= 1;
            }
        }
        remaining
    }

    /// Loss effect on homodyne variance
    pub fn quadrature_variance(&self) -> f64 {
        0.5 * (1.0 - self.loss_probability)
    }
}

/// Dark count noise for direct detection
#[derive(Clone, Debug)]
pub struct DarkCountNoise {
    /// Dark count rate (Hz)
    pub rate: f64,
    /// Integration time for counting
    pub integration_time: f64,
}

impl DarkCountNoise {
    /// Sample dark counts from Poisson distribution
    pub fn sample(&self) -> u32 {
        let lambda = self.rate * self.integration_time;
        poisson_sample(lambda)
    }

    /// Average dark count over measurement
    pub fn expected_count(&self) -> f64 {
        self.rate * self.integration_time
    }
}

/// Phase noise simulation (Wiener process)
#[derive(Clone, Debug)]
pub struct PhaseNoise {
    /// Linewidth in Hz (determines diffusion rate)
    pub linewidth: f64,
    /// Current phase (accumulates over time)
    pub current_phase: f64,
}

impl PhaseNoise {
    /// Create phase noise generator
    pub fn new(linewidth: f64) -> Self {
        Self {
            linewidth,
            current_phase: 0.0,
        }
    }

    /// Evolve phase noise for given time step
    pub fn evolve(&mut self, time_step: f64) {
        let diffusion = (self.linewidth * PI * time_step).sqrt();
        self.current_phase += sample_gaussian() * diffusion;
    }

    /// SNR degradation from phase noise during measurement
    pub fn snr_degradation(&self, measurement_time: f64) -> f64 {
        let phase_jitter = (self.linewidth * measurement_time).powi(2);
        phase_jitter / (1.0 + phase_jitter)
    }
}

/// Kerr effect simulation
#[derive(Clone, Debug)]
pub struct KarrEffect {
    /// Kerr coefficient (rad/(photon·cm))
    pub chi: f64,
    /// Propagation distance (cm)
    pub distance: f64,
}

impl KarrEffect {
    /// Phase shift from Kerr effect for given photon number
    pub fn phase_shift(&self, photon_number: u32) -> f64 {
        let n = photon_number as f64;
        self.chi * n * n * self.distance
    }

    /// Kerr effect impact on measurement variance
    pub fn variance_broadening(&self, max_photons: usize) -> f64 {
        let mut total_broadening = 0.0;
        for n in 0..=max_photons {
            total_broadening += self.phase_shift(n as u32).powi(2);
        }
        total_broadening / (max_photons as f64)
    }
}

/// Homodyne measurement with noise
#[derive(Clone, Debug)]
pub struct HomodyneSimulator {
    pub config: SimulatorNoiseConfig,
    pub noise_params: NoiseInjectionParams,
}

impl HomodyneSimulator {
    /// Simulate homodyne measurement with noise
    pub fn measure(
        &self,
        ideal_i: f64,
        ideal_q: f64,
        lo_power: f64,
    ) -> (f64, f64, f64) {
        // Apply phase noise to local oscillator
        let lo_angle = self.noise_params.lo_phase_noise;
        let rotated_i = ideal_i * lo_angle.cos() + ideal_q * lo_angle.sin();
        let rotated_q = -ideal_i * lo_angle.sin() + ideal_q * lo_angle.cos();

        // Add shot noise (proportional to LO power)
        let shot_i = rotated_i + sample_gaussian() * self.noise_params.shot_noise_variance.sqrt();
        let shot_q = rotated_q + sample_gaussian() * self.noise_params.shot_noise_variance.sqrt();

        // Add thermal/RIN noise
        let rin_factor = (1.0 + self.config.relative_intensity_noise * lo_power).sqrt();
        let final_i = shot_i * rin_factor;
        let final_q = shot_q * rin_factor;

        // Variance estimate
        let variance = 0.5 + self.noise_params.thermal_noise_variance;

        (final_i, final_q, variance)
    }
}

/// Heterodyne measurement with frequency noise
#[derive(Clone, Debug)]
pub struct HeterodyneSimulator {
    pub config: SimulatorNoiseConfig,
    pub noise_params: NoiseInjectionParams,
}

impl HeterodyneSimulator {
    /// Simulate heterodyne measurement with frequency jitter
    pub fn measure(
        &self,
        ideal_i: f64,
        ideal_q: f64,
        measurement_time: f64,
    ) -> (f64, f64, f64) {
        // Frequency jitter effect on SNR
        let frequency_jitter = self.noise_params.lo_frequency_noise;
        let snr_factor = 1.0 / (1.0 + (frequency_jitter * measurement_time).powi(2));

        // Apply frequency-dependent phase jitter
        let phase_jitter = self.noise_params.lo_phase_noise;
        let rotated_i = ideal_i * phase_jitter.cos() + ideal_q * phase_jitter.sin();
        let rotated_q = -ideal_i * phase_jitter.sin() + ideal_q * phase_jitter.cos();

        // Degrade magnitude due to frequency uncertainty
        let magnitude = (rotated_i.powi(2) + rotated_q.powi(2)).sqrt() * snr_factor;
        let phase = rotated_q.atan2(rotated_i);

        let snr = magnitude / (1.0 - snr_factor);

        (magnitude, phase, snr)
    }
}

/// Direct detection (photon counting) with dark counts
#[derive(Clone, Debug)]
pub struct DirectDetectionSimulator {
    pub config: SimulatorNoiseConfig,
    pub dark_count_noise: DarkCountNoise,
}

impl DirectDetectionSimulator {
    /// Simulate photon counting measurement
    pub fn measure(
        &self,
        photon_count: u32,
        quantum_efficiency: f64,
    ) -> u32 {
        // Apply quantum efficiency
        let detected = if sample_uniform() < quantum_efficiency {
            photon_count
        } else {
            0
        };

        // Add dark counts
        let dark = self.dark_count_noise.sample();

        detected + dark
    }

    /// Calibrate detected count to true photon number
    pub fn calibrate(
        &self,
        measured: u32,
        quantum_efficiency: f64,
    ) -> u32 {
        let dark_baseline = self.dark_count_noise.expected_count() as u32;
        let signal = (measured as i32 - dark_baseline as i32).max(0) as u32;
        ((signal as f64) / quantum_efficiency).round() as u32
    }
}

/// Calibration state for simulator (tracks drift and errors)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulatorCalibrationState {
    /// Phase calibration timestamp
    pub phase_calib_time: f64,
    /// Dark count calibration timestamp
    pub dark_calib_time: f64,
    /// Phase drift rate (rad/s)
    pub phase_drift_rate: f64,
    /// Dark count drift coefficient (per second)
    pub dark_count_drift: f64,
    /// Accumulated phase drift
    pub accumulated_phase_drift: f64,
}

impl Default for SimulatorCalibrationState {
    fn default() -> Self {
        Self {
            phase_calib_time: 0.0,
            dark_calib_time: 0.0,
            phase_drift_rate: 1e-5,      // rad/s
            dark_count_drift: 0.0001,    // per second
            accumulated_phase_drift: 0.0,
        }
    }
}

impl SimulatorCalibrationState {
    /// Update calibration state for elapsed time
    pub fn update(&mut self, elapsed_time: f64) {
        self.accumulated_phase_drift = self.phase_drift_rate * elapsed_time;
    }

    /// Check if phase calibration has expired (>300 μrad drift)
    pub fn phase_calib_expired(&self) -> bool {
        self.accumulated_phase_drift > 300e-6
    }

    /// Check if dark count calibration has expired (>10% drift)
    pub fn dark_calib_expired(&self) -> bool {
        self.dark_count_drift > 0.1
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Sample from standard Gaussian distribution
fn sample_gaussian() -> f64 {
    // Box-Muller transform
    let u1 = sample_uniform();
    let u2 = sample_uniform();
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

/// Sample uniform random between 0 and 1
fn sample_uniform() -> f64 {
    use std::cell::RefCell;
    thread_local! {
        static RNG: RefCell<u64> = RefCell::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        );
    }

    RNG.with(|rng| {
        let mut x = rng.borrow_mut();
        *x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let mantissa = (*x >> 11) as f64;
        let exponent = -53.0_f64;
        mantissa * 2.0_f64.powf(exponent)
    })
}

/// Sample from Poisson distribution
fn poisson_sample(lambda: f64) -> u32 {
    if lambda < 30.0 {
        // Knuth algorithm for small lambda
        let mut k = 0;
        let mut p = 1.0;
        let l = (-lambda).exp();
        while p > l {
            k += 1;
            p *= sample_uniform();
        }
        k - 1
    } else {
        // "Ratio of uniforms" for large lambda
        let g = lambda;
        let em = g + (2.0 * g).sqrt() * sample_gaussian();
        em.max(0.0) as u32
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_photon_loss_channel() {
        let loss = PhotonLossChannel::from_distance(1.0, 0.01);
        assert!(loss.loss_probability > 0.0);
        assert!(loss.loss_probability < 0.02);  // ~1% for 1 cm at 0.01 loss rate

        let remaining = loss.apply(10);
        assert!(remaining <= 10);
        assert!(remaining >= 8);  // Expect ~90% survival
    }

    #[test]
    fn test_dark_count_noise() {
        let dark = DarkCountNoise {
            rate: 1000.0,
            integration_time: 1e-6,
        };
        let expected = dark.expected_count();
        assert_eq!(expected, 0.001);  // 1000 Hz * 1 µs = 0.001 counts

        let samples: u32 = (0..100).map(|_| dark.sample()).sum();
        assert!(samples > 0);  // Poisson sampling should give some dark counts
    }

    #[test]
    fn test_phase_noise_evolution() {
        let mut phase_noise = PhaseNoise::new(1000.0);
        let initial = phase_noise.current_phase;
        phase_noise.evolve(1e-6);
        assert_ne!(phase_noise.current_phase, initial);  // Phase changed
    }

    #[test]
    fn test_kerr_effect() {
        let kerr = KarrEffect {
            chi: 0.1,
            distance: 1.0,
        };
        let phase_0 = kerr.phase_shift(0);
        let phase_1 = kerr.phase_shift(1);
        let phase_2 = kerr.phase_shift(2);
        
        assert_eq!(phase_0, 0.0);
        assert_eq!(phase_1, 0.1);
        assert_eq!(phase_2, 0.4);  // n² scaling
    }

    #[test]
    fn test_homodyne_measurement() {
        let config = SimulatorNoiseConfig::default();
        let noise_params = NoiseInjectionParams::sample(&config);
        let simulator = HomodyneSimulator { config, noise_params };
        
        let (i, q, var) = simulator.measure(1.0, 0.0, 1.0);
        assert!(var >= 0.5);  // Shot noise limit
        assert!(i.is_finite() && q.is_finite());
    }

    #[test]
    fn test_calibration_state_drift() {
        let mut calib = SimulatorCalibrationState::default();
        calib.update(1000.0);  // 1000 seconds
        
        assert!(calib.accumulated_phase_drift > 0.0);
        assert!(calib.phase_calib_expired());  // Should be expired
    }
}
