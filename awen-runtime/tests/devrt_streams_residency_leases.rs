use awen_compiler::{
    execute_kernel_reference, KernelAttributes, KernelData, KernelKind, KernelRequest,
    KernelTensor, AWENBLAS_VERSION,
};
use awen_runtime::devrt::lease::{Budget, LeaseManager, LeaseMode};
use awen_runtime::devrt::residency::{ResidencyTracker, HOST};
use awen_runtime::devrt::stream::Stream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn gemm(id: &str, m: usize, k: usize, n: usize, seed: f64) -> KernelRequest {
    let lhs: Vec<f64> = (0..m * k).map(|i| (i as f64 + seed).sin()).collect();
    let rhs: Vec<f64> = (0..k * n).map(|i| (i as f64 * 0.5 + seed).cos()).collect();
    KernelRequest {
        version: AWENBLAS_VERSION.to_string(),
        id: id.into(),
        kind: KernelKind::Gemm,
        inputs: vec![
            KernelTensor::real("lhs", vec![m, k], lhs),
            KernelTensor::real("rhs", vec![k, n], rhs),
        ],
        attributes: KernelAttributes::default(),
        accuracy: awen_compiler::ir::AccuracyContract {
            max_abs_error: 1e-9,
            max_rel_error: 1e-9,
            minimum_effective_bits: None,
        },
        calibration_inputs: Vec::new(),
    }
}

fn naive(request: &KernelRequest) -> Vec<f64> {
    let (m, k) = (request.inputs[0].shape[0], request.inputs[0].shape[1]);
    let n = request.inputs[1].shape[1];
    let (KernelData::Real(a), KernelData::Real(b)) =
        (&request.inputs[0].data, &request.inputs[1].data)
    else {
        panic!("real inputs")
    };
    let mut out = vec![0.0; m * n];
    for i in 0..m {
        for j in 0..n {
            out[i * n + j] = (0..k).map(|p| a[i * k + p] * b[p * n + j]).sum();
        }
    }
    out
}

#[test]
fn stream_runs_work_in_submission_order() {
    let stream = Stream::new("sim0");
    let log = Arc::new(Mutex::new(Vec::new()));
    for index in 0..200 {
        let log = log.clone();
        stream
            .submit(move || {
                log.lock().unwrap().push(index);
                Ok(())
            })
            .unwrap();
    }
    stream.synchronize().unwrap();
    assert_eq!(*log.lock().unwrap(), (0..200).collect::<Vec<_>>());
}

#[test]
fn event_orders_work_across_two_simulated_devices() {
    // Device A produces a GEMM result; device B must not consume it before
    // A's event fires, even though B's work is submitted first.
    let producer = Stream::new("sim0");
    let consumer = Stream::new("sim1");
    let slot: Arc<Mutex<Option<Vec<f64>>>> = Arc::new(Mutex::new(None));
    let gate = Arc::new(Mutex::new(()));
    let held = gate.lock().unwrap();

    let request = gemm("a", 8, 16, 4, 0.3);
    let expected = naive(&request);

    {
        let gate = gate.clone();
        let slot = slot.clone();
        producer
            .submit(move || {
                let _wait = gate.lock().unwrap();
                let result = execute_kernel_reference(&request)?;
                let KernelData::Real(values) = &result.outputs[0].data else {
                    anyhow::bail!("expected real")
                };
                *slot.lock().unwrap() = Some(values.clone());
                Ok(())
            })
            .unwrap();
    }
    let produced = producer.record_event().unwrap();
    consumer.wait_event(&produced).unwrap();
    let observed: Arc<Mutex<Option<Vec<f64>>>> = Arc::new(Mutex::new(None));
    {
        let slot = slot.clone();
        let observed = observed.clone();
        consumer
            .submit(move || {
                *observed.lock().unwrap() = slot.lock().unwrap().clone();
                Ok(())
            })
            .unwrap();
    }
    let consumed = consumer.record_event().unwrap();
    assert!(!consumed.wait_timeout(Duration::from_millis(50)));
    drop(held);
    consumed.synchronize().unwrap();
    let got = observed
        .lock()
        .unwrap()
        .clone()
        .expect("consumer saw output");
    assert_eq!(got.len(), expected.len());
    for (g, e) in got.iter().zip(&expected) {
        assert!((g - e).abs() < 1e-12);
    }
    assert_eq!(producer.device(), "sim0");
    assert_eq!(consumer.device(), "sim1");
}

