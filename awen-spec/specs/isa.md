# Virtual Photonic ISA (VPISA) v1 specification

Normative companion to AEP-0024. JSON schema:
`awen-spec/schemas/awen_vpisa.v1.json`. Reference implementation:
`awen-compiler/src/isa/`.

## Program

| Field | Meaning |
| --- | --- |
| `version` | `"awen.vpisa.v1"` |
| `abi` | `{major: 1, minor}` |
| `program_id` | 1–256 bytes UTF-8 |
| `requirements` | `min_effective_bits` (u16), `max_tile_k` (u32), `quantum`, `feed_forward` |
| `buffers` | `{id, space: host|device, dtype: f64|f32|bf16|f16|i8|c64, shape: 1–8 positive dims}` |
| `events`, `fields`, `meshes`, `modes`, `registers` | number of event, optical-field, weight-mesh, quantum-mode and classical-register slots; ids are `0..count` |
| `instructions` | non-empty list, executed in order |

## Instructions

Opcodes are stable within major version 1. Operand order in the binary form
is the order listed.

| Opcode | `op` | Domain | Operands |
| ---: | --- | --- | --- |
| 1 | `alloc` | classical | buffer u32 (device) |
| 2 | `free` | classical | buffer u32 (device, allocated) |
| 3 | `copy_to_device` | classical | src u32 (host), dst u32 (device); equal shape and dtype |
| 4 | `copy_to_host` | classical | src u32 (device), dst u32 (host) |
| 5 | `set_calibration` | classical | handle str, fingerprint str (`sha256:` + 64 hex) |
| 6 | `signal` | classical | event u32 |
| 7 | `wait` | classical | event u32 (must have been signaled) |
| 8 | `barrier` | classical | — |
| 9 | `program_weights` | classical | weights u32 (device `[n,k]`), mesh u32, dac_bits u8 (2–24) |
| 10 | `modulate` | classical | src u32 (device `[m,k]`), field u32, encoding u8 (amplitude/phase/intensity), wavelength u16, spatial_mode u16, polarization u8 (te/tm), dac_bits u8 |
| 11 | `mvm` | classical | field_in u32 (modulated), mesh u32 (programmed, same k), field_out u32, tile_k u32 > 0 |
| 12 | `detect` | classical | field u32 (multiplied), dst u32 (device `[m,n]`), scheme u8 (direct/balanced/coherent), adc_bits u8 |
| 13 | `gemm` | classical | a, b, c, m, n, k u32; transpose_b bool (b is `[n,k]` if true, else `[k,n]`); tile_k u32; min_effective_bits u16; placement u8 (photonic/cpu/gpu); fallback_reason str (required unless photonic) |
| 14 | `elementwise` | classical | kind u8 (add/mul/relu/gelu/copy), inputs u16 count + u32 each, dst u32; equal shapes; not c64 |
| 15 | `dft` | classical | src, dst u32 (c64, equal shape), inverse bool; along the last axis; inverse scales by 1/n |
| 32 | `prepare_fock` | quantum | mode u32, photons u8 |
| 33 | `prepare_coherent` | quantum | mode u32, re f64, im f64 |
| 34 | `prepare_squeezed` | quantum | mode u32, r f64 ≥ 0, phi f64 |
| 35 | `beamsplitter` | quantum | mode_a, mode_b u32 (distinct, prepared), theta f64, phi f64 |
| 36 | `phase_shift` | quantum | mode u32 (prepared), phi f64 |
| 64 | `measure_pnr` | interop | mode u32 → register u32 (destructive) |
| 65 | `measure_homodyne` | interop | mode u32, phi f64 → register u32 (destructive) |
| 66 | `conditional_phase` | interop | register u32 (written), mode u32; phase = scale·register + offset; needs `feed_forward` |
| 67 | `read_register` | interop | register u32 (written) → dst u32 (one-element host buffer) |

## Binary encoding

All integers are little-endian. Strings are a u16 byte length followed by
UTF-8. f64 values are stored as IEEE 754 bit patterns. Booleans are one byte,
0 or 1.

```text
8   "AWENISA\0"
2   major (= 1)          2   minor
str program_id
2   min_effective_bits   4   max_tile_k
1   flags: bit 0 quantum, bit 1 feed_forward (other bits must be 0)
4   buffer count; each: u32 id, u8 space (0 host, 1 device),
    u8 dtype (0 f64, 1 f32, 2 bf16, 3 f16, 4 i8, 5 c64), u8 rank, rank × u64
4×5 events, fields, meshes, modes, registers
4   instruction count; each: u8 opcode, then operands as in the table
```

Enum codes follow the order in the operand column, starting at 0. A decoder
must reject: bad magic, other major versions, truncation, invalid UTF-8,
unknown opcodes or enum codes, booleans other than 0/1, reserved flag bits,
and trailing bytes.

## Static rules

The reference validator checks, in instruction order:

- **Version:** supported version.
- **Buffers:** a valid program id; a non-empty instruction list; unique
  buffer ids and valid shapes; every referenced buffer is declared and in the
  right memory space; device buffers are used only between `alloc` and
  `free`, and none is left allocated at the end; shapes and dtypes match.
- **Converters and calibration:** converter resolutions are in 2–24 bits;
  photonic instructions follow `set_calibration`.
- **Dataflow:** fields and meshes are in a valid state (`modulate` before
  `mvm`, `mvm` before `detect`, `program_weights` before `mvm`); every `wait`
  follows a `signal`.
- **Domains:** quantum and interop instructions appear only in programs that
  declare `quantum`, and feed-forward only with `feed_forward`. Modes are
  prepared before use and not used after a destructive measurement.
  Registers are written before they are read. Every f64 parameter is finite.
- **Requirements:** `requirements` covers what the instructions need, and the
  `quantum`/`feed_forward` flags match the instructions used.

## Execution semantics (reference interpreter)

A photonic `gemm`, and the `program_weights → modulate → mvm → detect`
sequence, compute `x · wᵀ` on the AEP-0022 tile model. The decomposed form
takes its DAC and ADC resolutions from the instructions. Photonic operation
number *j* in a run uses noise seed `seed + j`. Electronic operations are
exact in f64. Results carry a trace with each instruction's placement and
fidelity label: photonic results are `simulated`, electronic ones `exact`.
