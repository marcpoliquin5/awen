//! Loader for awen-spec/hardware_assumptions.yaml: versions, units and nulls are checked.
use awen_compiler::assumptions::{
    Corner, HardwareAssumptions, SourceType, HARDWARE_ASSUMPTIONS_VERSION,
};

fn bundled() -> HardwareAssumptions {
    HardwareAssumptions::bundled().expect("bundled assumption set parses")
}

#[test]
fn bundled_set_parses_ranges_only() {
    let set = bundled();
    assert_eq!(set.version, HARDWARE_ASSUMPTIONS_VERSION);
    assert_eq!(
        set.value("dac.weight_dac_bits", "bits", Corner::Typical)
            .unwrap(),
        Some(8.0)
    );
    assert_eq!(
        set.value("dac.weight_dac_bits", "bits", Corner::Low)
            .unwrap(),
        None
    );
    assert_eq!(
        set.value("laser.rin", "dB/Hz", Corner::Typical).unwrap(),
        None
    );
    assert_eq!(
        set.value("system.pace_enob", "bits", Corner::Typical)
            .unwrap(),
        Some(7.61)
    );
    let dac = &set.parameters["dac.weight_dac_bits"];
    assert_eq!(
        (dac.source.as_deref(), dac.source_type),
        (Some("hua2025"), Some(SourceType::Measured))
    );
    assert_eq!(set.parameters["laser.rin"].source_type, None);
    // sweeps, keyed and point entries and the channel rule are not ranges
    for name in [
        "shot_noise.shot_noise_energy_per_detection_J",
        "adc.published_designs",
        "channels.rule",
    ] {
        assert!(!set.parameters.contains_key(name), "{name}");
    }
}

#[test]
fn bundled_text_is_the_spec_file() {
    let on_disk = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../awen-spec/hardware_assumptions.yaml"
    ))
    .unwrap();
    assert_eq!(HardwareAssumptions::parse(&on_disk).unwrap(), bundled());
}

#[test]
fn units_versions_and_values_are_checked() {
    let set = bundled();
    assert!(set
        .value("dac.weight_dac_bits", "pJ", Corner::Typical)
        .is_err());
    assert!(set
        .value("dac.not_a_parameter", "bits", Corner::Typical)
        .is_err());
    let doc = |version: &str, typical: &str| {
        format!("version: {version}\ndate: \"2026-10-18\"\nparameters:\n  laser:\n    rin: {{low: null, typical: {typical}, high: null, unit: dB/Hz}}\n")
    };
    let ok = HardwareAssumptions::parse(&doc(HARDWARE_ASSUMPTIONS_VERSION, "-140")).unwrap();
    assert_eq!(
        ok.value("laser.rin", "dB/Hz", Corner::Typical).unwrap(),
        Some(-140.0)
    );
    assert!(HardwareAssumptions::parse(&doc("awen.hardware-assumptions.v9", "-140")).is_err());
    assert!(HardwareAssumptions::parse(&doc(HARDWARE_ASSUMPTIONS_VERSION, "loud")).is_err());
    assert!(HardwareAssumptions::parse("version: [").is_err());
}

#[test]
fn source_type_comes_from_the_entry_else_its_source() {
    let doc = |rin_extra: &str| {
        format!("version: {HARDWARE_ASSUMPTIONS_VERSION}\ndate: \"2026-10-18\"\nsources:\n  v1: {{type: vendor}}\nparameters:\n  laser:\n    rin: {{low: null, typical: -140, high: null, unit: dB/Hz, source: v1{rin_extra}}}\n")
    };
    let rin = |set: HardwareAssumptions| set.parameters["laser.rin"].source_type;
    assert_eq!(
        rin(HardwareAssumptions::parse(&doc("")).unwrap()),
        Some(SourceType::Vendor)
    );
    let own = HardwareAssumptions::parse(&doc(", source_type: estimate")).unwrap();
    assert_eq!(rin(own), Some(SourceType::Estimate));
    assert!(HardwareAssumptions::parse(&doc(", source_type: rumor")).is_err());
}
