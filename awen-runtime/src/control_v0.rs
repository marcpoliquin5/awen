/// Phase 2.5: Control + Calibration Integration
///
/// Measurement-driven control and adaptive calibration framework for AWEN photonic simulator.
/// Enables real-time feedback, adaptive measurement selection, and automated calibration.
use std::collections::VecDeque;

/// Measurement result from detector
#[derive(Debug, Clone, Copy)]
pub struct MeasurementResult {
    pub i_quadrature: f64, // Homodyne I or magnitude
    pub q_quadrature: f64, // Homodyne Q or phase
    pub variance: f64,     // Measurement variance
    pub timestamp_ns: u64, // When measurement occurred
}

impl MeasurementResult {
    /// Extract phase from quadratures
    pub fn phase(&self) -> f64 {
        self.q_quadrature.atan2(self.i_quadrature)
    }

    /// Extract magnitude from quadratures
    pub fn magnitude(&self) -> f64 {
        (self.i_quadrature.powi(2) + self.q_quadrature.powi(2)).sqrt()
    }

    /// Estimate fidelity from excess variance
    pub fn estimated_fidelity(&self) -> f64 {
        let quantum_limit = 0.5;
        let excess_variance = (self.variance - quantum_limit).max(0.0);
        (1.0 - excess_variance / 2.0).max(0.0)
    }
}

/// Phase correction to apply based on measurement
#[derive(Debug, Clone, Copy)]
pub struct PhaseCorrection {
    pub delta_phi: f64,  // Phase shift correction (radians)
    pub confidence: f64, // Confidence in correction (0.0-1.0)
    pub applied_at: u64, // Timestamp when applied
}

/// Adaptive measurement selection decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementMode {
    Homodyne,
    Heterodyne,
    DirectDetection,
}

/// Measurement-conditioned feedback controller
pub struct FeedbackController {
    measurement_buffer: VecDeque<MeasurementResult>,
    correction_history: VecDeque<PhaseCorrection>,
    loop_latency_ns: u64, // Measured feedback latency
}

impl FeedbackController {
    /// Create new feedback controller
    pub fn new() -> Self {
        Self {
            measurement_buffer: VecDeque::with_capacity(100),
            correction_history: VecDeque::with_capacity(100),
            loop_latency_ns: 200, // ~200 ns typical feedback latency
        }
    }

    /// Record measurement result
    pub fn record_measurement(&mut self, result: MeasurementResult) {
        self.measurement_buffer.push_back(result);
        if self.measurement_buffer.len() > 100 {
            self.measurement_buffer.pop_front();
        }
    }

    /// Get latest measurement
    pub fn latest_measurement(&self) -> Option<MeasurementResult> {
        self.measurement_buffer.back().copied()
    }

    /// Generate phase correction from latest measurement
    pub fn compute_phase_correction(&self) -> Option<PhaseCorrection> {
        let measurement = self.latest_measurement()?;
        let phase = measurement.phase();

        // Simple feedback: correct phase toward zero
        let target_phase = 0.0;
        let phase_error = phase - target_phase;

        // Proportional gain: correct 50% of observed error
        let gain = 0.5;
        let delta_phi = -gain * phase_error;

        Some(PhaseCorrection {
            delta_phi,
            confidence: measurement.estimated_fidelity(),
            applied_at: measurement.timestamp_ns + self.loop_latency_ns,
        })
    }

    /// Record applied correction
    pub fn record_correction(&mut self, correction: PhaseCorrection) {
        self.correction_history.push_back(correction);
        if self.correction_history.len() > 100 {
            self.correction_history.pop_front();
        }
    }

    /// Get average feedback loop latency
    pub fn measured_loop_latency_ns(&self) -> u64 {
        if self.correction_history.len() < 2 {
            return self.loop_latency_ns;
        }

        let mut total_latency = 0u64;
        for window in self
            .correction_history
            .iter()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .windows(2)
        {
            if let [prev, curr] = window {
                total_latency += curr.applied_at.saturating_sub(prev.applied_at);
            }
        }

        total_latency / (self.correction_history.len().min(10) as u64).max(1)
    }
}

impl Default for FeedbackController {
    fn default() -> Self {
        Self::new()
    }
}

/// Adaptive measurement strategy selector
pub struct AdaptiveMeasurementSelector {
    last_signal_strength: f64,
    last_lo_linewidth: f64,
    time_remaining_ns: u64,
}

