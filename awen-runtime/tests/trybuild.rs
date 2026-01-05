use trybuild::TestCases;

#[test]
fn ui_tests() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/compile_fail_simulated_device.rs");
}
