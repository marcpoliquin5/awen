#ifndef AWEN_QPHOTONIC_OPS_TD
#define AWEN_QPHOTONIC_OPS_TD

include "mlir/IR/AttrTypeBase.td"
include "mlir/IR/OpBase.td"

def AwenQPhotonicDialect : Dialect {
  let name = "awen_qphotonic";
  let cppNamespace = "::mlir::awen::qphotonic";
  let summary = "Quantum-photonic state, circuit, and measurement operations";
  let useDefaultTypePrinterParser = 1;
}

class AwenQPhotonicType<string name, string typeMnemonic>
    : TypeDef<AwenQPhotonicDialect, name> {
  let mnemonic = typeMnemonic;
}

def AwenQuantumStateType : AwenQPhotonicType<"QuantumState", "state"> {
  let summary = "Legacy generic quantum state marker retained for bytecode compatibility";
}

def AwenFockStateType : AwenQPhotonicType<"FockState", "fock_state"> {
  let summary = "Discrete-variable Fock state with an explicit cutoff contract";
}

def AwenGaussianStateType : AwenQPhotonicType<"GaussianState", "gaussian_state"> {
  let summary = "Gaussian continuous-variable state";
}

def AwenSampleStreamType : AwenQPhotonicType<"SampleStream", "samples"> {
  let summary = "Seeded probabilistic measurement samples";
}

class AwenQPhotonicOp<string mnemonic, list<Trait> traits = []>
    : Op<AwenQPhotonicDialect, mnemonic, traits>;

def AwenQPhotonicPrepareFockOp : AwenQPhotonicOp<"prepare_fock"> {
  let summary = "Prepare a seeded Fock state";
  let arguments = (ins I64Attr:$modes,
                       I64Attr:$cutoff,
                       I64Attr:$seed,
                       I64Attr:$coherence_budget_ns);
  let results = (outs AwenFockStateType:$state);
  let hasVerifier = 1;
}

def AwenQPhotonicPrepareGaussianOp : AwenQPhotonicOp<"prepare_gaussian"> {
  let summary = "Prepare a seeded Gaussian CV state";
  let arguments = (ins I64Attr:$modes,
                       I64Attr:$seed,
                       I64Attr:$coherence_budget_ns);
  let results = (outs AwenGaussianStateType:$state);
  let hasVerifier = 1;
}

