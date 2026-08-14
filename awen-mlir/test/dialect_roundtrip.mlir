// Registered custom marker types must survive textual parser/printer round trips.
module {
  func.func private @abi_types(
    !awen_tensor.handle,
    !awen_photonic.optical_tile,
    !awen_qphotonic.state,
    !awen_device.command_buffer
  )

  func.func @typed_gemm(%lhs: tensor<4x8xf32>, %rhs: tensor<8x2xf32>) -> tensor<4x2xf32> {
    %0 = awen_tensor.gemm %lhs, %rhs {
      layout = "row_major",
      minimum_effective_bits = 8 : i64,
      transpose_lhs = false,
      transpose_rhs = false
    } : tensor<4x8xf32>, tensor<8x2xf32> -> tensor<4x2xf32>
    return %0 : tensor<4x2xf32>
  }

  func.func @separate_classical_and_quantum_contracts(%input: tensor<16xf32>) -> tensor<16xf32> {
    %modulated = "awen_photonic.modulate"(%input) <{
      modulation = "amplitude",
      carrier_wavelength_nm = 1.550000e+03 : f64,
      dac_bits = 12 : i64,
      calibration_fingerprint = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }> : (tensor<16xf32>) -> tensor<16xf32>

    %fock = "awen_qphotonic.prepare_fock"() <{
      modes = 2 : i64,
      cutoff = 2 : i64,
      seed = 17 : i64,
      coherence_budget_ns = 100 : i64
    }> : () -> !awen_qphotonic.fock_state
    %fourier = "awen_qphotonic.fourier"(%fock) <{
      coherence_cost_ns = 10 : i64
    }> : (!awen_qphotonic.fock_state) -> !awen_qphotonic.fock_state
    %samples = "awen_qphotonic.photon_count"(%fourier) <{
      shots = 100 : i64,
      seed = 17 : i64,
      confidence_level = 9.500000e-01 : f64,
      maximum_total_variation_distance = 1.000000e-01 : f64
    }> : (!awen_qphotonic.fock_state) -> !awen_qphotonic.samples

    %gaussian = "awen_qphotonic.prepare_gaussian"() <{
      modes = 1 : i64,
      seed = 29 : i64,
      coherence_budget_ns = 100 : i64
    }> : () -> !awen_qphotonic.gaussian_state
    %squeezed = "awen_qphotonic.squeeze"(%gaussian) <{
      magnitude = 5.000000e-01 : f64,
      angle_radians = 0.000000e+00 : f64,
      coherence_cost_ns = 10 : i64
    }> : (!awen_qphotonic.gaussian_state) -> !awen_qphotonic.gaussian_state
    %homodyne = "awen_qphotonic.homodyne_q"(%squeezed) <{
      shots = 100 : i64,
      seed = 29 : i64,
      confidence_level = 9.500000e-01 : f64,
      maximum_mean_error = 1.000000e-01 : f64
    }> : (!awen_qphotonic.gaussian_state) -> !awen_qphotonic.samples
    %corrected = "awen_qphotonic.feed_forward_phase"(%homodyne, %squeezed) <{
      scale = -1.000000e+00 : f64,
      offset = 0.000000e+00 : f64,
      maximum_latency_ns = 50 : i64
    }> : (!awen_qphotonic.samples, !awen_qphotonic.gaussian_state) -> !awen_qphotonic.gaussian_state
    %homodyne_p = "awen_qphotonic.homodyne_p"(%corrected) <{
      shots = 100 : i64,
      seed = 29 : i64,
      confidence_level = 9.500000e-01 : f64,
      maximum_mean_error = 1.000000e-01 : f64
    }> : (!awen_qphotonic.gaussian_state) -> !awen_qphotonic.samples
    %corrected_q = "awen_qphotonic.feed_forward_displacement_q"(%homodyne_p, %corrected) <{
      scale = 5.000000e-01 : f64,
      offset = 0.000000e+00 : f64,
      maximum_latency_ns = 50 : i64
    }> : (!awen_qphotonic.samples, !awen_qphotonic.gaussian_state) -> !awen_qphotonic.gaussian_state
    %corrected_p = "awen_qphotonic.feed_forward_displacement_p"(%homodyne_p, %corrected_q) <{
      scale = 5.000000e-01 : f64,
      offset = 0.000000e+00 : f64,
      maximum_latency_ns = 50 : i64
    }> : (!awen_qphotonic.samples, !awen_qphotonic.gaussian_state) -> !awen_qphotonic.gaussian_state
    %retuned = "awen_qphotonic.feed_forward_squeezing"(%homodyne_p, %corrected_p) <{
      scale = 2.500000e-01 : f64,
      offset = 0.000000e+00 : f64,
      maximum_latency_ns = 50 : i64
    }> : (!awen_qphotonic.samples, !awen_qphotonic.gaussian_state) -> !awen_qphotonic.gaussian_state
    return %modulated : tensor<16xf32>
  }
}
