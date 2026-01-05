use awen_runtime::chokepoint::{ExecContext, NonBypassableGateway, PhotonicOp};
use awen_runtime::ExecutionChokepoint;
use serde_json::json;

#[test]
fn chokepoint_accepts_valid_op() {
    let gateway = NonBypassableGateway::new();
    let op = PhotonicOp {
        op_id: "op1".into(),
        op_type: "classical:splitter".into(),
        targets: vec!["wg1".into()],
        params: Some(json!({"ratio": 0.5})),
        calibration_handle: None,
    };
    let ctx = ExecContext {
        run_id: "run1".into(),
        timestamp_ns: 12345,
    };

    let res = gateway.execute(&op, &ctx);
    assert!(res.ok, "Execution should succeed for a valid op");
}
#[test]
fn gateway_accepts_valid_op() {
    let gw = NonBypassableGateway::new();
    let op = PhotonicOp {
        op_id: "op-123".into(),
        op_type: "quantum:beam_splitter".into(),
        targets: vec!["wg0".into(), "wg1".into()],
        params: Some(json!({})),
        calibration_handle: None,
    };

    let ctx = ExecContext {
        run_id: "run-1".into(),
        timestamp_ns: 1_700_000_000_000_000_000,
    };

    let res = gw.execute(&op, &ctx);
    assert!(
        res.ok,
        "gateway should accept a valid op: {:?}",
        res.details
    );
}

#[test]
fn gateway_rejects_missing_op_id() {
    let gw = NonBypassableGateway::new();
    let op = PhotonicOp {
        op_id: "".into(),
        op_type: "classical:splitter".into(),
        targets: vec!["wg0".into()],
        params: Some(json!({})),
        calibration_handle: None,
    };

    let ctx = ExecContext {
        run_id: "run-2".into(),
        timestamp_ns: 1,
    };
    let res = gw.execute(&op, &ctx);
    assert!(!res.ok, "gateway should reject ops missing op_id");
}
