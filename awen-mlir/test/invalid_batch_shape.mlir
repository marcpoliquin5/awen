// RUN: awen-opt %s --awen-import-stablehlo --verify-diagnostics

module {
  func.func @invalid_batch_shape(%lhs: tensor<2x16x32xf32>, %rhs: tensor<3x32x8xf32>) -> tensor<2x16x8xf32> {
    // expected-error @+1 {{static batch dimensions do not match}}
    %0 = "stablehlo.dot_general"(%lhs, %rhs) {
      lhs_batching_dimensions = array<i64: 0>,
      rhs_batching_dimensions = array<i64: 0>,
      lhs_contracting_dimensions = array<i64: 2>,
      rhs_contracting_dimensions = array<i64: 1>
    } : (tensor<2x16x32xf32>, tensor<3x32x8xf32>) -> tensor<2x16x8xf32>
    return %0 : tensor<2x16x8xf32>
  }
}