#[test]
fn stream_errors_are_sticky_and_propagate_through_events() {
    let stream = Stream::new("sim0");
    let ran = Arc::new(Mutex::new(0));
    stream.submit(|| anyhow::bail!("injected fault")).unwrap();
    {
        let ran = ran.clone();
        stream
            .submit(move || {
                *ran.lock().unwrap() += 1;
                Ok(())
            })
            .unwrap();
    }
    let event = stream.record_event().unwrap();
    let error = event.synchronize().unwrap_err().to_string();
    assert!(error.contains("injected fault"));
    assert_eq!(*ran.lock().unwrap(), 0);

    let other = Stream::new("sim1");
    other.wait_event(&event).unwrap();
    assert!(other.synchronize().is_err());
}

#[test]
fn residency_evicts_lru_respects_pins_and_counts_transfers() {
    let mut tracker = ResidencyTracker::new();
    tracker.add_device("sim0", 100);
    tracker.add_device("sim1", 100);
    for id in ["a", "b", "c"] {
        tracker.allocate_host(id, 40).unwrap();
    }
    tracker.make_resident("a", "sim0").unwrap();
    tracker.make_resident("b", "sim0").unwrap();
    tracker.make_resident("a", "sim0").unwrap(); // touch a; b becomes LRU
    let evicted = tracker.make_resident("c", "sim0").unwrap();
    assert_eq!(evicted, vec!["b".to_string()]);
    assert_eq!(tracker.location("b"), Some(HOST));
    assert_eq!(tracker.used("sim0"), 80);

    tracker.set_pinned("a", true).unwrap();
    tracker.set_pinned("c", true).unwrap();
    assert!(tracker.make_resident("b", "sim0").is_err());

    tracker.make_resident("a", "sim1").unwrap();
    tracker.make_resident("a", HOST).unwrap();
    assert_eq!(tracker.stats.host_to_device_bytes, 120);
    assert_eq!(tracker.stats.device_to_host_bytes, 80);
    assert_eq!(tracker.stats.device_to_device_bytes, 40);
    assert_eq!(tracker.stats.evictions, 1);

    tracker.allocate_host("huge", 1000).unwrap();
    assert!(tracker.make_resident("huge", "sim1").is_err());
    assert!(tracker.allocate_host("a", 1).is_err());
    tracker.free("huge").unwrap();
    assert!(tracker.make_resident("huge", "sim1").is_err());
}

#[test]
fn leases_enforce_exclusivity_expiry_and_budgets() {
    let mut leases = LeaseManager::new();
    let shared_a = leases
        .acquire("alice", "sim0", LeaseMode::Shared, 10, Budget::default())
        .unwrap();
    let _shared_b = leases
        .acquire("bob", "sim0", LeaseMode::Shared, 5, Budget::default())
        .unwrap();
    assert!(leases
        .acquire("carol", "sim0", LeaseMode::Exclusive, 5, Budget::default())
        .is_err());
    assert_eq!(leases.advance(5).len(), 1);
    leases.release(shared_a).unwrap();
    let exclusive = leases
        .acquire(
            "carol",
            "sim0",
            LeaseMode::Exclusive,
            3,
            Budget {
                max_operations: Some(10),
                max_energy_pj: Some(5.0),
                max_wall_ns: None,
            },
        )
        .unwrap();
    assert!(leases
        .acquire("dave", "sim0", LeaseMode::Shared, 3, Budget::default())
        .is_err());
    leases.charge(exclusive, 6, 2.5, 100).unwrap();
    assert!(leases.charge(exclusive, 5, 0.0, 0).is_err());
    assert!(leases.charge(exclusive, 1, 3.0, 0).is_err());
    assert!(leases.charge(exclusive, 1, f64::NAN, 0).is_err());
    let usage = &leases.lease(exclusive).unwrap().usage;
    assert_eq!(usage.operations, 6);
    assert_eq!(usage.energy_pj, 2.5);
    leases.advance(3);
    assert!(leases.lease(exclusive).is_none());
    assert!(leases.charge(exclusive, 1, 0.0, 0).is_err());
    assert!(leases
        .acquire("e", "sim0", LeaseMode::Shared, 0, Budget::default())
        .is_err());
}
