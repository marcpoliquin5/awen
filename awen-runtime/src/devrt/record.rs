//! Deterministic mode and experiment recording.
//!
//! In deterministic mode every execution draws its simulator seed from a
//! [`DeterministicSeeds`] sequence derived from one root seed, and runs in a
//! fixed order. An [`ExperimentRecorder`] stores, per execution, SHA-256
//! fingerprints of the canonical request JSON and of the output bytes, the
//! placement and the seed. [`ExperimentRecord::verify_replay`] checks a new
//! run against a recording bit-for-bit.

use super::remap::{DevicePool, Placement, PlacementPolicy};
use anyhow::{bail, Context, Result};
use awen_compiler::{KernelData, KernelRequest, KernelResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// SplitMix64 sequence: a documented, platform-independent seed stream.
#[derive(Debug, Clone)]
pub struct DeterministicSeeds {
    state: u64,
}

impl DeterministicSeeds {
    pub fn new(root: u64) -> Self {
        Self { state: root }
    }

    pub fn next_seed(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

pub fn output_digest(result: &KernelResult) -> String {
    let mut hasher = Sha256::new();
    for output in &result.outputs {
        for dim in &output.shape {
            hasher.update((*dim as u64).to_le_bytes());
        }
        match &output.data {
            KernelData::Real(values) => {
                for value in values {
                    hasher.update(value.to_bits().to_le_bytes());
                }
            }
            KernelData::Complex(values) => {
                for value in values {
                    hasher.update(value.real.to_bits().to_le_bytes());
                    hasher.update(value.imaginary.to_bits().to_le_bytes());
                }
            }
        }
    }
    hex::encode(hasher.finalize())
}

pub fn request_digest(request: &KernelRequest) -> Result<String> {
    let bytes = serde_json::to_vec(request).context("serialize request")?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionEntry {
    pub index: usize,
    pub request_id: String,
    pub request_sha256: String,
    pub seed: u64,
    pub placement: Placement,
    pub simulated: bool,
    pub output_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExperimentRecord {
    pub version: String,
    pub name: String,
    pub root_seed: u64,
    pub deterministic: bool,
    pub entries: Vec<ExecutionEntry>,
}

pub const EXPERIMENT_RECORD_VERSION: &str = "awen.experiment-record.v1";

impl ExperimentRecord {
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(text: &str) -> Result<Self> {
        let record: Self = serde_json::from_str(text)?;
        if record.version != EXPERIMENT_RECORD_VERSION {
            bail!("unsupported experiment record version {}", record.version);
        }
        Ok(record)
    }

    /// Compare against a replay; returns the indices that differ.
    pub fn verify_replay(&self, replay: &ExperimentRecord) -> Vec<usize> {
        let mut mismatched: Vec<usize> = self
            .entries
            .iter()
            .zip(&replay.entries)
            .filter(|(a, b)| a != b)
            .map(|(a, _)| a.index)
            .collect();
        let shorter = self.entries.len().min(replay.entries.len());
        mismatched.extend(shorter..self.entries.len().max(replay.entries.len()));
        mismatched
    }
}

/// Runs kernels on a pool and records each execution.
pub struct ExperimentRecorder<'a> {
    pool: &'a DevicePool,
    policy: PlacementPolicy,
    seeds: DeterministicSeeds,
    record: ExperimentRecord,
}

impl<'a> ExperimentRecorder<'a> {
    pub fn new(
        name: impl Into<String>,
        pool: &'a DevicePool,
        policy: PlacementPolicy,
        root_seed: u64,
    ) -> Self {
        Self {
            pool,
            policy,
            seeds: DeterministicSeeds::new(root_seed),
            record: ExperimentRecord {
                version: EXPERIMENT_RECORD_VERSION.into(),
                name: name.into(),
                root_seed,
                deterministic: true,
                entries: Vec::new(),
            },
        }
    }

    pub fn run(&mut self, request: &KernelRequest) -> Result<KernelResult> {
        let seed = self.seeds.next_seed();
        let (placement, result) = self.pool.execute(request, self.policy, seed)?;
        self.record.entries.push(ExecutionEntry {
            index: self.record.entries.len(),
            request_id: request.id.clone(),
            request_sha256: request_digest(request)?,
            seed,
            placement,
            simulated: result.simulated,
            output_sha256: output_digest(&result),
        });
        Ok(result)
    }

    pub fn finish(self) -> ExperimentRecord {
        self.record
    }
}
