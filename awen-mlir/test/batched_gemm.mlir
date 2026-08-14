// RUN: awen-opt %s --awen-lower-stablehlo-to-device | FileCheck %s

module {
  func.func @batched(%lhs: tensor<2x16x32xf32>, %rhs: tensor<2x32x8xf32>) -> tensor<2x16x8xf32> {
    %0 = "stablehlo.dot_general"(%lhs, %rhs) {
      lhs_batching_dimensions = array<i64: 0>,
      rhs_batching_dimensions = array<i64: 0>,
      lhs_contracting_dimensions = array<i64: 2>,
      rhs_contracting_dimensions = array<i64: 1>,
      awen.minimum_effective_bits = 10 : i64,
      awen.layout = "column_major"
    } : (tensor<2x16x32xf32>, tensor<2x32x8xf32>) -> tensor<2x16x8xf32>
    return %0 : tensor<2x16x8xf32>
  }

  func.func @dynamic_batched(%lhs: tensor<?x?x32xbf16>, %rhs: tensor<?x32x?xbf16>) -> tensor<?x?x?xbf16> {
    %0 = "stablehlo.dot_general"(%lhs, %rhs) {
      lhs_batching_dimensions = array<i64: 0>,
      rhs_batching_dimensions = array<i64: 0>,
      lhs_contracting_dimensions = array<i64: 2>,
      rhs_contracting_dimensions = array<i64: 1>
    } : (tensor<?x?x32xbf16>, tensor<?x32x?xbf16>) -> tensor<?x?x?xbf16>
    return %0 : tensor<?x?x?xbf16>
  }
}

// CHECK-LABEL: func.func @batched
// CHECK: awen_device.execute_gemm
// CHECK-SAME: calibration = "required"
// CHECK-SAME: layout = "column_major"
// CHECK-SAME: minimum_effective_bits = 10 : i64
// CHECK-SAME: tensor<2x16x32xf32>, tensor<2x32x8xf32> -> tensor<2x16x8xf32>
// CHECK-LABEL: func.func @dynamic_batched
// CHECK: awen_device.execute_gemm
// CHECK-SAME: minimum_effective_bits = 8 : i64
// CHECK-SAME: tensor<?x?x32xbf16>, tensor<?x32x?xbf16> -> tensor<?x?x?xbf16>
