//! Physical-unit noise contracts and a seeded photonic GEMM noise model.
//!
//! Fidelity: **compact model**. Each term is a standard closed-form detector or
//! device model, not a full-wave or measured result. The model describes one
//! analog matrix-vector product per contraction tile on an amplitude-encoded
//! (coherent, balanced-detection) photonic matrix core:
//!
//! 1. Inputs and weights are scaled into `[-1, 1]` by their per-row max-abs and
//!    quantized by signed DACs with `dac_bits`.
//! 2. Each programmed weight `w = cos(theta)` carries a phase error
//!    `delta ~ N(0, sigma_phi^2 + drift^2 * t)`, giving the first-order weight
//!    error `sqrt(1 - w^2) * delta`.
//! 3. The normalized tile output is `z_o = (1/k) * sum_i w_oi x_i`.
//! 4. Adjacent output channels couple with power ratio `10^(crosstalk_db/10)`.
//! 5. Detection adds signal-independent shot noise with standard deviation
//!    `1/sqrt(N_fs)` relative to full scale, where
//!    `N_fs = photons_per_mac * k * 10^(-IL/10) * quantum_efficiency` is the
//!    full-scale photoelectron count; thermal noise `n_th / N_fs`; and laser
//!    relative intensity noise `|z| * sqrt(10^(RIN/10) * B)`.
//! 6. A signed ADC with `adc_bits` digitizes the tile output, which is then
//!    rescaled and accumulated digitally across tiles.
//!
//! The shot-noise term follows from Poisson photon statistics (variance equals
//! the mean count); the effective-number-of-bits relation is the IEEE 1241
//! definition `ENOB = (SINAD_dB - 1.76) / 6.02` for a full-scale sine.

use crate::ir::GemmShape;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const NOISE_CONTRACT_VERSION: &str = "awen.photonic-noise-contract.v1";
pub const GEMM_SIMULATION_REPORT_VERSION: &str = "awen.photonic-gemm-simulation.v1";

/// How a numerical result was obtained. Shared by every physical model.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Fidelity {
    Analytic,
    CompactModel,
    NumericalPropagation,
    FullWave,
    DigitalTwin,
    Measured,
}

/// Physical-unit description of the analog error sources of a photonic core.
///
/// Optional fields disable the corresponding term when `None`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PhotonicNoiseContract {
    pub version: String,
    /// Mean detected-path photons contributed by one multiply-accumulate at full scale.
    pub photons_per_mac: Option<f64>,
    /// Detector quantum efficiency in (0, 1].
    pub quantum_efficiency: f64,
    /// Total optical insertion loss from laser to detector, in dB (>= 0).
    pub insertion_loss_db: f64,
    /// Static phase-programming error, standard deviation in radians.
    pub phase_sigma_rad: f64,
    /// Phase random-walk drift coefficient in rad / sqrt(s).
    pub drift_rad_per_sqrt_s: f64,
    /// Time since the last calibration, in seconds.
    pub seconds_since_calibration: f64,
    /// Laser relative intensity noise in dB/Hz (negative).
    pub rin_db_per_hz: Option<f64>,
    /// Detection bandwidth in Hz, used for RIN integration.
    pub detection_bandwidth_hz: f64,
    /// Input-referred detector thermal noise in electrons rms per sample.
    pub thermal_noise_electrons: f64,
    /// Adjacent-channel power crosstalk in dB (negative).
    pub crosstalk_db: Option<f64>,
    /// Signed DAC resolution for inputs and weights.
    pub dac_bits: Option<u32>,
    /// Signed ADC resolution for tile outputs.
    pub adc_bits: Option<u32>,
}

impl Default for PhotonicNoiseContract {
    /// A deliberately modest illustrative operating point, not a measured device.
    fn default() -> Self {
        Self {
            version: NOISE_CONTRACT_VERSION.to_string(),
            photons_per_mac: Some(1000.0),
            quantum_efficiency: 0.8,
            insertion_loss_db: 6.0,
            phase_sigma_rad: 0.002,
            drift_rad_per_sqrt_s: 0.0,
            seconds_since_calibration: 0.0,
            rin_db_per_hz: Some(-150.0),
            detection_bandwidth_hz: 10e9,
            thermal_noise_electrons: 100.0,
            crosstalk_db: Some(-35.0),
            dac_bits: Some(8),
            adc_bits: Some(8),
        }
    }
}

