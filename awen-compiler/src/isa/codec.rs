//! Deterministic little-endian binary encoding of VPISA programs.
//!
//! Layout (all integers little-endian; strings are `u16` byte length + UTF-8;
//! `f64` values are their IEEE 754 bit patterns, so encoding is lossless):
//!
//! ```text
//! 8   magic "AWENISA\0"
//! 2   major            2  minor
//! str program_id
//! 2   requirements.min_effective_bits
//! 4   requirements.max_tile_k
//! 1   requirement flags (bit 0 quantum, bit 1 feed_forward; other bits zero)
//! 4   buffer count, then per buffer: u32 id, u8 space, u8 dtype, u8 rank, rank x u64
//! 4x5 events, fields, meshes, modes, registers
//! 4   instruction count, then per instruction: u8 opcode + operands
//! ```
//!
//! The JSON `version` string is implied by the magic and major version.
//! Decoding rejects bad magic, unsupported major versions, truncation, invalid
//! UTF-8, unknown opcodes and enum codes, non-0/1 booleans, reserved flag bits
//! and trailing bytes.

use super::*;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    BadMagic,
    UnsupportedMajor(u16),
    Truncated {
        offset: usize,
        needed: usize,
    },
    InvalidUtf8 {
        offset: usize,
    },
    UnknownOpcode {
        offset: usize,
        opcode: u8,
    },
    UnknownEnum {
        offset: usize,
        field: &'static str,
        value: u8,
    },
    InvalidBool {
        offset: usize,
        value: u8,
    },
    ReservedFlags {
        value: u8,
    },
    TrailingBytes {
        count: usize,
    },
    TooLong {
        field: &'static str,
    },
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VPISA decode error: {self:?}")
    }
}

impl std::error::Error for CodecError {}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    fn str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let len = u16::try_from(bytes.len()).expect("VPISA strings are at most 65535 bytes");
        self.u16(len);
        self.0.extend_from_slice(bytes);
    }
}

fn space_code(v: MemorySpace) -> u8 {
    match v {
        MemorySpace::Host => 0,
        MemorySpace::Device => 1,
    }
}
fn dtype_code(v: ElemType) -> u8 {
    match v {
        ElemType::F64 => 0,
        ElemType::F32 => 1,
        ElemType::Bf16 => 2,
        ElemType::F16 => 3,
        ElemType::I8 => 4,
        ElemType::C64 => 5,
    }
}
fn encoding_code(v: Encoding) -> u8 {
    match v {
        Encoding::Amplitude => 0,
        Encoding::Phase => 1,
        Encoding::Intensity => 2,
    }
}
fn polarization_code(v: Polarization) -> u8 {
    match v {
        Polarization::Te => 0,
        Polarization::Tm => 1,
    }
}
fn scheme_code(v: DetectionScheme) -> u8 {
    match v {
        DetectionScheme::Direct => 0,
        DetectionScheme::Balanced => 1,
        DetectionScheme::Coherent => 2,
    }
}
fn placement_code(v: Placement) -> u8 {
    match v {
        Placement::Photonic => 0,
        Placement::Cpu => 1,
        Placement::Gpu => 2,
    }
}
fn ew_code(v: ElementwiseKind) -> u8 {
    match v {
        ElementwiseKind::Add => 0,
        ElementwiseKind::Mul => 1,
        ElementwiseKind::Relu => 2,
        ElementwiseKind::Gelu => 3,
        ElementwiseKind::Copy => 4,
    }
}

/// Opcode of an instruction (stable for ABI major 1).
pub fn opcode(i: &Instruction) -> u8 {
    use Instruction::*;
    match i {
        Alloc { .. } => 1,
        Free { .. } => 2,
        CopyToDevice { .. } => 3,
        CopyToHost { .. } => 4,
        SetCalibration { .. } => 5,
        Signal { .. } => 6,
        Wait { .. } => 7,
        Barrier => 8,
        ProgramWeights { .. } => 9,
        Modulate { .. } => 10,
        Mvm { .. } => 11,
        Detect { .. } => 12,
        Gemm { .. } => 13,
        Elementwise { .. } => 14,
        Dft { .. } => 15,
        PrepareFock { .. } => 32,
        PrepareCoherent { .. } => 33,
        PrepareSqueezed { .. } => 34,
        Beamsplitter { .. } => 35,
        PhaseShift { .. } => 36,
        MeasurePnr { .. } => 64,
        MeasureHomodyne { .. } => 65,
        ConditionalPhase { .. } => 66,
        ReadRegister { .. } => 67,
    }
}

