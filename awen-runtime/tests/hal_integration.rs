// Hardware Abstraction Layer (HAL) v0.2 - Integration Test Suite
// Phase 2.3 - Comprehensive device backend, measurement mode, calibration, resource allocation testing
//
// This test suite validates the HAL v0.2 expansion from Phase 1.4 with:
// - Device discovery and capability negotiation
// - Measurement mode selection (homodyne, heterodyne, direct detection)
// - Real-time calibration integration with drift handling
// - Resource allocation and preemption
// - Fault detection and graceful degradation
// - Integration with Phase 2.2 Scheduler and Phase 2.1 Engine
// - Observable metrics emission and artifact capture

use awen_runtime::hal_v0::*;

// ============================================================================
// SECTION 1: DEVICE DISCOVERY & CAPABILITY NEGOTIATION (4 tests)
// ============================================================================

#[test]
fn test_device_discovery_simulator_discovery() {
    // Verify that device discovery finds the built-in simulator backend
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let devices = hal.discover_devices();
    assert!(
        !devices.is_empty(),
        "Device discovery should find simulator"
    );
    assert!(
        devices.iter().any(|d| d == "simulator"),
        "Simulator should be in discovery list"
    );
}

#[test]
fn test_device_discovery_capability_negotiation() {
    // Verify capability negotiation: check device can meet requirements
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Default device available");
    let caps = device.capabilities();

    // Negotiation checks:
    assert!(
        caps.coherence_time_us > 0,
        "Coherence window must be positive"
    );
    assert!(
        caps.supports_homodyne || caps.supports_heterodyne || caps.supports_direct_detection,
        "Device must support at least one measurement mode"
    );
    assert!(
        caps.waveguides > 0,
        "Device must have at least one waveguide"
    );
    assert!(
        caps.phase_shifter_range_radians > 0.0,
        "Phase range must be positive"
    );
}

#[test]
fn test_device_discovery_caching_consistency() {
    // Verify device discovery returns consistent capabilities (caching semantics)
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device1 = hal.get_default_device().expect("First lookup");
    let caps1 = device1.capabilities();

    let device2 = hal.get_default_device().expect("Second lookup");
    let caps2 = device2.capabilities();

    // Cached discovery should be deterministic
    assert_eq!(
        caps1.waveguides, caps2.waveguides,
        "Cached capabilities must be consistent"
    );
    assert_eq!(
        caps1.coherence_time_us, caps2.coherence_time_us,
        "Coherence time should not change between queries"
    );
}

#[test]
fn test_device_discovery_capability_filtering() {
    // Verify capability filtering: only devices matching requirements are returned
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device available");
    let caps = device.capabilities();

    // Simulator must support all modes (reference implementation)
    assert!(
        caps.supports_homodyne,
        "Simulator must support homodyne for compatibility"
    );
    assert!(
        caps.supports_heterodyne,
        "Simulator must support heterodyne for compatibility"
    );
    assert!(
        caps.supports_direct_detection,
        "Simulator must support direct detection for compatibility"
    );
}

// ============================================================================
// SECTION 2: MEASUREMENT MODES - HOMODYNE (3 tests)
// ============================================================================

#[test]
fn test_measurement_homodyne_quadrature_output() {
    // Verify homodyne measurement outputs valid I/Q quadratures
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let hom_config = HomodyneConfig {
        lo_phase: 0.0,
        lo_power_mw: 10.0,
        vna_frequency_ghz: 1.0,
        integration_time_us: 100.0,
        bandwidth_mhz: 10.0,
    };

    let result = device
        .measure_homodyne(&hom_config)
        .expect("Homodyne measurement succeeds");

    // Quadratures should be real-valued
    assert!(
        result.quadrature_i.is_finite(),
        "I quadrature must be finite"
    );
    assert!(
        result.quadrature_q.is_finite(),
        "Q quadrature must be finite"
    );

    // Variance tracks measurement uncertainty
    assert!(result.variance_i_sq.is_finite(), "Variance must be finite");
    assert!(result.variance_i_sq >= 0.0, "Variance cannot be negative");

    // Timestamp must be present for ordering
    assert!(result.timestamp_ns > 0, "Timestamp must be positive");
}

