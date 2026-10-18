//! Static validation of VPISA programs, version negotiation and target checks.
//!
//! [`validate`] walks the instruction stream once, tracking allocation,
//! calibration, field, mesh, event, mode and register state, and returns the
//! first rule violation as a typed [`IsaError`] carrying the instruction index.

use super::*;
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum IsaError {
    UnsupportedVersion {
        version: String,
        major: u16,
    },
    EmptyProgram,
    InvalidProgramId,
    DuplicateBuffer {
        buffer: u32,
    },
    InvalidBufferShape {
        buffer: u32,
    },
    UnknownBuffer {
        index: usize,
        buffer: u32,
    },
    WrongSpace {
        index: usize,
        buffer: u32,
        expected: MemorySpace,
    },
    NotAllocated {
        index: usize,
        buffer: u32,
    },
    AlreadyAllocated {
        index: usize,
        buffer: u32,
    },
    LeakedAllocation {
        buffer: u32,
    },
    ShapeMismatch {
        index: usize,
        detail: String,
    },
    WrongDtype {
        index: usize,
        buffer: u32,
    },
    InvalidParameter {
        index: usize,
        detail: String,
    },
    MissingCalibration {
        index: usize,
    },
    UnknownField {
        index: usize,
        field: u32,
    },
    FieldNotReady {
        index: usize,
        field: u32,
    },
    UnknownMesh {
        index: usize,
        mesh: u32,
    },
    MeshNotProgrammed {
        index: usize,
        mesh: u32,
    },
    UnknownEvent {
        index: usize,
        event: u32,
    },
    WaitWithoutSignal {
        index: usize,
        event: u32,
    },
    UndeclaredDomain {
        index: usize,
        domain: Domain,
    },
    UnknownMode {
        index: usize,
        mode: u32,
    },
    ModeNotPrepared {
        index: usize,
        mode: u32,
    },
    ModeMeasured {
        index: usize,
        mode: u32,
    },
    UnknownRegister {
        index: usize,
        register: u32,
    },
    RegisterNotWritten {
        index: usize,
        register: u32,
    },
    RequirementsIncomplete {
        detail: String,
    },
}

impl fmt::Display for IsaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VPISA validation error: {self:?}")
    }
}

impl std::error::Error for IsaError {}

/// Pick the version to run: same major, program minor not newer than the backend's.
pub fn negotiate(program: IsaVersion, supported: &[IsaVersion]) -> Option<IsaVersion> {
    supported
        .iter()
        .filter(|s| s.major == program.major && s.minor >= program.minor)
        .min()
        .copied()
}

/// What a target backend offers, for [`check_target`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IsaTarget {
    pub supported_versions: Vec<IsaVersion>,
    pub effective_bits: u16,
    pub max_tile_k: u32,
    pub quantum_modes: u32,
    pub feed_forward: bool,
}

/// Reasons a valid program cannot run on `target` (empty when it can).
pub fn check_target(p: &IsaProgram, target: &IsaTarget) -> Vec<String> {
    let mut reasons = Vec::new();
    if negotiate(p.abi, &target.supported_versions).is_none() {
        reasons.push(format!(
            "no compatible ISA version for {}.{}",
            p.abi.major, p.abi.minor
        ));
    }
    if p.requirements.min_effective_bits > target.effective_bits {
        reasons.push(format!(
            "needs {} effective bits, target offers {}",
            p.requirements.min_effective_bits, target.effective_bits
        ));
    }
    if p.requirements.max_tile_k > target.max_tile_k {
        reasons.push(format!(
            "needs tile_k {}, target maximum is {}",
            p.requirements.max_tile_k, target.max_tile_k
        ));
    }
    if p.requirements.quantum && p.modes > target.quantum_modes {
        reasons.push(format!(
            "needs {} quantum modes, target offers {}",
            p.modes, target.quantum_modes
        ));
    }
    if p.requirements.feed_forward && !target.feed_forward {
        reasons.push("needs measurement feed-forward".to_string());
    }
    reasons
}

#[derive(Clone, Copy)]
enum FieldState {
    Modulated { m: u64, k: u64 },
    Multiplied { m: u64, n: u64 },
}

