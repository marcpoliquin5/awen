//! Independent oracles for the awenBLAS kernels listed in directive item C9:
//! complex and batched GEMM, FFT, low-rank, reservoir update, attention and
//! feed-forward primitives. Each oracle below is written from the textbook
//! definition, separately from `awenblas::kernels`.

use awen_compiler::ops::{self, Array, ExactEngine};
use awen_compiler::{
    execute_kernel_reference, execute_kernel_simulator, ComplexValue, KernelAttributes, KernelData,
    KernelKind, KernelRequest, KernelSimulatorOptions, KernelTensor, TargetBackend,
    AWENBLAS_VERSION,
};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
    fn vec(&mut self, n: usize) -> Vec<f64> {
        (0..n).map(|_| self.next()).collect()
    }
    fn cvec(&mut self, n: usize) -> Vec<ComplexValue> {
        (0..n)
            .map(|_| ComplexValue::new(self.next(), self.next()))
            .collect()
    }
}

fn request(kind: KernelKind, inputs: Vec<KernelTensor>) -> KernelRequest {
    KernelRequest {
        version: AWENBLAS_VERSION.to_string(),
        id: format!("oracle.{kind:?}").to_ascii_lowercase(),
        kind,
        inputs,
        attributes: KernelAttributes::default(),
        accuracy: awen_compiler::ir::AccuracyContract {
            max_abs_error: 0.05,
            max_rel_error: 0.05,
            minimum_effective_bits: None,
        },
        calibration_inputs: Vec::new(),
    }
}

fn real_out(request: &KernelRequest) -> (Vec<usize>, Vec<f64>) {
    let result = execute_kernel_reference(request).expect("reference");
    match &result.outputs[0].data {
        KernelData::Real(v) => (result.outputs[0].shape.clone(), v.clone()),
        KernelData::Complex(_) => panic!("expected real"),
    }
}

fn complex_out(request: &KernelRequest) -> (Vec<usize>, Vec<ComplexValue>) {
    let result = execute_kernel_reference(request).expect("reference");
    match &result.outputs[0].data {
        KernelData::Complex(v) => (result.outputs[0].shape.clone(), v.clone()),
        KernelData::Real(_) => panic!("expected complex"),
    }
}

fn max_abs(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

fn cmax_abs(a: &[ComplexValue], b: &[ComplexValue]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| (x.real - y.real).hypot(x.imaginary - y.imaginary))
        .fold(0.0, f64::max)
}

fn naive_mm(a: &[f64], b: &[f64], m: usize, k: usize, n: usize) -> Vec<f64> {
    let mut c = vec![0.0; m * n];
    for i in 0..m {
        for j in 0..n {
            for p in 0..k {
                c[i * n + j] += a[i * k + p] * b[p * n + j];
            }
        }
    }
    c
}

fn transpose(a: &[f64], rows: usize, cols: usize) -> Vec<f64> {
    let mut t = vec![0.0; rows * cols];
    for i in 0..rows {
        for j in 0..cols {
            t[j * rows + i] = a[i * cols + j];
        }
    }
    t
}