impl AdaptiveMeasurementSelector {
    /// Create new adaptive selector
    pub fn new() -> Self {
        Self {
            last_signal_strength: 1.0,
            last_lo_linewidth: 1000.0, // 1 kHz
            time_remaining_ns: 100_000,
        }
    }

    /// Select measurement mode based on context
    pub fn select_mode(
        &self,
        expected_photons: f64,
        lo_linewidth_hz: f64,
        deadline_ns: u64,
    ) -> MeasurementMode {
        // Decision tree from specification

        // Signal strength decision
        if expected_photons > 1.0 {
            // Strong signal, can use heterodyne

            // Frequency stability decision
            if lo_linewidth_hz > 10_000.0 {
                // High linewidth, avoid heterodyne frequency jitter
                return MeasurementMode::Homodyne;
            }

            // Deadline decision
            if deadline_ns > 1_000_000 {
                // Plenty of time, heterodyne gives better SNR
                return MeasurementMode::Heterodyne;
            }
        }

        // Weak signal or tight deadline
        if deadline_ns < 500_000 {
            // Very tight deadline, use fast measurement
            return MeasurementMode::DirectDetection;
        }

        // Default to homodyne
        MeasurementMode::Homodyne
    }

    /// Update selector state
    pub fn update(&mut self, signal_strength: f64, lo_linewidth: f64, time_remaining: u64) {
        self.last_signal_strength = signal_strength;
        self.last_lo_linewidth = lo_linewidth;
        self.time_remaining_ns = time_remaining;
    }
}

impl Default for AdaptiveMeasurementSelector {
    fn default() -> Self {
        Self::new()
    }
}

/// Calibration state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationState {
    Operational,
    PhaseCalibrationNeeded,
    DarkCountCalibrationNeeded,
    MeasuringPhaseDrift,
    MeasuringDarkCount,
    UpdatingCoefficients,
}

/// Phase calibration data
#[derive(Debug, Clone)]
pub struct PhaseCalibration {
    pub correction_factor: f64, // α_phase
    pub last_calibrated_ns: u64,
    pub drift_rate_urad_per_s: f64,
    pub expiration_threshold_urad: f64,
}

impl PhaseCalibration {
    /// Create with default values
    pub fn new() -> Self {
        Self {
            correction_factor: 1.0,
            last_calibrated_ns: 0,
            drift_rate_urad_per_s: 1.0,
            expiration_threshold_urad: 300.0,
        }
    }

    /// Check if calibration has expired
    pub fn is_expired(&self, current_time_ns: u64) -> bool {
        let elapsed_s = (current_time_ns - self.last_calibrated_ns) as f64 / 1e9;
        let accumulated_drift_urad = self.drift_rate_urad_per_s * elapsed_s;
        accumulated_drift_urad > self.expiration_threshold_urad
    }

    /// Get accumulated drift in microradians
    pub fn accumulated_drift_urad(&self, current_time_ns: u64) -> f64 {
        let elapsed_s = (current_time_ns - self.last_calibrated_ns) as f64 / 1e9;
        self.drift_rate_urad_per_s * elapsed_s
    }

    /// Update calibration with measured data
    pub fn update_from_measurement(
        &mut self,
        test_phase_shift: f64,
        measured_response: f64,
        current_time_ns: u64,
    ) {
        // Calculate correction factor
        self.correction_factor = test_phase_shift / measured_response.abs().max(0.001);
        self.last_calibrated_ns = current_time_ns;
    }

    /// Apply calibration correction to measured phase
    pub fn correct_phase(&self, measured_phase: f64) -> f64 {
        measured_phase * self.correction_factor
    }
}

impl Default for PhaseCalibration {
    fn default() -> Self {
        Self::new()
    }
}

/// Dark count calibration data
#[derive(Debug, Clone)]
pub struct DarkCountCalibration {
    pub baseline_rate_hz: f64, // λ_dark at reference temperature
    pub last_calibrated_ns: u64,
    pub temperature_coefficient_per_k: f64,
    pub expiration_threshold_pct: f64,
}

impl DarkCountCalibration {
    /// Create with default values
    pub fn new() -> Self {
        Self {
            baseline_rate_hz: 1000.0,
            last_calibrated_ns: 0,
            temperature_coefficient_per_k: 0.0001, // 0.01%/K
            expiration_threshold_pct: 10.0,
        }
    }

