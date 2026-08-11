// RUN: awen-opt %s --awen-lower-stablehlo-to-device --mlir-print-debuginfo | FileCheck %s

#model_loc = loc("model.py":12:3)
module {
  func.func @located(%lhs: tensor<4x8xf16>, %rhs: tensor<8x2xf16>) -> tensor<4x2xf16> {
    %0 = "stablehlo.dot_general"(%lhs, %rhs) {
      lhs_batching_dimensions = array<i64>,
      rhs_batching_dimensions = array<i64>,
      lhs_contracting_dimensions = array<i64: 1>,
      rhs_contracting_dimensions = array<i64: 0>
    } : (tensor<4x8xf16>, tensor<8x2xf16>) -> tensor<4x2xf16> loc(#model_loc)
    return %0 : tensor<4x2xf16>
  }
}

// CHECK: awen_device.execute_gemm
// CHECK-SAME: loc([[MODEL_LOC:#[A-Za-z0-9]+]])
// CHECK: [[MODEL_LOC]] = loc("model.py":12:3)