fn encode_instruction(w: &mut Writer, i: &Instruction) {
    use Instruction::*;
    w.u8(opcode(i));
    match i {
        Alloc { buffer } | Free { buffer } => w.u32(*buffer),
        CopyToDevice { src, dst } | CopyToHost { src, dst } => {
            w.u32(*src);
            w.u32(*dst);
        }
        SetCalibration {
            handle,
            fingerprint,
        } => {
            w.str(handle);
            w.str(fingerprint);
        }
        Signal { event } | Wait { event } => w.u32(*event),
        Barrier => {}
        ProgramWeights {
            weights,
            mesh,
            dac_bits,
        } => {
            w.u32(*weights);
            w.u32(*mesh);
            w.u8(*dac_bits);
        }
        Modulate {
            src,
            field,
            encoding,
            wavelength,
            spatial_mode,
            polarization,
            dac_bits,
        } => {
            w.u32(*src);
            w.u32(*field);
            w.u8(encoding_code(*encoding));
            w.u16(*wavelength);
            w.u16(*spatial_mode);
            w.u8(polarization_code(*polarization));
            w.u8(*dac_bits);
        }
        Mvm {
            field_in,
            mesh,
            field_out,
            tile_k,
        } => {
            w.u32(*field_in);
            w.u32(*mesh);
            w.u32(*field_out);
            w.u32(*tile_k);
        }
        Detect {
            field,
            dst,
            scheme,
            adc_bits,
        } => {
            w.u32(*field);
            w.u32(*dst);
            w.u8(scheme_code(*scheme));
            w.u8(*adc_bits);
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
            min_effective_bits,
            placement,
            fallback_reason,
        } => {
            for v in [a, b, c, m, n, k] {
                w.u32(*v);
            }
            w.bool(*transpose_b);
            w.u32(*tile_k);
            w.u16(*min_effective_bits);
            w.u8(placement_code(*placement));
            w.str(fallback_reason);
        }
        Elementwise { kind, inputs, dst } => {
            w.u8(ew_code(*kind));
            w.u16(u16::try_from(inputs.len()).expect("at most 65535 inputs"));
            for v in inputs {
                w.u32(*v);
            }
            w.u32(*dst);
        }
        Dft { src, dst, inverse } => {
            w.u32(*src);
            w.u32(*dst);
            w.bool(*inverse);
        }
        PrepareFock { mode, photons } => {
            w.u32(*mode);
            w.u8(*photons);
        }
        PrepareCoherent { mode, re, im } => {
            w.u32(*mode);
            w.f64(*re);
            w.f64(*im);
        }
        PrepareSqueezed { mode, r, phi } => {
            w.u32(*mode);
            w.f64(*r);
            w.f64(*phi);
        }
        Beamsplitter {
            mode_a,
            mode_b,
            theta,
            phi,
        } => {
            w.u32(*mode_a);
            w.u32(*mode_b);
            w.f64(*theta);
            w.f64(*phi);
        }
        PhaseShift { mode, phi } => {
            w.u32(*mode);
            w.f64(*phi);
        }
        MeasurePnr { mode, register } => {
            w.u32(*mode);
            w.u32(*register);
        }
        MeasureHomodyne {
            mode,
            phi,
            register,
        } => {
            w.u32(*mode);
            w.f64(*phi);
            w.u32(*register);
        }
        ConditionalPhase {
            register,
            mode,
            scale,
            offset,
        } => {
            w.u32(*register);
            w.u32(*mode);
            w.f64(*scale);
            w.f64(*offset);
        }
        ReadRegister { register, dst } => {
            w.u32(*register);
            w.u32(*dst);
        }
    }
}