    /// Check if calibration has expired
    pub fn is_expired(&self, current_time_ns: u64, temperature_drift_k: f64) -> bool {
        let pct_change = self.temperature_coefficient_per_k * 100.0 * temperature_drift_k;
        pct_change.abs() > self.expiration_threshold_pct
    }

    /// Get current dark count rate
    pub fn current_rate_hz(&self, temperature_drift_k: f64) -> f64 {
        let factor = 1.0 + self.temperature_coefficient_per_k * temperature_drift_k;
        self.baseline_rate_hz * factor.max(0.001)
    }

    /// Subtract dark counts from measurement
    pub fn subtract_dark_counts(
        &self,
        measured_count: u32,
        integration_time_ns: u64,
        temperature_drift_k: f64,
    ) -> u32 {
        let dark_count_rate = self.current_rate_hz(temperature_drift_k);
        let integration_time_s = integration_time_ns as f64 / 1e9;
        let expected_dark = (dark_count_rate * integration_time_s) as u32;

        measured_count.saturating_sub(expected_dark)
    }

    /// Update calibration with measured data
    pub fn update_from_measurement(
        &mut self,
        measured_dark_count: u32,
        integration_time_ns: u64,
        current_time_ns: u64,
    ) {
        let integration_time_s = integration_time_ns as f64 / 1e9;
        self.baseline_rate_hz = measured_dark_count as f64 / integration_time_s;
        self.last_calibrated_ns = current_time_ns;
    }
}

impl Default for DarkCountCalibration {
    fn default() -> Self {
        Self::new()
    }
}

/// Adaptive calibration manager
pub struct AdaptiveCalibrationManager {
    phase_calib: PhaseCalibration,
    dark_count_calib: DarkCountCalibration,
    state: CalibrationState,
    next_recalibration_ns: u64,
}

impl AdaptiveCalibrationManager {
    /// Create new calibration manager
    pub fn new() -> Self {
        Self {
            phase_calib: PhaseCalibration::new(),
            dark_count_calib: DarkCountCalibration::new(),
            state: CalibrationState::Operational,
            next_recalibration_ns: 0,
        }
    }

    /// Check if recalibration needed and return action
    pub fn check_recalibration_needed(
        &mut self,
        current_time_ns: u64,
        temperature_drift_k: f64,
    ) -> CalibrationState {
        if self.phase_calib.is_expired(current_time_ns) {
            self.state = CalibrationState::PhaseCalibrationNeeded;
        } else if self
            .dark_count_calib
            .is_expired(current_time_ns, temperature_drift_k)
        {
            self.state = CalibrationState::DarkCountCalibrationNeeded;
        } else {
            self.state = CalibrationState::Operational;
        }

        self.state
    }

    /// Update phase calibration
    pub fn update_phase_calibration(
        &mut self,
        test_phase_shift: f64,
        measured_response: f64,
        current_time_ns: u64,
    ) {
        self.phase_calib.update_from_measurement(
            test_phase_shift,
            measured_response,
            current_time_ns,
        );
    }

    /// Update dark count calibration
    pub fn update_dark_count_calibration(
        &mut self,
        measured_dark_count: u32,
        integration_time_ns: u64,
        current_time_ns: u64,
    ) {
        self.dark_count_calib.update_from_measurement(
            measured_dark_count,
            integration_time_ns,
            current_time_ns,
        );
    }

    /// Get phase correction for measurement
    pub fn correct_phase(&self, measured_phase: f64) -> f64 {
        self.phase_calib.correct_phase(measured_phase)
    }

    /// Get dark count subtraction
    pub fn subtract_dark_counts(
        &self,
        measured_count: u32,
        integration_time_ns: u64,
        temperature_drift_k: f64,
    ) -> u32 {
        self.dark_count_calib.subtract_dark_counts(
            measured_count,
            integration_time_ns,
            temperature_drift_k,
        )
    }
}

impl Default for AdaptiveCalibrationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Real-time fidelity estimator
pub struct FidelityEstimator {
    fidelity_history: VecDeque<f64>,
    measurement_threshold: f64,
}

