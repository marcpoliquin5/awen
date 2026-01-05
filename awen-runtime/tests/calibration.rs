use awen_runtime::calibration::{
    basic_apply_to_params, basic_generate_default_state, BasicCalibrationState,
};
use serde_json::json;

#[test]
fn apply_calibration_scales_params() {
    let mut st: BasicCalibrationState = basic_generate_default_state();
    // override default factor for deterministic test
    st.scale_factors.insert("power".to_string(), 2.0);

    let params = json!({"power": 1.5, "duration": 10});
    let out = basic_apply_to_params(&st, Some(params)).unwrap();
    assert_eq!(out["power"].as_f64().unwrap(), 3.0);
    assert_eq!(out["duration"].as_i64().unwrap(), 10);
}
