pub mod loader;
pub mod reference_sim;
pub mod registry;

pub use loader::PluginLoader;
pub use reference_sim::run_reference_simulator;
pub use registry::{
    BackendDiscoveryReport, BackendPluginContract, DiscoveredBackend, DiscoveryDiagnostic,
    HealthQuery, PluginManifest, PluginRegistry, PLUGIN_MANIFEST_VERSION,
};
