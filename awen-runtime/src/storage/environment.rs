//! Environment capture for reproducibility

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Runtime environment information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub runtime_name: String,
    pub version: String,
    pub build_timestamp: String,
    pub build_profile: String,
    pub rust_version: String,
    pub features: Vec<String>,
    pub plugins: Vec<PluginInfo>,
}

/// Plugin information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
}

/// System information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub cpu_model: String,
    pub cpu_cores: usize,
    pub memory_gb: u64,
    pub hostname: String,
}

/// Device information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_type: String,
    pub device_id: String,
    pub capabilities: DeviceCapabilities,
    pub firmware_version: Option<String>,
    pub calibration_date: Option<String>,
}

/// Device capabilities
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub channels: usize,
    pub max_frequency_hz: f64,
    pub wavelength_range_nm: [f64; 2],
    pub phase_resolution_rad: f64,
    pub power_range_dbm: [f64; 2],
}

/// Capture current environment
pub fn capture_environment() -> super::EnvironmentSnapshot {
    super::EnvironmentSnapshot {
        runtime: capture_runtime(),
        system: capture_system(),
        device: capture_device(),
    }
}

fn capture_runtime() -> RuntimeInfo {
    RuntimeInfo {
        runtime_name: "awen-runtime".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        build_timestamp: option_env!("BUILD_TIMESTAMP").unwrap_or("unknown").to_string(),
        build_profile: if cfg!(debug_assertions) { "debug" } else { "release" }.to_string(),
        rust_version: option_env!("RUSTC_VERSION").unwrap_or("unknown").to_string(),
        features: vec![
            #[cfg(feature = "observability")]
            "observability".to_string(),
            #[cfg(feature = "gradients")]
            "gradients".to_string(),
            #[cfg(feature = "quantum")]
            "quantum".to_string(),
        ],
        plugins: vec![
            PluginInfo {
                name: "reference_sim".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        ],
    }
}

fn capture_system() -> SystemInfo {
    SystemInfo {
        os: std::env::consts::OS.to_string(),
        os_version: get_os_version(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_model: get_cpu_model(),
        cpu_cores: num_cpus::get(),
        memory_gb: get_memory_gb(),
        hostname: std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "unknown".to_string()),
    }
}

fn capture_device() -> DeviceInfo {
    // For v0.2, capture simulated device info
    // Real device discovery will be in HAL v0.3
    DeviceInfo {
        device_type: "simulated".to_string(),
        device_id: "sim_reference_0".to_string(),
        capabilities: DeviceCapabilities {
            channels: 64,
            max_frequency_hz: 1e15,
            wavelength_range_nm: [1530.0, 1570.0],
            phase_resolution_rad: 0.001,
            power_range_dbm: [-30.0, 10.0],
        },
        firmware_version: None,
        calibration_date: None,
    }
}

fn get_os_version() -> String {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|line| line.starts_with("PRETTY_NAME="))
                    .map(|line| line.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
            })
            .unwrap_or_else(|| "Linux (unknown)".to_string())
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        "unknown".to_string()
    }
}

fn get_cpu_model() -> String {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|line| line.starts_with("model name"))
                    .and_then(|line| line.split(':').nth(1))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        std::env::consts::ARCH.to_string()
    }
}

fn get_memory_gb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|line| line.starts_with("MemTotal:"))
                    .and_then(|line| line.split_whitespace().nth(1))
                    .and_then(|kb_str| kb_str.parse::<u64>().ok())
                    .map(|kb| kb / (1024 * 1024)) // KB to GB
            })
            .unwrap_or(0)
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capture_environment() {
        let env = capture_environment();
        
        assert_eq!(env.runtime.runtime_name, "awen-runtime");
        assert!(!env.runtime.version.is_empty());
        assert!(!env.system.os.is_empty());
        assert!(env.system.cpu_cores > 0);
        assert_eq!(env.device.device_type, "simulated");
    }
}
