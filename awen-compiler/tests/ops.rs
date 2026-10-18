//! Operator library: reference vs direct formulas, photonic engine vs reference.

use awen_compiler::noise::{NoiseRng, PhotonicNoiseContract};
use awen_compiler::ops::*;
use std::collections::BTreeMap;

fn rand(rng: &mut NoiseRng, shape: &[usize]) -> Array {
    let n = shape.iter().product();
    Array::new(
        shape.to_vec(),
        (0..n).map(|_| rng.uniform() * 2.0 - 1.0).collect(),
    )
    .unwrap()
}

fn close(a: &Array, b: &Array, tol: f64) {
    assert_eq!(a.shape, b.shape);
    for (x, y) in a.data.iter().zip(&b.data) {
        assert!((x - y).abs() <= tol, "{x} vs {y}");
    }
}

#[test]
fn conv2d_matches_direct_loops() {
    let mut rng = NoiseRng::new(1);
    let x = rand(&mut rng, &[2, 6, 5]);
    let w = rand(&mut rng, &[3, 2, 3, 2]);
    let b = rand(&mut rng, &[3]);
    let y = conv2d(&mut ExactEngine::default(), &x, &w, Some(&b), 2, 1).unwrap();
    let (ho, wo) = ((6 + 2 - 3) / 2 + 1, (5 + 2 - 2) / 2 + 1);
    assert_eq!(y.shape, vec![3, ho, wo]);
    for o in 0..3 {
        for oy in 0..ho {
            for ox in 0..wo {
                let mut acc = b.data[o];
                for c in 0..2 {
                    for dy in 0..3 {
                        for dx in 0..2 {
                            let (iy, ix) = ((oy * 2 + dy) as isize - 1, (ox * 2 + dx) as isize - 1);
                            if (0..6).contains(&iy) && (0..5).contains(&ix) {
                                acc += x.data[(c * 6 + iy as usize) * 5 + ix as usize]
                                    * w.data[((o * 2 + c) * 3 + dy) * 2 + dx];
                            }
                        }
                    }
                }
                assert!((y.data[(o * ho + oy) * wo + ox] - acc).abs() < 1e-12);
            }
        }
    }
}

#[test]
fn dft_round_trips_and_matches_parseval() {
    let mut rng = NoiseRng::new(2);
    let xr = rand(&mut rng, &[3, 16]);
    let xi = rand(&mut rng, &[3, 16]);
    let mut e = ExactEngine::default();
    let (fr, fi) = dft(&mut e, &xr, &xi, false).unwrap();
    let (br, bi) = dft(&mut e, &fr, &fi, true).unwrap();
    close(&br, &xr, 1e-12);
    close(&bi, &xi, 1e-12);
    let energy = |r: &Array, i: &Array| {
        r.data
            .iter()
            .zip(&i.data)
            .map(|(a, b)| a * a + b * b)
            .sum::<f64>()
    };
    assert!((energy(&fr, &fi) / 16.0 - energy(&xr, &xi)).abs() < 1e-9);
}

#[test]
fn spectral_conv_equals_circular_convolution() {
    let mut rng = NoiseRng::new(3);
    let n = 12;
    let x = rand(&mut rng, &[2, n]);
    let kernel = rand(&mut rng, &[1, n]);
    let mut e = ExactEngine::default();
    let (hr, hi) = dft(&mut e, &kernel, &Array::zeros(vec![1, n]), false).unwrap();
    let hr = Array::new(vec![n], hr.data).unwrap();
    let hi = Array::new(vec![n], hi.data).unwrap();
    let y = spectral_conv1d(&mut e, &x, &hr, &hi).unwrap();
    for b in 0..2 {
        for t in 0..n {
            let direct: f64 = (0..n)
                .map(|s| x.data[b * n + s] * kernel.data[(t + n - s) % n])
                .sum();
            assert!((y.data[b * n + t] - direct).abs() < 1e-10);
        }
    }
}

