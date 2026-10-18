//! VPISA codec, validator and interpreter tests.

use awen_compiler::isa::codec::{decode, encode, CodecError};
use awen_compiler::isa::*;
use awen_compiler::noise::{NoiseRng, PhotonicNoiseContract};
use std::collections::HashMap;

const FP: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

fn gemm_program() -> IsaProgram {
    IsaProgram::lower_gemm("gemm-4x8x96", 4, 8, 96, 64, 8, 8, 4, "cal-0", FP)
}

fn spec_fixture(name: &str) -> IsaProgram {
    let path = format!(
        "{}/../awen-spec/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn operands(seed: u64, m: usize, n: usize, k: usize) -> (Vec<f64>, Vec<f64>) {
    let mut rng = NoiseRng::new(seed);
    let x = (0..m * k).map(|_| rng.uniform() * 2.0 - 1.0).collect();
    let w = (0..n * k).map(|_| rng.uniform() * 2.0 - 1.0).collect();
    (x, w)
}

fn exact(x: &[f64], w: &[f64], m: usize, n: usize, k: usize) -> Vec<f64> {
    let mut y = vec![0.0; m * n];
    for r in 0..m {
        for o in 0..n {
            y[r * n + o] = (0..k).map(|i| x[r * k + i] * w[o * k + i]).sum();
        }
    }
    y
}

// ------------------------------------------------------------------ codec

fn random_instruction(rng: &mut NoiseRng) -> Instruction {
    use Instruction::*;
    let u = |rng: &mut NoiseRng| (rng.next_u64() % 1000) as u32;
    let f = |rng: &mut NoiseRng| rng.normal() * 3.0;
    let s = |rng: &mut NoiseRng| {
        let len = (rng.next_u64() % 12) as usize;
        (0..len)
            .map(|i| ['a', 'Z', '7', 'λ', '-', '光'][(i + rng.next_u64() as usize) % 6])
            .collect::<String>()
    };
    match rng.next_u64() % 24 {
        0 => Alloc { buffer: u(rng) },
        1 => Free { buffer: u(rng) },
        2 => CopyToDevice {
            src: u(rng),
            dst: u(rng),
        },
        3 => CopyToHost {
            src: u(rng),
            dst: u(rng),
        },
        4 => SetCalibration {
            handle: s(rng),
            fingerprint: s(rng),
        },
        5 => Signal { event: u(rng) },
        6 => Wait { event: u(rng) },
        7 => Barrier,
        8 => ProgramWeights {
            weights: u(rng),
            mesh: u(rng),
            dac_bits: rng.next_u64() as u8,
        },
        9 => Modulate {
            src: u(rng),
            field: u(rng),
            encoding: [Encoding::Amplitude, Encoding::Phase, Encoding::Intensity]
                [(rng.next_u64() % 3) as usize],
            wavelength: rng.next_u64() as u16,
            spatial_mode: rng.next_u64() as u16,
            polarization: if rng.next_u64() & 1 == 0 {
                Polarization::Te
            } else {
                Polarization::Tm
            },
            dac_bits: rng.next_u64() as u8,
        },
        10 => Mvm {
            field_in: u(rng),
            mesh: u(rng),
            field_out: u(rng),
            tile_k: u(rng),
        },
        11 => Detect {
            field: u(rng),
            dst: u(rng),
            scheme: [
                DetectionScheme::Direct,
                DetectionScheme::Balanced,
                DetectionScheme::Coherent,
            ][(rng.next_u64() % 3) as usize],
            adc_bits: rng.next_u64() as u8,
        },
        12 => Gemm {
            a: u(rng),
            b: u(rng),
            c: u(rng),
            m: u(rng),
            n: u(rng),
            k: u(rng),
            transpose_b: rng.next_u64() & 1 == 0,
            tile_k: u(rng),
            min_effective_bits: rng.next_u64() as u16,
            placement: [Placement::Photonic, Placement::Cpu, Placement::Gpu]
                [(rng.next_u64() % 3) as usize],
            fallback_reason: s(rng),
        },
        13 => Elementwise {
            kind: [
                ElementwiseKind::Add,
                ElementwiseKind::Mul,
                ElementwiseKind::Relu,
                ElementwiseKind::Gelu,
                ElementwiseKind::Copy,
            ][(rng.next_u64() % 5) as usize],
            inputs: (0..rng.next_u64() % 4).map(|_| u(rng)).collect(),
            dst: u(rng),
        },
        14 => Dft {
            src: u(rng),
            dst: u(rng),
            inverse: rng.next_u64() & 1 == 0,
        },
        15 => PrepareFock {
            mode: u(rng),
            photons: rng.next_u64() as u8,
        },
        16 => PrepareCoherent {
            mode: u(rng),
            re: f(rng),
            im: f(rng),
        },
        17 => PrepareSqueezed {
            mode: u(rng),
            r: f(rng),
            phi: f(rng),
        },
        18 => Beamsplitter {
            mode_a: u(rng),
            mode_b: u(rng),
            theta: f(rng),
            phi: f(rng),
        },
        19 => PhaseShift {
            mode: u(rng),
            phi: f(rng),
        },
        20 => MeasurePnr {
            mode: u(rng),
            register: u(rng),
        },
        21 => MeasureHomodyne {
            mode: u(rng),
            phi: f(rng),
            register: u(rng),
        },
        22 => ConditionalPhase {
            register: u(rng),
            mode: u(rng),
            scale: f(rng),
            offset: f(rng),
        },
        _ => ReadRegister {
            register: u(rng),
            dst: u(rng),
        },
    }
}

fn random_program(rng: &mut NoiseRng) -> IsaProgram {
    let dtypes = [
        ElemType::F64,
        ElemType::F32,
        ElemType::Bf16,
        ElemType::F16,
        ElemType::I8,
        ElemType::C64,
    ];
    let buffers = (0..rng.next_u64() % 5)
        .map(|i| BufferDecl {
            id: i as u32,
            space: if rng.next_u64() & 1 == 0 {
                MemorySpace::Host
            } else {
                MemorySpace::Device
            },
            dtype: dtypes[(rng.next_u64() % 6) as usize],
            shape: (0..rng.next_u64() % 4)
                .map(|_| rng.next_u64() % 100_000)
                .collect(),
        })
        .collect();
    IsaProgram {
        version: ISA_JSON_VERSION.to_string(),
        abi: IsaVersion {
            major: 1,
            minor: (rng.next_u64() % 4) as u16,
        },
        program_id: format!("p{}", rng.next_u64()),
        requirements: Requirements {
            min_effective_bits: rng.next_u64() as u16,
            max_tile_k: rng.next_u64() as u32,
            quantum: rng.next_u64() & 1 == 0,
            feed_forward: rng.next_u64() & 1 == 0,
        },
        buffers,
        events: rng.next_u64() as u32,
        fields: rng.next_u64() as u32,
        meshes: rng.next_u64() as u32,
        modes: rng.next_u64() as u32,
        registers: rng.next_u64() as u32,
        instructions: (0..rng.next_u64() % 40)
            .map(|_| random_instruction(rng))
            .collect(),
    }
}

#[test]
fn every_instruction_kind_round_trips_byte_for_byte() {
    let mut rng = NoiseRng::new(2024);
    let mut kinds = std::collections::HashSet::new();
    for _ in 0..2000 {
        let program = random_program(&mut rng);
        for i in &program.instructions {
            kinds.insert(i.name());
        }
        let bytes = encode(&program);
        let decoded = decode(&bytes).expect("decodes");
        assert_eq!(decoded, program);
        assert_eq!(encode(&decoded), bytes);
        let json = serde_json::to_string(&program).unwrap();
        assert_eq!(serde_json::from_str::<IsaProgram>(&json).unwrap(), program);
    }
    assert_eq!(kinds.len(), 24, "generator covers every instruction kind");
}

#[test]
fn every_truncation_is_rejected() {
    let bytes = encode(&gemm_program());
    for cut in 0..bytes.len() {
        assert!(
            decode(&bytes[..cut]).is_err(),
            "prefix of {cut} bytes decoded"
        );
    }
}

#[test]
fn malformed_binaries_get_typed_errors() {
    let bytes = encode(&gemm_program());
    let mut extra = bytes.clone();
    extra.push(0);
    assert_eq!(decode(&extra), Err(CodecError::TrailingBytes { count: 1 }));
    let mut magic = bytes.clone();
    magic[0] = b'X';
    assert_eq!(decode(&magic), Err(CodecError::BadMagic));
    let mut major = bytes.clone();
    major[8] = 2;
    assert_eq!(decode(&major), Err(CodecError::UnsupportedMajor(2)));
    // Header layout: magic 8, major 2, minor 2, id (2 + len), bits 2, tile 4, flags 1.
    let id_len = gemm_program().program_id.len();
    let flags_at = 8 + 4 + 2 + id_len + 2 + 4;
    let mut flags = bytes.clone();
    flags[flags_at] = 0b100;
    assert_eq!(decode(&flags), Err(CodecError::ReservedFlags { value: 4 }));
    // The first instruction opcode follows the buffers and the five counts.
    let one = IsaProgram {
        instructions: vec![Instruction::Barrier],
        buffers: vec![],
        ..gemm_program()
    };
    let mut b = encode(&one);
    let at = b.len() - 1;
    b[at] = 200;
    assert!(matches!(
        decode(&b),
        Err(CodecError::UnknownOpcode { opcode: 200, .. })
    ));
    let dft = IsaProgram {
        instructions: vec![Instruction::Dft {
            src: 0,
            dst: 1,
            inverse: true,
        }],
        buffers: vec![],
        ..gemm_program()
    };
    let mut b = encode(&dft);
    let at = b.len() - 1;
    b[at] = 7;
    assert!(matches!(
        decode(&b),
        Err(CodecError::InvalidBool { value: 7, .. })
    ));
    let cal = IsaProgram {
        instructions: vec![Instruction::SetCalibration {
            handle: "ab".into(),
            fingerprint: String::new(),
        }],
        buffers: vec![],
        ..gemm_program()
    };
    let mut b = encode(&cal);
    let at = b.len() - 4; // handle bytes are followed by the empty fingerprint length
    b[at] = 0xFF;
    assert!(matches!(decode(&b), Err(CodecError::InvalidUtf8 { .. })));
    let det = IsaProgram {
        instructions: vec![Instruction::Detect {
            field: 0,
            dst: 0,
            scheme: DetectionScheme::Direct,
            adc_bits: 8,
        }],
        buffers: vec![],
        ..gemm_program()
    };
    let mut b = encode(&det);
    let at = b.len() - 2;
    b[at] = 9;
    assert!(matches!(
        decode(&b),
        Err(CodecError::UnknownEnum {
            field: "scheme",
            value: 9,
            ..
        })
    ));
}

#[test]
fn json_rejects_unknown_fields_and_ops() {
    let mut v = serde_json::to_value(gemm_program()).unwrap();
    v["surprise"] = serde_json::json!(1);
    assert!(serde_json::from_value::<IsaProgram>(v).is_err());
    let mut v = serde_json::to_value(gemm_program()).unwrap();
    v["instructions"][0] = serde_json::json!({"op": "teleport", "buffer": 1});
    assert!(serde_json::from_value::<IsaProgram>(v).is_err());
}

#[test]
fn digest_is_stable_and_content_addressed() {
    let a = gemm_program();
    assert_eq!(a.digest(), gemm_program().digest());
    let mut b = gemm_program();
    b.program_id.push('x');
    assert_ne!(a.digest(), b.digest());
    assert!(a.digest().starts_with("sha256:") && a.digest().len() == 71);
}

// --------------------------------------------------------------- validator

#[test]
fn reference_programs_validate() {
    validate(&gemm_program()).unwrap();
    validate(&spec_fixture("vpisa_quantum_feed_forward.json")).unwrap();
    validate(&spec_fixture("vpisa_gemm.json")).unwrap();
}

fn expect(p: &IsaProgram, check: impl Fn(&IsaError) -> bool) {
    let err = validate(p).expect_err("program should be rejected");
    assert!(check(&err), "unexpected error {err:?}");
}

fn with(f: impl FnOnce(&mut IsaProgram)) -> IsaProgram {
    let mut p = gemm_program();
    f(&mut p);
    p
}

fn quantum(instructions: Vec<Instruction>, feed_forward: bool) -> IsaProgram {
    let mut p = spec_fixture("vpisa_quantum_feed_forward.json");
    p.requirements.feed_forward = feed_forward;
    p.instructions = instructions;
    p
}

#[test]
fn validator_rejects_each_rule() {
    use Instruction::*;
    expect(&with(|p| p.version = "awen.vpisa.v2".into()), |e| {
        matches!(e, IsaError::UnsupportedVersion { .. })
    });
    expect(&with(|p| p.abi.major = 2), |e| {
        matches!(e, IsaError::UnsupportedVersion { major: 2, .. })
    });
    expect(&with(|p| p.instructions.clear()), |e| {
        *e == IsaError::EmptyProgram
    });
    expect(&with(|p| p.program_id.clear()), |e| {
        *e == IsaError::InvalidProgramId
    });
    expect(&with(|p| p.buffers[1].id = 0), |e| {
        *e == IsaError::DuplicateBuffer { buffer: 0 }
    });
    expect(&with(|p| p.buffers[0].shape = vec![4, 0]), |e| {
        *e == IsaError::InvalidBufferShape { buffer: 0 }
    });
    expect(
        &with(|p| p.instructions[3] = CopyToDevice { src: 99, dst: 10 }),
        |e| {
            *e == IsaError::UnknownBuffer {
                index: 3,
                buffer: 99,
            }
        },
    );
    expect(&with(|p| p.instructions[0] = Alloc { buffer: 0 }), |e| {
        matches!(e, IsaError::WrongSpace { buffer: 0, .. })
    });
    expect(
        &with(|p| {
            p.instructions.remove(0);
        }),
        |e| matches!(e, IsaError::NotAllocated { buffer: 10, .. }),
    );
    expect(
        &with(|p| p.instructions.insert(1, Alloc { buffer: 10 })),
        |e| matches!(e, IsaError::AlreadyAllocated { buffer: 10, .. }),
    );
    expect(
        &with(|p| {
            p.instructions.pop();
        }),
        |e| *e == IsaError::LeakedAllocation { buffer: 12 },
    );
    expect(&with(|p| p.buffers[0].shape = vec![4, 95]), |e| {
        matches!(e, IsaError::ShapeMismatch { index: 3, .. })
    });
    expect(&with(|p| p.buffers[0].dtype = ElemType::F32), |e| {
        matches!(e, IsaError::WrongDtype { index: 3, .. })
    });
    expect(
        &with(|p| {
            p.instructions[6] = ProgramWeights {
                weights: 11,
                mesh: 0,
                dac_bits: 1,
            }
        }),
        |e| matches!(e, IsaError::InvalidParameter { index: 6, .. }),
    );
    expect(
        &with(|p| {
            p.instructions.remove(5);
        }),
        |e| *e == IsaError::MissingCalibration { index: 5 },
    );
    expect(&with(|p| p.fields = 1), |e| {
        matches!(e, IsaError::UnknownField { field: 1, .. })
    });
    expect(
        &with(|p| {
            p.instructions.remove(7);
        }),
        |e| matches!(e, IsaError::FieldNotReady { field: 0, .. }),
    );
    expect(&with(|p| p.meshes = 0), |e| {
        matches!(e, IsaError::UnknownMesh { mesh: 0, .. })
    });
    expect(
        &with(|p| {
            p.meshes = 2;
            p.instructions[8] = Mvm {
                field_in: 0,
                mesh: 1,
                field_out: 1,
                tile_k: 64,
            };
        }),
        |e| matches!(e, IsaError::MeshNotProgrammed { mesh: 1, .. }),
    );
    expect(&with(|p| p.instructions[10] = Signal { event: 5 }), |e| {
        matches!(e, IsaError::UnknownEvent { event: 5, .. })
    });
    expect(
        &with(|p| {
            p.instructions.remove(10);
        }),
        |e| {
            *e == IsaError::WaitWithoutSignal {
                index: 10,
                event: 0,
            }
        },
    );
    expect(&with(|p| p.requirements.max_tile_k = 32), |e| {
        matches!(e, IsaError::RequirementsIncomplete { .. })
    });
    expect(&with(|p| p.requirements.quantum = true), |e| {
        matches!(e, IsaError::RequirementsIncomplete { .. })
    });
}

#[test]
fn gemm_rules() {
    use Instruction::*;
    let gemm = |placement, reason: &str, bits| Gemm {
        a: 10,
        b: 11,
        c: 12,
        m: 4,
        n: 8,
        k: 96,
        transpose_b: true,
        tile_k: 64,
        min_effective_bits: bits,
        placement,
        fallback_reason: reason.into(),
    };
    let mut p = gemm_program();
    p.instructions
        .splice(6..10, [gemm(Placement::Photonic, "", 4)]);
    validate(&p).unwrap();
    let mut q = p.clone();
    q.instructions[6] = gemm(Placement::Cpu, "", 0);
    expect(&q, |e| {
        matches!(e, IsaError::InvalidParameter { index: 6, .. })
    });
    q.instructions[6] = gemm(
        Placement::Gpu,
        "operator not supported by the photonic core",
        0,
    );
    validate(&q).unwrap();
    q.instructions[6] = gemm(Placement::Photonic, "", 9);
    expect(&q, |e| matches!(e, IsaError::RequirementsIncomplete { .. }));
    let mut flipped = gemm(Placement::Photonic, "", 4);
    if let Gemm { transpose_b, .. } = &mut flipped {
        *transpose_b = false;
    }
    q.instructions[6] = flipped;
    expect(&q, |e| {
        matches!(e, IsaError::ShapeMismatch { index: 6, .. })
    });
}

#[test]
fn domains_only_meet_through_interop() {
    use Instruction::*;
    // Quantum instructions in a program that did not declare the domain.
    let mut classical = gemm_program();
    classical.modes = 1;
    classical.instructions.insert(
        0,
        PrepareFock {
            mode: 0,
            photons: 1,
        },
    );
    expect(&classical, |e| {
        matches!(
            e,
            IsaError::UndeclaredDomain {
                index: 0,
                domain: Domain::Quantum
            }
        )
    });
    // Feed-forward without declaring it.
    let p = quantum(
        vec![
            PrepareFock {
                mode: 0,
                photons: 1,
            },
            PrepareFock {
                mode: 1,
                photons: 0,
            },
            MeasurePnr {
                mode: 0,
                register: 0,
            },
            ConditionalPhase {
                register: 0,
                mode: 1,
                scale: 1.0,
                offset: 0.0,
            },
        ],
        false,
    );
    expect(&p, |e| {
        matches!(
            e,
            IsaError::UndeclaredDomain {
                index: 3,
                domain: Domain::Interop
            }
        )
    });
    // A register can only be read after a measurement wrote it.
    let p = quantum(
        vec![
            PrepareFock {
                mode: 1,
                photons: 0,
            },
            ConditionalPhase {
                register: 0,
                mode: 1,
                scale: 1.0,
                offset: 0.0,
            },
        ],
        true,
    );
    expect(&p, |e| {
        *e == IsaError::RegisterNotWritten {
            index: 1,
            register: 0,
        }
    });
    let p = quantum(
        vec![
            PrepareFock {
                mode: 0,
                photons: 1,
            },
            MeasurePnr {
                mode: 0,
                register: 3,
            },
        ],
        false,
    );
    expect(&p, |e| {
        matches!(e, IsaError::UnknownRegister { register: 3, .. })
    });
    let p = quantum(vec![PhaseShift { mode: 0, phi: 0.1 }], false);
    expect(&p, |e| {
        *e == IsaError::ModeNotPrepared { index: 0, mode: 0 }
    });
    let p = quantum(
        vec![PrepareFock {
            mode: 5,
            photons: 1,
        }],
        false,
    );
    expect(&p, |e| matches!(e, IsaError::UnknownMode { mode: 5, .. }));
    let p = quantum(
        vec![
            PrepareFock {
                mode: 0,
                photons: 1,
            },
            MeasurePnr {
                mode: 0,
                register: 0,
            },
            PhaseShift { mode: 0, phi: 0.1 },
        ],
        false,
    );
    expect(&p, |e| *e == IsaError::ModeMeasured { index: 2, mode: 0 });
    let p = quantum(
        vec![
            PrepareFock {
                mode: 0,
                photons: 1,
            },
            Beamsplitter {
                mode_a: 0,
                mode_b: 0,
                theta: 0.3,
                phi: 0.0,
            },
        ],
        false,
    );
    expect(&p, |e| {
        matches!(e, IsaError::InvalidParameter { index: 1, .. })
    });
    let p = quantum(
        vec![PrepareSqueezed {
            mode: 0,
            r: f64::NAN,
            phi: 0.0,
        }],
        false,
    );
    expect(&p, |e| {
        matches!(e, IsaError::InvalidParameter { index: 0, .. })
    });
}

#[test]
fn negotiation_and_target_checks() {
    let v = |major, minor| IsaVersion { major, minor };
    assert_eq!(
        negotiate(v(1, 0), &[v(1, 2), v(1, 1), v(2, 0)]),
        Some(v(1, 1))
    );
    assert_eq!(negotiate(v(1, 3), &[v(1, 2)]), None);
    assert_eq!(negotiate(v(1, 0), &[v(2, 0)]), None);
    let target = IsaTarget {
        supported_versions: vec![v(1, 0)],
        effective_bits: 6,
        max_tile_k: 128,
        quantum_modes: 0,
        feed_forward: false,
    };
    assert!(check_target(&gemm_program(), &target).is_empty());
    let weak = IsaTarget {
        effective_bits: 3,
        max_tile_k: 32,
        ..target.clone()
    };
    assert_eq!(check_target(&gemm_program(), &weak).len(), 2);
    let q = spec_fixture("vpisa_quantum_feed_forward.json");
    assert_eq!(check_target(&q, &target).len(), 2);
}

// ------------------------------------------------------------- interpreter

#[test]
fn decomposed_sequence_matches_fused_gemm_and_exact_math() {
    let (m, n, k) = (4usize, 8usize, 96usize);
    let (x, w) = operands(5, m, n, k);
    let inputs: HashMap<u32, Vec<f64>> = [(0, x.clone()), (1, w.clone())].into();
    let reference = exact(&x, &w, m, n, k);

    let ideal = run(&gemm_program(), &inputs, &PhotonicNoiseContract::ideal(), 0).unwrap();
    for (a, b) in ideal.host[&2].iter().zip(&reference) {
        assert!((a - b).abs() < 1e-9);
    }

    let contract = PhotonicNoiseContract::default();
    let decomposed = run(&gemm_program(), &inputs, &contract, 11).unwrap();
    let mut fused = gemm_program();
    fused.instructions.splice(
        6..10,
        [Instruction::Gemm {
            a: 10,
            b: 11,
            c: 12,
            m: 4,
            n: 8,
            k: 96,
            transpose_b: true,
            tile_k: 64,
            min_effective_bits: 4,
            placement: Placement::Photonic,
            fallback_reason: String::new(),
        }],
    );
    let fused_run = run(&fused, &inputs, &contract, 11).unwrap();
    assert_eq!(
        decomposed.host[&2], fused_run.host[&2],
        "same noise seed, same converters"
    );
    assert_ne!(decomposed.host[&2], reference, "default contract is noisy");
    assert_eq!(decomposed.photonic_ops, 1);
    assert!(decomposed
        .trace
        .iter()
        .any(|t| t.op == "detect" && t.fidelity.starts_with("simulated")));
    assert_eq!(decomposed.program_digest, gemm_program().digest());
}

#[test]
fn fallback_gemm_and_electronic_ops_are_exact() {
    use Instruction::*;
    let (m, n, k) = (3usize, 2usize, 5usize);
    let (x, w) = operands(9, m, n, k);
    // b stored [k, n] (transpose_b = false).
    let wt: Vec<f64> = (0..k)
        .flat_map(|i| (0..n).map(move |o| (i, o)))
        .map(|(i, o)| w[o * k + i])
        .collect();
    let decl = |id, space, shape: Vec<u64>| BufferDecl {
        id,
        space,
        dtype: ElemType::F64,
        shape,
    };
    let p = IsaProgram {
        version: ISA_JSON_VERSION.into(),
        abi: IsaVersion::CURRENT,
        program_id: "fallback-mlp".into(),
        requirements: Requirements {
            min_effective_bits: 0,
            max_tile_k: 0,
            quantum: false,
            feed_forward: false,
        },
        buffers: vec![
            decl(0, MemorySpace::Host, vec![3, 5]),
            decl(1, MemorySpace::Host, vec![5, 2]),
            decl(2, MemorySpace::Host, vec![3, 2]),
            decl(10, MemorySpace::Device, vec![3, 5]),
            decl(11, MemorySpace::Device, vec![5, 2]),
            decl(12, MemorySpace::Device, vec![3, 2]),
            decl(13, MemorySpace::Device, vec![3, 2]),
        ],
        events: 0,
        fields: 0,
        meshes: 0,
        modes: 0,
        registers: 0,
        instructions: vec![
            Alloc { buffer: 10 },
            Alloc { buffer: 11 },
            Alloc { buffer: 12 },
            Alloc { buffer: 13 },
            CopyToDevice { src: 0, dst: 10 },
            CopyToDevice { src: 1, dst: 11 },
            Gemm {
                a: 10,
                b: 11,
                c: 12,
                m: 3,
                n: 2,
                k: 5,
                transpose_b: false,
                tile_k: 64,
                min_effective_bits: 0,
                placement: Placement::Cpu,
                fallback_reason: "no calibrated photonic core".into(),
            },
            Elementwise {
                kind: ElementwiseKind::Relu,
                inputs: vec![12],
                dst: 13,
            },
            Elementwise {
                kind: ElementwiseKind::Add,
                inputs: vec![13, 12],
                dst: 13,
            },
            CopyToHost { src: 13, dst: 2 },
            Free { buffer: 10 },
            Free { buffer: 11 },
            Free { buffer: 12 },
            Free { buffer: 13 },
        ],
    };
    let out = run(
        &p,
        &[(0, x.clone()), (1, wt)].into(),
        &PhotonicNoiseContract::default(),
        0,
    )
    .unwrap();
    let y = exact(&x, &w, m, n, k);
    let want: Vec<f64> = y.iter().map(|v| v.max(0.0) + v).collect();
    for (a, b) in out.host[&2].iter().zip(&want) {
        assert!((a - b).abs() < 1e-12);
    }
    assert_eq!(out.photonic_ops, 0);
    assert!(out.trace.iter().all(|t| t.fidelity.starts_with("exact")));
}

#[test]
fn dft_round_trip_and_delta_spectrum() {
    use Instruction::*;
    let c = |id, space| BufferDecl {
        id,
        space,
        dtype: ElemType::C64,
        shape: vec![8],
    };
    let p = IsaProgram {
        version: ISA_JSON_VERSION.into(),
        abi: IsaVersion::CURRENT,
        program_id: "dft".into(),
        requirements: Requirements {
            min_effective_bits: 0,
            max_tile_k: 0,
            quantum: false,
            feed_forward: false,
        },
        buffers: vec![
            c(0, MemorySpace::Host),
            c(1, MemorySpace::Host),
            c(2, MemorySpace::Host),
        ],
        events: 0,
        fields: 0,
        meshes: 0,
        modes: 0,
        registers: 0,
        instructions: vec![
            Dft {
                src: 0,
                dst: 1,
                inverse: false,
            },
            Dft {
                src: 1,
                dst: 2,
                inverse: true,
            },
        ],
    };
    let mut delta = vec![0.0; 16];
    delta[0] = 1.0;
    let out = run(
        &p,
        &[(0, delta.clone())].into(),
        &PhotonicNoiseContract::ideal(),
        0,
    )
    .unwrap();
    for f in 0..8 {
        assert!((out.host[&1][2 * f] - 1.0).abs() < 1e-12 && out.host[&1][2 * f + 1].abs() < 1e-12);
    }
    for (a, b) in out.host[&2].iter().zip(&delta) {
        assert!((a - b).abs() < 1e-12);
    }
}

#[test]
fn quantum_programs_are_refused_by_the_classical_interpreter() {
    let q = spec_fixture("vpisa_quantum_feed_forward.json");
    assert!(matches!(
        run(&q, &HashMap::new(), &PhotonicNoiseContract::ideal(), 0),
        Err(InterpreterError::Unsupported {
            index: 0,
            op: "prepare_squeezed"
        })
    ));
}

#[test]
fn gemm_fixture_equals_the_lowering() {
    assert_eq!(spec_fixture("vpisa_gemm.json"), gemm_program());
}

#[test]
fn json_schema_lists_exactly_the_implemented_instructions() {
    let path = format!(
        "{}/../awen-spec/schemas/awen_vpisa.v1.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut in_schema: Vec<String> = schema["$defs"]
        .as_object()
        .unwrap()
        .values()
        .map(|d| d["properties"]["op"]["const"].as_str().unwrap().to_string())
        .collect();
    in_schema.sort();
    let mut rng = NoiseRng::new(1);
    let mut implemented = std::collections::BTreeSet::new();
    while implemented.len() < 24 {
        implemented.insert(random_instruction(&mut rng).name().to_string());
    }
    assert_eq!(in_schema, implemented.into_iter().collect::<Vec<_>>());
}