/// Encode a program. Panics only if a string exceeds 65,535 bytes or a
/// buffer has more than 255 dimensions, which [`super::validate`] rejects.
pub fn encode(p: &IsaProgram) -> Vec<u8> {
    let mut w = Writer(Vec::with_capacity(64 + 32 * p.instructions.len()));
    w.0.extend_from_slice(ISA_MAGIC);
    w.u16(p.abi.major);
    w.u16(p.abi.minor);
    w.str(&p.program_id);
    w.u16(p.requirements.min_effective_bits);
    w.u32(p.requirements.max_tile_k);
    w.u8(p.requirements.quantum as u8 | (p.requirements.feed_forward as u8) << 1);
    w.u32(p.buffers.len() as u32);
    for b in &p.buffers {
        w.u32(b.id);
        w.u8(space_code(b.space));
        w.u8(dtype_code(b.dtype));
        w.u8(u8::try_from(b.shape.len()).expect("at most 255 dimensions"));
        for d in &b.shape {
            w.u64(*d);
        }
    }
    for v in [p.events, p.fields, p.meshes, p.modes, p.registers] {
        w.u32(v);
    }
    w.u32(p.instructions.len() as u32);
    for i in &p.instructions {
        encode_instruction(&mut w, i);
    }
    w.0
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        if self.bytes.len() - self.pos < n {
            return Err(CodecError::Truncated {
                offset: self.pos,
                needed: n,
            });
        }
        let s = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f64(&mut self) -> Result<f64, CodecError> {
        Ok(f64::from_bits(self.u64()?))
    }
    fn bool(&mut self) -> Result<bool, CodecError> {
        let offset = self.pos;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(CodecError::InvalidBool { offset, value }),
        }
    }
    fn str(&mut self) -> Result<String, CodecError> {
        let len = self.u16()? as usize;
        let offset = self.pos;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| CodecError::InvalidUtf8 { offset })
    }
    fn code<T>(
        &mut self,
        field: &'static str,
        f: impl Fn(u8) -> Option<T>,
    ) -> Result<T, CodecError> {
        let offset = self.pos;
        let value = self.u8()?;
        f(value).ok_or(CodecError::UnknownEnum {
            offset,
            field,
            value,
        })
    }
}

fn space_from(v: u8) -> Option<MemorySpace> {
    [MemorySpace::Host, MemorySpace::Device]
        .get(v as usize)
        .copied()
}
fn dtype_from(v: u8) -> Option<ElemType> {
    use ElemType::*;
    [F64, F32, Bf16, F16, I8, C64].get(v as usize).copied()
}
fn encoding_from(v: u8) -> Option<Encoding> {
    use Encoding::*;
    [Amplitude, Phase, Intensity].get(v as usize).copied()
}
fn polarization_from(v: u8) -> Option<Polarization> {
    [Polarization::Te, Polarization::Tm]
        .get(v as usize)
        .copied()
}
fn scheme_from(v: u8) -> Option<DetectionScheme> {
    use DetectionScheme::*;
    [Direct, Balanced, Coherent].get(v as usize).copied()
}
fn placement_from(v: u8) -> Option<Placement> {
    use Placement::*;
    [Photonic, Cpu, Gpu].get(v as usize).copied()
}
fn ew_from(v: u8) -> Option<ElementwiseKind> {
    use ElementwiseKind::*;
    [Add, Mul, Relu, Gelu, Copy].get(v as usize).copied()
}

