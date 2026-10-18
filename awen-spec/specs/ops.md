# Operator library and fidelity levels (`awen.op-table.v1`)

Status: implemented in `awen-compiler/src/ops.rs`, exposed to Python as
`awen_native.op_table()` and `awen_native.run_op()`.

Every operator is written once against a GEMM engine. It decomposes into
matrix products, which are the part a photonic matrix unit would execute, and
electronic steps after detection (softmax, activations, elementwise products,
accumulation, gathers). Two engines exist:

* `ExactEngine`: f64 math. Responses say `fidelity: "exact (f64 reference)"`.
* `PhotonicEngine`: every product runs on the AEP-0022 compact noise model
  (`noise::photonic_gemm`) with a seeded RNG. Responses say
  `fidelity: "simulated (compact noise model)"`. This is a simulation, not a
  hardware measurement.

## Levels

Each operator records the highest level it has reached. A level implies all
lower ones.

| Level | Meaning | Evidence required |
|---|---|---|
| `math_reference` | f64 reference implementation | matches an independent NumPy oracle (`awen-native/tests/test_ops.py`) |
| `photonic_sim` | also runs through the photonic noise model | ideal contract reproduces the reference; noise falls as photon budget rises (tests) |
| `hardware_lowerable` | the compiler emits device commands (AWENEXE or VPISA) a conforming backend would execute | compiler tests that produce the commands |
| `hardware_measured` | measured on photonic hardware | a hardware-in-the-loop artifact. **No operator is at this level.** |

## Operators

| Operator | Level | Photonic part | Electronic part |
|---|---|---|---|
| `gemm` | hardware_lowerable | `x · wᵀ` | tile accumulation |
| `complex_matmul` | photonic_sim | four real products | two additions |
| `dft` | photonic_sim | complex product with the n×n DFT matrix (O(n²), not a butterfly FFT) | additions |
| `conv2d` | photonic_sim | im2col patches · kernelsᵀ | im2col gather, bias |
| `correlate1d` | photonic_sim | sliding windows · kernel | window gather |
| `low_rank_linear` | photonic_sim | two rank-r products | none |
| `attention` | photonic_sim | Q/K/V/O projections, QKᵀ, PV | scale, causal mask, softmax |
| `ffn` | photonic_sim | two projections | biases, tanh-GELU |
| `spectral_conv1d` | photonic_sim | forward and inverse DFT products | pointwise complex multiply |
| `rnn_step` | photonic_sim | recurrent and input products | sum, bias, tanh |
| `reservoir_step` | photonic_sim | reservoir and input products | tanh, leaky integration |
| `intensity_layer` | photonic_sim | coherent real and imaginary products | square-law detection `|·|²` |

## Modeling limits

* The noise model treats every product as a real signed GEMM. Complex
  products are split into real ones; a coherent device would execute them
  natively with different noise statistics, which is not modeled yet.
* `attention` multiplies two activations (QKᵀ, PV). On hardware this requires
  reprogramming weights per token or an activation-activation architecture;
  the simulator does not charge that reconfiguration cost.
* `intensity_layer` models ideal square-law detection. Detector noise is the
  noise model's output-referred terms, not a separate detector model.

## Request format

```json
{"op": "conv2d",
 "inputs": {"x": {"shape": [1, 4, 4], "data": [...]},
            "w": {"shape": [2, 1, 3, 3], "data": [...]}},
 "params": {"stride": 1, "padding": 1},
 "contract": null, "tile_k": 64, "seed": 0}
```

`contract: null` runs the reference. Unknown operators, missing inputs,
non-finite values and shape mismatches are errors.
