// RUN: awen-opt %s --verify-diagnostics

module {
  func.func @invalid(%lhs: tensor<4xf32>, %rhs: tensor<4xf32>) -> tensor<4xf32> {
    // expected-error @+1 {{requires matching rank-two or rank-three lhs, rhs, and result tensors}}
    %0 = awen_tensor.gemm %lhs, %rhs {
      layout = "row_major",
      minimum_effective_bits = 8 : i64,
      transpose_lhs = false,
      transpose_rhs = false
    } : tensor<4xf32>, tensor<4xf32> -> tensor<4xf32>
    return %0 : tensor<4xf32>
  }
}
