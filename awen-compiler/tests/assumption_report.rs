//! Opting in to the hardware assumption set never changes a default silently.
use awen_compiler::assumptions::{
    Corner, HardwareAssumptions, SourceType, HARDWARE_ASSUMPTIONS_VERSION,
};
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
fn opting_in_changes_only_sourced_fields_and_never_the_default() {
    let set = bundled();
    let default = PhotonicNoiseContract::default();
    let x: Vec<f64> = (0..4 * 64).map(|i| (i as f64 * 0.37).sin()).collect();
    let w: Vec<f64> = (0..8 * 64).map(|i| (i as f64 * 0.11).cos()).collect();
    let shape = GemmShape { m: 4, k: 64, n: 8 };
    let (y_default, _) = photonic_gemm(&x, &w, shape, &default, 16, 7).unwrap();
    for corner in Corner::ALL {
        let (contract, report) = default.with_assumptions(&set, corner).unwrap();
        let applied: Vec<_> = report.applied().map(|f| f.field.as_str()).collect();
        if corner == Corner::Typical {
            // dac_bits 8 equals the default; photodetector.bandwidth (chen2016, typical only) is
            // the one opted-in value that differs from the default contract (D-042)
            assert_eq!(applied, vec!["dac_bits", "detection_bandwidth_hz"]);
            let mut expected = default.clone();
            expected.detection_bandwidth_hz = 67.0e9;
            assert_eq!(contract, expected);
            // a wider RIN integration bandwidth changes the noisy outputs at this corner
            let (y, _) = photonic_gemm(&x, &w, shape, &contract, 16, 7).unwrap();
            assert_ne!(y, y_default);
        } else {
            // laser.rin (vaskasi2022 bound, high corner only) equals the default -150 dB/Hz (D-044)
            let expected: Vec<&str> = if corner == Corner::High {
                vec!["rin_db_per_hz"]
            } else {
                Vec::new()
            };
            assert_eq!(applied, expected, "{corner:?}");
            assert_eq!(contract, default, "{corner:?}");
            let (y, _) = photonic_gemm(&x, &w, shape, &contract, 16, 7).unwrap();
            assert_eq!(y, y_default);
        }
    }
    // the default contract itself never reads the file
    assert_eq!(
        PhotonicNoiseContract::default().detection_bandwidth_hz,
        10.0e9
    );
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
    assert_eq!(last.source, ParameterSource::Assumed); // nothing applied

    let set = HardwareAssumptions::parse(&synthetic("-140", "6", "20", "GHz")).unwrap();
    let (opted, report) = model.with_assumptions(&set, Corner::Typical).unwrap();
    assert_eq!(opted.modulator_energy_pj_per_sample, 0.25); // 250 fJ
    assert_eq!(report.applied().count(), 1);
}

#[test]
fn applied_fields_carry_their_source_and_unapplied_ones_none() {
    let (_, report) = PhotonicNoiseContract::default()
        .with_assumptions(&bundled(), Corner::Typical)
        .unwrap();
    for field in &report.fields {
        let label = (field.source.as_deref(), field.source_type);
        if field.field == "dac_bits" {
            assert_eq!(label, (Some("hua2025"), Some(SourceType::Measured)));
        } else if field.field == "detection_bandwidth_hz" {
            assert_eq!(label, (Some("chen2016"), Some(SourceType::Measured)));
            assert!(field.from_assumptions);
            assert_eq!(field.value, 67.0e9);
        } else {
            assert_eq!(label, (None, None), "{}", field.field);
        }
    }
}

#[test]
fn cost_provenance_source_is_derived_from_the_applied_fields() {
    let model = CostModelInputs::from_capabilities(
        &DeviceCapabilities::pace_like_128(),
        ParameterSource::Assumed,
    );
    for (label, expected) in [
        ("source: m1", ParameterSource::Measured),
        ("source: v1", ParameterSource::VendorSpecified),
        (
            "source: m1, source_type: estimate",
            ParameterSource::Assumed,
        ),
        ("source: null", ParameterSource::Assumed),
    ] {
        let set = HardwareAssumptions::parse(&format!(
            "version: {HARDWARE_ASSUMPTIONS_VERSION}\ndate: \"2026-10-18\"\nsources:\n  m1: {{type: measured}}\n  v1: {{type: vendor}}\nparameters:\n  modulator:\n    energy_per_symbol: {{low: null, typical: 250, high: null, unit: fJ, {label}}}\n"
        ))
        .unwrap();
        let (opted, report) = model
            .clone()
            .with_assumptions(&set, Corner::Typical)
            .unwrap();
        assert_eq!(report.provenance_source(), expected, "{label}");
        assert_eq!(opted.provenance.last().unwrap().source, expected, "{label}");
    }
}

#[test]
fn mzi_extinction_ratio_is_stored_but_never_applied_as_crosstalk() {
    // D-043: demarinis2021 per-MZI extinction ratio is data only; `mzi.crosstalk` stays null
    let set = bundled();
    let expected = [
        (Corner::Low, 22.9),
        (Corner::Typical, 26.8),
        (Corner::High, 42.5),
    ];
    for (corner, er) in expected {
        assert_eq!(
            set.value("mzi.extinction_ratio", "dB", corner).unwrap(),
            Some(er)
        );
        assert_eq!(set.value("mzi.crosstalk", "dB", corner).unwrap(), None);
        let (contract, report) = PhotonicNoiseContract::default()
            .with_assumptions(&set, corner)
            .unwrap();
        assert_eq!(contract.crosstalk_db, Some(-35.0));
        let field = report
            .fields
            .iter()
            .find(|f| f.field == "crosstalk_db")
            .unwrap();
        assert!(!field.from_assumptions);
        assert_eq!(field.source, None);
    }
}

#[test]
fn laser_rin_bound_applies_only_at_the_high_corner_and_equals_the_default() {
    // D-044: vaskasi2022 states RIN below -150 dB/Hz up to 6 GHz, stored as laser.rin high only
    let set = bundled();
    let rin = &set.parameters["laser.rin"];
    assert_eq!(rin.source.as_deref(), Some("vaskasi2022"));
    for (corner, stored, applied) in [
        (Corner::Low, None, false),
        (Corner::Typical, None, false),
        (Corner::High, Some(-150.0), true),
    ] {
        assert_eq!(set.value("laser.rin", "dB/Hz", corner).unwrap(), stored);
        let (contract, report) = PhotonicNoiseContract::default()
            .with_assumptions(&set, corner)
            .unwrap();
        // the bound equals the built-in illustrative value, so no corner changes the number
        assert_eq!(contract.rin_db_per_hz, Some(-150.0));
        assert_eq!(
            contract.rin_db_per_hz,
            PhotonicNoiseContract::default().rin_db_per_hz
        );
        let field = report
            .fields
            .iter()
            .find(|f| f.field == "rin_db_per_hz")
            .unwrap();
        assert_eq!(field.from_assumptions, applied);
        assert_eq!(field.source.as_deref(), applied.then_some("vaskasi2022"));
    }
}
