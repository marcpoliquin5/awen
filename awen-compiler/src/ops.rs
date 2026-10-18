//! Operator library beyond a single GEMM.
//!
//! Every operator is written once, generically over a [`GemmEngine`], and
//! decomposes into matrix products (the photonic part) plus electronic
//! post-detection steps (softmax, activations, elementwise products,
//! accumulation). Running it with [`ExactEngine`] gives the f64 math
//! reference. Running it with [`PhotonicEngine`] executes every matrix product
//! on the AEP-0022 compact noise model, so the result is **simulated**.
//!
//! [`op_table`] is the machine-readable support table. Each operator records
//! the highest level it has reached:
//!
//! * `math_reference`: an f64 reference exists and matches an independent
//!   NumPy oracle in tests;
//! * `photonic_sim`: also runs on the photonic noise model;
//! * `hardware_lowerable`: the compiler also lowers it to device commands
//!   (AWENEXE or VPISA) that a conforming backend would execute;
//! * `hardware_measured`: also measured on photonic hardware with a HIL
//!   artifact. No operator is at this level: no artifact exists.

use crate::ir::GemmShape;
use crate::noise::{photonic_gemm, PhotonicNoiseContract};
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const OP_TABLE_VERSION: &str = "awen.op-table.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum OpLevel {
    MathReference,
    PhotonicSim,
    HardwareLowerable,
    HardwareMeasured,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpInfo {
    pub name: String,
    pub level: OpLevel,
    pub inputs: Vec<String>,
    pub params: Vec<String>,
    pub outputs: Vec<String>,
    /// What runs as photonic matrix products.
    pub photonic: String,
    /// What runs electronically after detection.
    pub electronic: String,
}

/// Dense row-major f64 array.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Array {
    pub shape: Vec<usize>,
    pub data: Vec<f64>,
}

impl Array {
    pub fn new(shape: Vec<usize>, data: Vec<f64>) -> Result<Self> {
        ensure!(
            shape.iter().product::<usize>() == data.len(),
            "array data length {} does not match shape {:?}",
            data.len(),
            shape
        );
        ensure!(
            data.iter().all(|v| v.is_finite()),
            "array values must be finite"
        );
        Ok(Self { shape, data })
    }

    pub fn zeros(shape: Vec<usize>) -> Self {
        let n = shape.iter().product();
        Self {
            shape,
            data: vec![0.0; n],
        }
    }

    fn dims2(&self, name: &str) -> Result<(usize, usize)> {
        ensure!(
            self.shape.len() == 2,
            "{name} must be rank 2, got {:?}",
            self.shape
        );
        Ok((self.shape[0], self.shape[1]))
    }

    fn map(&self, f: impl Fn(f64) -> f64) -> Self {
        Self {
            shape: self.shape.clone(),
            data: self.data.iter().map(|v| f(*v)).collect(),
        }
    }

    fn zip(&self, other: &Self, f: impl Fn(f64, f64) -> f64) -> Result<Self> {
        ensure!(
            self.shape == other.shape,
            "shape mismatch {:?} vs {:?}",
            self.shape,
            other.shape
        );
        Ok(Self {
            shape: self.shape.clone(),
            data: self
                .data
                .iter()
                .zip(&other.data)
                .map(|(a, b)| f(*a, *b))
                .collect(),
        })
    }

    fn transpose2(&self) -> Self {
        let (r, c) = (self.shape[0], self.shape[1]);
        let mut data = vec![0.0; r * c];
        for i in 0..r {
            for j in 0..c {
                data[j * r + i] = self.data[i * c + j];
            }
        }
        Self {
            shape: vec![c, r],
            data,
        }
    }

    fn add_row_bias(&self, bias: &Array) -> Result<Self> {
        let (r, c) = self.dims2("activation")?;
        ensure!(
            bias.data.len() == c,
            "bias length {} != {}",
            bias.data.len(),
            c
        );
        let mut out = self.clone();
        for i in 0..r {
            for j in 0..c {
                out.data[i * c + j] += bias.data[j];
            }
        }
        Ok(out)
    }
}