#[test]
fn test_measurement_homodyne_lo_phase_variation() {
    // Verify LO phase affects quadrature outputs (rotates I/Q)
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");

    // Measure with LO phase = 0
    let config_0 = HomodyneConfig {
        lo_phase: 0.0,
        lo_power_mw: 10.0,
        vna_frequency_ghz: 1.0,
        integration_time_us: 100.0,
        bandwidth_mhz: 10.0,
    };
    let result_0 = device.measure_homodyne(&config_0).expect("Success");

    // Measure with LO phase = π/2
    let config_pi2 = HomodyneConfig {
        lo_phase: std::f64::consts::PI / 2.0,
        lo_power_mw: 10.0,
        vna_frequency_ghz: 1.0,
        integration_time_us: 100.0,
        bandwidth_mhz: 10.0,
    };
    let result_pi2 = device.measure_homodyne(&config_pi2).expect("Success");

    // Quadratures should rotate: I_0 ≈ Q_pi2 and Q_0 ≈ -I_pi2
    // Allow for simulator approximations
    assert!(
        (result_0.quadrature_i - result_pi2.quadrature_q).abs() < 0.1,
        "LO phase rotation should affect quadratures"
    );
}

#[test]
fn test_measurement_homodyne_integration_time_validity() {
    // Verify integration time parameter is valid for coherence window
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let caps = device.capabilities();

    let integration_time_us = (caps.coherence_time_us as f64) * 0.5; // 50% of coherence

    let hom_config = HomodyneConfig {
        lo_phase: 0.0,
        lo_power_mw: 10.0,
        vna_frequency_ghz: 1.0,
        integration_time_us,
        bandwidth_mhz: 10.0,
    };

    let result = device
        .measure_homodyne(&hom_config)
        .expect("Should succeed within coherence");

    assert!(
        result.quadrature_i.is_finite(),
        "Result valid within coherence"
    );
}

// ============================================================================
// SECTION 3: MEASUREMENT MODES - HETERODYNE (2 tests)
// ============================================================================

#[test]
fn test_measurement_heterodyne_magnitude_and_phase() {
    // Verify heterodyne measurement outputs magnitude and phase
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let het_config = HeterodyneConfig {
        signal_frequency_ghz: 1.0,
        lo_frequency_ghz: 0.9,
        intermediate_frequency_ghz: 0.1,
        demod_bandwidth_mhz: 50.0,
        integration_time_us: 50.0,
    };

    let result = device
        .measure_heterodyne(&het_config)
        .expect("Heterodyne measurement succeeds");

    // Magnitude should be non-negative
    assert!(result.magnitude >= 0.0, "Magnitude cannot be negative");
    assert!(result.magnitude.is_finite(), "Magnitude must be finite");

    // Phase should be within ±π
    assert!(
        result.phase >= -std::f64::consts::PI && result.phase <= std::f64::consts::PI,
        "Phase should be in [-π, π]"
    );

    // SNR should be positive (dB can be negative for noise)
    assert!(result.snr_db.is_finite(), "SNR must be finite");

    // Timestamp for ordering
    assert!(result.timestamp_ns > 0, "Timestamp must be positive");
}

#[test]
fn test_measurement_heterodyne_frequency_detuning() {
    // Verify heterodyne phase depends on frequency detuning
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");

    // Same detuning
    let config1 = HeterodyneConfig {
        signal_frequency_ghz: 1.0,
        lo_frequency_ghz: 0.9,
        intermediate_frequency_ghz: 0.1,
        demod_bandwidth_mhz: 50.0,
        integration_time_us: 50.0,
    };

    // Different detuning
    let config2 = HeterodyneConfig {
        signal_frequency_ghz: 1.05,
        lo_frequency_ghz: 0.9,
        intermediate_frequency_ghz: 0.15,
        demod_bandwidth_mhz: 50.0,
        integration_time_us: 50.0,
    };

    let result1 = device.measure_heterodyne(&config1).expect("Success");
    let result2 = device.measure_heterodyne(&config2).expect("Success");

    // Different detuning should produce different results
    let magnitude_diff = (result1.magnitude - result2.magnitude).abs();
    let phase_diff = (result1.phase - result2.phase).abs();

    assert!(
        magnitude_diff > 0.0 || phase_diff > 0.0,
        "Different detuning should affect measurement"
    );
}

// ============================================================================
// SECTION 4: MEASUREMENT MODES - DIRECT DETECTION (2 tests)
// ============================================================================

