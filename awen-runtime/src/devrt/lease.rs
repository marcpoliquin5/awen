//! Device leases and resource budgets.
//!
//! A lease grants a tenant exclusive or shared use of a device until a logical
//! deadline. Budgets cap caller-declared resource usage (operations, optical
//! energy in picojoules as estimated by the simulator, wall time). Budgets are
//! enforced on reported usage; they are not measured by this module.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaseMode {
    Exclusive,
    Shared,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Budget {
    pub max_operations: Option<u64>,
    pub max_energy_pj: Option<f64>,
    pub max_wall_ns: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub operations: u64,
    pub energy_pj: f64,
    pub wall_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Lease {
    pub id: u64,
    pub tenant: String,
    pub device: String,
    pub mode: LeaseMode,
    pub expires_at: u64,
    pub budget: Budget,
    pub usage: Usage,
}

/// Grants and tracks leases against a caller-advanced logical clock so tests
/// and replays are deterministic.
#[derive(Debug, Default)]
pub struct LeaseManager {
    now: u64,
    next_id: u64,
    leases: BTreeMap<u64, Lease>,
}

impl LeaseManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn now(&self) -> u64 {
        self.now
    }

    /// Advance the logical clock and drop expired leases. Returns expired ids.
    pub fn advance(&mut self, ticks: u64) -> Vec<u64> {
        self.now += ticks;
        let now = self.now;
        let expired: Vec<u64> = self
            .leases
            .values()
            .filter(|lease| lease.expires_at <= now)
            .map(|lease| lease.id)
            .collect();
        for id in &expired {
            self.leases.remove(id);
        }
        expired
    }

    pub fn active(&self, device: &str) -> Vec<&Lease> {
        self.leases
            .values()
            .filter(|lease| lease.device == device)
            .collect()
    }

    pub fn acquire(
        &mut self,
        tenant: impl Into<String>,
        device: impl Into<String>,
        mode: LeaseMode,
        duration: u64,
        budget: Budget,
    ) -> Result<u64> {
        let device = device.into();
        if duration == 0 {
            bail!("lease duration must be positive");
        }
        let held = self.active(&device);
        let conflict = match mode {
            LeaseMode::Exclusive => !held.is_empty(),
            LeaseMode::Shared => held.iter().any(|lease| lease.mode == LeaseMode::Exclusive),
        };
        if conflict {
            bail!("device {device} is not available for a {mode:?} lease");
        }
        self.next_id += 1;
        let id = self.next_id;
        self.leases.insert(
            id,
            Lease {
                id,
                tenant: tenant.into(),
                device,
                mode,
                expires_at: self.now + duration,
                budget,
                usage: Usage::default(),
            },
        );
        Ok(id)
    }

    pub fn release(&mut self, id: u64) -> Result<Lease> {
        self.leases
            .remove(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown or expired lease {id}"))
    }

    pub fn lease(&self, id: u64) -> Option<&Lease> {
        self.leases.get(&id)
    }

    /// Charge usage to a lease. Rejects (without charging) any charge that
    /// would exceed a budget limit.
    pub fn charge(&mut self, id: u64, operations: u64, energy_pj: f64, wall_ns: u64) -> Result<()> {
        let Some(lease) = self.leases.get_mut(&id) else {
            bail!("unknown or expired lease {id}");
        };
        if !(energy_pj.is_finite() && energy_pj >= 0.0) {
            bail!("energy charge must be finite and non-negative");
        }
        let next = Usage {
            operations: lease.usage.operations + operations,
            energy_pj: lease.usage.energy_pj + energy_pj,
            wall_ns: lease.usage.wall_ns + wall_ns,
        };
        if lease
            .budget
            .max_operations
            .is_some_and(|max| next.operations > max)
        {
            bail!("lease {id} operation budget exceeded");
        }
        if lease
            .budget
            .max_energy_pj
            .is_some_and(|max| next.energy_pj > max)
        {
            bail!("lease {id} energy budget exceeded");
        }
        if lease
            .budget
            .max_wall_ns
            .is_some_and(|max| next.wall_ns > max)
        {
            bail!("lease {id} wall-time budget exceeded");
        }
        lease.usage = next;
        Ok(())
    }
}