/// Computes `x[m,k] · w[n,k]ᵀ`.
pub trait GemmEngine {
    fn gemm(&mut self, x: &Array, w: &Array) -> Result<Array>;
}

/// f64 reference engine.
#[derive(Default)]
pub struct ExactEngine {
    pub calls: usize,
}

impl GemmEngine for ExactEngine {
    fn gemm(&mut self, x: &Array, w: &Array) -> Result<Array> {
        let (m, k) = x.dims2("x")?;
        let (n, kw) = w.dims2("w")?;
        ensure!(k == kw, "inner dimensions differ: {k} vs {kw}");
        self.calls += 1;
        let mut y = vec![0.0; m * n];
        for r in 0..m {
            for o in 0..n {
                y[r * n + o] = (0..k).map(|i| x.data[r * k + i] * w.data[o * k + i]).sum();
            }
        }
        Ok(Array {
            shape: vec![m, n],
            data: y,
        })
    }
}

/// Runs every product on the compact noise model; call `j` uses seed `seed + j`.
pub struct PhotonicEngine {
    pub contract: PhotonicNoiseContract,
    pub tile_k: usize,
    pub seed: u64,
    pub calls: usize,
    pub macs: u64,
}

impl PhotonicEngine {
    pub fn new(contract: PhotonicNoiseContract, tile_k: usize, seed: u64) -> Self {
        Self {
            contract,
            tile_k,
            seed,
            calls: 0,
            macs: 0,
        }
    }
}

impl GemmEngine for PhotonicEngine {
    fn gemm(&mut self, x: &Array, w: &Array) -> Result<Array> {
        let (m, k) = x.dims2("x")?;
        let (n, kw) = w.dims2("w")?;
        ensure!(k == kw, "inner dimensions differ: {k} vs {kw}");
        let (y, _) = photonic_gemm(
            &x.data,
            &w.data,
            GemmShape { m, n, k },
            &self.contract,
            self.tile_k,
            self.seed + self.calls as u64,
        )?;
        self.calls += 1;
        self.macs += (m * n * k) as u64;
        Ok(Array {
            shape: vec![m, n],
            data: y,
        })
    }
}

fn gelu(x: f64) -> f64 {
    0.5 * x * (1.0 + ((2.0 / std::f64::consts::PI).sqrt() * (x + 0.044715 * x.powi(3))).tanh())
}

fn softmax_rows(a: &Array) -> Result<Array> {
    let (r, c) = a.dims2("scores")?;
    let mut out = a.clone();
    for i in 0..r {
        let row = &mut out.data[i * c..(i + 1) * c];
        let max = row.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mut sum = 0.0;
        for v in row.iter_mut() {
            *v = (*v - max).exp();
            sum += *v;
        }
        for v in row.iter_mut() {
            *v /= sum;
        }
    }
    Ok(out)
}

/// `(yr, yi) = (xr + i xi) · (wr + i wi)ᵀ` with four real products.
pub fn complex_matmul(
    e: &mut dyn GemmEngine,
    xr: &Array,
    xi: &Array,
    wr: &Array,
    wi: &Array,
) -> Result<(Array, Array)> {
    let rr = e.gemm(xr, wr)?;
    let ii = e.gemm(xi, wi)?;
    let ri = e.gemm(xr, wi)?;
    let ir = e.gemm(xi, wr)?;
    Ok((rr.zip(&ii, |a, b| a - b)?, ri.zip(&ir, |a, b| a + b)?))
}

/// DFT matrix `F[f, t] = exp(-2πi f t / n)` (or its inverse with `1/n` scaling).
pub fn dft_matrix(n: usize, inverse: bool) -> (Array, Array) {
    let sign = if inverse { 1.0 } else { -1.0 };
    let scale = if inverse { 1.0 / n as f64 } else { 1.0 };
    let mut re = vec![0.0; n * n];
    let mut im = vec![0.0; n * n];
    for f in 0..n {
        for t in 0..n {
            let ang = sign * 2.0 * std::f64::consts::PI * ((f * t) % n) as f64 / n as f64;
            re[f * n + t] = ang.cos() * scale;
            im[f * n + t] = ang.sin() * scale;
        }
    }
    (
        Array {
            shape: vec![n, n],
            data: re,
        },
        Array {
            shape: vec![n, n],
            data: im,
        },
    )
}

