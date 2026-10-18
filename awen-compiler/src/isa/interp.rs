//! Reference interpreter for the classical VPISA domain.
//!
//! Photonic instructions (`gemm` with photonic placement, and the decomposed
//! `program_weights` → `modulate` → `mvm` → `detect` sequence) execute on the
//! compact photonic noise model ([`crate::noise::photonic_gemm`]); results are
//! **simulated**. Electronic instructions (`elementwise`, `dft`, CPU/GPU
//! fallback `gemm`) execute exactly in f64 on the host. Declared element types
//! are not emulated: storage is f64 (two f64 per `c64` element). In the
//! decomposed sequence the converter resolutions come from the instructions
//! (the larger of the `modulate` and `program_weights` DAC bits, and the
//! `detect` ADC bits) whenever the contract enables that converter.
//! Quantum and interop instructions are rejected with
//! [`InterpreterError::Unsupported`]; they need a quantum-photonic backend or
//! simulator.

use super::*;
use crate::ir::GemmShape;
use crate::noise::{photonic_gemm, PhotonicNoiseContract};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum InterpreterError {
    Invalid(IsaError),
    MissingInput {
        buffer: u32,
    },
    InputLength {
        buffer: u32,
        expected: usize,
        got: usize,
    },
    Unsupported {
        index: usize,
        op: &'static str,
    },
    Simulator {
        index: usize,
        message: String,
    },
}

impl fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VPISA interpreter error: {self:?}")
    }
}

impl std::error::Error for InterpreterError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceEntry {
    pub index: usize,
    pub op: String,
    pub placement: String,
    pub fidelity: String,
}

#[derive(Debug, Clone)]
pub struct IsaRun {
    /// Final contents of every host buffer.
    pub host: HashMap<u32, Vec<f64>>,
    pub trace: Vec<TraceEntry>,
    pub program_digest: String,
    pub photonic_ops: usize,
}

/// Operands of a multiplied-but-not-yet-detected field: x, w, shape, tile_k, DAC bits (x, w).
type PendingProduct = (Vec<f64>, Vec<f64>, GemmShape, usize, u8, u8);

fn storage_len(b: &BufferDecl) -> usize {
    let n = b.elements() as usize;
    if b.dtype == ElemType::C64 {
        2 * n
    } else {
        n
    }
}

fn gelu(x: f64) -> f64 {
    // tanh approximation (Hendrycks & Gimpel 2016), as used by GPT-2.
    0.5 * x * (1.0 + ((2.0 / std::f64::consts::PI).sqrt() * (x + 0.044715 * x.powi(3))).tanh())
}

