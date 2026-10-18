//! AWEN Virtual Photonic ISA (VPISA) v1.
//!
//! A typed, linear instruction stream that sits below the graph-level photonic
//! and quantum-photonic programs (`awen.photonic.program.v1`,
//! `awen.qphotonic.program.v1`) and above a backend's private command format.
//! Specified in `awen-spec/specs/isa.md` and AEP-0024.
//!
//! * [`IsaProgram`] is the in-memory and canonical JSON form
//!   (`awen-spec/schemas/awen_vpisa.v1.json`).
//! * [`codec`] is the deterministic little-endian binary encoding (`AWENISA\0`).
//! * [`validate`] checks every static rule and returns a typed [`IsaError`].
//! * [`interp`] is a reference interpreter for the classical domain that runs
//!   photonic instructions on the compact noise model ([`crate::noise`]).
//!
//! Instructions belong to one of three domains. Classical instructions touch
//! buffers, optical fields and meshes. Quantum instructions touch only quantum
//! modes. Interop instructions are the only bridge: measurements write
//! classical registers from modes, and feed-forward reads registers into
//! quantum parameters or host buffers.

pub mod codec;
pub mod interp;
pub mod validate;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use codec::{decode, encode, CodecError};
pub use interp::{run, InterpreterError, IsaRun, TraceEntry};
pub use validate::{check_target, negotiate, validate, IsaError, IsaTarget};

pub const ISA_JSON_VERSION: &str = "awen.vpisa.v1";
pub const ISA_MAGIC: &[u8; 8] = b"AWENISA\0";
pub const ISA_MAJOR: u16 = 1;
pub const ISA_MINOR: u16 = 0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct IsaVersion {
    pub major: u16,
    pub minor: u16,
}

impl IsaVersion {
    pub const CURRENT: IsaVersion = IsaVersion {
        major: ISA_MAJOR,
        minor: ISA_MINOR,
    };
}

