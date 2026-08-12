# AWEN precision and error-report contract

Status: Implemented (`awen.precision.v1`, `awen.error-report.v1`)

The normative design and conformance requirements are defined by
[AEP-0017](../aeps/AEP-0017-precision-and-error-contracts.md). The machine
contracts are:

- [`awen_precision.v1.json`](../schemas/awen_precision.v1.json);
- [`awen_error_report.v1.json`](../schemas/awen_error_report.v1.json);
- [`awen_tensor_ir.v1.json`](../schemas/awen_tensor_ir.v1.json);
- [`awen_photonic_ir.classical.v1.json`](../schemas/awen_photonic_ir.classical.v1.json);
- [`awen_device_ir.v1.json`](../schemas/awen_device_ir.v1.json); and
- [`awen_device_capability.v1.json`](../schemas/awen_device_capability.v1.json).

## Minimal operation policy

```json
{
  "version": "awen.precision.v1",
  "tensors": [],
  "operations": [{
    "op_id": "projection",
    "compute_dtype": "f16",
    "output_dtype": "f32",
    "accumulator_dtype": "f32",
    "minimum_accumulator_bits": 32,
    "allowed_bit_slicing_modes": ["none", "twos_complement"],
    "stochastic_seed": 24301
  }]
}
```

Mixed Tensor IR input dtypes require this explicit operation policy. Tensor
quantization policies are optional; when absent, lowering constructs a scaled
policy from the selected compute/effective width and known literal range.

## Tensor quantization policy

```json
{
  "tensor_id": "weights",
  "storage_dtype": "int8",
  "quantization": {
    "encoding": "affine_integer",
    "bits": 8,
    "signed": true,
    "granularity": "per_channel",
    "axis": 0,
    "scales": [0.01, 0.02],
    "zero_points": [0, 0],
    "clipping_min": -1.27,
    "clipping_max": 1.27,
    "rounding": "nearest_even",
    "overflow": "saturate"
  }
}
```

The number of scales and zero points must equal one for `per_tensor`, the
selected axis length for `per_channel`, or the ceiling of element count divided
by block size for `per_block`.

## Error report

Static values are fractions used during planning. Observed values are absolute
errors measured during conformance execution. Both contain named components
for quantization, four analog-noise sources, calibration residual,
floating-point accumulation, integer overflow, clipping, propagated input, and
their checked total. Consumers must inspect the named components rather than
reverse-engineering attribution from the total.
