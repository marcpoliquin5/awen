fn main() {
    // Attempt to construct the crate-private simulated device from outside the crate
    // This should fail to compile because SimulatedDevice::new is pub(crate)
    let _ = awen_runtime::hal::SimulatedDevice::new();
}
