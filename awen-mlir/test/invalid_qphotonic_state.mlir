module {
  func.func @fock_state_cannot_enter_gaussian_gate(%state: !awen_qphotonic.fock_state) {
    // expected-error @+1 {{operand #0 must be Gaussian continuous-variable state}}
    %0 = "awen_qphotonic.squeeze"(%state) <{
      magnitude = 5.000000e-01 : f64,
      angle_radians = 0.000000e+00 : f64,
      coherence_cost_ns = 10 : i64
    }> : (!awen_qphotonic.fock_state) -> !awen_qphotonic.gaussian_state
    return
  }
}
