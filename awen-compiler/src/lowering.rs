use crate::capability::{AccumulationMode, CalibrationProfile, DeviceCapabilities};
use crate::cost::{PlacementDecision, TargetBackend};
use crate::ir::{DType, GemmShape, Layout, Tensor, TensorOp, TensorProgram, ValidatedGemm};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const PHOTONIC_IR_VERSION: &str = "awen.photonic.classical.v1";
pub const DEVICE_IR_VERSION: &str = "awen.device.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompiledTensor {
    pub id: String,
    pub shape: Vec<usize>,
    pub dtype: DType,
    pub layout: Layout,
}

impl From<&Tensor> for CompiledTensor {
    fn from(tensor: &Tensor) -> Self {
        Self {
            id: tensor.id.clone(),
            shape: tensor.shape.clone(),
            dtype: tensor.dtype,
            layout: tensor.layout,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tile {
    pub m_offset: usize,
    pub n_offset: usize,
    pub k_offset: usize,
    pub m: usize,
    pub n: usize,
    pub k: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrecisionPlan {
    pub source_dtype: DType,
    pub optical_effective_bits: u8,
    pub dac_bits: u8,
    pub adc_bits: u8,
    pub digital_accumulation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhotonicGemmTile {
    pub op_id: String,
    #[serde(rename = "type")]
    pub op_type: String,
    pub lhs: String,
    pub rhs: String,
    pub output: String,
    pub transpose_lhs: bool,
    pub transpose_rhs: bool,
    pub tile: Tile,
    pub precision: PrecisionPlan,
    pub wavelength_channels: Vec<f64>,
    pub accumulation_mode: AccumulationMode,
    pub calibration_handle: Option<String>,
    pub timing: Timing,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Timing {
    pub start_ns: f64,
    pub duration_ns: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostFallbackOp {
    pub op_id: String,
    pub op_type: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClassicalPhotonicProgram {
    pub ir_version: String,
    pub source_ir_version: String,
    pub backend_id: String,
    pub tensors: Vec<CompiledTensor>,
    pub ops: Vec<PhotonicGemmTile>,
    pub host_fallback_ops: Vec<HostFallbackOp>,
    pub calibration: Option<CalibrationProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceProgram {
    pub ir_version: String,
    pub backend_id: String,
    pub commands: Vec<DeviceCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum DeviceCommand {
    Calibrate {
        profile_id: String,
    },
    ConfigureMatrix {
        op_id: String,
        tile: Tile,
        precision: PrecisionPlan,
    },
    UploadTile {
        tensor: String,
        row_offset: usize,
        column_offset: usize,
        rows: usize,
        columns: usize,
        dac_bits: u8,
    },
    ExecuteGemm {
        op_id: String,
        tile: Tile,
        wavelength_channels: Vec<f64>,
    },
    Accumulate {
        tensor: String,
        tile: Tile,
        mode: AccumulationMode,
    },
    Download {
        tensor: String,
        adc_bits: u8,
    },
    HostGemm {
        op_id: String,
        reason: String,
    },
}

pub fn lower(
    program: &TensorProgram,
    validated: &[ValidatedGemm<'_>],
    decisions: &[PlacementDecision],
    capabilities: &DeviceCapabilities,
) -> Result<(ClassicalPhotonicProgram, DeviceProgram)> {
    let decision_by_id: HashMap<&str, &PlacementDecision> = decisions
        .iter()
        .map(|decision| (decision.op_id.as_str(), decision))
        .collect();
    let mut photonic_ops = Vec::new();
    let mut host_fallback_ops = Vec::new();
    let mut commands = Vec::new();
    let mut calibrated = false;
    let mut current_time_ns = 0.0;

    for gemm in validated {
        let decision = decision_by_id
            .get(gemm.op.id())
            .copied()
            .with_context(|| format!("missing placement decision for '{}'", gemm.op.id()))?;
        if decision.selected_backend == TargetBackend::Photonic {
            if !calibrated {
                if let Some(profile) = &capabilities.calibration_profile {
                    commands.push(DeviceCommand::Calibrate {
                        profile_id: profile.id.clone(),
                    });
                    current_time_ns += capabilities.reconfiguration_latency_ns;
                }
                calibrated = true;
            }
            lower_photonic_gemm(
                gemm,
                capabilities,
                &mut current_time_ns,
                &mut photonic_ops,
                &mut commands,
            );
            commands.push(DeviceCommand::Download {
                tensor: gemm.output.id.clone(),
                adc_bits: capabilities.adc_bits,
            });
        } else {
            let fallback = HostFallbackOp {
                op_id: gemm.op.id().to_string(),
                op_type: "gemm".to_string(),
                reason: decision.rationale.clone(),
            };
            commands.push(DeviceCommand::HostGemm {
                op_id: fallback.op_id.clone(),
                reason: fallback.reason.clone(),
            });
            host_fallback_ops.push(fallback);
        }
    }

    let tensors = program.tensors.iter().map(CompiledTensor::from).collect();
    Ok((
        ClassicalPhotonicProgram {
            ir_version: PHOTONIC_IR_VERSION.to_string(),
            source_ir_version: program.ir_version.clone(),
            backend_id: capabilities.backend_id.clone(),
            tensors,
            ops: photonic_ops,
            host_fallback_ops,
            calibration: capabilities.calibration_profile.clone(),
        },
        DeviceProgram {
            ir_version: DEVICE_IR_VERSION.to_string(),
            backend_id: capabilities.backend_id.clone(),
            commands,
        },
    ))
}

fn lower_photonic_gemm(
    gemm: &ValidatedGemm<'_>,
    capabilities: &DeviceCapabilities,
    current_time_ns: &mut f64,
    ops: &mut Vec<PhotonicGemmTile>,
    commands: &mut Vec<DeviceCommand>,
) {
    let TensorOp::Gemm {
        lhs,
        rhs,
        output,
        transpose_lhs,
        transpose_rhs,
        ..
    } = gemm.op;
    let precision = PrecisionPlan {
        source_dtype: gemm.lhs.dtype,
        optical_effective_bits: capabilities.effective_bits,
        dac_bits: capabilities.dac_bits,
        adc_bits: capabilities.adc_bits,
        digital_accumulation: true,
    };
    let accumulation_mode = if capabilities
        .accumulation_modes
        .contains(&AccumulationMode::Hybrid)
    {
        AccumulationMode::Hybrid
    } else {
        AccumulationMode::Digital
    };

    for tile in tiles(gemm.shape, capabilities) {
        let duration_ns = tile_duration_ns(tile, capabilities);
        let wavelength_count = capabilities
            .simultaneous_channels
            .min(capabilities.supported_wavelengths_nm.len())
            .min(tile.k);
        let wavelengths = capabilities.supported_wavelengths_nm[..wavelength_count].to_vec();
        let op_id = format!(
            "{}__m{}_n{}_k{}",
            gemm.op.id(),
            tile.m_offset,
            tile.n_offset,
            tile.k_offset
        );
        let calibration_handle = capabilities
            .calibration_profile
            .as_ref()
            .map(|profile| profile.id.clone());
        ops.push(PhotonicGemmTile {
            op_id: op_id.clone(),
            op_type: "photonic.gemm".to_string(),
            lhs: lhs.clone(),
            rhs: rhs.clone(),
            output: output.clone(),
            transpose_lhs: *transpose_lhs,
            transpose_rhs: *transpose_rhs,
            tile,
            precision: precision.clone(),
            wavelength_channels: wavelengths.clone(),
            accumulation_mode,
            calibration_handle,
            timing: Timing {
                start_ns: *current_time_ns,
                duration_ns,
            },
        });

        commands.push(DeviceCommand::ConfigureMatrix {
            op_id: op_id.clone(),
            tile,
            precision: precision.clone(),
        });
        let (lhs_row_offset, lhs_column_offset, lhs_rows, lhs_columns) = if *transpose_lhs {
            (tile.k_offset, tile.m_offset, tile.k, tile.m)
        } else {
            (tile.m_offset, tile.k_offset, tile.m, tile.k)
        };
        commands.push(DeviceCommand::UploadTile {
            tensor: lhs.clone(),
            row_offset: lhs_row_offset,
            column_offset: lhs_column_offset,
            rows: lhs_rows,
            columns: lhs_columns,
            dac_bits: capabilities.dac_bits,
        });
        let (rhs_row_offset, rhs_column_offset, rhs_rows, rhs_columns) = if *transpose_rhs {
            (tile.n_offset, tile.k_offset, tile.n, tile.k)
        } else {
            (tile.k_offset, tile.n_offset, tile.k, tile.n)
        };
        commands.push(DeviceCommand::UploadTile {
            tensor: rhs.clone(),
            row_offset: rhs_row_offset,
            column_offset: rhs_column_offset,
            rows: rhs_rows,
            columns: rhs_columns,
            dac_bits: capabilities.dac_bits,
        });
        commands.push(DeviceCommand::ExecuteGemm {
            op_id,
            tile,
            wavelength_channels: wavelengths,
        });
        commands.push(DeviceCommand::Accumulate {
            tensor: output.clone(),
            tile,
            mode: accumulation_mode,
        });
        *current_time_ns += duration_ns;
    }
}

pub fn tiles(shape: GemmShape, capabilities: &DeviceCapabilities) -> Vec<Tile> {
    let core = capabilities.matrix_core;
    let mut result = Vec::new();
    for m_offset in (0..shape.m).step_by(core.m) {
        for n_offset in (0..shape.n).step_by(core.n) {
            for k_offset in (0..shape.k).step_by(core.k) {
                result.push(Tile {
                    m_offset,
                    n_offset,
                    k_offset,
                    m: core.m.min(shape.m - m_offset),
                    n: core.n.min(shape.n - n_offset),
                    k: core.k.min(shape.k - k_offset),
                });
            }
        }
    }
    result
}

fn tile_duration_ns(tile: Tile, capabilities: &DeviceCapabilities) -> f64 {
    let conversion_samples = tile.m + tile.k + tile.n;
    conversion_samples as f64 / capabilities.sample_rate_gsps
        + 1.0 / capabilities.modulation_rate_gbaud
}