#[test]
fn complex_gemm_matches_real_block_embedding() {
    // (Ar + iAi)(Br + iBi) == [[Ar, -Ai], [Ai, Ar]] [[Br], [Bi]] as real GEMM.
    let (m, k, n) = (5, 7, 3);
    let mut rng = Lcg(1);
    let a = rng.cvec(m * k);
    let b = rng.cvec(k * n);
    let (shape, got) = complex_out(&request(
        KernelKind::ComplexGemm,
        vec![
            KernelTensor::complex("a", vec![m, k], a.clone()),
            KernelTensor::complex("b", vec![k, n], b.clone()),
        ],
    ));
    assert_eq!(shape, vec![m, n]);
    let mut big_a = vec![0.0; 2 * m * 2 * k];
    for i in 0..m {
        for p in 0..k {
            let z = a[i * k + p];
            big_a[i * 2 * k + p] = z.real;
            big_a[i * 2 * k + k + p] = -z.imaginary;
            big_a[(m + i) * 2 * k + p] = z.imaginary;
            big_a[(m + i) * 2 * k + k + p] = z.real;
        }
    }
    let mut big_b = vec![0.0; 2 * k * n];
    for p in 0..k {
        for j in 0..n {
            big_b[p * n + j] = b[p * n + j].real;
            big_b[(k + p) * n + j] = b[p * n + j].imaginary;
        }
    }
    let c = naive_mm(&big_a, &big_b, 2 * m, 2 * k, n);
    let expected: Vec<ComplexValue> = (0..m * n)
        .map(|idx| ComplexValue::new(c[idx], c[m * n + idx]))
        .collect();
    assert!(cmax_abs(&got, &expected) < 1e-12);
}

#[test]
fn batched_real_and_complex_gemm_match_per_batch_oracles() {
    let (batch, m, k, n) = (4, 3, 5, 2);
    let mut rng = Lcg(2);
    let a = rng.vec(batch * m * k);
    let b = rng.vec(batch * k * n);
    let (shape, got) = real_out(&request(
        KernelKind::BatchedGemm,
        vec![
            KernelTensor::real("a", vec![batch, m, k], a.clone()),
            KernelTensor::real("b", vec![batch, k, n], b.clone()),
        ],
    ));
    assert_eq!(shape, vec![batch, m, n]);
    let expected: Vec<f64> = (0..batch)
        .flat_map(|i| naive_mm(&a[i * m * k..][..m * k], &b[i * k * n..][..k * n], m, k, n))
        .collect();
    assert!(max_abs(&got, &expected) < 1e-12);

    let ca = rng.cvec(batch * m * k);
    let cb = rng.cvec(batch * k * n);
    let (shape, got) = complex_out(&request(
        KernelKind::BatchedComplexGemm,
        vec![
            KernelTensor::complex("a", vec![batch, m, k], ca.clone()),
            KernelTensor::complex("b", vec![batch, k, n], cb.clone()),
        ],
    ));
    assert_eq!(shape, vec![batch, m, n]);
    let mut expected = Vec::new();
    for bi in 0..batch {
        for i in 0..m {
            for j in 0..n {
                let (mut re, mut im) = (0.0, 0.0);
                for p in 0..k {
                    let x = ca[bi * m * k + i * k + p];
                    let y = cb[bi * k * n + p * n + j];
                    re += x.real * y.real - x.imaginary * y.imaginary;
                    im += x.real * y.imaginary + x.imaginary * y.real;
                }
                expected.push(ComplexValue::new(re, im));
            }
        }
    }
    assert!(cmax_abs(&got, &expected) < 1e-12);

    // Mismatched batch sizes and real inputs are rejected.
    assert!(execute_kernel_reference(&request(
        KernelKind::BatchedComplexGemm,
        vec![
            KernelTensor::complex("a", vec![2, m, k], rng.cvec(2 * m * k)),
            KernelTensor::complex("b", vec![3, k, n], rng.cvec(3 * k * n)),
        ],
    ))
    .is_err());
    assert!(execute_kernel_reference(&request(
        KernelKind::BatchedComplexGemm,
        vec![
            KernelTensor::real("a", vec![1, 1, 1], vec![1.0]),
            KernelTensor::real("b", vec![1, 1, 1], vec![1.0]),
        ],
    ))
    .is_err());
}

