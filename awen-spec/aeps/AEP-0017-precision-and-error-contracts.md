# AEP-0017: Precision, scaling, bit slicing, and error contracts

Status: Accepted and implemented

## Summary

AWEN makes numerical precision an explicit, versioned part of Tensor IR,
Photonic IR, Device IR, capability negotiation, cost estimation, autotuning,
and conformance evidence. The system represents storage, compute, output, and
accumulator formats independently; supports tensor, channel, and block scaling;
defines signed bit-slice semantics; models saturation and accumulator overflow;
applies deterministic analog noise and measured calibration compensation; and
reports every error source separately.

The normative serialized contracts introduced by this proposal are:

- `awen.precision.v1`, embedded in `awen.tensor.v1`;
- the expanded precision record in `awen.photonic.classical.v1`;
- explicit conversion, bit-slice, accumulation, download, and rescale commands
  in `awen.device.v1`; and
- `awen.error-report.v1` for static and empirical attribution.

## Motivation

A photonic compilation cannot treat dtype as one label inherited from the
source tensor. The stored values, digital conversion, optical effective bits,
accumulator, and returned values may all have different representations.
Calibration and analog noise introduce errors that are qualitatively different
from quantization or floating-point summation. A compiler that combines these
effects into one opaque tolerance cannot explain a rejection, compare tuning
plans fairly, or reproduce a numerical result.

The previous lowering recorded only a source dtype, ADC/DAC width, effective
bits, bit-slice count, and whether accumulation was digital. Conversion and
scaling were implicit. The simulator used one max-absolute symmetric
quantizer, calibration was exactly inverted, analog noise was absent, and the
cost model returned one aggregate fraction. That was insufficient for signed
integer extremes, per-channel quantization, block floating point, accumulator
overflow, calibration-aware rescaling, or evidence-backed error contracts.

## Precision domains

Each GEMM has four independent precision domains:

1. Storage dtype is the dtype declared on each Tensor IR value.
2. Compute dtype is the representation selected for the photonic operation.
3. Accumulator dtype is the representation used to combine partial products,
   K tiles, and bit-slice passes.
4. Output dtype is the dtype promised to downstream Tensor IR consumers.

The v1 dtypes are `f32`, `f16`, `bf16`, `int8`, `int4`, and `complex_f32`.
The v1 accumulator dtypes are `f32`, `f64`, `i32`, `i64`, and `optical`.
Optical accumulation requires the optical accumulator. Digital and hybrid
accumulation require a digital accumulator. Integer compute requires i32 or
i64 digital accumulation; floating/complex compute requires f32 or f64 digital
accumulation. A numeric-class change must be an explicit conversion outside
the accumulator.

Mixed input dtypes are legal only when the operation has an explicit precision
policy. The policy names its compute dtype, output dtype, accumulator dtype,
minimum accumulator width, permitted bit-slicing modes, and stochastic seed.
The output tensor dtype must equal the policy's output dtype. A real/complex
boundary may not be crossed implicitly.

## Encodings

`QuantizationSpec.encoding` distinguishes:

- `ieee_float` for IEEE binary floating-point storage or conversion;
- `bfloat` for bfloat representations;
- `affine_integer` for scaled integer codes and zero points;
- `block_floating_point` for groups sharing power-of-two scales;
- `optical_effective_bits` for an analog path described by effective bits; and
- `backend_native` for an explicitly named vendor representation.

A backend-native encoding must carry a non-empty backend identifier. Other
encodings must not carry that identifier. Block-floating-point policies must
use per-block granularity and exact power-of-two scales.

## Scaling, zero points, and clipping

A quantization record contains:

- bit width and signedness;
- tensor, channel, or block granularity;
- an axis for per-channel scaling or block size for per-block scaling;
- one finite positive scale and one in-range zero point per scale group;
- finite increasing clipping bounds;
- nearest-even, toward-zero, or stochastic rounding; and
- saturating or error overflow behavior.