/// DFT along the last axis of `[batch, n]` as a complex matrix product.
pub fn dft(
    e: &mut dyn GemmEngine,
    xr: &Array,
    xi: &Array,
    inverse: bool,
) -> Result<(Array, Array)> {
    let (_, n) = xr.dims2("xr")?;
    let (fr, fi) = dft_matrix(n, inverse);
    complex_matmul(e, xr, xi, &fr, &fi)
}

/// Single-image 2-D convolution (cross-correlation, as in deep learning) via im2col.
/// `x: [C, H, W]`, `w: [O, C, KH, KW]`, `bias: [O]` → `[O, HO, WO]`.
pub fn conv2d(
    e: &mut dyn GemmEngine,
    x: &Array,
    w: &Array,
    bias: Option<&Array>,
    stride: usize,
    padding: usize,
) -> Result<Array> {
    ensure!(
        x.shape.len() == 3 && w.shape.len() == 4,
        "conv2d expects x [C,H,W] and w [O,C,KH,KW]"
    );
    ensure!(stride > 0, "stride must be positive");
    let (c, h, wd) = (x.shape[0], x.shape[1], x.shape[2]);
    let (o, cw, kh, kw) = (w.shape[0], w.shape[1], w.shape[2], w.shape[3]);
    ensure!(c == cw, "channel mismatch {c} vs {cw}");
    ensure!(
        h + 2 * padding >= kh && wd + 2 * padding >= kw,
        "kernel larger than padded input"
    );
    let ho = (h + 2 * padding - kh) / stride + 1;
    let wo = (wd + 2 * padding - kw) / stride + 1;
    let k = c * kh * kw;
    let mut cols = vec![0.0; ho * wo * k];
    for oy in 0..ho {
        for ox in 0..wo {
            let row = oy * wo + ox;
            for ci in 0..c {
                for dy in 0..kh {
                    for dx in 0..kw {
                        let iy = (oy * stride + dy) as isize - padding as isize;
                        let ix = (ox * stride + dx) as isize - padding as isize;
                        let v = if iy >= 0 && ix >= 0 && (iy as usize) < h && (ix as usize) < wd {
                            x.data[(ci * h + iy as usize) * wd + ix as usize]
                        } else {
                            0.0
                        };
                        cols[row * k + (ci * kh + dy) * kw + dx] = v;
                    }
                }
            }
        }
    }
    let patches = Array {
        shape: vec![ho * wo, k],
        data: cols,
    };
    let weights = Array {
        shape: vec![o, k],
        data: w.data.clone(),
    };
    let mut y = e.gemm(&patches, &weights)?; // [HO*WO, O]
    if let Some(b) = bias {
        y = y.add_row_bias(b)?;
    }
    let t = y.transpose2();
    Ok(Array {
        shape: vec![o, ho, wo],
        data: t.data,
    })
}

/// 1-D valid cross-correlation of each row of `x: [B, L]` with `kernel: [K]` → `[B, L-K+1]`.
pub fn correlate1d(e: &mut dyn GemmEngine, x: &Array, kernel: &Array) -> Result<Array> {
    let (b, l) = x.dims2("x")?;
    let kk = kernel.data.len();
    ensure!(kk >= 1 && kk <= l, "kernel length must be in 1..=L");
    let out = l - kk + 1;
    let mut cols = vec![0.0; b * out * kk];
    for r in 0..b {
        for p in 0..out {
            cols[(r * out + p) * kk..(r * out + p + 1) * kk]
                .copy_from_slice(&x.data[r * l + p..r * l + p + kk]);
        }
    }
    let y = e.gemm(
        &Array {
            shape: vec![b * out, kk],
            data: cols,
        },
        &Array {
            shape: vec![1, kk],
            data: kernel.data.clone(),
        },
    )?;
    Ok(Array {
        shape: vec![b, out],
        data: y.data,
    })
}