def AwenQPhotonicBeamSplitterFockOp : AwenQPhotonicOp<"beam_splitter_fock"> {
  let summary = "Two-mode Fock-space beam splitter";
  let arguments = (ins AwenFockStateType:$input,
                       F64Attr:$theta_radians,
                       F64Attr:$phi_radians,
                       I64Attr:$coherence_cost_ns);
  let results = (outs AwenFockStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicBeamSplitterGaussianOp
    : AwenQPhotonicOp<"beam_splitter_gaussian"> {
  let summary = "Two-mode Gaussian CV beam splitter";
  let arguments = (ins AwenGaussianStateType:$input,
                       F64Attr:$theta_radians,
                       F64Attr:$phi_radians,
                       I64Attr:$coherence_cost_ns);
  let results = (outs AwenGaussianStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicPhaseShiftFockOp : AwenQPhotonicOp<"phase_shift_fock"> {
  let summary = "Fock-space phase shift";
  let arguments = (ins AwenFockStateType:$input,
                       F64Attr:$radians,
                       I64Attr:$coherence_cost_ns);
  let results = (outs AwenFockStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicPhaseShiftGaussianOp
    : AwenQPhotonicOp<"phase_shift_gaussian"> {
  let summary = "Gaussian CV phase shift";
  let arguments = (ins AwenGaussianStateType:$input,
                       F64Attr:$radians,
                       I64Attr:$coherence_cost_ns);
  let results = (outs AwenGaussianStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicSqueezeOp : AwenQPhotonicOp<"squeeze"> {
  let summary = "Gaussian CV squeezing gate";
  let arguments = (ins AwenGaussianStateType:$input,
                       F64Attr:$magnitude,
                       F64Attr:$angle_radians,
                       I64Attr:$coherence_cost_ns);
  let results = (outs AwenGaussianStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicDisplaceOp : AwenQPhotonicOp<"displace"> {
  let summary = "Gaussian CV displacement gate";
  let arguments = (ins AwenGaussianStateType:$input,
                       F64Attr:$q,
                       F64Attr:$p,
                       I64Attr:$coherence_cost_ns);
  let results = (outs AwenGaussianStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicControlledXOp : AwenQPhotonicOp<"controlled_x"> {
  let summary = "Discrete-variable controlled-X gate";
  let arguments = (ins AwenFockStateType:$input,
                       I64Attr:$dimension,
                       I64Attr:$coherence_cost_ns);
  let results = (outs AwenFockStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicFourierOp : AwenQPhotonicOp<"fourier"> {
  let summary = "Discrete-variable Fourier gate";
  let arguments = (ins AwenFockStateType:$input,
                       I64Attr:$coherence_cost_ns);
  let results = (outs AwenFockStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicPhotonCountOp : AwenQPhotonicOp<"photon_count"> {
  let summary = "Seeded destructive photon-counting measurement";
  let arguments = (ins AwenFockStateType:$input,
                       I64Attr:$shots,
                       I64Attr:$seed,
                       F64Attr:$confidence_level,
                       F64Attr:$maximum_total_variation_distance);
  let results = (outs AwenSampleStreamType:$samples);
  let hasVerifier = 1;
}

def AwenQPhotonicHomodyneQOp : AwenQPhotonicOp<"homodyne_q"> {
  let summary = "Seeded Q-quadrature homodyne measurement";
  let arguments = (ins AwenGaussianStateType:$input,
                       I64Attr:$shots,
                       I64Attr:$seed,
                       F64Attr:$confidence_level,
                       F64Attr:$maximum_mean_error);
  let results = (outs AwenSampleStreamType:$samples);
  let hasVerifier = 1;
}

def AwenQPhotonicHomodynePOp : AwenQPhotonicOp<"homodyne_p"> {
  let summary = "Seeded P-quadrature homodyne measurement";
  let arguments = (ins AwenGaussianStateType:$input,
                       I64Attr:$shots,
                       I64Attr:$seed,
                       F64Attr:$confidence_level,
                       F64Attr:$maximum_mean_error);
  let results = (outs AwenSampleStreamType:$samples);
  let hasVerifier = 1;
}

def AwenQPhotonicHeterodyneOp : AwenQPhotonicOp<"heterodyne"> {
  let summary = "Seeded Gaussian CV heterodyne measurement";
  let arguments = (ins AwenGaussianStateType:$input,
                       I64Attr:$shots,
                       I64Attr:$seed,
                       F64Attr:$confidence_level,
                       F64Attr:$maximum_mean_error);
  let results = (outs AwenSampleStreamType:$samples);
  let hasVerifier = 1;
}

def AwenQPhotonicFeedForwardPhaseOp
    : AwenQPhotonicOp<"feed_forward_phase"> {
  let summary = "Narrow measurement-to-phase feed-forward boundary";
  let arguments = (ins AwenSampleStreamType:$samples,
                       AwenGaussianStateType:$input,
                       F64Attr:$scale,
                       F64Attr:$offset,
                       I64Attr:$maximum_latency_ns);
  let results = (outs AwenGaussianStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicFeedForwardDisplacementQOp
    : AwenQPhotonicOp<"feed_forward_displacement_q"> {
  let summary = "Narrow measurement-to-Q-displacement feed-forward boundary";
  let arguments = (ins AwenSampleStreamType:$samples,
                       AwenGaussianStateType:$input,
                       F64Attr:$scale,
                       F64Attr:$offset,
                       I64Attr:$maximum_latency_ns);
  let results = (outs AwenGaussianStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicFeedForwardDisplacementPOp
    : AwenQPhotonicOp<"feed_forward_displacement_p"> {
  let summary = "Narrow measurement-to-P-displacement feed-forward boundary";
  let arguments = (ins AwenSampleStreamType:$samples,
                       AwenGaussianStateType:$input,
                       F64Attr:$scale,
                       F64Attr:$offset,
                       I64Attr:$maximum_latency_ns);
  let results = (outs AwenGaussianStateType:$result);
  let hasVerifier = 1;
}

def AwenQPhotonicFeedForwardSqueezingOp
    : AwenQPhotonicOp<"feed_forward_squeezing"> {
  let summary = "Narrow measurement-to-squeezing feed-forward boundary";
  let arguments = (ins AwenSampleStreamType:$samples,
                       AwenGaussianStateType:$input,
                       F64Attr:$scale,
                       F64Attr:$offset,
                       I64Attr:$maximum_latency_ns);
  let results = (outs AwenGaussianStateType:$result);
  let hasVerifier = 1;
}

#endif
