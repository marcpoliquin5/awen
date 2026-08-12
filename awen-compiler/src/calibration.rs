use crate::capability::{
    BackendHealth, CalibrationEnvironment, CalibrationProfile, DeviceCapabilities, HealthStatus,
    MatrixCore,
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const CALIBRATION_DECISION_VERSION: &str = "awen.calibration-decision.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CellRemap {
    pub disabled_cell: String,
    pub replacement_cell: String,
    pub logical_row: usize,
    pub logical_column: usize,
    pub replacement_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EffectiveTransfer {
    pub gain: f64,
    pub offset: f64,
    pub phase_error_radians: f64,
    pub insertion_loss_db: f64,
    pub uncertainty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationDecisionImpact {
    pub decision_version: String,
    pub op_id: String,
    pub disabled_components: Vec<String>,
    pub cell_remaps: Vec<CellRemap>,
    pub excluded_channels: Vec<String>,
    pub selected_channels: Vec<String>,
    pub selected_tile_shape: [usize; 3],
    pub capacity_loss_fraction: f64,
    pub estimated_error_fraction: f64,
    pub effective_transfer: EffectiveTransfer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationArtifactRecord {
    pub snapshot_version: String,
    pub snapshot_id: String,
    pub fingerprint: String,
    pub parent_id: Option<String>,
    pub backend_id: String,
    pub topology_fingerprint: String,
    pub measured_at: String,
    pub environment: CalibrationEnvironment,
    pub uncertainty: f64,
    pub health_observed_at: String,
    pub health_status: HealthStatus,
    pub health_fingerprint: String,
    pub decision_impacts: Vec<CalibrationDecisionImpact>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CalibrationRoutingPlan {
    pub wavelength_channels: Vec<f64>,
    pub impact: CalibrationDecisionImpact,
}

pub(crate) fn route_calibrated_hardware(
    op_id: &str,
    requested_channels: usize,
    selected_tile_shape: [usize; 3],
    capabilities: &DeviceCapabilities,
    health: &BackendHealth,
) -> Result<CalibrationRoutingPlan> {
    let Some(profile) = &capabilities.calibration_profile else {
        let wavelength_channels = capabilities
            .supported_wavelengths_nm
            .iter()
            .copied()
            .take(requested_channels.min(health.available_channels))
            .collect::<Vec<_>>();
        return Ok(CalibrationRoutingPlan {
            impact: CalibrationDecisionImpact {
                decision_version: CALIBRATION_DECISION_VERSION.to_string(),
                op_id: op_id.to_string(),
                disabled_components: health.disabled_components.clone(),
                cell_remaps: Vec::new(),
                excluded_channels: Vec::new(),
                selected_channels: wavelength_channels
                    .iter()
                    .map(|wavelength| format!("wavelength-{wavelength:.4}"))
                    .collect(),
                selected_tile_shape,
                capacity_loss_fraction: 1.0
                    - health.available_channels as f64
                        / capabilities.simultaneous_channels.max(1) as f64,
                estimated_error_fraction: 0.0,
                effective_transfer: EffectiveTransfer {
                    gain: 1.0,
                    offset: 0.0,
                    phase_error_radians: 0.0,
                    insertion_loss_db: 0.0,
                    uncertainty: 0.0,
                },
            },
            wavelength_channels,
        });
    };

    let disabled = health
        .disabled_components
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut disabled_cells = profile
        .cells
        .iter()
        .filter(|cell| disabled.contains(cell.id.as_str()))
        .collect::<Vec<_>>();
    disabled_cells.sort_by(|left, right| left.id.cmp(&right.id));
    let mut available_spares = profile
        .spare_cells
        .iter()
        .filter(|spare| !disabled.contains(spare.id.as_str()))
        .collect::<Vec<_>>();
    available_spares.sort_by(|left, right| {
        transfer_score(
            left.gain,
            left.offset,
            left.phase_error_radians,
            left.insertion_loss_db,
            left.uncertainty,
        )
        .total_cmp(&transfer_score(
            right.gain,
            right.offset,
            right.phase_error_radians,
            right.insertion_loss_db,
            right.uncertainty,
        ))
        .then_with(|| left.id.cmp(&right.id))
    });
    if disabled_cells.len() > available_spares.len() {
        bail!(
            "calibration remap capacity exhausted: {} disabled cells require spares but only {} are healthy",
            disabled_cells.len(),
            available_spares.len()
        );
    }
    let cell_remaps = disabled_cells
        .iter()
        .zip(&available_spares)
        .map(|(cell, spare)| CellRemap {
            disabled_cell: cell.id.clone(),
            replacement_cell: spare.id.clone(),
            logical_row: cell.row,
            logical_column: cell.column,
            replacement_score: transfer_score(
                spare.gain,
                spare.offset,
                spare.phase_error_radians,
                spare.insertion_loss_db,
                spare.uncertainty,
            ),
        })
        .collect::<Vec<_>>();

    let mut calibrated_channels = capabilities
        .supported_wavelengths_nm
        .iter()
        .enumerate()
        .filter_map(|(index, wavelength)| {
            let measured = profile
                .channels
                .iter()
                .find(|channel| channel.wavelength_nm.to_bits() == wavelength.to_bits());
            let id =
                measured.map_or_else(|| format!("channel-{index}"), |channel| channel.id.clone());
            (!disabled.contains(id.as_str())).then(|| {
                let score = measured.map_or(1.0, |channel| {
                    transfer_score(
                        channel.gain,
                        0.0,
                        channel.phase_error_radians,
                        channel.insertion_loss_db,
                        channel.uncertainty,
                    )
                });
                (id, *wavelength, score, measured)
            })
        })
        .collect::<Vec<_>>();
    calibrated_channels.sort_by(|left, right| {
        left.2
            .total_cmp(&right.2)
            .then_with(|| left.1.total_cmp(&right.1))
    });
    let selection_count = requested_channels
        .min(health.available_channels)
        .min(calibrated_channels.len());
    if selection_count == 0 {
        bail!("calibration routing found no healthy wavelength channels");
    }
    let selected = &calibrated_channels[..selection_count];
    let selected_ids = selected
        .iter()
        .map(|(id, _, _, _)| id.clone())
        .collect::<Vec<_>>();
    let wavelengths = selected
        .iter()
        .map(|(_, wavelength, _, _)| *wavelength)
        .collect::<Vec<_>>();
    let selected_id_set = selected_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut excluded_channels = profile
        .channels
        .iter()
        .filter(|channel| !selected_id_set.contains(channel.id.as_str()))
        .map(|channel| channel.id.clone())
        .collect::<Vec<_>>();
    excluded_channels.sort();

    let channel_count = selected.iter().filter(|entry| entry.3.is_some()).count() as f64;
    let (channel_gain, channel_phase, channel_loss, channel_uncertainty) =
        selected
            .iter()
            .fold((0.0, 0.0, 0.0, 0.0), |totals, (_, _, _, channel)| {
                channel.map_or(totals, |channel| {
                    (
                        totals.0 + channel.gain,
                        totals.1 + channel.phase_error_radians,
                        totals.2 + channel.insertion_loss_db,
                        totals.3 + channel.uncertainty,
                    )
                })
            });
    let channel_divisor = channel_count.max(1.0);
    let remap_count = cell_remaps.len() as f64;
    let active_cells = profile
        .cells
        .iter()
        .filter(|cell| !disabled.contains(cell.id.as_str()))
        .collect::<Vec<_>>();
    let active_cell_count = active_cells.len() as f64;
    let (cell_gain, cell_offset, cell_phase, cell_loss, cell_uncertainty) = active_cells
        .iter()
        .fold((0.0, 0.0, 0.0, 0.0, 0.0), |totals, cell| {
            (
                totals.0 + cell.gain,
                totals.1 + cell.offset,
                totals.2 + cell.phase_error_radians,
                totals.3 + cell.insertion_loss_db,
                totals.4 + cell.uncertainty,
            )
        });
    let active_cell_divisor = active_cell_count.max(1.0);
    let (spare_gain, spare_offset, spare_phase, spare_loss, spare_uncertainty) = available_spares
        .iter()
        .take(cell_remaps.len())
        .fold((0.0, 0.0, 0.0, 0.0, 0.0), |totals, spare| {
            (
                totals.0 + spare.gain,
                totals.1 + spare.offset,
                totals.2 + spare.phase_error_radians,
                totals.3 + spare.insertion_loss_db,
                totals.4 + spare.uncertainty,
            )
        });
    let remap_divisor = remap_count.max(1.0);
    let effective_transfer = EffectiveTransfer {
        gain: profile.gain
            * if channel_count == 0.0 {
                1.0
            } else {
                channel_gain / channel_divisor
            }
            * if remap_count == 0.0 {
                1.0
            } else {
                spare_gain / remap_divisor
            }
            * if active_cell_count == 0.0 {
                1.0
            } else {
                cell_gain / active_cell_divisor
            },
        offset: profile.offset
            + if active_cell_count == 0.0 {
                0.0
            } else {
                cell_offset / active_cell_divisor
            }
            + if remap_count == 0.0 {
                0.0
            } else {
                spare_offset / remap_divisor
            },
        phase_error_radians: profile.phase_error_radians
            + channel_phase / channel_divisor
            + cell_phase / active_cell_divisor
            + spare_phase / remap_divisor,
        insertion_loss_db: profile.insertion_loss_db
            + channel_loss / channel_divisor
            + cell_loss / active_cell_divisor
            + spare_loss / remap_divisor,
        uncertainty: profile.uncertainty
            + channel_uncertainty / channel_divisor
            + cell_uncertainty / active_cell_divisor
            + spare_uncertainty / remap_divisor,
    };
    let estimated_error_fraction = effective_transfer.uncertainty
        + effective_transfer.phase_error_radians.abs() * 0.01
        + effective_transfer.insertion_loss_db * 0.000_1;
    Ok(CalibrationRoutingPlan {
        wavelength_channels: wavelengths,
        impact: CalibrationDecisionImpact {
            decision_version: CALIBRATION_DECISION_VERSION.to_string(),
            op_id: op_id.to_string(),
            disabled_components: health.disabled_components.clone(),
            cell_remaps,
            excluded_channels,
            selected_channels: selected_ids,
            selected_tile_shape,
            capacity_loss_fraction: 1.0
                - calibrated_channels.len() as f64
                    / capabilities.supported_wavelengths_nm.len().max(1) as f64,
            estimated_error_fraction,
            effective_transfer,
        },
    })
}

pub(crate) fn derated_matrix_core(
    capabilities: &DeviceCapabilities,
    health: &BackendHealth,
) -> MatrixCore {
    let Some(profile) = &capabilities.calibration_profile else {
        return capabilities.matrix_core;
    };
    let disabled = health
        .disabled_components
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let disabled_cells = profile
        .cells
        .iter()
        .filter(|cell| disabled.contains(cell.id.as_str()))
        .count();
    let high_error_cell = profile.cells.iter().any(|cell| {
        !disabled.contains(cell.id.as_str())
            && transfer_score(
                cell.gain,
                cell.offset,
                cell.phase_error_radians,
                cell.insertion_loss_db,
                cell.uncertainty,
            ) > 0.05
    });
    let mut healthy_spares = profile
        .spare_cells
        .iter()
        .filter(|spare| !disabled.contains(spare.id.as_str()))
        .collect::<Vec<_>>();
    healthy_spares.sort_by(|left, right| {
        transfer_score(
            left.gain,
            left.offset,
            left.phase_error_radians,
            left.insertion_loss_db,
            left.uncertainty,
        )
        .total_cmp(&transfer_score(
            right.gain,
            right.offset,
            right.phase_error_radians,
            right.insertion_loss_db,
            right.uncertainty,
        ))
        .then_with(|| left.id.cmp(&right.id))
    });
    let high_error_required_spare = healthy_spares
        .into_iter()
        .take(disabled_cells)
        .any(|spare| {
            transfer_score(
                spare.gain,
                spare.offset,
                spare.phase_error_radians,
                spare.insertion_loss_db,
                spare.uncertainty,
            ) > 0.05
        });
    if disabled_cells == 0 && !high_error_cell && !high_error_required_spare {
        return capabilities.matrix_core;
    }
    MatrixCore {
        m: capabilities.matrix_core.m.div_ceil(2),
        n: capabilities.matrix_core.n.div_ceil(2),
        k: capabilities.matrix_core.k.div_ceil(2),
    }
}

pub(crate) fn artifact_record(
    profile: &CalibrationProfile,
    health: &BackendHealth,
    decision_impacts: Vec<CalibrationDecisionImpact>,
) -> CalibrationArtifactRecord {
    let health_bytes = serde_json::to_vec(health).expect("health snapshots are serializable");
    CalibrationArtifactRecord {
        snapshot_version: profile.snapshot_version.clone(),
        snapshot_id: profile.id.clone(),
        fingerprint: profile.fingerprint.clone(),
        parent_id: profile.parent_id.clone(),
        backend_id: profile.backend_id.clone(),
        topology_fingerprint: profile.topology_fingerprint.clone(),
        measured_at: profile.measured_at.clone(),
        environment: profile.environment.clone(),
        uncertainty: profile.uncertainty,
        health_observed_at: health.observed_at.clone(),
        health_status: health.status,
        health_fingerprint: format!("fnv1a64:{:016x}", fnv1a64(&health_bytes)),
        decision_impacts,
    }
}

fn transfer_score(
    gain: f64,
    offset: f64,
    phase_error_radians: f64,
    insertion_loss_db: f64,
    uncertainty: f64,
) -> f64 {
    (gain - 1.0).abs()
        + offset.abs()
        + phase_error_radians.abs()
        + insertion_loss_db * 0.01
        + uncertainty
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