impl PhotonicNoiseContract {
    /// Contract with every noise source disabled (exact arithmetic).
    pub fn ideal() -> Self {
        Self {
            version: NOISE_CONTRACT_VERSION.to_string(),
            photons_per_mac: None,
            quantum_efficiency: 1.0,
            insertion_loss_db: 0.0,
            phase_sigma_rad: 0.0,
            drift_rad_per_sqrt_s: 0.0,
            seconds_since_calibration: 0.0,
            rin_db_per_hz: None,
            detection_bandwidth_hz: 1.0,
            thermal_noise_electrons: 0.0,
            crosstalk_db: None,
            dac_bits: None,
            adc_bits: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != NOISE_CONTRACT_VERSION {
            bail!("noise contract version must be {NOISE_CONTRACT_VERSION}");
        }
        let finite = [
            self.quantum_efficiency,
            self.insertion_loss_db,
            self.phase_sigma_rad,
            self.drift_rad_per_sqrt_s,
            self.seconds_since_calibration,
            self.detection_bandwidth_hz,
            self.thermal_noise_electrons,
        ];
        if finite.iter().any(|v| !v.is_finite()) {
            bail!("noise contract values must be finite");
        }
        if let Some(p) = self.photons_per_mac {
            if !(p.is_finite() && p > 0.0) {
                bail!("photons_per_mac must be positive");
            }
        }
        if !(self.quantum_efficiency > 0.0 && self.quantum_efficiency <= 1.0) {
            bail!("quantum_efficiency must be in (0, 1]");
        }
        if self.insertion_loss_db < 0.0 {
            bail!("insertion_loss_db must be >= 0");
        }
        if self.phase_sigma_rad < 0.0 || self.drift_rad_per_sqrt_s < 0.0 {
            bail!("phase noise parameters must be >= 0");
        }
        if self.seconds_since_calibration < 0.0 {
            bail!("seconds_since_calibration must be >= 0");
        }
        if let Some(rin) = self.rin_db_per_hz {
            if !(rin.is_finite() && rin < 0.0) {
                bail!("rin_db_per_hz must be negative");
            }
        }
        if self.detection_bandwidth_hz <= 0.0 {
            bail!("detection_bandwidth_hz must be positive");
        }
        if self.thermal_noise_electrons < 0.0 {
            bail!("thermal_noise_electrons must be >= 0");
        }
        if let Some(xt) = self.crosstalk_db {
            if !(xt.is_finite() && xt < 0.0) {
                bail!("crosstalk_db must be negative");
            }
        }
        for (name, bits) in [("dac_bits", self.dac_bits), ("adc_bits", self.adc_bits)] {
            if let Some(b) = bits {
                if !(2..=24).contains(&b) {
                    bail!("{name} must be in 2..=24");
                }
            }
        }
        Ok(())
    }

    /// Optical power transmission `10^(-IL/10)`.
    pub fn transmission(&self) -> f64 {
        10f64.powf(-self.insertion_loss_db / 10.0)
    }

    /// Total weight phase error standard deviation including drift.
    pub fn total_phase_sigma(&self) -> f64 {
        (self.phase_sigma_rad.powi(2)
            + self.drift_rad_per_sqrt_s.powi(2) * self.seconds_since_calibration)
            .sqrt()
    }

    /// Full-scale photoelectron count for a contraction of length `k`.
    pub fn full_scale_photoelectrons(&self, k: usize) -> Option<f64> {
        self.photons_per_mac
            .map(|p| p * k as f64 * self.transmission() * self.quantum_efficiency)
    }

    /// Analytic per-term standard deviations (normalized to full scale) for a
    /// tile of length `k` and normalized signal magnitude `signal`.
    pub fn predicted_sigmas(&self, k: usize, signal: f64) -> NoiseBudget {
        let n_fs = self.full_scale_photoelectrons(k);
        let shot = n_fs.map(|n| 1.0 / n.sqrt()).unwrap_or(0.0);
        let thermal = match n_fs {
            Some(n) => self.thermal_noise_electrons / n,
            None => 0.0,
        };
        let rin = self
            .rin_db_per_hz
            .map(|r| signal.abs() * (10f64.powf(r / 10.0) * self.detection_bandwidth_hz).sqrt())
            .unwrap_or(0.0);
        let adc = self
            .adc_bits
            .map(|b| quantization_step(b) / 12f64.sqrt())
            .unwrap_or(0.0);
        let analog = (shot * shot + thermal * thermal + rin * rin).sqrt();
        let total = (analog * analog + adc * adc).sqrt();
        NoiseBudget {
            full_scale_photoelectrons: n_fs,
            shot,
            thermal,
            rin,
            adc_quantization: adc,
            total,
            effective_bits: effective_bits_from_sigma(total),
        }
    }
}

/// Analytic noise budget in units of the normalized full scale.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoiseBudget {
    pub full_scale_photoelectrons: Option<f64>,
    pub shot: f64,
    pub thermal: f64,
    pub rin: f64,
    pub adc_quantization: f64,
    pub total: f64,
    /// ENOB for a full-scale sine with this noise (infinite when noiseless).
    pub effective_bits: f64,
}