fn decode_instruction(r: &mut Reader<'_>) -> Result<Instruction, CodecError> {
    use Instruction::*;
    let offset = r.pos;
    let op = r.u8()?;
    Ok(match op {
        1 => Alloc { buffer: r.u32()? },
        2 => Free { buffer: r.u32()? },
        3 => CopyToDevice {
            src: r.u32()?,
            dst: r.u32()?,
        },
        4 => CopyToHost {
            src: r.u32()?,
            dst: r.u32()?,
        },
        5 => SetCalibration {
            handle: r.str()?,
            fingerprint: r.str()?,
        },
        6 => Signal { event: r.u32()? },
        7 => Wait { event: r.u32()? },
        8 => Barrier,
        9 => ProgramWeights {
            weights: r.u32()?,
            mesh: r.u32()?,
            dac_bits: r.u8()?,
        },
        10 => Modulate {
            src: r.u32()?,
            field: r.u32()?,
            encoding: r.code("encoding", encoding_from)?,
            wavelength: r.u16()?,
            spatial_mode: r.u16()?,
            polarization: r.code("polarization", polarization_from)?,
            dac_bits: r.u8()?,
        },
        11 => Mvm {
            field_in: r.u32()?,
            mesh: r.u32()?,
            field_out: r.u32()?,
            tile_k: r.u32()?,
        },
        12 => Detect {
            field: r.u32()?,
            dst: r.u32()?,
            scheme: r.code("scheme", scheme_from)?,
            adc_bits: r.u8()?,
        },
        13 => Gemm {
            a: r.u32()?,
            b: r.u32()?,
            c: r.u32()?,
            m: r.u32()?,
            n: r.u32()?,
            k: r.u32()?,
            transpose_b: r.bool()?,
            tile_k: r.u32()?,
            min_effective_bits: r.u16()?,
            placement: r.code("placement", placement_from)?,
            fallback_reason: r.str()?,
        },
        14 => {
            let kind = r.code("elementwise", ew_from)?;
            let count = r.u16()? as usize;
            let mut inputs = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                inputs.push(r.u32()?);
            }
            Elementwise {
                kind,
                inputs,
                dst: r.u32()?,
            }
        }
        15 => Dft {
            src: r.u32()?,
            dst: r.u32()?,
            inverse: r.bool()?,
        },
        32 => PrepareFock {
            mode: r.u32()?,
            photons: r.u8()?,
        },
        33 => PrepareCoherent {
            mode: r.u32()?,
            re: r.f64()?,
            im: r.f64()?,
        },
        34 => PrepareSqueezed {
            mode: r.u32()?,
            r: r.f64()?,
            phi: r.f64()?,
        },
        35 => Beamsplitter {
            mode_a: r.u32()?,
            mode_b: r.u32()?,
            theta: r.f64()?,
            phi: r.f64()?,
        },
        36 => PhaseShift {
            mode: r.u32()?,
            phi: r.f64()?,
        },
        64 => MeasurePnr {
            mode: r.u32()?,
            register: r.u32()?,
        },
        65 => MeasureHomodyne {
            mode: r.u32()?,
            phi: r.f64()?,
            register: r.u32()?,
        },
        66 => ConditionalPhase {
            register: r.u32()?,
            mode: r.u32()?,
            scale: r.f64()?,
            offset: r.f64()?,
        },
        67 => ReadRegister {
            register: r.u32()?,
            dst: r.u32()?,
        },
        opcode => return Err(CodecError::UnknownOpcode { offset, opcode }),
    })
}

/// Decode a program; the result re-encodes to exactly `bytes`.
pub fn decode(bytes: &[u8]) -> Result<IsaProgram, CodecError> {
    let mut r = Reader { bytes, pos: 0 };
    if r.take(8).map_err(|_| CodecError::BadMagic)? != ISA_MAGIC {
        return Err(CodecError::BadMagic);
    }
    let major = r.u16()?;
    if major != ISA_MAJOR {
        return Err(CodecError::UnsupportedMajor(major));
    }
    let minor = r.u16()?;
    let program_id = r.str()?;
    let min_effective_bits = r.u16()?;
    let max_tile_k = r.u32()?;
    let flags = r.u8()?;
    if flags & !0b11 != 0 {
        return Err(CodecError::ReservedFlags { value: flags });
    }
    let buffer_count = r.u32()? as usize;
    let mut buffers = Vec::with_capacity(buffer_count.min(1024));
    for _ in 0..buffer_count {
        let id = r.u32()?;
        let space = r.code("space", space_from)?;
        let dtype = r.code("dtype", dtype_from)?;
        let rank = r.u8()? as usize;
        let mut shape = Vec::with_capacity(rank);
        for _ in 0..rank {
            shape.push(r.u64()?);
        }
        buffers.push(BufferDecl {
            id,
            space,
            dtype,
            shape,
        });
    }
    let events = r.u32()?;
    let fields = r.u32()?;
    let meshes = r.u32()?;
    let modes = r.u32()?;
    let registers = r.u32()?;
    let count = r.u32()? as usize;
    let mut instructions = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        instructions.push(decode_instruction(&mut r)?);
    }
    if r.pos != bytes.len() {
        return Err(CodecError::TrailingBytes {
            count: bytes.len() - r.pos,
        });
    }
    Ok(IsaProgram {
        version: ISA_JSON_VERSION.to_string(),
        abi: IsaVersion { major, minor },
        program_id,
        requirements: Requirements {
            min_effective_bits,
            max_tile_k,
            quantum: flags & 1 != 0,
            feed_forward: flags & 2 != 0,
        },
        buffers,
        events,
        fields,
        meshes,
        modes,
        registers,
        instructions,
    })
}
