use awen_runtime::hal::hardware::{
    DetectorAdcDriver, HardwareDescription, MicroringBankDriver, MziMeshDriver, C64,
};
use awen_runtime::hal::Device;
use awen_runtime::schema::Validator;
use serde_json::{json, Value};
use std::f64::consts::PI;

const SCHEMA: &str = include_str!("../../awen-spec/schemas/awen_hardware_description.v1.json");
const FIXTURE: &str = include_str!("../../awen-spec/fixtures/hardware_reference_simulator.v1.json");

fn validator() -> Validator {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    awen_runtime::schema::compile(&schema).expect("schema compiles")
}

fn fixture() -> HardwareDescription {
    HardwareDescription::from_json(FIXTURE).unwrap()
}

#[test]
fn schema_accepts_fixture_covering_every_component_class_and_rejects_bad_documents() {
    let validator = validator();
    let document: Value = serde_json::from_str(FIXTURE).unwrap();
    assert!(validator.is_valid(&document));
    let description = fixture();
    assert_eq!(description.provenance_source(), Some("reference_simulator"));
    assert_eq!(
        description.classes(),
        vec![
            "adc",
            "dac",
            "detector",
            "diffractive",
            "laser",
            "memory",
            "microring_bank",
            "mzi_mesh",
            "quantum",
            "reservoir",
            "wdm"
        ]
    );

    let mutate = |f: &dyn Fn(&mut Value)| {
        let mut bad = document.clone();
        f(&mut bad);
        bad
    };
    let bad_documents = [
        mutate(&|d| d["components"][0]["class"] = json!("warp_drive")),
        mutate(&|d| d["components"][0]["modes"] = json!(1)),
        mutate(&|d| d["components"][0]["vendor_secret"] = json!(true)),
        mutate(&|d| {
            d.as_object_mut().unwrap().remove("provenance");
        }),
        mutate(&|d| d["provenance"]["source"] = json!("marketing")),
        mutate(&|d| d["components"][1]["self_coupling_r"] = json!(1.0)),
        mutate(&|d| d["components"][2]["crosstalk_db"] = json!(5.0)),
        mutate(&|d| d["components"][10]["detection_efficiency"] = json!(1.2)),
        mutate(&|d| d["components"] = json!([])),
        mutate(&|d| d["version"] = json!("awen.hardware-description.v0")),
    ];
    for (index, bad) in bad_documents.iter().enumerate() {
        assert!(
            !validator.is_valid(bad),
            "bad document {index} was accepted"
        );
    }
    assert!(HardwareDescription::from_json(&FIXTURE.replace(".v1\"", ".v0\"")).is_err());
}

fn mesh() -> MziMeshDriver {
    MziMeshDriver::from_component(fixture().component("mzi_mesh", "mesh0").unwrap()).unwrap()
}

#[test]
fn clements_mesh_is_unitary_when_lossless_and_conserves_power() {
    let lossless = {
        let mut component = fixture().component("mzi_mesh", "mesh0").unwrap().clone();
        component["insertion_loss_db_per_mzi"] = json!(0.0);
        MziMeshDriver::from_component(&component).unwrap()
    };
    assert_eq!(lossless.modes(), 4);
    assert_eq!(lossless.mzi_count(), 6); // N(N-1)/2 for Clements
    for k in 0..lossless.mzi_count() {
        lossless
            .set_param(&format!("mzi_{k}:theta"), 0.3 + k as f64)
            .unwrap();
        lossless
            .set_param(&format!("mzi_{k}:phi"), 1.1 * k as f64)
            .unwrap();
    }
    let u = lossless.transfer_matrix();
    let n = 4;
    for i in 0..n {
        for j in 0..n {
            let dot = (0..n).fold(C64::ZERO, |acc, r| {
                acc.plus(u[r * n + i].conj().times(u[r * n + j]))
            });
            let expected = if i == j { 1.0 } else { 0.0 };
            assert!((dot.re - expected).abs() < 1e-12 && dot.im.abs() < 1e-12);
        }
    }
    lossless.set_param("in_0:re", 0.6).unwrap();
    lossless.set_param("in_2:im", 0.8).unwrap();
    let total: f64 = (0..n)
        .map(|j| lossless.read_sensor(&format!("out_{j}:power")).unwrap())
        .sum();
    assert!((total - 1.0).abs() < 1e-12);
}