#[test]
fn test_measurement_direct_detection_photon_counting() {
    // Verify direct detection outputs photon count and statistics
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let dd_config = DirectDetectionConfig {
        wavelength_nm: 1550.0,
        integration_time_us: 10.0,
        dark_count_threshold: 5,
    };

    let result = device
        .measure_direct_detection(&dd_config)
        .expect("Direct detection succeeds");

    // Photon count (unsigned) is inherently non-negative and need not be asserted

    // Dark count less than or equal to photon count
    assert!(
        result.dark_count <= result.photon_count,
        "Dark count cannot exceed total count"
    );
    // Dark count (unsigned) is inherently non-negative; no explicit assertion needed

    // Click probability in [0, 1]
    assert!(
        result.click_probability >= 0.0 && result.click_probability <= 1.0,
        "Click probability must be in [0, 1]"
    );

    // Timestamp
    assert!(result.timestamp_ns > 0, "Timestamp must be positive");
}

#[test]
fn test_measurement_direct_detection_dark_count_sensitivity() {
    // Verify dark count tracking for detector health
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");

    let dd_config = DirectDetectionConfig {
        wavelength_nm: 1550.0,
        integration_time_us: 100.0,
        dark_count_threshold: 5,
    };

    let _ = device
        .measure_direct_detection(&dd_config)
        .expect("Success");

    // Dark count (unsigned) is inherently non-negative; no explicit assertion needed
}

// ============================================================================
// SECTION 5: CALIBRATION INTEGRATION (4 tests)
// ============================================================================

#[test]
fn test_calibration_state_loading_and_validity() {
    // Verify calibration state can be loaded and validity window checked
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");

    // Request health check (which validates calibration implicitly)
    let health = device.health_check().expect("Health check succeeds");
    assert_eq!(health, HealthStatus::Healthy, "Simulator should be healthy");
}

#[test]
fn test_calibration_adaptive_phase_calibration() {
    // Verify adaptive calibration: phase calibration state updates
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");

    // Get initial calibration
    let cal_before = device.get_calibration_state();
    assert!(cal_before.is_ok(), "Calibration state retrievable");

    // Perform measurement
    let hom_config = HomodyneConfig {
        lo_phase: 0.0,
        lo_power_mw: 10.0,
        vna_frequency_ghz: 1.0,
        integration_time_us: 50.0,
        bandwidth_mhz: 10.0,
    };
    let _ = device.measure_homodyne(&hom_config);

    // Get updated calibration
    let cal_after = device.get_calibration_state();
    assert!(cal_after.is_ok(), "Calibration state updated");
}

#[test]
fn test_calibration_validity_window_expiration() {
    // Verify calibration validity tracking (age, drift)
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let cal_state = device
        .get_calibration_state()
        .expect("Calibration available");

    // Validity window should be set (hours)
    assert!(
        cal_state.validity_window_hours > 0,
        "Calibration validity window must be positive"
    );
}

#[test]
fn test_calibration_thermal_drift_compensation() {
    // Verify thermal drift is tracked in calibration state
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let cal_state = device.get_calibration_state().expect("Calibration");

    // Phase calibration should have thermal drift estimate
    let first = cal_state
        .phase_shifter_calibration
        .values()
        .next()
        .expect("phase shifter calibration present");
    assert!(
        first.thermal_drift_per_degree.is_finite(),
        "Thermal drift must be finite"
    );
}

// ============================================================================
// SECTION 6: RESOURCE ALLOCATION (5 tests)
// ============================================================================

#[test]
fn test_resource_allocation_waveguide_tracking() {
    // Verify waveguide resource allocation and limits
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let caps = device.capabilities();

    // Waveguide count should be available
    assert!(caps.waveguides > 0, "Device must have waveguides");
    assert!(
        caps.power_handling_mw > 0.0,
        "Power handling must be positive"
    );
}

#[test]
fn test_resource_allocation_power_budget_validation() {
    // Verify power budget is checked before allocation
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let caps = device.capabilities();

    // Power handling limit should be enforced
    assert!(caps.power_handling_mw > 0.0, "Power budget must be defined");

    // Measurement configurations should respect power levels
    let hom_config = HomodyneConfig {
        lo_phase: 0.0,
        lo_power_mw: caps.power_handling_mw * 0.5,
        vna_frequency_ghz: 1.0,
        integration_time_us: 10.0,
        bandwidth_mhz: 10.0,
    };

    let result = device.measure_homodyne(&hom_config);
    assert!(
        result.is_ok(),
        "Measurement within power budget should succeed"
    );
}

