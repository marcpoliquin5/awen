//! Opting in to the hardware assumption set never changes a default silently.
use awen_compiler::assumptions::{Corner, HardwareAssumptions, HARDWARE_ASSUMPTIONS_VERSION};
use awen_compiler::capability::DeviceCapabilities;
use awen_compiler::cost::{CostModelInputs, ParameterSource};
use awen_compiler::ir::GemmShape;
use awen_compiler::noise::{photonic_gemm, PhotonicNoiseContract};

fn bundled() -> HardwareAssumptions {
    HardwareAssumptions::bundled().expect("bundled assumption set parses")
}

fn synthetic(rin: &str, bits: &str, bandwidth: &str, unit: &str) -> String {
    format!(
        "version: {HARDWARE_ASSUMPTIONS_VERSION}\ndate: \"2026-10-18\"\nparameters:\n  laser:\n    rin: {{low: null, typical: {rin}, high: null, unit: dB/Hz, source: null}}\n  dac:\n    weight_dac_bits: {{low: null, typical: {bits}, high: null, unit: bits, source: null}}\n  photodetector:\n    bandwidth: {{low: null, typical: {bandwidth}, high: null, unit: {unit}, source: null}}\n  mzi:\n    crosstalk: {{low: null, typical: null, high: null, unit: dB, source: null}}\n  modulator:\n    energy_per_symbol: {{low: null, typical: 250, high: null, unit: fJ, source: null}}\n  shot_noise:\n    e: {{kind: sweep, unit: J, values: {{4: 1.0}}}}\n"
    )
}

#[test]
fn mismatched_units_and_fractional_bits_are_rejected() {
    let wrong_unit = HardwareAssumptions::parse(&synthetic("-140", "6", "20", "MHz")).unwrap();
    assert!(PhotonicNoiseContract::default()
        .with_assumptions(&wrong_unit, Corner::Typical)
        .is_err());
    let fractional = HardwareAssumptions::parse(&synthetic("-140", "6.5", "20", "GHz")).unwrap();
    assert!(PhotonicNoiseContract::default()
        .with_assumptions(&fractional, Corner::Typical)
        .is_err());
}

#[test]
fn defaults_are_unchanged_by_the_v0_set_at_every_corner() {
    let set = bundled();
    let default = PhotonicNoiseContract::default();
    let x: Vec<f64> = (0..4 * 64).map(|i| (i as f64 * 0.37).sin()).collect();
    let w: Vec<f64> = (0..8 * 64).map(|i| (i as f64 * 0.11).cos()).collect();
    let shape = GemmShape { m: 4, k: 64, n: 8 };
    let (y_default, _) = photonic_gemm(&x, &w, shape, &default, 16, 7).unwrap();
    for corner in Corner::ALL {
        let (contract, report) = default.with_assumptions(&set, corner).unwrap();
        assert_eq!(contract, default, "{corner:?}");
        let applied: Vec<_> = report.applied().map(|f| f.field.as_str()).collect();
        assert_eq!(
            applied,
            if corner == Corner::Typical {
                vec!["dac_bits"]
            } else {
                vec![]
            }
        );
        let (y, _) = photonic_gemm(&x, &w, shape, &contract, 16, 7).unwrap();
        assert_eq!(y, y_default);
    }
}

#[test]
fn values_present_in_the_file_are_applied_with_unit_conversion() {
    let set = HardwareAssumptions::parse(&synthetic("-140", "6", "20", "GHz")).unwrap();
    let (contract, report) = PhotonicNoiseContract::default()
        .with_assumptions(&set, Corner::Typical)
        .unwrap();
    assert_eq!(contract.rin_db_per_hz, Some(-140.0));
    assert_eq!(contract.dac_bits, Some(6));
    assert_eq!(contract.detection_bandwidth_hz, 20.0e9);
    assert_eq!(
        contract.crosstalk_db,
        PhotonicNoiseContract::default().crosstalk_db
    );
    assert_eq!(report.applied().count(), 3);
    // a disabled term stays disabled
    let ideal = PhotonicNoiseContract::ideal();
    let (still_ideal, _) = ideal.with_assumptions(&set, Corner::Typical).unwrap();
    assert_eq!(still_ideal.rin_db_per_hz, None);
    assert_eq!(still_ideal.dac_bits, None);
}

#[test]
fn cost_model_records_the_assumption_set_and_keeps_numbers() {
    let caps = DeviceCapabilities::pace_like_128();
    let model = CostModelInputs::from_capabilities(&caps, ParameterSource::Assumed);
    let (opted, report) = model
        .clone()
        .with_assumptions(&bundled(), Corner::Typical)
        .unwrap();
    assert_eq!(report.applied().count(), 0);
    assert_eq!(
        opted.modulator_energy_pj_per_sample,
        model.modulator_energy_pj_per_sample
    );
    assert_eq!(opted.provenance.len(), model.provenance.len() + 1);
    let last = opted.provenance.last().unwrap();
    assert!(
        last.reference.starts_with(HARDWARE_ASSUMPTIONS_VERSION),
        "{}",
        last.reference
    );
    let mut without = opted.clone();
    without.provenance.pop();
    assert_eq!(without, model);

    let set = HardwareAssumptions::parse(&synthetic("-140", "6", "20", "GHz")).unwrap();
    let (opted, report) = model.with_assumptions(&set, Corner::Typical).unwrap();
    assert_eq!(opted.modulator_energy_pj_per_sample, 0.25); // 250 fJ
    assert_eq!(report.applied().count(), 1);
}