/// What a backend must provide to run the program.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    /// Largest per-instruction `min_effective_bits` in the program.
    pub min_effective_bits: u16,
    /// Largest contraction tile any photonic instruction uses.
    pub max_tile_k: u32,
    pub quantum: bool,
    pub feed_forward: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySpace {
    Host,
    Device,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ElemType {
    F64,
    F32,
    Bf16,
    F16,
    I8,
    /// Complex number as two f64 (real, imaginary).
    C64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BufferDecl {
    pub id: u32,
    pub space: MemorySpace,
    pub dtype: ElemType,
    pub shape: Vec<u64>,
}

impl BufferDecl {
    pub fn elements(&self) -> u64 {
        self.shape.iter().product()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    Amplitude,
    Phase,
    Intensity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Polarization {
    Te,
    Tm,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetectionScheme {
    Direct,
    Balanced,
    Coherent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Photonic,
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ElementwiseKind {
    Add,
    Mul,
    Relu,
    Gelu,
    Copy,
}

impl ElementwiseKind {
    pub fn arity(self) -> usize {
        match self {
            ElementwiseKind::Add | ElementwiseKind::Mul => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Classical,
    Quantum,
    Interop,
}

/// One VPISA instruction. JSON uses `"op"` as the tag.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Instruction {
    // ---- classical: memory and transfers
    Alloc {
        buffer: u32,
    },
    Free {
        buffer: u32,
    },
    CopyToDevice {
        src: u32,
        dst: u32,
    },
    CopyToHost {
        src: u32,
        dst: u32,
    },
    // ---- classical: calibration and synchronization
    SetCalibration {
        handle: String,
        fingerprint: String,
    },
    Signal {
        event: u32,
    },
    Wait {
        event: u32,
    },
    Barrier,
    // ---- classical: photonic datapath
    ProgramWeights {
        weights: u32,
        mesh: u32,
        dac_bits: u8,
    },
    Modulate {
        src: u32,
        field: u32,
        encoding: Encoding,
        wavelength: u16,
        spatial_mode: u16,
        polarization: Polarization,
        dac_bits: u8,
    },
    Mvm {
        field_in: u32,
        mesh: u32,
        field_out: u32,
        tile_k: u32,
    },
    Detect {
        field: u32,
        dst: u32,
        scheme: DetectionScheme,
        adc_bits: u8,
    },
    /// `c[m,n] = a[m,k] · b` with `b` stored `[n,k]` (`transpose_b`) or `[k,n]`.
    Gemm {
        a: u32,
        b: u32,
        c: u32,
        m: u32,
        n: u32,
        k: u32,
        transpose_b: bool,
        tile_k: u32,
        min_effective_bits: u16,
        placement: Placement,
        /// Why a non-photonic placement was chosen (empty for photonic).
        fallback_reason: String,
    },
    /// Electronic elementwise operation (always executed after detection).
    Elementwise {
        kind: ElementwiseKind,
        inputs: Vec<u32>,
        dst: u32,
    },
    /// Discrete Fourier transform along the last axis of a `c64` buffer.
    Dft {
        src: u32,
        dst: u32,
        inverse: bool,
    },
    // ---- quantum domain
    PrepareFock {
        mode: u32,
        photons: u8,
    },
    PrepareCoherent {
        mode: u32,
        re: f64,
        im: f64,
    },
    PrepareSqueezed {
        mode: u32,
        r: f64,
        phi: f64,
    },
    Beamsplitter {
        mode_a: u32,
        mode_b: u32,
        theta: f64,
        phi: f64,
    },
    PhaseShift {
        mode: u32,
        phi: f64,
    },
    // ---- interop (the only bridge between the domains)
    MeasurePnr {
        mode: u32,
        register: u32,
    },
    MeasureHomodyne {
        mode: u32,
        phi: f64,
        register: u32,
    },
    /// Feed-forward: `phase = scale * register + offset` applied to `mode`.
    ConditionalPhase {
        register: u32,
        mode: u32,
        scale: f64,
        offset: f64,
    },
    /// Copy a measurement register into a one-element host buffer.
    ReadRegister {
        register: u32,
        dst: u32,
    },
}

impl Instruction {
    pub fn domain(&self) -> Domain {
        use Instruction::*;
        match self {
            PrepareFock { .. }
            | PrepareCoherent { .. }
            | PrepareSqueezed { .. }
            | Beamsplitter { .. }
            | PhaseShift { .. } => Domain::Quantum,
            MeasurePnr { .. }
            | MeasureHomodyne { .. }
            | ConditionalPhase { .. }
            | ReadRegister { .. } => Domain::Interop,
            _ => Domain::Classical,
        }
    }

    pub fn name(&self) -> &'static str {
        use Instruction::*;
        match self {
            Alloc { .. } => "alloc",
            Free { .. } => "free",
            CopyToDevice { .. } => "copy_to_device",
            CopyToHost { .. } => "copy_to_host",
            SetCalibration { .. } => "set_calibration",
            Signal { .. } => "signal",
            Wait { .. } => "wait",
            Barrier => "barrier",
            ProgramWeights { .. } => "program_weights",
            Modulate { .. } => "modulate",
            Mvm { .. } => "mvm",
            Detect { .. } => "detect",
            Gemm { .. } => "gemm",
            Elementwise { .. } => "elementwise",
            Dft { .. } => "dft",
            PrepareFock { .. } => "prepare_fock",
            PrepareCoherent { .. } => "prepare_coherent",
            PrepareSqueezed { .. } => "prepare_squeezed",
            Beamsplitter { .. } => "beamsplitter",
            PhaseShift { .. } => "phase_shift",
            MeasurePnr { .. } => "measure_pnr",
            MeasureHomodyne { .. } => "measure_homodyne",
            ConditionalPhase { .. } => "conditional_phase",
            ReadRegister { .. } => "read_register",
        }
    }
}

/// A complete VPISA program.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IsaProgram {
    pub version: String,
    pub abi: IsaVersion,
    pub program_id: String,
    pub requirements: Requirements,
    pub buffers: Vec<BufferDecl>,
    /// Number of synchronization events (ids `0..events`).
    pub events: u32,
    /// Number of optical field slots (ids `0..fields`).
    pub fields: u32,
    /// Number of programmable weight meshes (ids `0..meshes`).
    pub meshes: u32,
    /// Number of quantum modes (ids `0..modes`).
    pub modes: u32,
    /// Number of classical measurement registers (ids `0..registers`).
    pub registers: u32,
    pub instructions: Vec<Instruction>,
}

impl IsaProgram {
    pub fn buffer(&self, id: u32) -> Option<&BufferDecl> {
        self.buffers.iter().find(|b| b.id == id)
    }

    /// `sha256:` digest of the canonical binary encoding, for provenance and replay.
    pub fn digest(&self) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(encode(self))))
    }

    /// Lower one GEMM `c = a · bᵀ` (`a: [m,k]`, `b: [n,k]`, host buffers 0, 1, 2) into
    /// the decomposed photonic sequence: allocate, copy in, calibrate, program the
    /// mesh, modulate, multiply, detect and copy out.
    #[allow(clippy::too_many_arguments)]
    pub fn lower_gemm(
        program_id: &str,
        m: u32,
        n: u32,
        k: u32,
        tile_k: u32,
        dac_bits: u8,
        adc_bits: u8,
        min_effective_bits: u16,
        calibration_handle: &str,
        calibration_fingerprint: &str,
    ) -> IsaProgram {
        let decl = |id, space, shape: Vec<u64>| BufferDecl {
            id,
            space,
            dtype: ElemType::F64,
            shape,
        };
        let (m64, n64, k64) = (m as u64, n as u64, k as u64);
        use Instruction::*;
        IsaProgram {
            version: ISA_JSON_VERSION.to_string(),
            abi: IsaVersion::CURRENT,
            program_id: program_id.to_string(),
            requirements: Requirements {
                min_effective_bits,
                max_tile_k: tile_k,
                quantum: false,
                feed_forward: false,
            },
            buffers: vec![
                decl(0, MemorySpace::Host, vec![m64, k64]),
                decl(1, MemorySpace::Host, vec![n64, k64]),
                decl(2, MemorySpace::Host, vec![m64, n64]),
                decl(10, MemorySpace::Device, vec![m64, k64]),
                decl(11, MemorySpace::Device, vec![n64, k64]),
                decl(12, MemorySpace::Device, vec![m64, n64]),
            ],
            events: 1,
            fields: 2,
            meshes: 1,
            modes: 0,
            registers: 0,
            instructions: vec![
                Alloc { buffer: 10 },
                Alloc { buffer: 11 },
                Alloc { buffer: 12 },
                CopyToDevice { src: 0, dst: 10 },
                CopyToDevice { src: 1, dst: 11 },
                SetCalibration {
                    handle: calibration_handle.to_string(),
                    fingerprint: calibration_fingerprint.to_string(),
                },
                ProgramWeights {
                    weights: 11,
                    mesh: 0,
                    dac_bits,
                },
                Modulate {
                    src: 10,
                    field: 0,
                    encoding: Encoding::Amplitude,
                    wavelength: 0,
                    spatial_mode: 0,
                    polarization: Polarization::Te,
                    dac_bits,
                },
                Mvm {
                    field_in: 0,
                    mesh: 0,
                    field_out: 1,
                    tile_k,
                },
                Detect {
                    field: 1,
                    dst: 12,
                    scheme: DetectionScheme::Balanced,
                    adc_bits,
                },
                Signal { event: 0 },
                Wait { event: 0 },
                CopyToHost { src: 12, dst: 2 },
                Free { buffer: 10 },
                Free { buffer: 11 },
                Free { buffer: 12 },
            ],
        }
    }
}