Affine, block, optical-effective-bit, and backend-native quantization applies
`round(value / scale + zero_point)`, validates the encoded integer range, and
dequantizes with `(code - zero_point) * scale`. IEEE fp32, IEEE fp16, and
bfloat16 use their native nearest-even exponent/mantissa conversion semantics,
including finite-range saturation or rejection. The result records codes,
dequantized values, clipping count, saturation count, and maximum absolute
error.

Compiler-generated defaults map the literal tensor range into the selected
effective width. Output scaling uses the exact digital reference range when
literal inputs exist and a conservative product bound otherwise. User-supplied
tensor policies take precedence over compiler defaults.

## Bit slicing

Bit slicing represents one signed value as multiple unsigned passes. The v1
modes are:

- `none`, legal only for one pass;
- `twos_complement`, which slices the fixed-width two's-complement code; and
- `signed_magnitude`, which transports magnitude slices plus an explicit sign.

The implementation validates total width, slice width, pass count, supported
backend modes, and source range. Saturating policies clamp out-of-range values;
error policies reject them. Reconstruction validates every slice before
combining it. Tests cover every representable signed int4 value, the `-8`
two's-complement edge, the signed-magnitude asymmetry, positive overflow,
negative overflow, saturation, and rejection.

When multiple passes are selected, Device IR emits one `bit_slice` command for
each input. It records total bits, slice bits, pass count, signed mode, and
overflow behavior. Each subsequent accumulator command records the accumulator
dtype and minimum width, so digital combination of passes is not implicit.

## Accumulation and overflow

Integer accumulation performs checked multiplication and checked addition in a
wide intermediate, then applies the declared accumulator range. Saturating
mode records the overflow and clamps to the endpoint. Error mode rejects the
operation. The result contains the accumulated values, overflow count, and
saturation count.

Static planning predicts integer overflow from the declared maximum input
magnitude, K length, and accumulator width. Plans that can overflow are
rejected when the backend's saturation policy is `error`; otherwise overflow
appears as a separate error component. Floating accumulators estimate rounding
growth from K and the accumulator's unit roundoff. Optical and hybrid partial
accumulation add their own effective-bit term.

## Analog noise

Capabilities declare four non-negative finite components:

- shot-noise fraction;
- thermal-noise fraction;
- phase noise in radians; and
- detector-noise fraction.

The conformance simulator uses a local SplitMix64 generator and Box-Muller
Gaussian samples. Every operation receives an explicit seed from its selected
plan. Shot, thermal, phase, and detector samples are retained in separate
vectors before their sum is applied. The same source program, capabilities,
cost inputs, plan, and seed produce byte-identical reports.

These component models are conformance approximations, not claims that a
reference profile matches a shipping device. Measured hardware profiles must
replace their parameters and provenance without changing the contract.

## Calibration-aware compensation

A selected calibration profile supplies measured gain, offset, phase error,
and uncertainty. Lowering rejects zero or non-finite gain. Photonic IR records
the measured transfer and its inverse:

```text
rescale = 1 / measured_gain
rebias  = -measured_offset / measured_gain
```

Device IR emits `rescale` after download and names the calibration handle. The
simulator applies the measured transfer, then the emitted inverse. Floating
residual is measured empirically. Static attribution retains a compensated
fraction of profile uncertainty plus live health drift and model-fit offset.
Calibration uncertainty is therefore visible without being confused with
quantization.

## Static error attribution

Every `CostEstimate` contains `ErrorAttribution` with exactly:

- quantization;
- shot noise;
- thermal noise;
- phase noise;
- detector noise;
- calibration residual;
- floating-point or partial-product accumulation;
- integer overflow;
- clipping;
- propagated input error; and
- the checked sum, capped at one.

All components must be finite and non-negative. The total is derived and is
never trusted from unvalidated input.

