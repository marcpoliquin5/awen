// RUN: awen-opt %s --awen-import-stablehlo --verify-diagnostics

module {
  func.func @unsupported(%lhs: tensor<4xf32>, %rhs: tensor<4xf32>) -> tensor<4xf32> {
    // expected-error @+1 {{unsupported StableHLO operation; AWEN v1 imports only rank-two or equal-batch rank-three dot_general GEMM}}
    %0 = "stablehlo.add"(%lhs, %rhs) : (tensor<4xf32>, tensor<4xf32>) -> tensor<4xf32>
    return %0 : tensor<4xf32>
  }
}
