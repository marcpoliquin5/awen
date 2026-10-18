//! Health-aware placement with CPU fallback.
//!
//! A [`DevicePool`] holds simulated photonic devices with a health state and
//! a set of supported kernel kinds. [`DevicePool::execute`] places a kernel on
//! the first healthy device that supports it, remaps away from degraded or
//! failed devices, and falls back to the CPU reference when no photonic device
//! can run it. Every result carries the placement decision and its reason.

use anyhow::{bail, Result};
use awen_compiler::{
    execute_kernel_reference, execute_kernel_simulator, KernelKind, KernelRequest, KernelResult,
    KernelSimulatorOptions, TargetBackend,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const CPU_DEVICE: &str = "cpu";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Health {
    Healthy,
    /// Usable only when the caller allows degraded devices.
    Degraded {
        reason: String,
    },
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulatedDevice {
    pub id: String,
    pub supported: BTreeSet<KernelKind>,
    pub effective_bits: u8,
    pub noise_fraction: f64,
    pub health: Health,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Placement {
    pub device: String,
    pub fell_back_to_cpu: bool,
    pub skipped: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlacementPolicy {
    pub allow_degraded: bool,
    /// When false, a kernel no photonic device can run is an error instead of
    /// a CPU fallback.
    pub forbid_cpu_fallback: bool,
}

#[derive(Debug, Default)]
pub struct DevicePool {
    devices: Vec<SimulatedDevice>,
}

impl DevicePool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, device: SimulatedDevice) {
        self.devices.push(device);
    }

    pub fn devices(&self) -> &[SimulatedDevice] {
        &self.devices
    }

    pub fn set_health(&mut self, id: &str, health: Health) -> Result<()> {
        match self.devices.iter_mut().find(|device| device.id == id) {
            Some(device) => {
                device.health = health;
                Ok(())
            }
            None => bail!("unknown device {id}"),
        }
    }

    pub fn place(&self, kind: KernelKind, policy: PlacementPolicy) -> Result<Placement> {
        let mut skipped = Vec::new();
        for device in &self.devices {
            if !device.supported.contains(&kind) {
                skipped.push(format!("{}: unsupported {kind:?}", device.id));
                continue;
            }
            match &device.health {
                Health::Healthy => {
                    return Ok(Placement {
                        device: device.id.clone(),
                        fell_back_to_cpu: false,
                        reason: if skipped.is_empty() {
                            "first healthy supporting device".into()
                        } else {
                            "remapped past unavailable devices".into()
                        },
                        skipped,
                    })
                }
                Health::Degraded { reason } if policy.allow_degraded => {
                    return Ok(Placement {
                        device: device.id.clone(),
                        fell_back_to_cpu: false,
                        reason: format!("degraded device allowed by policy: {reason}"),
                        skipped,
                    })
                }
                Health::Degraded { reason } => {
                    skipped.push(format!("{}: degraded ({reason})", device.id))
                }
                Health::Failed { reason } => {
                    skipped.push(format!("{}: failed ({reason})", device.id))
                }
            }
        }
        if policy.forbid_cpu_fallback {
            bail!(
                "no photonic device can run {kind:?} and CPU fallback is forbidden: {}",
                skipped.join("; ")
            );
        }
        Ok(Placement {
            device: CPU_DEVICE.into(),
            fell_back_to_cpu: true,
            reason: "no healthy photonic device supports this kernel".into(),
            skipped,
        })
    }

    /// Place and run. Photonic placements run on the noise-model simulator
    /// (`result.simulated == true`); CPU placements run the exact reference.
    pub fn execute(
        &self,
        request: &KernelRequest,
        policy: PlacementPolicy,
        seed: u64,
    ) -> Result<(Placement, KernelResult)> {
        let placement = self.place(request.kind, policy)?;
        let result = if placement.fell_back_to_cpu {
            execute_kernel_reference(request)?
        } else {
            let device = self
                .devices
                .iter()
                .find(|device| device.id == placement.device)
                .expect("placed device exists");
            execute_kernel_simulator(
                request,
                KernelSimulatorOptions {
                    target: TargetBackend::Photonic,
                    effective_bits: device.effective_bits,
                    noise_fraction: device.noise_fraction,
                    seed,
                },
            )?
        };
        Ok((placement, result))
    }
}
