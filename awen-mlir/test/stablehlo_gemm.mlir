// RUN: awen-opt %s --awen-lower-stablehlo-to-device | FileCheck %s

module {
  func.func @gemm(%lhs: tensor<256x128xf32>, %rhs: tensor<128x64xf32>) -> tensor<256x64xf32> {
    %0 = "stablehlo.dot_general"(%lhs, %rhs) {
      lhs_batching_dimensions = array<i64>,
      rhs_batching_dimensions = array<i64>,
      lhs_contracting_dimensions = array<i64: 1>,
      rhs_contracting_dimensions = array<i64: 0>,
      awen.minimum_effective_bits = 8 : i64,
      awen.layout = "row_major"
    } : (tensor<256x128xf32>, tensor<128x64xf32>) -> tensor<256x64xf32>
    return %0 : tensor<256x64xf32>
  }
}

// CHECK: module attributes {
// CHECK-DAG: awen.device.version = 1 : i64
// CHECK-DAG: awen.executable.abi_major = 1 : i64
// CHECK-DAG: awen.executable.abi_minor = 0 : i64
// CHECK-DAG: awen.photonic.version = 1 : i64
// CHECK-DAG: awen.tensor.version = 1 : i64
// CHECK: awen_device.execute_gemm
// CHECK-SAME: backend = "awen.reference.v1"
// CHECK-SAME: calibration = "required"
// CHECK-SAME: layout = "row_major"
// CHECK-SAME: minimum_effective_bits = 8 : i64
// CHECK-SAME: tile_k = 128 : i64
// CHECK-SAME: tile_m = 128 : i64
// CHECK-SAME: tile_n = 128 : i64