impl FidelityEstimator {
    /// Create new fidelity estimator
    pub fn new() -> Self {
        Self {
            fidelity_history: VecDeque::with_capacity(50),
            measurement_threshold: 0.85,
        }
    }

    /// Record fidelity measurement
    pub fn record_measurement(&mut self, fidelity: f64) {
        self.fidelity_history.push_back(fidelity.clamp(0.0, 1.0));
        if self.fidelity_history.len() > 50 {
            self.fidelity_history.pop_front();
        }
    }

    /// Get average fidelity
    pub fn average_fidelity(&self) -> f64 {
        if self.fidelity_history.is_empty() {
            return 1.0;
        }

        let sum: f64 = self.fidelity_history.iter().sum();
        sum / self.fidelity_history.len() as f64
    }

    /// Check if fidelity needs attention
    pub fn needs_correction(&self) -> bool {
        self.average_fidelity() < self.measurement_threshold
    }

    /// Get fidelity status
    pub fn fidelity_status(&self) -> &'static str {
        match self.average_fidelity() {
            f if f > 0.95 => "Excellent",
            f if f > 0.90 => "Good",
            f if f > 0.85 => "Acceptable",
            _ => "Poor",
        }
    }
}

impl Default for FidelityEstimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measurement_result_phase() {
        let result = MeasurementResult {
            i_quadrature: 1.0,
            q_quadrature: 1.0,
            variance: 0.5,
            timestamp_ns: 0,
        };
        let phase = result.phase();
        assert!((phase - std::f64::consts::PI / 4.0).abs() < 0.01);
    }

    #[test]
    fn test_feedback_controller_latency() {
        let mut controller = FeedbackController::new();
        assert_eq!(controller.measured_loop_latency_ns(), 200);
    }

    #[test]
    fn test_adaptive_measurement_selection() {
        let selector = AdaptiveMeasurementSelector::new();

        // Strong signal, good frequency, time available
        let mode = selector.select_mode(2.0, 500.0, 2_000_000);
        assert_eq!(mode, MeasurementMode::Heterodyne);

        // Strong signal, poor frequency, avoid heterodyne
        let mode = selector.select_mode(2.0, 20_000.0, 2_000_000);
        assert_eq!(mode, MeasurementMode::Homodyne);

        // Very tight deadline
        let mode = selector.select_mode(1.0, 1_000.0, 300_000);
        assert_eq!(mode, MeasurementMode::DirectDetection);
    }

    #[test]
    fn test_phase_calibration_expiration() {
        let mut calib = PhaseCalibration::new();
        calib.last_calibrated_ns = 0;

        // Not expired at 200s (drift: 200 µrad < 300 µrad)
        assert!(!calib.is_expired(200_000_000_000));

        // Expired at 350s (drift: 350 µrad > 300 µrad)
        assert!(calib.is_expired(350_000_000_000));
    }

    #[test]
    fn test_phase_correction_application() {
        let mut calib = PhaseCalibration::new();
        calib.update_from_measurement(std::f64::consts::PI / 4.0, 0.7, 0);

        let measured = 0.5;
        let corrected = calib.correct_phase(measured);
        assert!(corrected > measured);
    }

    #[test]
    fn test_dark_count_subtraction() {
        let mut dark_calib = DarkCountCalibration::new();
        dark_calib.baseline_rate_hz = 1000.0;

        let measured = 100u32; // 100 photons in 100 ms
        let integration_time_ns = 100_000_000; // 100 ms

        let true_count = dark_calib.subtract_dark_counts(measured, integration_time_ns, 0.0);
        assert!(true_count < measured);
    }

    #[test]
    fn test_fidelity_estimator() {
        let mut estimator = FidelityEstimator::new();
        estimator.record_measurement(0.95);
        estimator.record_measurement(0.96);
        estimator.record_measurement(0.94);

        assert!(!estimator.needs_correction());
        assert_eq!(estimator.fidelity_status(), "Excellent");
    }

    #[test]
    fn test_calibration_state_transitions() {
        let mut manager = AdaptiveCalibrationManager::new();
        let mut phase_calib = PhaseCalibration::new();
        phase_calib.last_calibrated_ns = 0;
        manager.phase_calib = phase_calib;

        // Initially operational
        let state = manager.check_recalibration_needed(100_000_000_000, 0.0);
        assert_eq!(state, CalibrationState::PhaseCalibrationNeeded);
    }
}
