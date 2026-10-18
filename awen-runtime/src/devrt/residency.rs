//! Memory residency tracking across host and device memories.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const HOST: &str = "host";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BufferRecord {
    pub id: String,
    pub bytes: u64,
    pub location: String,
    pub pinned: bool,
    last_use: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferStats {
    pub host_to_device_bytes: u64,
    pub device_to_host_bytes: u64,
    pub device_to_device_bytes: u64,
    pub evictions: u64,
}

/// Tracks where each buffer lives. Devices have finite capacity; the host is
/// treated as unbounded. Making a buffer resident on a full device evicts the
/// least recently used unpinned buffer back to the host.
#[derive(Debug, Default)]
pub struct ResidencyTracker {
    capacity: BTreeMap<String, u64>,
    buffers: BTreeMap<String, BufferRecord>,
    clock: u64,
    pub stats: TransferStats,
}

impl ResidencyTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_device(&mut self, device: impl Into<String>, capacity_bytes: u64) {
        self.capacity.insert(device.into(), capacity_bytes);
    }

    pub fn used(&self, device: &str) -> u64 {
        self.buffers
            .values()
            .filter(|buffer| buffer.location == device)
            .map(|buffer| buffer.bytes)
            .sum()
    }

    pub fn location(&self, id: &str) -> Option<&str> {
        self.buffers.get(id).map(|buffer| buffer.location.as_str())
    }

    pub fn buffer(&self, id: &str) -> Option<&BufferRecord> {
        self.buffers.get(id)
    }

    /// Register a new host-resident buffer.
    pub fn allocate_host(&mut self, id: impl Into<String>, bytes: u64) -> Result<()> {
        let id = id.into();
        if self.buffers.contains_key(&id) {
            bail!("buffer {id} already exists");
        }
        self.clock += 1;
        self.buffers.insert(
            id.clone(),
            BufferRecord {
                id,
                bytes,
                location: HOST.into(),
                pinned: false,
                last_use: self.clock,
            },
        );
        Ok(())
    }

    pub fn free(&mut self, id: &str) -> Result<()> {
        self.buffers
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| anyhow::anyhow!("unknown buffer {id}"))
    }

    pub fn set_pinned(&mut self, id: &str, pinned: bool) -> Result<()> {
        match self.buffers.get_mut(id) {
            Some(buffer) => {
                buffer.pinned = pinned;
                Ok(())
            }
            None => bail!("unknown buffer {id}"),
        }
    }

    /// Ensure `id` is resident on `target` (a device or [`HOST`]), evicting
    /// as needed. Returns the ids evicted to the host.
    pub fn make_resident(&mut self, id: &str, target: &str) -> Result<Vec<String>> {
        let Some(record) = self.buffers.get(id).cloned() else {
            bail!("unknown buffer {id}");
        };
        self.clock += 1;
        let mut evicted = Vec::new();
        if record.location != target {
            if target != HOST {
                let Some(&capacity) = self.capacity.get(target) else {
                    bail!("unknown device {target}");
                };
                if record.bytes > capacity {
                    bail!(
                        "buffer {id} ({} B) exceeds capacity of {target} ({capacity} B)",
                        record.bytes
                    );
                }
                while self.used(target) + record.bytes > capacity {
                    let victim = self
                        .buffers
                        .values()
                        .filter(|buffer| buffer.location == target && !buffer.pinned)
                        .min_by_key(|buffer| buffer.last_use)
                        .map(|buffer| buffer.id.clone());
                    let Some(victim) = victim else {
                        bail!("device {target} is full of pinned buffers");
                    };
                    let bytes = self.buffers[&victim].bytes;
                    self.stats.device_to_host_bytes += bytes;
                    self.stats.evictions += 1;
                    self.buffers.get_mut(&victim).expect("victim").location = HOST.into();
                    evicted.push(victim);
                }
            }
            match (record.location.as_str(), target) {
                (HOST, _) => self.stats.host_to_device_bytes += record.bytes,
                (_, HOST) => self.stats.device_to_host_bytes += record.bytes,
                _ => self.stats.device_to_device_bytes += record.bytes,
            }
        }
        let buffer = self.buffers.get_mut(id).expect("buffer");
        buffer.location = target.into();
        buffer.last_use = self.clock;
        Ok(evicted)
    }
}