#[test]
fn fft_matches_naive_dft_parseval_and_inverse() {
    let mut rng = Lcg(3);
    for n in [1usize, 2, 8, 64, 12] {
        let x = rng.cvec(n);
        let (_, got) = complex_out(&request(
            KernelKind::Fft,
            vec![KernelTensor::complex("x", vec![n], x.clone())],
        ));
        let expected: Vec<ComplexValue> = (0..n)
            .map(|f| {
                let (mut re, mut im) = (0.0, 0.0);
                for (t, z) in x.iter().enumerate() {
                    let angle = -2.0 * std::f64::consts::PI * (f * t) as f64 / n as f64;
                    re += z.real * angle.cos() - z.imaginary * angle.sin();
                    im += z.real * angle.sin() + z.imaginary * angle.cos();
                }
                ComplexValue::new(re, im)
            })
            .collect();
        assert!(cmax_abs(&got, &expected) < 1e-9 * n as f64, "n={n}");
        let energy_t: f64 = x.iter().map(|z| z.real.powi(2) + z.imaginary.powi(2)).sum();
        let energy_f: f64 = got
            .iter()
            .map(|z| z.real.powi(2) + z.imaginary.powi(2))
            .sum();
        assert!((energy_f / n as f64 - energy_t).abs() < 1e-9 * energy_t.max(1.0));
        let mut inverse = request(
            KernelKind::Fft,
            vec![KernelTensor::complex("X", vec![n], got)],
        );
        inverse.attributes.inverse = true;
        let (_, back) = complex_out(&inverse);
        assert!(cmax_abs(&back, &x) < 1e-12, "inverse n={n}");
    }
}

#[test]
fn low_rank_gemm_equals_dense_product_with_reconstructed_matrix() {
    let (m, k, n, r) = (6, 8, 5, 2);
    let mut rng = Lcg(4);
    let a = rng.vec(m * k);
    let u = rng.vec(k * r);
    let v = rng.vec(n * r);
    let mut req = request(
        KernelKind::LowRankGemm,
        vec![
            KernelTensor::real("a", vec![m, k], a.clone()),
            KernelTensor::real("u", vec![k, r], u.clone()),
            KernelTensor::real("v", vec![n, r], v.clone()),
        ],
    );
    req.attributes.rank = r;
    let (shape, got) = real_out(&req);
    assert_eq!(shape, vec![m, n]);
    let w = naive_mm(&u, &transpose(&v, n, r), k, r, n); // U V^T, rank r
    assert!(max_abs(&got, &naive_mm(&a, &w, m, k, n)) < 1e-12);
    req.attributes.rank = 3;
    assert!(execute_kernel_reference(&req).is_err());
}

#[test]
fn reservoir_step_matches_leaky_tanh_update() {
    let size = 6;
    let mut rng = Lcg(5);
    let state = rng.vec(size);
    let w = rng.vec(size * size);
    let w_in = rng.vec(size);
    let mut req = request(
        KernelKind::ReservoirStep,
        vec![
            KernelTensor::real("s", vec![size], state.clone()),
            KernelTensor::real("w", vec![size, size], w.clone()),
            KernelTensor::real("w_in", vec![size, 1], w_in.clone()),
        ],
    );
    req.attributes.scale = 0.7; // external scalar drive u
    req.attributes.leakage = 0.3;
    let (_, got) = real_out(&req);
    let expected: Vec<f64> = (0..size)
        .map(|i| {
            let pre: f64 =
                w_in[i] * 0.7 + (0..size).map(|j| w[i * size + j] * state[j]).sum::<f64>();
            0.7 * state[i] + 0.3 * pre.tanh()
        })
        .collect();
    assert!(max_abs(&got, &expected) < 1e-12);
}

fn sdpa(
    q: &[f64],
    k: &[f64],
    v: &[f64],
    dims: (usize, usize, usize, usize),
    scale: f64,
) -> Vec<f64> {
    let (m, n, d, dv) = dims;
    let mut out = vec![0.0; m * dv];
    for i in 0..m {
        let logits: Vec<f64> = (0..n)
            .map(|j| scale * (0..d).map(|p| q[i * d + p] * k[j * d + p]).sum::<f64>())
            .collect();
        let peak = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let weights: Vec<f64> = logits.iter().map(|l| (l - peak).exp()).collect();
        let total: f64 = weights.iter().sum();
        for c in 0..dv {
            out[i * dv + c] = (0..n).map(|j| weights[j] / total * v[j * dv + c]).sum();
        }
    }
    out
}