/// `y = (x · uᵀ) · vᵀ` with `u: [r, k]`, `v: [n, r]` (rank-r factorized linear map).
pub fn low_rank_linear(e: &mut dyn GemmEngine, x: &Array, u: &Array, v: &Array) -> Result<Array> {
    let t = e.gemm(x, u)?;
    e.gemm(&t, v)
}

/// Single-head scaled dot-product self-attention with projections.
/// `x: [T, d]`; `wq, wk, wv: [d_h, d]`; `wo: [d, d_h]`; optional causal mask.
pub fn attention(
    e: &mut dyn GemmEngine,
    x: &Array,
    wq: &Array,
    wk: &Array,
    wv: &Array,
    wo: &Array,
    causal: bool,
) -> Result<Array> {
    let q = e.gemm(x, wq)?;
    let k = e.gemm(x, wk)?;
    let v = e.gemm(x, wv)?;
    let (t, dh) = q.dims2("q")?;
    let mut scores = e.gemm(&q, &k)?; // [T, T], both operands are activations
    let scale = 1.0 / (dh as f64).sqrt();
    for i in 0..t {
        for j in 0..t {
            let s = &mut scores.data[i * t + j];
            *s = if causal && j > i {
                f64::NEG_INFINITY
            } else {
                *s * scale
            };
        }
    }
    let p = softmax_rows(&scores)?;
    let ctx = e.gemm(&p, &v.transpose2())?; // [T, d_h]
    e.gemm(&ctx, wo)
}

/// Transformer feed-forward block `W2 · gelu(W1 x + b1) + b2`.
pub fn ffn(
    e: &mut dyn GemmEngine,
    x: &Array,
    w1: &Array,
    b1: &Array,
    w2: &Array,
    b2: &Array,
) -> Result<Array> {
    let h = e.gemm(x, w1)?.add_row_bias(b1)?.map(gelu);
    e.gemm(&h, w2)?.add_row_bias(b2)
}

/// Circular spectral convolution: `IDFT(DFT(x) ⊙ H)` for real `x: [B, N]` and complex `H: [N]`.
pub fn spectral_conv1d(e: &mut dyn GemmEngine, x: &Array, hr: &Array, hi: &Array) -> Result<Array> {
    let (b, n) = x.dims2("x")?;
    ensure!(
        hr.data.len() == n && hi.data.len() == n,
        "filter length must equal N"
    );
    let zeros = Array::zeros(vec![b, n]);
    let (xr, xi) = dft(e, x, &zeros, false)?;
    let mut yr = xr.clone();
    let mut yi = xi.clone();
    for r in 0..b {
        for f in 0..n {
            let (a, c) = (xr.data[r * n + f], xi.data[r * n + f]);
            yr.data[r * n + f] = a * hr.data[f] - c * hi.data[f];
            yi.data[r * n + f] = a * hi.data[f] + c * hr.data[f];
        }
    }
    let (out, _) = dft(e, &yr, &yi, true)?;
    Ok(out)
}

/// Elman recurrent update `h' = tanh(h Wᵀ + x Uᵀ + b)`.
pub fn rnn_step(
    e: &mut dyn GemmEngine,
    h: &Array,
    x: &Array,
    w: &Array,
    u: &Array,
    b: &Array,
) -> Result<Array> {
    let a = e.gemm(h, w)?;
    let c = e.gemm(x, u)?;
    Ok(a.zip(&c, |p, q| p + q)?.add_row_bias(b)?.map(f64::tanh))
}

/// Leaky echo-state reservoir update `s' = (1-a) s + a tanh(s Wᵀ + x W_inᵀ)`.
pub fn reservoir_step(
    e: &mut dyn GemmEngine,
    s: &Array,
    x: &Array,
    w: &Array,
    w_in: &Array,
    leak: f64,
) -> Result<Array> {
    ensure!((0.0..=1.0).contains(&leak), "leak must be in [0, 1]");
    let pre = e
        .gemm(s, w)?
        .zip(&e.gemm(x, w_in)?, |p, q| p + q)?
        .map(f64::tanh);
    s.zip(&pre, |old, new| (1.0 - leak) * old + leak * new)
}