#[test]
fn single_mzi_splitting_ratio_and_insertion_loss_match_closed_form() {
    let component = json!({
        "class": "mzi_mesh", "id": "m2", "modes": 2, "topology": "clements",
        "phase_shifter": {"mechanism": "thermo_optic", "v_pi_v": 2.0, "bandwidth_mhz": 0.1},
        "insertion_loss_db_per_mzi": 0.5, "extinction_ratio_db": 30.0
    });
    let driver = MziMeshDriver::from_component(&component).unwrap();
    assert_eq!(driver.mzi_count(), 1);
    driver.set_param("in_0:re", 1.0).unwrap();
    let loss = 10f64.powf(-0.05);
    for theta in [0.0, PI / 3.0, PI / 2.0, PI] {
        driver.set_param("mzi_0:theta", theta).unwrap();
        let bar = driver.read_sensor("out_0:power").unwrap();
        let cross = driver.read_sensor("out_1:power").unwrap();
        assert!((bar - loss * (theta / 2.0).cos().powi(2)).abs() < 1e-12);
        assert!((cross - loss * (theta / 2.0).sin().powi(2)).abs() < 1e-12);
    }
    assert!(driver.set_param("mzi_1:theta", 0.0).is_err());
    assert!(driver.set_param("mzi_0:theta", f64::NAN).is_err());
    assert!(driver.read_sensor("out_2:power").is_err());
    let mut reck = component.clone();
    reck["topology"] = json!("reck");
    assert!(MziMeshDriver::from_component(&reck).is_err());
    let lossy = mesh();
    assert_eq!(lossy.capabilities()[0].name, "mzi_mesh");
}

#[test]
fn microring_transmission_matches_all_pass_formula_and_critical_coupling() {
    let driver = MicroringBankDriver::from_component(
        fixture().component("microring_bank", "rings0").unwrap(),
    )
    .unwrap();
    let (r, a): (f64, f64) = (0.95, 0.98);
    driver.set_param("ring_0:phase", 0.0).unwrap();
    let on = driver.read_sensor("ring_0:transmission").unwrap();
    assert!((on - (a - r).powi(2) / (1.0 - a * r).powi(2)).abs() < 1e-12);
    driver.set_param("ring_1:phase", PI).unwrap();
    let off = driver.read_sensor("ring_1:transmission").unwrap();
    assert!((off - (a + r).powi(2) / (1.0 + a * r).powi(2)).abs() < 1e-12);
    assert!(on < off && off <= 1.0);

    let mut critical = fixture()
        .component("microring_bank", "rings0")
        .unwrap()
        .clone();
    critical["round_trip_amplitude_a"] = json!(0.95);
    let critical = MicroringBankDriver::from_component(&critical).unwrap();
    assert!(critical.transmission(0.0).abs() < 1e-12);
    let mut lossless = ring_component();
    lossless["round_trip_amplitude_a"] = json!(1.0);
    let lossless = MicroringBankDriver::from_component(&lossless).unwrap();
    for phase in [0.0, 0.3, 2.0] {
        assert!((lossless.transmission(phase) - 1.0).abs() < 1e-12);
    }
    assert!(driver.read_sensor("ring_9:transmission").is_err());
}

fn ring_component() -> Value {
    fixture()
        .component("microring_bank", "rings0")
        .unwrap()
        .clone()
}

#[test]
fn detector_adc_chain_quantizes_offsets_and_saturates() {
    let description = fixture();
    let driver = DetectorAdcDriver::from_components(
        description.component("detector", "pd0").unwrap(),
        description.component("adc", "adc0").unwrap(),
    )
    .unwrap();
    assert_eq!(driver.max_code(), 255.0);
    // Dark current only: 10 nA * 1 kOhm = 10 uV -> code 0.
    assert_eq!(driver.read_sensor("pd_0:code").unwrap(), 0.0);
    assert!((driver.read_sensor("pd_0:current_a").unwrap() - 1e-8).abs() < 1e-20);
    // 0.5 mW * 1 A/W * 1 kOhm = 0.5 V (+10 uV) -> round(0.50001 * 255) = 128.
    driver.set_param("pd_0:power_w", 0.5e-3).unwrap();
    assert_eq!(driver.read_sensor("pd_0:code").unwrap(), 128.0);
    driver.set_param("pd_0:power_w", 5e-3).unwrap();
    assert_eq!(driver.read_sensor("pd_0:code").unwrap(), 255.0);
    driver.set_param("tia:ohm", 100.0).unwrap();
    assert_eq!(driver.read_sensor("pd_0:code").unwrap(), 128.0);
    let mut previous = -1.0;
    for step in 0..50 {
        driver
            .set_param("pd_1:power_w", step as f64 * 2e-4)
            .unwrap();
        let code = driver.read_sensor("pd_1:code").unwrap();
        assert!(code >= previous);
        previous = code;
    }
    assert!(driver.set_param("pd_0:power_w", -1.0).is_err());
    assert!(driver.read_sensor("pd_7:code").is_err());
    assert!(DetectorAdcDriver::from_components(
        description.component("adc", "adc0").unwrap(),
        description.component("detector", "pd0").unwrap(),
    )
    .is_err());
}
