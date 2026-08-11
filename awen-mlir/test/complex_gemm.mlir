// RUN: awen-opt %s --awen-import-stablehlo | FileCheck %s

module {
  func.func @complex_gemm(%lhs: tensor<4x8xcomplex<f32>>, %rhs: tensor<8x2xcomplex<f32>>) -> tensor<4x2xcomplex<f32>> {
    %0 = "stablehlo.dot_general"(%lhs, %rhs) {
      lhs_batching_dimensions = array<i64>,
      rhs_batching_dimensions = array<i64>,
      lhs_contracting_dimensions = array<i64: 1>,
      rhs_contracting_dimensions = array<i64: 0>
    } : (tensor<4x8xcomplex<f32>>, tensor<8x2xcomplex<f32>>) -> tensor<4x2xcomplex<f32>>
    return %0 : tensor<4x2xcomplex<f32>>
  }
}

// CHECK: awen_tensor.gemm
// CHECK: tensor<4x8xcomplex<f32>>, tensor<8x2xcomplex<f32>> -> tensor<4x2xcomplex<f32>>
