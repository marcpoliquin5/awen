// RUN: awen-opt %s --awen-import-stablehlo | FileCheck %s

module {
  func.func @dynamic_gemm(%lhs: tensor<?x128xbf16>, %rhs: tensor<128x?xbf16>) -> tensor<?x?xbf16> {
    %0 = "stablehlo.dot_general"(%lhs, %rhs) {
      lhs_batching_dimensions = array<i64>,
      rhs_batching_dimensions = array<i64>,
      lhs_contracting_dimensions = array<i64: 1>,
      rhs_contracting_dimensions = array<i64: 0>
    } : (tensor<?x128xbf16>, tensor<128x?xbf16>) -> tensor<?x?xbf16>
    return %0 : tensor<?x?xbf16>
  }
}

// CHECK: awen_tensor.gemm
// CHECK-SAME: minimum_effective_bits = 8 : i64
// CHECK: tensor<?x128xbf16>, tensor<128x?xbf16> -> tensor<?x?xbf16>
