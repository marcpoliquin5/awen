use crate::capability::{AccumulationMode, CalibrationProfile, DeviceCapabilities};
use crate::cost::{PlacementDecision, TargetBackend, TuningPlan};
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
    pub bit_slices: u8,
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
                decision.selected_plan,
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
    selected_plan: Option<TuningPlan>,
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
    let plan = selected_plan.unwrap_or(TuningPlan {
        tile_m: capabilities.matrix_core.m,
        tile_n: capabilities.matrix_core.n,
        tile_k: capabilities.matrix_core.k,
        bit_slices: 1,
        wavelength_channels: capabilities.simultaneous_channels,
        accumulation_mode: if capabilities
            .accumulation_modes
            .contains(&AccumulationMode::Hybrid)
        {
            AccumulationMode::Hybrid
        } else {
            AccumulationMode::Digital
        },
        batch_size: 1,
        fuse_boundaries: false,
    });
    let digital_accumulation = matches!(
        plan.accumulation_mode,
        AccumulationMode::Digital | AccumulationMode::Hybrid
    );
    let precision = PrecisionPlan {
        source_dtype: gemm.lhs.dtype,
        optical_effective_bits: capabilities.effective_bits.saturating_mul(plan.bit_slices),
        dac_bits: capabilities.dac_bits,
        adc_bits: capabilities.adc_bits,
        bit_slices: plan.bit_slices,
        digital_accumulation,
    };
    let accumulation_mode = plan.accumulation_mode;

    for tile in tiles_with_plan(gemm.shape, plan) {
        let wavelength_count = plan
            .wavelength_channels
            .min(capabilities.simultaneous_channels)
            .min(capabilities.supported_wavelengths_nm.len())
            .min(tile.k);
        let duration_ns = tile_duration_ns(tile, capabilities, plan.bit_slices, wavelength_count);
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
    tiles_for_shape(shape, core.m, core.n, core.k)
}

pub fn tiles_with_plan(shape: GemmShape, plan: TuningPlan) -> Vec<Tile> {
    tiles_for_shape(shape, plan.tile_m, plan.tile_n, plan.tile_k)
}

fn tiles_for_shape(shape: GemmShape, tile_m: usize, tile_n: usize, tile_k: usize) -> Vec<Tile> {
    let mut result = Vec::new();
    for m_offset in (0..shape.m).step_by(tile_m) {
        for n_offset in (0..shape.n).step_by(tile_n) {
            for k_offset in (0..shape.k).step_by(tile_k) {
                result.push(Tile {
                    m_offset,
                    n_offset,
                    k_offset,
                    m: tile_m.min(shape.m - m_offset),
                    n: tile_n.min(shape.n - n_offset),
                    k: tile_k.min(shape.k - k_offset),
                });
            }
        }
    }
    result
}

fn tile_duration_ns(
    tile: Tile,
    capabilities: &DeviceCapabilities,
    bit_slices: u8,
    wavelength_count: usize,
) -> f64 {
    let conversion_samples = tile.m + tile.k + tile.n;
    let slices = f64::from(bit_slices);
    conversion_samples as f64 * slices / capabilities.sample_rate_gsps
        + slices / (capabilities.modulation_rate_gbaud * wavelength_count.max(1) as f64)
}