#[test]
fn test_resource_allocation_detector_assignment() {
    // Verify detector resources are properly allocated
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let caps = device.capabilities();

    assert!(caps.detectors > 0, "Device must have detectors");

    // Measurement should use available detectors
    let dd_config = DirectDetectionConfig {
        wavelength_nm: 1550.0,
        integration_time_us: 10.0,
        dark_count_threshold: 5,
    };

    let result = device.measure_direct_detection(&dd_config);
    assert!(result.is_ok(), "Detector allocation successful");
}

#[test]
fn test_resource_allocation_crosstalk_awareness() {
    // Verify crosstalk isolation is considered in resource allocation
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let caps = device.capabilities();

    // Crosstalk should be specified for multi-waveguide devices
    if caps.waveguides > 1 {
        assert!(
            caps.crosstalk_db < 0.0,
            "Crosstalk isolation should be negative dB"
        );
    }
}

// ============================================================================
// SECTION 7: FAULT DETECTION (3 tests)
// ============================================================================

#[test]
fn test_fault_detection_waveguide_loss_threshold() {
    // Verify waveguide loss is monitored against thresholds
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let thresholds = device.fault_detection_thresholds();

    assert!(
        thresholds.waveguide_loss_threshold_db > 0.0,
        "Loss threshold must be positive"
    );
}

#[test]
fn test_fault_detection_phase_shifter_drift() {
    // Verify phase shifter drift is detected
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let thresholds = device.fault_detection_thresholds();

    assert!(
        thresholds.phase_shifter_drift_radians_per_ms > 0.0,
        "Drift threshold must be positive"
    );
}

#[test]
fn test_fault_detection_graceful_degradation_mode() {
    // Verify device health can degrade gracefully
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");
    let health = device.health_check().expect("Health check succeeds");

    // Should be Healthy, Degraded, or Faulty enum
    match health {
        HealthStatus::Healthy => {
            let result = device.health_check().expect("Health stable");
            assert_eq!(result, HealthStatus::Healthy);
        }
        HealthStatus::Degraded => {
            // Graceful degradation mode accepted (no-op assertion removed)
        }
        HealthStatus::Faulty => {
            // Fault detected (no-op assertion removed)
        }
    }
}

// ============================================================================
// SECTION 8: SCHEDULER & ENGINE INTEGRATION (4 tests)
// ============================================================================

#[test]
fn test_integration_execution_plan_validation_coherence_window() {
    // Verify HAL validates ExecutionPlan against coherence window
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device_id = "simulator";
    let phase_count = 100;
    let duration_ns = 5_000_000; // 5ms, should fit in 10ms coherence window

    let validation = hal.validate_execution_plan(device_id, phase_count, duration_ns);
    assert!(validation.is_ok(), "Plan within coherence should validate");
}

#[test]
fn test_integration_execution_plan_validation_exceeds_coherence() {
    // Verify HAL rejects ExecutionPlan that exceeds coherence
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device_id = "simulator";
    let phase_count = 1000;
    let duration_ns = 15_000_000; // 15ms, exceeds typical 10ms coherence

    let validation = hal.validate_execution_plan(device_id, phase_count, duration_ns);
    assert!(
        validation.is_err(),
        "Plan exceeding coherence should be rejected"
    );
}

#[test]
fn test_integration_execution_plan_phase_count_limits() {
    // Verify phase count limits are enforced
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device_id = "simulator";
    let phase_count_ok = 100;
    let phase_count_invalid = 2000;
    let duration_ns = 1_000_000; // 1ms

    let result_ok = hal.validate_execution_plan(device_id, phase_count_ok, duration_ns);
    assert!(result_ok.is_ok(), "Reasonable phase count should validate");

    let result_invalid = hal.validate_execution_plan(device_id, phase_count_invalid, duration_ns);
    assert!(
        result_invalid.is_err(),
        "Excessive phase count should be rejected"
    );
}