/// Validate then execute `program`.
///
/// `inputs` supplies every host buffer that is read before it is written
/// (missing host buffers start as zeros). Photonic instruction number `j`
/// uses noise seed `seed + j`, so equivalent programs see identical noise.
pub fn run(
    program: &IsaProgram,
    inputs: &HashMap<u32, Vec<f64>>,
    contract: &PhotonicNoiseContract,
    seed: u64,
) -> Result<IsaRun, InterpreterError> {
    validate(program).map_err(InterpreterError::Invalid)?;
    let mut mem: HashMap<u32, Vec<f64>> = HashMap::new();
    for b in &program.buffers {
        if b.space == MemorySpace::Host {
            let len = storage_len(b);
            let data = match inputs.get(&b.id) {
                Some(v) if v.len() == len => v.clone(),
                Some(v) => {
                    return Err(InterpreterError::InputLength {
                        buffer: b.id,
                        expected: len,
                        got: v.len(),
                    })
                }
                None => vec![0.0; len],
            };
            mem.insert(b.id, data);
        }
    }
    let decl = |id: u32| program.buffer(id).expect("validated");
    let mut meshes: HashMap<u32, (Vec<f64>, u8)> = HashMap::new();
    let mut fields: HashMap<u32, (Vec<f64>, usize, u8)> = HashMap::new();
    let mut pending: HashMap<u32, PendingProduct> = HashMap::new();
    let mut trace = Vec::new();
    let mut photonic = 0usize;
    let fidelity = |placement: &str| {
        if placement == "photonic" {
            "simulated (compact noise model)"
        } else {
            "exact (host f64)"
        }
    };
    for (index, ins) in program.instructions.iter().enumerate() {
        use Instruction::*;
        let mut placement = "host";
        match ins {
            Alloc { buffer } => {
                mem.insert(*buffer, vec![0.0; storage_len(decl(*buffer))]);
            }
            Free { buffer } => {
                mem.remove(buffer);
            }
            CopyToDevice { src, dst } | CopyToHost { src, dst } => {
                let data = mem[src].clone();
                mem.insert(*dst, data);
            }
            SetCalibration { .. } | Signal { .. } | Wait { .. } | Barrier => {}
            ProgramWeights {
                weights,
                mesh,
                dac_bits,
            } => {
                placement = "photonic";
                meshes.insert(*mesh, (mem[weights].clone(), *dac_bits));
            }
            Modulate {
                src,
                field,
                dac_bits,
                ..
            } => {
                placement = "photonic";
                let b = decl(*src);
                fields.insert(*field, (mem[src].clone(), b.shape[0] as usize, *dac_bits));
            }
            Mvm {
                field_in,
                mesh,
                field_out,
                tile_k,
            } => {
                placement = "photonic";
                let (x, m, x_bits) = fields.remove(field_in).expect("validated");
                let (w, w_bits) = meshes[mesh].clone();
                let k = x.len() / m;
                let n = w.len() / k;
                pending.insert(
                    *field_out,
                    (
                        x,
                        w,
                        GemmShape { m, n, k },
                        *tile_k as usize,
                        x_bits,
                        w_bits,
                    ),
                );
            }
            Detect {
                field,
                dst,
                adc_bits,
                ..
            } => {
                placement = "photonic";
                let (x, w, shape, tile_k, x_bits, w_bits) =
                    pending.remove(field).expect("validated");
                let mut c = contract.clone();
                if c.dac_bits.is_some() {
                    c.dac_bits = Some(x_bits.max(w_bits) as u32);
                }
                if c.adc_bits.is_some() {
                    c.adc_bits = Some(*adc_bits as u32);
                }
                let (y, _) = photonic_gemm(&x, &w, shape, &c, tile_k, seed + photonic as u64)
                    .map_err(|e| InterpreterError::Simulator {
                        index,
                        message: e.to_string(),
                    })?;
                photonic += 1;
                mem.insert(*dst, y);
            }
            Gemm {
                a,
                b,
                c,
                m,
                n,
                k,
                transpose_b,
                tile_k,
                placement: target,
                ..
            } => {
                let (m, n, k) = (*m as usize, *n as usize, *k as usize);
                let x = mem[a].clone();
                let raw = &mem[b];
                // The simulator takes weights as [n, k].
                let w: Vec<f64> = if *transpose_b {
                    raw.clone()
                } else {
                    (0..n)
                        .flat_map(|o| (0..k).map(move |i| (o, i)))
                        .map(|(o, i)| raw[i * n + o])
                        .collect()
                };
                let y = if *target == Placement::Photonic {
                    placement = "photonic";
                    let (y, _) = photonic_gemm(
                        &x,
                        &w,
                        GemmShape { m, n, k },
                        contract,
                        *tile_k as usize,
                        seed + photonic as u64,
                    )
                    .map_err(|e| InterpreterError::Simulator {
                        index,
                        message: e.to_string(),
                    })?;
                    photonic += 1;
                    y
                } else {
                    placement = if *target == Placement::Gpu {
                        "gpu (host f64 reference)"
                    } else {
                        "cpu"
                    };
                    let mut y = vec![0.0; m * n];
                    for r in 0..m {
                        for o in 0..n {
                            y[r * n + o] = (0..k).map(|i| x[r * k + i] * w[o * k + i]).sum();
                        }
                    }
                    y
                };
                mem.insert(*c, y);
            }
            Elementwise { kind, inputs, dst } => {
                let a = &mem[&inputs[0]];
                let out: Vec<f64> = match kind {
                    ElementwiseKind::Add => {
                        a.iter().zip(&mem[&inputs[1]]).map(|(x, y)| x + y).collect()
                    }
                    ElementwiseKind::Mul => {
                        a.iter().zip(&mem[&inputs[1]]).map(|(x, y)| x * y).collect()
                    }
                    ElementwiseKind::Relu => a.iter().map(|x| x.max(0.0)).collect(),
                    ElementwiseKind::Gelu => a.iter().map(|x| gelu(*x)).collect(),
                    ElementwiseKind::Copy => a.clone(),
                };
                mem.insert(*dst, out);
            }
            Dft { src, dst, inverse } => {
                let b = decl(*src);
                let n = *b.shape.last().expect("validated") as usize;
                let data = &mem[src];
                let rows = data.len() / (2 * n);
                let sign = if *inverse { 1.0 } else { -1.0 };
                let mut out = vec![0.0; data.len()];
                for r in 0..rows {
                    for f in 0..n {
                        let (mut re, mut im) = (0.0, 0.0);
                        for t in 0..n {
                            let ang = sign * 2.0 * std::f64::consts::PI * (f * t) as f64 / n as f64;
                            let (xr, xi) = (data[2 * (r * n + t)], data[2 * (r * n + t) + 1]);
                            re += xr * ang.cos() - xi * ang.sin();
                            im += xr * ang.sin() + xi * ang.cos();
                        }
                        let scale = if *inverse { 1.0 / n as f64 } else { 1.0 };
                        out[2 * (r * n + f)] = re * scale;
                        out[2 * (r * n + f) + 1] = im * scale;
                    }
                }
                mem.insert(*dst, out);
            }
            other => {
                return Err(InterpreterError::Unsupported {
                    index,
                    op: other.name(),
                })
            }
        }
        trace.push(TraceEntry {
            index,
            op: ins.name().to_string(),
            placement: placement.to_string(),
            fidelity: fidelity(placement).to_string(),
        });
    }
    let host = program
        .buffers
        .iter()
        .filter(|b| b.space == MemorySpace::Host)
        .map(|b| (b.id, mem.remove(&b.id).unwrap_or_default()))
        .collect();
    Ok(IsaRun {
        host,
        trace,
        program_digest: program.digest(),
        photonic_ops: photonic,
    })
}