For an absolute error contract and a known output magnitude, the planner
multiplies the contract-relevant fraction by that magnitude. Relative contracts
compare the fraction directly. Literal GEMMs use their exact digital reference
magnitude; programs without literal data remain conservative. A plan that
cannot satisfy the requested effective bits or either error bound is not a
legal candidate. Forced photonic placement fails. Automatic placement falls
back to the winning digital backend with a recorded reason.

## Empirical error attribution

`awen.error-report.v1` contains:

- operation ID and deterministic seed;
- static fractional attribution from the selected cost estimate;
- empirically observed absolute attribution;
- maximum absolute and relative error;
- the pass/fail result; and
- provenance strings naming quantizers, seed, accumulator, and calibration.

The simulator measures input/output quantization, each injected analog-noise
vector, calibration compensation residual, tiled-versus-full accumulation,
integer overflow, and clipping independently. The final output is compared
element by element with the digital reference using the declared combined
absolute-plus-relative tolerance. Static fractional totals are capped at one;
observed absolute totals retain their full magnitude.

## Cost model and autotuning

The autotuner enumerates supported real compute dtypes, legal bit-slice counts,
supported signed slicing modes, accumulation modes, digital or optical
accumulator dtypes, tile sizes, wavelength counts, and boundary fusion. An
explicit operation policy narrows those choices.

Format conversion work is included in digital latency and energy. Wider
accumulators carry a cost factor. Lower precision can win a loose latency or
energy objective; stricter error contracts remove insufficient candidates and
select a wider compute and accumulator format. The plan key and fingerprint
include compute dtype, accumulator dtype, slicing mode, slice count, seed, and
the remaining topology choices.

## IR execution requirements

For every photonic GEMM, lowering emits:

1. `convert_tensor` for both inputs, including source and target dtype,
   quantization record, and seed;
2. `bit_slice` for each input when more than one pass is required;
3. `configure_matrix` carrying the complete precision plan;
4. quantized `upload_tile` commands;
5. `execute_gemm` per tile;
6. `accumulate` with explicit dtype, width, mode, and overflow behavior;
7. `download` with ADC width and measured dtype;
8. calibration `rescale` when a profile exists; and
9. output `convert_tensor` into the declared output dtype.

A runtime must not infer or silently replace these conversions.

## Backwards compatibility

Tensor programs without a precision field deserialize with an empty
`awen.precision.v1` configuration and preserve the existing single-dtype
behavior. New fields in Rust structs have serde defaults only where omission
is intentionally compatible. The versioned published schemas reject unknown
fields.

Photonic IR and Device IR consumers that validate the v1 schemas must update
for the expanded precision record and commands. That is an intentional schema
contract change within the experimental pre-1.0 repository. Future incompatible
precision semantics require a new schema identifier.

## Security and robustness

Shape products and integer arithmetic use checked operations. Quantizers reject
non-finite values, invalid scales, invalid zero points, invalid axes, invalid
block sizes, impossible code widths, and non-finite noise results. Calibration
gain must be invertible. Seeds are numeric data, not shell or code input.

Backend-native encoding identifiers are inert strings. A runtime must map them
through a trusted backend contract and must not interpret them as executable
code.

## Test plan

Required conformance covers:

- every v1 dtype and encoding;
- tensor, channel, and block quantization;
- zero points, clipping, saturation, and error overflow;
- every signed int4 value in both slicing modes;
- negative minimum, positive maximum, saturation, and rejection edges;
- i32/i64 accumulation overflow and saturation;
- explicit mixed storage, compute, accumulator, and output dtypes;
- explicit input conversion, bit slicing, accumulation, download, rescale, and
  output conversion commands;
- deterministic shot, thermal, phase, and detector noise;
- calibrated transfer compensation;
- separate static and empirical error components with provenance;
- forced rejection and automatic fallback for impossible contracts;
- changed compute and accumulator selection under loose versus strict
  contracts;
- source, precision, capability, Photonic IR, Device IR, and error-report JSON
  Schema validation; and
- the complete compiler and runtime quality gates under current stable Rust.