#[test]
fn attention_primitives_match_softmax_oracle_and_ops_library() {
    let (m, n, d, dv) = (4, 6, 8, 3);
    let mut rng = Lcg(6);
    let q = rng.vec(m * d);
    let k = rng.vec(n * d);
    let v = rng.vec(n * dv);
    let scale = 1.0 / (d as f64).sqrt();
    let mut req = request(
        KernelKind::ScaledDotProductAttention,
        vec![
            KernelTensor::real("q", vec![m, d], q.clone()),
            KernelTensor::real("k", vec![n, d], k.clone()),
            KernelTensor::real("v", vec![n, dv], v.clone()),
        ],
    );
    req.attributes.scale = scale;
    let (shape, got) = real_out(&req);
    assert_eq!(shape, vec![m, dv]);
    assert!(max_abs(&got, &sdpa(&q, &k, &v, (m, n, d, dv), scale)) < 1e-12);

    // Softmax rows sum to one: V = ones gives ones; huge logits stay finite.
    let mut ones = req.clone();
    ones.inputs[2] = KernelTensor::real("v", vec![n, 1], vec![1.0; n]);
    ones.attributes.scale = 1e3;
    let (_, out) = real_out(&ones);
    assert!(out.iter().all(|x| (x - 1.0).abs() < 1e-12));

    // The existing AttentionScores kernel is scale * Q K^T.
    let mut scores = request(
        KernelKind::AttentionScores,
        vec![
            KernelTensor::real("q", vec![m, d], q.clone()),
            KernelTensor::real("k", vec![n, d], k.clone()),
        ],
    );
    scores.attributes.scale = scale;
    let (_, s) = real_out(&scores);
    let expected: Vec<f64> = naive_mm(&q, &transpose(&k, n, d), m, d, n)
        .into_iter()
        .map(|x| x * scale)
        .collect();
    assert!(max_abs(&s, &expected) < 1e-12);

    // Cross-check against the op library's attention with identity projections.
    let t = 5;
    let x = rng.vec(t * d);
    let mut eye = vec![0.0; d * d];
    for i in 0..d {
        eye[i * d + i] = 1.0;
    }
    let eye = Array::new(vec![d, d], eye).unwrap();
    let ops_out = ops::attention(
        &mut ExactEngine::default(),
        &Array::new(vec![t, d], x.clone()).unwrap(),
        &eye,
        &eye,
        &eye,
        &eye,
        false,
    )
    .unwrap();
    let mut self_attn = request(
        KernelKind::ScaledDotProductAttention,
        vec![
            KernelTensor::real("q", vec![t, d], x.clone()),
            KernelTensor::real("k", vec![t, d], x.clone()),
            KernelTensor::real("v", vec![t, d], x.clone()),
        ],
    );
    self_attn.attributes.scale = scale;
    let (_, blas_out) = real_out(&self_attn);
    assert!(max_abs(&blas_out, &ops_out.data) < 1e-12);
}