#[test]
fn causal_attention_first_row_attends_only_to_itself() {
    let mut rng = NoiseRng::new(4);
    let x = rand(&mut rng, &[5, 8]);
    let wq = rand(&mut rng, &[4, 8]);
    let wk = rand(&mut rng, &[4, 8]);
    let wv = rand(&mut rng, &[4, 8]);
    let wo = rand(&mut rng, &[8, 4]);
    let mut e = ExactEngine::default();
    let y = attention(&mut e, &x, &wq, &wk, &wv, &wo, true).unwrap();
    // Row 0 output = Wo · (Wv x0).
    let mut e2 = ExactEngine::default();
    let x0 = Array::new(vec![1, 8], x.data[..8].to_vec()).unwrap();
    let v0 = e2.gemm(&x0, &wv).unwrap();
    let want = e2.gemm(&v0, &wo).unwrap();
    for j in 0..8 {
        assert!((y.data[j] - want.data[j]).abs() < 1e-12);
    }
    assert_eq!(e.calls, 6);
}

fn request(
    op: &str,
    inputs: Vec<(&str, Array)>,
    contract: Option<PhotonicNoiseContract>,
) -> OpRequest {
    OpRequest {
        op: op.into(),
        inputs: inputs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        params: BTreeMap::new(),
        contract,
        tile_k: 32,
        seed: 3,
    }
}

#[test]
fn photonic_engine_is_exact_under_the_ideal_contract_and_noisy_by_default() {
    let mut rng = NoiseRng::new(5);
    let inputs = vec![
        ("x", rand(&mut rng, &[6, 40])),
        ("w1", rand(&mut rng, &[24, 40])),
        ("b1", rand(&mut rng, &[24])),
        ("w2", rand(&mut rng, &[40, 24])),
        ("b2", rand(&mut rng, &[40])),
    ];
    let reference = run_op(&request("ffn", inputs.clone(), None)).unwrap();
    let ideal = run_op(&request(
        "ffn",
        inputs.clone(),
        Some(PhotonicNoiseContract::ideal()),
    ))
    .unwrap();
    let noisy = run_op(&request(
        "ffn",
        inputs.clone(),
        Some(PhotonicNoiseContract::default()),
    ))
    .unwrap();
    close(&ideal.outputs["y"], &reference.outputs["y"], 1e-10);
    assert!(noisy.outputs["y"].data != reference.outputs["y"].data);
    assert_eq!(noisy.gemm_calls, 2);
    assert_eq!(noisy.photonic_macs, (6 * 24 * 40 + 6 * 40 * 24) as u64);
    assert!(noisy.fidelity.starts_with("simulated"));
    assert!(reference.fidelity.starts_with("exact"));
    let again = run_op(&request(
        "ffn",
        inputs,
        Some(PhotonicNoiseContract::default()),
    ))
    .unwrap();
    assert_eq!(again.outputs["y"], noisy.outputs["y"], "seeded");
}

#[test]
fn op_table_is_complete_and_never_claims_measurement() {
    let table = op_table();
    assert_eq!(table.len(), 12);
    assert!(table.iter().all(|o| o.level < OpLevel::HardwareMeasured));
    let mut names: Vec<_> = table.iter().map(|o| o.name.clone()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 12);
    assert!(run_op(&request("teleport", vec![], None)).is_err());
}

#[test]
fn bad_inputs_are_rejected() {
    let x = Array {
        shape: vec![2, 2],
        data: vec![1.0, f64::NAN, 0.0, 1.0],
    };
    assert!(run_op(&request("gemm", vec![("x", x.clone()), ("w", x)], None)).is_err());
    assert!(Array::new(vec![2, 3], vec![0.0; 5]).is_err());
    let mut rng = NoiseRng::new(6);
    let missing = request("gemm", vec![("x", rand(&mut rng, &[2, 2]))], None);
    assert!(run_op(&missing)
        .unwrap_err()
        .to_string()
        .contains("needs input"));
}
