module {
  func.func @invalid_contracts(%state: !awen_qphotonic.fock_state) {
    // expected-error @+1 {{shots must be positive}}
    %0 = "awen_qphotonic.photon_count"(%state) <{
      shots = 0 : i64,
      seed = 17 : i64,
      confidence_level = 9.500000e-01 : f64,
      maximum_total_variation_distance = 1.000000e-01 : f64
    }> : (!awen_qphotonic.fock_state) -> !awen_qphotonic.samples
    return
  }
}