/// Coherent linear layer followed by square-law detection: `|x · (wr + i wi)ᵀ|²` for real `x`.
pub fn intensity_layer(e: &mut dyn GemmEngine, x: &Array, wr: &Array, wi: &Array) -> Result<Array> {
    let yr = e.gemm(x, wr)?;
    let yi = e.gemm(x, wi)?;
    yr.zip(&yi, |a, b| a * a + b * b)
}

/// The support table. Levels are assigned here and checked by tests.
pub fn op_table() -> Vec<OpInfo> {
    let info = |name: &str,
                level,
                inputs: &[&str],
                params: &[&str],
                outputs: &[&str],
                photonic: &str,
                electronic: &str| OpInfo {
        name: name.to_string(),
        level,
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
        params: params.iter().map(|s| s.to_string()).collect(),
        outputs: outputs.iter().map(|s| s.to_string()).collect(),
        photonic: photonic.to_string(),
        electronic: electronic.to_string(),
    };
    use OpLevel::*;
    vec![
        info(
            "gemm",
            HardwareLowerable,
            &["x", "w"],
            &[],
            &["y"],
            "x · wᵀ",
            "tile accumulation (AWENEXE execute_gemm and VPISA gemm lowering exist)",
        ),
        info(
            "complex_matmul",
            PhotonicSim,
            &["xr", "xi", "wr", "wi"],
            &[],
            &["yr", "yi"],
            "four real products",
            "two additions",
        ),
        info(
            "dft",
            PhotonicSim,
            &["xr", "xi"],
            &["inverse"],
            &["yr", "yi"],
            "complex product with the DFT matrix",
            "additions; 1/n scaling folded into the inverse matrix",
        ),
        info(
            "conv2d",
            PhotonicSim,
            &["x", "w", "bias?"],
            &["stride", "padding"],
            &["y"],
            "im2col patches · kernelsᵀ",
            "im2col gather, bias",
        ),
        info(
            "correlate1d",
            PhotonicSim,
            &["x", "kernel"],
            &[],
            &["y"],
            "sliding windows · kernel",
            "window gather",
        ),
        info(
            "low_rank_linear",
            PhotonicSim,
            &["x", "u", "v"],
            &[],
            &["y"],
            "two rank-r products",
            "none",
        ),
        info(
            "attention",
            PhotonicSim,
            &["x", "wq", "wk", "wv", "wo"],
            &["causal"],
            &["y"],
            "Q/K/V/O projections, QKᵀ and PV (activation-activation products)",
            "scaling, causal mask, softmax",
        ),
        info(
            "ffn",
            PhotonicSim,
            &["x", "w1", "b1", "w2", "b2"],
            &[],
            &["y"],
            "two projections",
            "biases, GELU",
        ),
        info(
            "spectral_conv1d",
            PhotonicSim,
            &["x", "hr", "hi"],
            &[],
            &["y"],
            "forward and inverse DFT products",
            "spectral pointwise complex multiply",
        ),
        info(
            "rnn_step",
            PhotonicSim,
            &["h", "x", "w", "u", "b"],
            &[],
            &["h_next"],
            "recurrent and input products",
            "sum, bias, tanh",
        ),
        info(
            "reservoir_step",
            PhotonicSim,
            &["s", "x", "w", "w_in"],
            &["leak"],
            &["s_next"],
            "reservoir and input products",
            "tanh, leaky integration",
        ),
        info(
            "intensity_layer",
            PhotonicSim,
            &["x", "wr", "wi"],
            &[],
            &["y"],
            "coherent real and imaginary products",
            "square-law detection |·|²",
        ),
    ]
}

/// Request for [`run_op`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpRequest {
    pub op: String,
    pub inputs: BTreeMap<String, Array>,
    #[serde(default)]
    pub params: BTreeMap<String, serde_json::Value>,
    /// `None` runs the f64 reference; `Some` runs every product on the noise model.
    #[serde(default)]
    pub contract: Option<PhotonicNoiseContract>,
    #[serde(default = "default_tile_k")]
    pub tile_k: usize,
    #[serde(default)]
    pub seed: u64,
}