#[derive(Clone, Copy, PartialEq)]
enum ModeState {
    Prepared,
    Measured,
}

fn check_bits(index: usize, name: &str, bits: u8) -> Result<(), IsaError> {
    if (2..=24).contains(&bits) {
        Ok(())
    } else {
        Err(IsaError::InvalidParameter {
            index,
            detail: format!("{name} must be in 2..=24, got {bits}"),
        })
    }
}

fn check_finite(index: usize, values: &[f64]) -> Result<(), IsaError> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(IsaError::InvalidParameter {
            index,
            detail: "non-finite parameter".to_string(),
        })
    }
}

struct State<'p> {
    p: &'p IsaProgram,
    allocated: HashSet<u32>,
    calibrated: bool,
    fields: HashMap<u32, FieldState>,
    meshes: HashMap<u32, (u64, u64)>,
    signaled: HashSet<u32>,
    modes: HashMap<u32, ModeState>,
    registers: HashSet<u32>,
    max_bits: u16,
    max_tile: u32,
    uses_quantum: bool,
    uses_feed_forward: bool,
}

impl<'p> State<'p> {
    fn buffer(&self, index: usize, id: u32) -> Result<&'p BufferDecl, IsaError> {
        self.p
            .buffer(id)
            .ok_or(IsaError::UnknownBuffer { index, buffer: id })
    }

    /// A buffer that is readable or writable now (host buffers always are).
    fn live(&self, index: usize, id: u32) -> Result<&'p BufferDecl, IsaError> {
        let b = self.buffer(index, id)?;
        if b.space == MemorySpace::Device && !self.allocated.contains(&id) {
            return Err(IsaError::NotAllocated { index, buffer: id });
        }
        Ok(b)
    }

    fn live_in(
        &self,
        index: usize,
        id: u32,
        space: MemorySpace,
    ) -> Result<&'p BufferDecl, IsaError> {
        let b = self.live(index, id)?;
        if b.space != space {
            return Err(IsaError::WrongSpace {
                index,
                buffer: id,
                expected: space,
            });
        }
        Ok(b)
    }

    fn field(&self, index: usize, field: u32) -> Result<(), IsaError> {
        if field >= self.p.fields {
            Err(IsaError::UnknownField { index, field })
        } else {
            Ok(())
        }
    }

    fn mesh(&self, index: usize, mesh: u32) -> Result<(), IsaError> {
        if mesh >= self.p.meshes {
            Err(IsaError::UnknownMesh { index, mesh })
        } else {
            Ok(())
        }
    }

    fn mode(&self, index: usize, mode: u32, need_prepared: bool) -> Result<(), IsaError> {
        if mode >= self.p.modes {
            return Err(IsaError::UnknownMode { index, mode });
        }
        if need_prepared {
            match self.modes.get(&mode) {
                None => return Err(IsaError::ModeNotPrepared { index, mode }),
                Some(ModeState::Measured) => return Err(IsaError::ModeMeasured { index, mode }),
                Some(ModeState::Prepared) => {}
            }
        }
        Ok(())
    }

    fn register(&self, index: usize, register: u32, need_written: bool) -> Result<(), IsaError> {
        if register >= self.p.registers {
            return Err(IsaError::UnknownRegister { index, register });
        }
        if need_written && !self.registers.contains(&register) {
            return Err(IsaError::RegisterNotWritten { index, register });
        }
        Ok(())
    }

    fn calibrated(&self, index: usize) -> Result<(), IsaError> {
        if self.calibrated {
            Ok(())
        } else {
            Err(IsaError::MissingCalibration { index })
        }
    }

    fn step(&mut self, index: usize, ins: &Instruction) -> Result<(), IsaError> {
        use Instruction::*;
        let shape_err = |detail: String| IsaError::ShapeMismatch { index, detail };
        match ins.domain() {
            Domain::Quantum | Domain::Interop if !self.p.requirements.quantum => {
                return Err(IsaError::UndeclaredDomain {
                    index,
                    domain: ins.domain(),
                })
            }
            Domain::Quantum | Domain::Interop => self.uses_quantum = true,
            Domain::Classical => {}
        }
        match ins {
            Alloc { buffer } => {
                let b = self.buffer(index, *buffer)?;
                if b.space != MemorySpace::Device {
                    return Err(IsaError::WrongSpace {
                        index,
                        buffer: *buffer,
                        expected: MemorySpace::Device,
                    });
                }
                if !self.allocated.insert(*buffer) {
                    return Err(IsaError::AlreadyAllocated {
                        index,
                        buffer: *buffer,
                    });
                }
            }
            Free { buffer } => {
                self.live_in(index, *buffer, MemorySpace::Device)?;
                self.allocated.remove(buffer);
            }
            CopyToDevice { src, dst } | CopyToHost { src, dst } => {
                let (from, to) = if matches!(ins, CopyToDevice { .. }) {
                    (MemorySpace::Host, MemorySpace::Device)
                } else {
                    (MemorySpace::Device, MemorySpace::Host)
                };
                let s = self.live_in(index, *src, from)?;
                let d = self.live_in(index, *dst, to)?;
                if s.shape != d.shape {
                    return Err(shape_err(format!("copy {:?} -> {:?}", s.shape, d.shape)));
                }
                if s.dtype != d.dtype {
                    return Err(IsaError::WrongDtype {
                        index,
                        buffer: *dst,
                    });
                }
            }
            SetCalibration {
                handle,
                fingerprint,
            } => {
                if handle.is_empty()
                    || !fingerprint.starts_with("sha256:")
                    || fingerprint.len() != 71
                {
                    return Err(IsaError::InvalidParameter {
                        index,
                        detail: "calibration needs a handle and a sha256:<64 hex> fingerprint"
                            .to_string(),
                    });
                }
                self.calibrated = true;
            }
            Signal { event } => {
                if *event >= self.p.events {
                    return Err(IsaError::UnknownEvent {
                        index,
                        event: *event,
                    });
                }
                self.signaled.insert(*event);
            }
            Wait { event } => {
                if *event >= self.p.events {
                    return Err(IsaError::UnknownEvent {
                        index,
                        event: *event,
                    });
                }
                if !self.signaled.contains(event) {
                    return Err(IsaError::WaitWithoutSignal {
                        index,
                        event: *event,
                    });
                }
            }
            Barrier => {}
            ProgramWeights {
                weights,
                mesh,
                dac_bits,
            } => {
                self.calibrated(index)?;
                check_bits(index, "dac_bits", *dac_bits)?;
                self.mesh(index, *mesh)?;
                let w = self.live_in(index, *weights, MemorySpace::Device)?;
                if w.shape.len() != 2 {
                    return Err(shape_err("weights must be [n, k]".to_string()));
                }
                self.meshes.insert(*mesh, (w.shape[0], w.shape[1]));
            }
            Modulate {
                src,
                field,
                dac_bits,
                ..
            } => {
                check_bits(index, "dac_bits", *dac_bits)?;
                self.field(index, *field)?;
                let s = self.live_in(index, *src, MemorySpace::Device)?;
                if s.shape.len() != 2 {
                    return Err(shape_err("modulated source must be [m, k]".to_string()));
                }
                self.fields.insert(
                    *field,
                    FieldState::Modulated {
                        m: s.shape[0],
                        k: s.shape[1],
                    },
                );
            }
            Mvm {
                field_in,
                mesh,
                field_out,
                tile_k,
            } => {
                self.calibrated(index)?;
                self.field(index, *field_in)?;
                self.field(index, *field_out)?;
                self.mesh(index, *mesh)?;
                if *tile_k == 0 {
                    return Err(IsaError::InvalidParameter {
                        index,
                        detail: "tile_k must be positive".to_string(),
                    });
                }
                self.max_tile = self.max_tile.max(*tile_k);
                let (m, k) = match self.fields.get(field_in) {
                    Some(FieldState::Modulated { m, k }) => (*m, *k),
                    _ => {
                        return Err(IsaError::FieldNotReady {
                            index,
                            field: *field_in,
                        })
                    }
                };
                let (n, wk) = *self
                    .meshes
                    .get(mesh)
                    .ok_or(IsaError::MeshNotProgrammed { index, mesh: *mesh })?;
                if wk != k {
                    return Err(shape_err(format!("field k={k} but mesh k={wk}")));
                }
                self.fields
                    .insert(*field_out, FieldState::Multiplied { m, n });
            }
            Detect {
                field,
                dst,
                adc_bits,
                ..
            } => {
                check_bits(index, "adc_bits", *adc_bits)?;
                self.field(index, *field)?;
                let (m, n) = match self.fields.get(field) {
                    Some(FieldState::Multiplied { m, n }) => (*m, *n),
                    _ => {
                        return Err(IsaError::FieldNotReady {
                            index,
                            field: *field,
                        })
                    }
                };
                let d = self.live_in(index, *dst, MemorySpace::Device)?;
                if d.shape != [m, n] {
                    return Err(shape_err(format!(
                        "detect into {:?}, expected [{m}, {n}]",
                        d.shape
                    )));
                }
                self.fields.remove(field);
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
                if *m == 0 || *n == 0 || *k == 0 || *tile_k == 0 {
                    return Err(IsaError::InvalidParameter {
                        index,
                        detail: "GEMM dimensions and tile_k must be positive".to_string(),
                    });
                }
                match placement {
                    Placement::Photonic => {
                        self.calibrated(index)?;
                        if *min_effective_bits == 0 {
                            return Err(IsaError::InvalidParameter {
                                index,
                                detail: "photonic GEMM needs min_effective_bits > 0".to_string(),
                            });
                        }
                        self.max_bits = self.max_bits.max(*min_effective_bits);
                        self.max_tile = self.max_tile.max(*tile_k);
                    }
                    _ if fallback_reason.is_empty() => {
                        return Err(IsaError::InvalidParameter {
                            index,
                            detail: "electronic fallback needs a fallback_reason".to_string(),
                        })
                    }
                    _ => {}
                }
                let (m, n, k) = (*m as u64, *n as u64, *k as u64);
                let expect_b = if *transpose_b { [n, k] } else { [k, n] };
                for (id, want) in [(*a, [m, k]), (*b, expect_b), (*c, [m, n])] {
                    let buf = self.live_in(index, id, MemorySpace::Device)?;
                    if buf.shape != want {
                        return Err(shape_err(format!(
                            "buffer {id} is {:?}, expected {want:?}",
                            buf.shape
                        )));
                    }
                }
            }
            Elementwise { kind, inputs, dst } => {
                if inputs.len() != kind.arity() {
                    return Err(IsaError::InvalidParameter {
                        index,
                        detail: format!("{kind:?} takes {} inputs", kind.arity()),
                    });
                }
                let d = self.live(index, *dst)?;
                for id in inputs {
                    let s = self.live(index, *id)?;
                    if s.shape != d.shape {
                        return Err(shape_err(format!("{:?} vs {:?}", s.shape, d.shape)));
                    }
                    if s.dtype == ElemType::C64 || d.dtype == ElemType::C64 {
                        return Err(IsaError::WrongDtype { index, buffer: *id });
                    }
                }
            }
            Dft { src, dst, .. } => {
                let s = self.live(index, *src)?;
                let d = self.live(index, *dst)?;
                for (id, buf) in [(*src, s), (*dst, d)] {
                    if buf.dtype != ElemType::C64 {
                        return Err(IsaError::WrongDtype { index, buffer: id });
                    }
                }
                if s.shape != d.shape {
                    return Err(shape_err(format!("{:?} vs {:?}", s.shape, d.shape)));
                }
            }
            PrepareFock { mode, .. } => {
                self.mode(index, *mode, false)?;
                self.modes.insert(*mode, ModeState::Prepared);
            }
            PrepareCoherent { mode, re, im } => {
                check_finite(index, &[*re, *im])?;
                self.mode(index, *mode, false)?;
                self.modes.insert(*mode, ModeState::Prepared);
            }
            PrepareSqueezed { mode, r, phi } => {
                check_finite(index, &[*r, *phi])?;
                if *r < 0.0 {
                    return Err(IsaError::InvalidParameter {
                        index,
                        detail: "squeezing r must be non-negative".to_string(),
                    });
                }
                self.mode(index, *mode, false)?;
                self.modes.insert(*mode, ModeState::Prepared);
            }
            Beamsplitter {
                mode_a,
                mode_b,
                theta,
                phi,
            } => {
                check_finite(index, &[*theta, *phi])?;
                if mode_a == mode_b {
                    return Err(IsaError::InvalidParameter {
                        index,
                        detail: "beamsplitter needs two distinct modes".to_string(),
                    });
                }
                self.mode(index, *mode_a, true)?;
                self.mode(index, *mode_b, true)?;
            }
            PhaseShift { mode, phi } => {
                check_finite(index, &[*phi])?;
                self.mode(index, *mode, true)?;
            }
            MeasurePnr { mode, register } => {
                self.mode(index, *mode, true)?;
                self.register(index, *register, false)?;
                self.modes.insert(*mode, ModeState::Measured);
                self.registers.insert(*register);
            }
            MeasureHomodyne {
                mode,
                phi,
                register,
            } => {
                check_finite(index, &[*phi])?;
                self.mode(index, *mode, true)?;
                self.register(index, *register, false)?;
                self.modes.insert(*mode, ModeState::Measured);
                self.registers.insert(*register);
            }
            ConditionalPhase {
                register,
                mode,
                scale,
                offset,
            } => {
                check_finite(index, &[*scale, *offset])?;
                if !self.p.requirements.feed_forward {
                    return Err(IsaError::UndeclaredDomain {
                        index,
                        domain: Domain::Interop,
                    });
                }
                self.uses_feed_forward = true;
                self.register(index, *register, true)?;
                self.mode(index, *mode, true)?;
            }
            ReadRegister { register, dst } => {
                self.register(index, *register, true)?;
                let d = self.live_in(index, *dst, MemorySpace::Host)?;
                if d.elements() != 1 {
                    return Err(shape_err(
                        "register destination must hold one element".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Check every static VPISA rule. Returns the first violation.
pub fn validate(p: &IsaProgram) -> Result<(), IsaError> {
    if p.version != ISA_JSON_VERSION || p.abi.major != ISA_MAJOR {
        return Err(IsaError::UnsupportedVersion {
            version: p.version.clone(),
            major: p.abi.major,
        });
    }
    if p.program_id.is_empty() || p.program_id.len() > 256 {
        return Err(IsaError::InvalidProgramId);
    }
    if p.instructions.is_empty() {
        return Err(IsaError::EmptyProgram);
    }
    let mut ids = HashSet::new();
    for b in &p.buffers {
        if !ids.insert(b.id) {
            return Err(IsaError::DuplicateBuffer { buffer: b.id });
        }
        if b.shape.is_empty() || b.shape.len() > 8 || b.shape.contains(&0) {
            return Err(IsaError::InvalidBufferShape { buffer: b.id });
        }
    }
    let mut state = State {
        p,
        allocated: HashSet::new(),
        calibrated: false,
        fields: HashMap::new(),
        meshes: HashMap::new(),
        signaled: HashSet::new(),
        modes: HashMap::new(),
        registers: HashSet::new(),
        max_bits: 0,
        max_tile: 0,
        uses_quantum: false,
        uses_feed_forward: false,
    };
    for (index, ins) in p.instructions.iter().enumerate() {
        state.step(index, ins)?;
    }
    if let Some(buffer) = state.allocated.iter().min() {
        return Err(IsaError::LeakedAllocation { buffer: *buffer });
    }
    let r = &p.requirements;
    if r.min_effective_bits < state.max_bits || r.max_tile_k < state.max_tile {
        return Err(IsaError::RequirementsIncomplete {
            detail: format!(
                "declared bits {} / tile {} but instructions need {} / {}",
                r.min_effective_bits, r.max_tile_k, state.max_bits, state.max_tile
            ),
        });
    }
    if r.quantum != state.uses_quantum || r.feed_forward != state.uses_feed_forward {
        return Err(IsaError::RequirementsIncomplete {
            detail: "quantum/feed_forward flags must match the instructions used".to_string(),
        });
    }
    Ok(())
}
