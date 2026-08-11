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
}