fn default_tile_k() -> usize {
    64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpResponse {
    pub version: String,
    pub op: String,
    pub fidelity: String,
    pub gemm_calls: usize,
    pub photonic_macs: u64,
    pub outputs: BTreeMap<String, Array>,
}

/// Run one table operator by name, on the reference or the photonic engine.
pub fn run_op(req: &OpRequest) -> Result<OpResponse> {
    let get = |name: &str| -> Result<&Array> {
        req.inputs
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("{} needs input {name:?}", req.op))
    };
    for a in req.inputs.values() {
        Array::new(a.shape.clone(), a.data.clone())?;
    }
    let flag = |name: &str| {
        req.params
            .get(name)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };
    let num = |name: &str, default: f64| {
        req.params
            .get(name)
            .and_then(|v| v.as_f64())
            .unwrap_or(default)
    };
    let mut exact = ExactEngine::default();
    let mut photonic = req
        .contract
        .clone()
        .map(|c| PhotonicEngine::new(c, req.tile_k, req.seed));
    if let Some(p) = &photonic {
        p.contract.validate()?;
    }
    let e: &mut dyn GemmEngine = match photonic.as_mut() {
        Some(p) => p,
        None => &mut exact,
    };
    let mut out = BTreeMap::new();
    match req.op.as_str() {
        "gemm" => {
            out.insert("y".into(), e.gemm(get("x")?, get("w")?)?);
        }
        "complex_matmul" => {
            let (yr, yi) = complex_matmul(e, get("xr")?, get("xi")?, get("wr")?, get("wi")?)?;
            out.insert("yr".into(), yr);
            out.insert("yi".into(), yi);
        }
        "dft" => {
            let (yr, yi) = dft(e, get("xr")?, get("xi")?, flag("inverse"))?;
            out.insert("yr".into(), yr);
            out.insert("yi".into(), yi);
        }
        "conv2d" => {
            let stride = num("stride", 1.0) as usize;
            let padding = num("padding", 0.0) as usize;
            out.insert(
                "y".into(),
                conv2d(
                    e,
                    get("x")?,
                    get("w")?,
                    req.inputs.get("bias"),
                    stride,
                    padding,
                )?,
            );
        }
        "correlate1d" => {
            out.insert("y".into(), correlate1d(e, get("x")?, get("kernel")?)?);
        }
        "low_rank_linear" => {
            out.insert(
                "y".into(),
                low_rank_linear(e, get("x")?, get("u")?, get("v")?)?,
            );
        }
        "attention" => {
            let y = attention(
                e,
                get("x")?,
                get("wq")?,
                get("wk")?,
                get("wv")?,
                get("wo")?,
                flag("causal"),
            )?;
            out.insert("y".into(), y);
        }
        "ffn" => {
            out.insert(
                "y".into(),
                ffn(e, get("x")?, get("w1")?, get("b1")?, get("w2")?, get("b2")?)?,
            );
        }
        "spectral_conv1d" => {
            out.insert(
                "y".into(),
                spectral_conv1d(e, get("x")?, get("hr")?, get("hi")?)?,
            );
        }
        "rnn_step" => {
            out.insert(
                "h_next".into(),
                rnn_step(e, get("h")?, get("x")?, get("w")?, get("u")?, get("b")?)?,
            );
        }
        "reservoir_step" => {
            let s = reservoir_step(
                e,
                get("s")?,
                get("x")?,
                get("w")?,
                get("w_in")?,
                num("leak", 0.3),
            )?;
            out.insert("s_next".into(), s);
        }
        "intensity_layer" => {
            out.insert(
                "y".into(),
                intensity_layer(e, get("x")?, get("wr")?, get("wi")?)?,
            );
        }
        other => bail!("unknown operator {other:?}; see op_table()"),
    }
    let (calls, macs, fidelity) = match &photonic {
        Some(p) => (p.calls, p.macs, "simulated (compact noise model)"),
        None => (exact.calls, 0, "exact (f64 reference)"),
    };
    Ok(OpResponse {
        version: OP_TABLE_VERSION.to_string(),
        op: req.op.clone(),
        fidelity: fidelity.to_string(),
        gemm_calls: calls,
        photonic_macs: macs,
        outputs: out,
    })
}