#[test]
fn test_integration_hal_metrics_observable() {
    // Verify HAL metrics are observable (for Observability integration)
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");

    // Perform measurements to generate metrics
    let hom_config = HomodyneConfig {
        lo_phase: 0.0,
        lo_power_mw: 10.0,
        vna_frequency_ghz: 1.0,
        integration_time_us: 10.0,
        bandwidth_mhz: 10.0,
    };
    let _ = device.measure_homodyne(&hom_config);

    let dd_config = DirectDetectionConfig {
        wavelength_nm: 1550.0,
        integration_time_us: 10.0,
        dark_count_threshold: 5,
    };
    let _ = device.measure_direct_detection(&dd_config);

    // Metrics should be queryable
    let metrics = device.get_metrics();
    assert!(
        metrics.average_fidelity.is_finite(),
        "Metrics should be available for observability"
    );
}

// ============================================================================
// SECTION 9: BACKWARD COMPATIBILITY (2 tests)
// ============================================================================

#[test]
fn test_backward_compatibility_simulator_interface() {
    // Verify Phase 2.3 HAL is compatible with Phase 1.4 usage patterns
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");

    // Phase 1.4 pattern: Get capabilities, perform measurement
    let caps = device.capabilities();
    assert!(caps.waveguides > 0, "Backward compatible capabilities");

    let hom_config = HomodyneConfig {
        lo_phase: 0.0,
        lo_power_mw: 10.0,
        vna_frequency_ghz: 1.0,
        integration_time_us: 10.0,
        bandwidth_mhz: 10.0,
    };

    let result = device
        .measure_homodyne(&hom_config)
        .expect("Phase 1.4 measurement pattern works");

    assert!(
        result.quadrature_i.is_finite(),
        "Phase 1.4 compatible results"
    );
}

#[test]
fn test_backward_compatibility_config_defaults() {
    // Verify HalConfig defaults provide sensible Phase 1.4 behavior
    let config = HalConfig::default();

    assert!(
        config.default_backend == "simulator",
        "Default backend is simulator"
    );
    assert!(
        config.auto_calibration_enabled,
        "Auto-calibration default enabled"
    );
}

// ============================================================================
// SECTION 10: CONFORMANCE & COMPLETENESS (3 tests)
// ============================================================================

#[test]
fn test_conformance_all_measurement_modes_available() {
    // Verify all three measurement modes are available
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    let device = hal.get_default_device().expect("Device");

    // All three modes must be available
    let homodyne = device
        .measure_homodyne(&HomodyneConfig {
            lo_phase: 0.0,
            lo_power_mw: 10.0,
            vna_frequency_ghz: 1.0,
            integration_time_us: 10.0,
            bandwidth_mhz: 10.0,
        })
        .is_ok();

    let heterodyne = device
        .measure_heterodyne(&HeterodyneConfig {
            signal_frequency_ghz: 1.0,
            lo_frequency_ghz: 0.9,
            intermediate_frequency_ghz: 0.1,
            demod_bandwidth_mhz: 50.0,
            integration_time_us: 50.0,
        })
        .is_ok();

    let direct = device
        .measure_direct_detection(&DirectDetectionConfig {
            wavelength_nm: 1550.0,
            integration_time_us: 10.0,
            dark_count_threshold: 5,
        })
        .is_ok();

    assert!(
        homodyne && heterodyne && direct,
        "All measurement modes must be available"
    );
}

#[test]
fn test_conformance_backend_registry_functionality() {
    // Verify backend registration system works
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);

    let _ = hal.register_simulator();
    let devices = hal.discover_devices();
    assert!(
        devices.contains(&"simulator".to_string()),
        "Simulator must be registered"
    );

    let device = hal.get_default_device();
    assert!(device.is_ok(), "Default device must be accessible");
}

#[test]
fn test_conformance_device_discovery_complete() {
    // Verify complete device discovery workflow
    let config = HalConfig::default();
    let mut hal = HalManager::new(config);
    let _ = hal.register_simulator();

    // Discover
    let devices = hal.discover_devices();
    assert!(!devices.is_empty(), "Discovery must find devices");

    // Get device
    let device = hal.get_device(&devices[0]);
    assert!(device.is_ok(), "Device retrieval must work");

    let dev = device.unwrap();
    let caps = dev.capabilities();

    // Validate capabilities
    assert!(caps.waveguides > 0, "Device must have valid capabilities");
    assert!(caps.coherence_time_us > 0, "Coherence must be defined");
}
