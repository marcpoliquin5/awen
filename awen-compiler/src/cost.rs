use crate::capability::DeviceCapabilities;
use crate::ir::{DType, GemmShape};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationObjective {
    Latency,
    Energy,
    Accuracy,
    Throughput,
}

impl OptimizationObjective {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "latency" => Some(Self::Latency),
            "energy" => Some(Self::Energy),
            "accuracy" => Some(Self::Accuracy),
            "throughput" => Some(Self::Throughput),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetBackend {
    Auto,
    Cpu,
    Photonic,
}

impl TargetBackend {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "cpu" => Some(Self::Cpu),
            "photonic" => Some(Self::Photonic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CostEstimate {
    pub latency_ns: f64,
    pub energy_uj: f64,
    pub throughput_gops: f64,
    pub effective_bits: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlacementDecision {
    pub op_id: String,
    pub selected_backend: TargetBackend,
    pub objective: OptimizationObjective,
    pub cpu: CostEstimate,
    pub photonic: Option<CostEstimate>,
    pub optical_electrical_boundary_crossings: u32,
    pub tile_count: usize,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy)]
pub struct DigitalBaseline {
    pub throughput_tops: f64,
    pub energy_pj_per_mac: f64,
    pub launch_latency_ns: f64,
    pub effective_bits: u8,
}

impl Default for DigitalBaseline {
    fn default() -> Self {
        Self {
            throughput_tops: 25.0,
            energy_pj_per_mac: 20.0,
            launch_latency_ns: 2_500.0,
            effective_bits: 16,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn decide_placement(
    op_id: &str,
    shape: GemmShape,
    dtype: DType,
    minimum_effective_bits: Option<u8>,
    capabilities: &DeviceCapabilities,
    objective: OptimizationObjective,
    requested_target: TargetBackend,
    digital: DigitalBaseline,
) -> PlacementDecision {
    let cpu = estimate_cpu(shape, digital);
    let photonic = capabilities
        .supports(dtype)
        .then(|| estimate_photonic(shape, dtype, capabilities));
    let tile_count = tile_count(shape, capabilities);
    let precision_ok = photonic
        .as_ref()
        .map(|estimate| minimum_effective_bits.is_none_or(|bits| estimate.effective_bits >= bits))
        .unwrap_or(false);

    let (selected_backend, rationale) = match requested_target {
        TargetBackend::Cpu => (TargetBackend::Cpu, "CPU placement was explicitly requested".to_string()),
        TargetBackend::Photonic if photonic.is_none() => (
            TargetBackend::Cpu,
            format!("photonic placement was requested but dtype {dtype:?} is unsupported"),
        ),
        TargetBackend::Photonic if !precision_ok => (
            TargetBackend::Cpu,
            "photonic placement was requested but its effective precision violates the accuracy contract"
                .to_string(),
        ),
        TargetBackend::Photonic => (
            TargetBackend::Photonic,
            "photonic placement was explicitly requested and is supported".to_string(),
        ),
        TargetBackend::Auto if photonic.is_none() => (
            TargetBackend::Cpu,
            format!("dtype {dtype:?} is not supported by the photonic backend"),
        ),
        TargetBackend::Auto if !precision_ok => (
            TargetBackend::Cpu,
            "photonic effective precision violates the operation accuracy contract".to_string(),
        ),
        TargetBackend::Auto => {
            let optical = photonic.as_ref().expect("checked above");
            let choose_photonic = match objective {
                OptimizationObjective::Latency => optical.latency_ns < cpu.latency_ns,
                OptimizationObjective::Energy => optical.energy_uj < cpu.energy_uj,
                OptimizationObjective::Accuracy => optical.effective_bits >= cpu.effective_bits,
                OptimizationObjective::Throughput => optical.throughput_gops > cpu.throughput_gops,
            };
            if choose_photonic {
                (
                    TargetBackend::Photonic,
                    format!(
                        "photonic estimate wins the {objective:?} objective after including two conversion boundaries"
                    ),
                )
            } else {
                (
                    TargetBackend::Cpu,
                    format!(
                        "CPU estimate wins the {objective:?} objective after conversion and reconfiguration costs"
                    ),
                )
            }
        }
    };

    PlacementDecision {
        op_id: op_id.to_string(),
        selected_backend,
        objective,
        cpu,
        photonic,
        optical_electrical_boundary_crossings: if selected_backend == TargetBackend::Photonic {
            2
        } else {
            0
        },
        tile_count,
        rationale,
    }
}

pub fn tile_count(shape: GemmShape, capabilities: &DeviceCapabilities) -> usize {
    div_ceil(shape.m, capabilities.matrix_core.m)
        * div_ceil(shape.n, capabilities.matrix_core.n)
        * div_ceil(shape.k, capabilities.matrix_core.k)
}

fn estimate_cpu(shape: GemmShape, baseline: DigitalBaseline) -> CostEstimate {
    let macs = shape.m as f64 * shape.n as f64 * shape.k as f64;
    let operations = 2.0 * macs;
    let latency_ns = baseline.launch_latency_ns + operations / (baseline.throughput_tops * 1_000.0);
    CostEstimate {
        latency_ns,
        energy_uj: macs * baseline.energy_pj_per_mac / 1_000_000.0,
        throughput_gops: operations / latency_ns,
        effective_bits: baseline.effective_bits,
    }
}

fn estimate_photonic(
    shape: GemmShape,
    dtype: DType,
    capabilities: &DeviceCapabilities,
) -> CostEstimate {
    let tiles = tile_count(shape, capabilities) as f64;
    let bytes = (shape.m * shape.k + shape.k * shape.n + shape.m * shape.n) as f64
        * dtype.bits() as f64
        / 8.0;
    let transfer_ns = bytes * 8.0 / capabilities.host_bandwidth_gbps;
    let samples_per_tile = (capabilities.matrix_core.m
        + capabilities.matrix_core.k
        + capabilities.matrix_core.n) as f64;
    let conversion_ns = samples_per_tile / capabilities.sample_rate_gsps;
    let optical_compute_ns = 1.0 / capabilities.modulation_rate_gbaud;
    let latency_ns = 2.0 * capabilities.boundary_latency_ns
        + transfer_ns
        + capabilities.reconfiguration_latency_ns
        + tiles * (conversion_ns + optical_compute_ns);
    let conversion_samples = samples_per_tile * tiles;
    let conversion_energy_uj = conversion_samples
        * (capabilities.dac_energy_pj_per_sample + capabilities.adc_energy_pj_per_sample)
        / 1_000_000.0;
    let laser_energy_uj = capabilities.laser_power_mw * latency_ns / 1_000_000.0;
    let operations = 2.0 * shape.m as f64 * shape.n as f64 * shape.k as f64;
    CostEstimate {
        latency_ns,
        energy_uj: conversion_energy_uj + laser_energy_uj,
        throughput_gops: operations / latency_ns,
        effective_bits: capabilities.effective_bits,
    }
}

fn div_ceil(value: usize, divisor: usize) -> usize {
    value.div_ceil(divisor)
}