/// Quantization step of a signed converter spanning `[-1, 1]`.
pub fn quantization_step(bits: u32) -> f64 {
    1.0 / ((1u64 << (bits - 1)) - 1) as f64
}

/// Quantize to a signed converter over `[-1, 1]` with clamping.
pub fn quantize_signed(value: f64, bits: u32) -> f64 {
    let levels = ((1u64 << (bits - 1)) - 1) as f64;
    (value.clamp(-1.0, 1.0) * levels).round() / levels
}

/// ENOB of a full-scale sine (power 1/2) in additive noise of std `sigma`.
pub fn effective_bits_from_sigma(sigma: f64) -> f64 {
    if sigma <= 0.0 {
        return f64::INFINITY;
    }
    let sinad_db = 10.0 * (0.5 / (sigma * sigma)).log10();
    (sinad_db - 1.76) / 6.02
}

/// Shot-noise-limited ENOB for a full-scale photoelectron count.
pub fn shot_limited_effective_bits(full_scale_photoelectrons: f64) -> f64 {
    effective_bits_from_sigma(1.0 / full_scale_photoelectrons.sqrt())
}

/// Deterministic SplitMix64 generator with Box–Muller normals, so results are
/// reproducible across platforms and language bindings for a given seed.
#[derive(Debug, Clone)]
pub struct NoiseRng {
    state: u64,
    spare: Option<f64>,
}

impl NoiseRng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
            spare: None,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in (0, 1).
    pub fn uniform(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    pub fn normal(&mut self) -> f64 {
        if let Some(value) = self.spare.take() {
            return value;
        }
        let u1 = self.uniform();
        let u2 = self.uniform();
        let radius = (-2.0 * u1.ln()).sqrt();
        let angle = 2.0 * std::f64::consts::PI * u2;
        self.spare = Some(radius * angle.sin());
        radius * angle.cos()
    }
}

/// Result of a simulated photonic GEMM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GemmSimulationReport {
    pub version: String,
    pub fidelity: Fidelity,
    pub evidence: String,
    pub rows: usize,
    pub inner: usize,
    pub outputs: usize,
    pub tile_k: usize,
    pub tiles_per_output: usize,
    pub seed: u64,
    pub contract: PhotonicNoiseContract,
    /// Analytic budget for one full tile at full-scale signal.
    pub tile_budget: NoiseBudget,
    pub max_abs_error_vs_exact: f64,
    pub rms_error_vs_exact: f64,
}

fn row_scales(values: &[f64], rows: usize, cols: usize) -> Vec<f64> {
    (0..rows)
        .map(|r| {
            values[r * cols..(r + 1) * cols]
                .iter()
                .fold(0.0f64, |m, v| m.max(v.abs()))
        })
        .collect()
}