#[test]
fn feed_forward_matches_gelu_oracle_and_ops_library() {
    let (m, d, h, o) = (3, 4, 10, 2);
    let mut rng = Lcg(7);
    let x = rng.vec(m * d);
    let w1 = rng.vec(d * h);
    let b1 = rng.vec(h);
    let w2 = rng.vec(h * o);
    let b2 = rng.vec(o);
    let req = request(
        KernelKind::FeedForward,
        vec![
            KernelTensor::real("x", vec![m, d], x.clone()),
            KernelTensor::real("w1", vec![d, h], w1.clone()),
            KernelTensor::real("b1", vec![h], b1.clone()),
            KernelTensor::real("w2", vec![h, o], w2.clone()),
            KernelTensor::real("b2", vec![o], b2.clone()),
        ],
    );
    let (shape, got) = real_out(&req);
    assert_eq!(shape, vec![m, o]);
    let gelu = |z: f64| {
        let c = (2.0 / std::f64::consts::PI).sqrt();
        0.5 * z * (1.0 + (c * (z + 0.044715 * z.powi(3))).tanh())
    };
    let mut hidden = naive_mm(&x, &w1, m, d, h);
    for (i, value) in hidden.iter_mut().enumerate() {
        *value = gelu(*value + b1[i % h]);
    }
    let mut expected = naive_mm(&hidden, &w2, m, h, o);
    for (i, value) in expected.iter_mut().enumerate() {
        *value += b2[i % o];
    }
    assert!(max_abs(&got, &expected) < 1e-12);

    // ops::ffn uses x · Wᵀ weights.
    let ops_out = ops::ffn(
        &mut ExactEngine::default(),
        &Array::new(vec![m, d], x).unwrap(),
        &Array::new(vec![h, d], transpose(&w1, d, h)).unwrap(),
        &Array::new(vec![h], b1).unwrap(),
        &Array::new(vec![o, h], transpose(&w2, h, o)).unwrap(),
        &Array::new(vec![o], b2).unwrap(),
    )
    .unwrap();
    assert!(max_abs(&got, &ops_out.data) < 1e-12);

    let mut bad = req.clone();
    bad.inputs[2] = KernelTensor::real("b1", vec![h + 1], vec![0.0; h + 1]);
    assert!(execute_kernel_reference(&bad).is_err());
}

#[test]
fn simulator_tracks_reference_for_new_kernels_and_noise_increases_error() {
    let mut rng = Lcg(8);
    let (m, d, h, o) = (4, 6, 8, 3);
    let ffn = request(
        KernelKind::FeedForward,
        vec![
            KernelTensor::real("x", vec![m, d], rng.vec(m * d)),
            KernelTensor::real("w1", vec![d, h], rng.vec(d * h)),
            KernelTensor::real("b1", vec![h], rng.vec(h)),
            KernelTensor::real("w2", vec![h, o], rng.vec(h * o)),
            KernelTensor::real("b2", vec![o], rng.vec(o)),
        ],
    );
    let mut attn = request(
        KernelKind::ScaledDotProductAttention,
        vec![
            KernelTensor::real("q", vec![m, d], rng.vec(m * d)),
            KernelTensor::real("k", vec![5, d], rng.vec(5 * d)),
            KernelTensor::real("v", vec![5, 2], rng.vec(10)),
        ],
    );
    attn.attributes.scale = 0.4;
    let cgemm = request(
        KernelKind::BatchedComplexGemm,
        vec![
            KernelTensor::complex("a", vec![2, 3, 4], rng.cvec(24)),
            KernelTensor::complex("b", vec![2, 4, 3], rng.cvec(24)),
        ],
    );
    for req in [ffn, attn, cgemm] {
        let exact = execute_kernel_reference(&req).unwrap();
        let mut errors = Vec::new();
        for noise in [0.0, 0.01, 0.1] {
            let sim = execute_kernel_simulator(
                &req,
                KernelSimulatorOptions {
                    target: TargetBackend::Photonic,
                    effective_bits: 16,
                    noise_fraction: noise,
                    seed: 11,
                },
            )
            .unwrap();
            assert!(sim.simulated);
            let err = match (&sim.outputs[0].data, &exact.outputs[0].data) {
                (KernelData::Real(a), KernelData::Real(b)) => max_abs(a, b),
                (KernelData::Complex(a), KernelData::Complex(b)) => cmax_abs(a, b),
                _ => panic!("dtype mismatch"),
            };
            errors.push(err);
        }
        assert!(
            errors[0] < 1e-3,
            "{:?}: 16-bit noiseless error {}",
            req.kind,
            errors[0]
        );
        assert!(errors[2] > errors[0], "{:?}: {errors:?}", req.kind);
    }
}