/// Simulate `Y = X Wᵀ` with row-major `X: [m, k]` and `W: [n, k]` (`shape` uses the IR
/// convention: `m` rows, `k` contraction length, `n` outputs)
/// on a photonic core with contraction tiles of `tile_k`, applying `contract`.
pub fn photonic_gemm(
    x: &[f64],
    w: &[f64],
    shape: GemmShape,
    contract: &PhotonicNoiseContract,
    tile_k: usize,
    seed: u64,
) -> Result<(Vec<f64>, GemmSimulationReport)> {
    let GemmShape {
        m: rows,
        k: inner,
        n: outputs,
    } = shape;
    contract.validate()?;
    if rows == 0 || inner == 0 || outputs == 0 || tile_k == 0 {
        bail!("GEMM dimensions and tile_k must be positive");
    }
    if x.len() != rows * inner || w.len() != outputs * inner {
        bail!("GEMM operand lengths do not match their shapes");
    }
    if x.iter().chain(w.iter()).any(|v| !v.is_finite()) {
        bail!("GEMM operands must be finite");
    }
    let mut rng = NoiseRng::new(seed);
    let tiles = inner.div_ceil(tile_k);
    let crosstalk = contract.crosstalk_db.map(|db| 10f64.powf(db / 10.0));
    let phase_sigma = contract.total_phase_sigma();
    let rin_rel = contract
        .rin_db_per_hz
        .map(|r| (10f64.powf(r / 10.0) * contract.detection_bandwidth_hz).sqrt());
    let mut y = vec![0.0; rows * outputs];

    for tile in 0..tiles {
        let k0 = tile * tile_k;
        let k1 = (k0 + tile_k).min(inner);
        let k = k1 - k0;
        // Per-tile operand slices and scales (one DAC full scale per vector).
        let xt: Vec<f64> = (0..rows)
            .flat_map(|r| x[r * inner + k0..r * inner + k1].iter().copied())
            .collect();
        let wt: Vec<f64> = (0..outputs)
            .flat_map(|o| w[o * inner + k0..o * inner + k1].iter().copied())
            .collect();
        let sx = row_scales(&xt, rows, k);
        let sw = row_scales(&wt, outputs, k);
        let xn: Vec<f64> = xt
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let s = sx[i / k];
                let n = if s > 0.0 { v / s } else { 0.0 };
                contract.dac_bits.map_or(n, |b| quantize_signed(n, b))
            })
            .collect();
        // Weights are programmed once per tile; phase errors are static per programming.
        let wn: Vec<f64> = wt
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let s = sw[i / k];
                let n = if s > 0.0 { v / s } else { 0.0 };
                let q = contract.dac_bits.map_or(n, |b| quantize_signed(n, b));
                if phase_sigma > 0.0 {
                    (q + (1.0 - q * q).max(0.0).sqrt() * phase_sigma * rng.normal())
                        .clamp(-1.0, 1.0)
                } else {
                    q
                }
            })
            .collect();
        let n_fs = contract.full_scale_photoelectrons(k);
        let shot_sigma = n_fs.map(|n| 1.0 / n.sqrt());
        let thermal_sigma = n_fs.map(|n| contract.thermal_noise_electrons / n);
        for r in 0..rows {
            let mut z: Vec<f64> = (0..outputs)
                .map(|o| {
                    let mut acc = 0.0;
                    for i in 0..k {
                        acc += wn[o * k + i] * xn[r * k + i];
                    }
                    acc / k as f64
                })
                .collect();
            if let Some(c) = crosstalk {
                let clean = z.clone();
                for o in 0..outputs {
                    let left = if o > 0 { clean[o - 1] } else { 0.0 };
                    let right = if o + 1 < outputs { clean[o + 1] } else { 0.0 };
                    z[o] = clean[o] + c * (left + right);
                }
            }
            for (o, value) in z.iter_mut().enumerate() {
                let mut v = *value;
                if let Some(rel) = rin_rel {
                    v += v.abs() * rel * rng.normal();
                }
                if let Some(s) = shot_sigma {
                    v += s * rng.normal();
                }
                if let Some(s) = thermal_sigma {
                    if s > 0.0 {
                        v += s * rng.normal();
                    }
                }
                if let Some(b) = contract.adc_bits {
                    v = quantize_signed(v, b);
                }
                y[r * outputs + o] += v * k as f64 * sx[r] * sw[o];
            }
        }
    }

    let mut max_abs: f64 = 0.0;
    let mut sq = 0.0;
    for r in 0..rows {
        for o in 0..outputs {
            let exact: f64 = (0..inner)
                .map(|i| x[r * inner + i] * w[o * inner + i])
                .sum();
            let e = y[r * outputs + o] - exact;
            max_abs = max_abs.max(e.abs());
            sq += e * e;
        }
    }
    let report = GemmSimulationReport {
        version: GEMM_SIMULATION_REPORT_VERSION.to_string(),
        fidelity: Fidelity::CompactModel,
        evidence: "simulated with the AWEN compact noise model; not a hardware measurement"
            .to_string(),
        rows,
        inner,
        outputs,
        tile_k,
        tiles_per_output: tiles,
        seed,
        contract: contract.clone(),
        tile_budget: contract.predicted_sigmas(tile_k.min(inner), 1.0),
        max_abs_error_vs_exact: max_abs,
        rms_error_vs_exact: (sq / (rows * outputs) as f64).sqrt(),
    };
    Ok((y, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(f: impl FnOnce(&mut PhotonicNoiseContract)) -> PhotonicNoiseContract {
        let mut c = PhotonicNoiseContract::ideal();
        f(&mut c);
        c
    }

    fn std_dev(values: &[f64]) -> f64 {
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64).sqrt()
    }

    /// Normalized tile outputs for `samples` rows of the all-ones input against
    /// one weight row of constant value `weight`, with k = `k`.
    fn normalized_outputs(
        c: &PhotonicNoiseContract,
        k: usize,
        weight: f64,
        samples: usize,
        seed: u64,
    ) -> Vec<f64> {
        let x = vec![1.0; samples * k];
        let w = vec![weight; k];
        let (y, _) = photonic_gemm(
            &x,
            &w,
            GemmShape {
                m: samples,
                k,
                n: 1,
            },
            c,
            k,
            seed,
        )
        .unwrap();
        // Undo the digital rescale (k * sx * sw) to recover the normalized output.
        let scale = k as f64 * 1.0 * weight.abs().max(f64::MIN_POSITIVE);
        y.iter().map(|v| v / scale).collect()
    }

    #[test]
    fn ideal_contract_is_exact() {
        let x = [1.0, -2.0, 0.5, 3.0, 4.0, -1.0];
        let w = [0.5, 1.0, -1.0, 2.0, 0.0, 1.5];
        let (y, report) = photonic_gemm(
            &x,
            &w,
            GemmShape { m: 2, k: 3, n: 2 },
            &PhotonicNoiseContract::ideal(),
            2,
            1,
        )
        .unwrap();
        assert!(report.max_abs_error_vs_exact < 1e-12, "{report:?}");
        assert!((y[0] - (0.5 - 2.0 - 0.5)).abs() < 1e-12);
        assert_eq!(report.tiles_per_output, 2);
        assert_eq!(report.fidelity, Fidelity::CompactModel);
    }

    #[test]
    fn shot_noise_matches_poisson_prediction_within_3_percent() {
        let c = only(|c| c.photons_per_mac = Some(50.0));
        let k = 64;
        let outputs = normalized_outputs(&c, k, 1.0, 20_000, 7);
        let predicted = c.predicted_sigmas(k, 1.0).shot;
        assert!((predicted - 1.0 / (50.0f64 * 64.0).sqrt()).abs() < 1e-15);
        let measured = std_dev(&outputs);
        assert!(
            (measured / predicted - 1.0).abs() < 0.03,
            "measured {measured} predicted {predicted}"
        );
    }

    #[test]
    fn insertion_loss_scales_shot_noise_by_sqrt_of_transmission() {
        let lossless = only(|c| c.photons_per_mac = Some(50.0));
        let lossy = only(|c| {
            c.photons_per_mac = Some(50.0);
            c.insertion_loss_db = 10.0;
        });
        let a = std_dev(&normalized_outputs(&lossless, 32, 1.0, 20_000, 3));
        let b = std_dev(&normalized_outputs(&lossy, 32, 1.0, 20_000, 3));
        assert!(
            ((b / a) - 10f64.sqrt()).abs() / 10f64.sqrt() < 0.04,
            "ratio {}",
            b / a
        );
    }

    #[test]
    fn thermal_noise_matches_prediction() {
        let c = only(|c| {
            c.photons_per_mac = Some(1e12);
            c.thermal_noise_electrons = 2.0e8;
        });
        let k = 16;
        let budget = c.predicted_sigmas(k, 1.0);
        let measured = std_dev(&normalized_outputs(&c, k, 1.0, 20_000, 5));
        let expected = (budget.thermal.powi(2) + budget.shot.powi(2)).sqrt();
        assert!(
            (measured / expected - 1.0).abs() < 0.03,
            "{measured} vs {expected}"
        );
        assert!(budget.thermal > 10.0 * budget.shot);
    }

    #[test]
    fn rin_is_multiplicative_in_signal() {
        let c = only(|c| {
            c.rin_db_per_hz = Some(-120.0);
            c.detection_bandwidth_hz = 1e10;
        });
        // Signal 0.5: weights of 0.5 normalize to 1.0, so use mixed weights.
        let k = 4;
        let samples = 20_000;
        let x = vec![1.0; samples * k];
        let w = vec![1.0, 1.0, 0.0, 0.0]; // z = 0.5
        let (y, _) = photonic_gemm(
            &x,
            &w,
            GemmShape {
                m: samples,
                k,
                n: 1,
            },
            &c,
            k,
            11,
        )
        .unwrap();
        let z: Vec<f64> = y.iter().map(|v| v / k as f64).collect();
        let predicted = c.predicted_sigmas(k, 0.5).rin;
        assert!((predicted - 0.5 * (1e-12f64 * 1e10).sqrt()).abs() < 1e-15);
        assert!((std_dev(&z) / predicted - 1.0).abs() < 0.03);
    }

    #[test]
    fn phase_error_follows_first_order_weight_model() {
        let c = only(|c| c.phase_sigma_rad = 0.01);
        // One weight value 0.6 relative to a row max of 1.0: use [1.0, 0.6] rows and
        // read the second output across many independent programmings (seeds).
        let mut samples = Vec::new();
        for seed in 0..4000u64 {
            let (y, _) = photonic_gemm(
                &[0.0, 1.0],
                &[1.0, 0.6],
                GemmShape { m: 1, k: 2, n: 1 },
                &c,
                2,
                seed,
            )
            .unwrap();
            samples.push(y[0]);
        }
        let predicted = (1.0f64 - 0.36).sqrt() * 0.01;
        let measured = std_dev(&samples);
        assert!(
            (measured / predicted - 1.0).abs() < 0.05,
            "{measured} vs {predicted}"
        );
    }

    #[test]
    fn drift_adds_in_quadrature() {
        let c = only(|c| {
            c.phase_sigma_rad = 0.003;
            c.drift_rad_per_sqrt_s = 0.001;
            c.seconds_since_calibration = 16.0;
        });
        assert!((c.total_phase_sigma() - 0.005).abs() < 1e-12);
    }

    #[test]
    fn crosstalk_couples_adjacent_channels_exactly() {
        let c = only(|c| c.crosstalk_db = Some(-20.0));
        // Three outputs; only the middle weight row is nonzero.
        let (y, _) = photonic_gemm(
            &[1.0],
            &[0.0, 1.0, 0.0],
            GemmShape { m: 1, k: 1, n: 3 },
            &c,
            1,
            0,
        )
        .unwrap();
        // Rows with zero weights have zero scale, so leakage is invisible there; check
        // with all rows equal instead.
        assert!((y[1] - 1.0).abs() < 1e-12);
        let (y, _) = photonic_gemm(
            &[1.0],
            &[1.0, 1.0, 1.0],
            GemmShape { m: 1, k: 1, n: 3 },
            &c,
            1,
            0,
        )
        .unwrap();
        assert!(
            (y[0] - 1.01).abs() < 1e-12
                && (y[1] - 1.02).abs() < 1e-12
                && (y[2] - 1.01).abs() < 1e-12
        );
    }

    #[test]
    fn adc_quantization_error_is_bounded_and_uniform() {
        let c = only(|c| c.adc_bits = Some(6));
        let step = quantization_step(6);
        let mut rng = NoiseRng::new(9);
        let mut errors = Vec::new();
        for _ in 0..20_000 {
            let v = rng.uniform() * 2.0 - 1.0;
            let q = quantize_signed(v, 6);
            assert!((q - v).abs() <= step / 2.0 + 1e-15);
            errors.push(q - v);
        }
        let measured = std_dev(&errors);
        let predicted = c.predicted_sigmas(1, 1.0).adc_quantization;
        assert!((measured / predicted - 1.0).abs() < 0.03);
    }

    #[test]
    fn shot_limited_enob_formula() {
        // N = 2 * 10^((6.02 * b + 1.76) / 10) photoelectrons gives exactly b bits.
        for bits in [4.0f64, 8.0, 12.0] {
            let n = 2.0 * 10f64.powf((6.02 * bits + 1.76) / 10.0);
            assert!((shot_limited_effective_bits(n) - bits).abs() < 1e-9);
        }
    }

    #[test]
    fn simulation_is_deterministic_for_a_seed() {
        let c = PhotonicNoiseContract::default();
        let x: Vec<f64> = (0..64).map(|i| (i as f64 * 0.37).sin()).collect();
        let w: Vec<f64> = (0..128).map(|i| (i as f64 * 0.11).cos()).collect();
        let a = photonic_gemm(&x, &w, GemmShape { m: 2, k: 32, n: 4 }, &c, 16, 42).unwrap();
        let b = photonic_gemm(&x, &w, GemmShape { m: 2, k: 32, n: 4 }, &c, 16, 42).unwrap();
        let other = photonic_gemm(&x, &w, GemmShape { m: 2, k: 32, n: 4 }, &c, 16, 43).unwrap();
        assert_eq!(a, b);
        assert_ne!(a.0, other.0);
        assert!(a.1.rms_error_vs_exact > 0.0);
    }

    #[test]
    fn invalid_contracts_are_rejected() {
        let bad = [
            only(|c| c.quantum_efficiency = 1.5),
            only(|c| c.insertion_loss_db = -1.0),
            only(|c| c.rin_db_per_hz = Some(3.0)),
            only(|c| c.crosstalk_db = Some(0.0)),
            only(|c| c.adc_bits = Some(1)),
            only(|c| c.photons_per_mac = Some(0.0)),
            only(|c| c.version = "v0".into()),
        ];
        for contract in bad {
            assert!(contract.validate().is_err(), "{contract:?}");
        }
        assert!(PhotonicNoiseContract::default().validate().is_ok());
    }

    #[test]
    fn default_contract_matches_the_checked_schema_fixture() {
        let fixture: PhotonicNoiseContract = serde_json::from_str(include_str!(
            "../../awen-spec/fixtures/photonic_noise_contract.default.json"
        ))
        .unwrap();
        assert_eq!(fixture, PhotonicNoiseContract::default());
    }

    #[test]
    fn contract_json_round_trips_and_rejects_unknown_fields() {
        let c = PhotonicNoiseContract::default();
        let text = serde_json::to_string(&c).unwrap();
        let back: PhotonicNoiseContract = serde_json::from_str(&text).unwrap();
        assert_eq!(c, back);
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["laser_color"] = serde_json::json!("blue");
        assert!(serde_json::from_value::<PhotonicNoiseContract>(value).is_err());
    }
}
