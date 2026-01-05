//! Memory primitive abstractions for photonic computation

use super::QuantumMode;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Memory primitive trait
pub trait MemoryPrimitive: Send + Sync {
    /// Write photonic mode to memory
    fn write(&mut self, mode: QuantumMode, timestamp_ns: u64) -> Result<()>;

    /// Read photonic mode from memory (may be lossy/delayed)
    fn read(&mut self, timestamp_ns: u64) -> Result<Option<QuantumMode>>;

    /// Get lifetime/decay characteristics
    fn lifetime_ns(&self) -> u64;

    /// Check if data still valid at given time
    fn is_valid(&self, write_time_ns: u64, read_time_ns: u64) -> bool;
}

/// Photonic FIFO buffer (optical delay line)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DelayBuffer {
    pub id: String,
    pub latency_ns: u64,
    pub insertion_loss_db: f64,
    pub coherence_time_ns: u64,
    pub bandwidth_thz: f64,
    pub capacity_modes: usize,

    // Internal state
    #[serde(skip)]
    buffer: Vec<(QuantumMode, u64)>, // (mode, write_time)
}

impl DelayBuffer {
    pub fn new(
        id: String,
        latency_ns: u64,
        insertion_loss_db: f64,
        coherence_time_ns: u64,
    ) -> Self {
        Self {
            id,
            latency_ns,
            insertion_loss_db,
            coherence_time_ns,
            bandwidth_thz: 40.0, // C-band default
            capacity_modes: 80,  // WDM capacity
            buffer: Vec::new(),
        }
    }

    fn apply_loss(&self, mode: &mut QuantumMode) {
        let transmission = 10.0_f64.powf(-self.insertion_loss_db / 10.0);
        if let Some(ref mut amps) = mode.amplitudes {
            for a in amps.iter_mut() {
                *a *= transmission.sqrt();
            }
        }
    }
}

impl MemoryPrimitive for DelayBuffer {
    fn write(&mut self, mode: QuantumMode, timestamp_ns: u64) -> Result<()> {
        if self.buffer.len() >= self.capacity_modes {
            bail!(
                "DelayBuffer {} full (capacity: {})",
                self.id,
                self.capacity_modes
            );
        }
        self.buffer.push((mode, timestamp_ns));
        Ok(())
    }

    fn read(&mut self, timestamp_ns: u64) -> Result<Option<QuantumMode>> {
        // FIFO: find first mode that has completed delay
        let mut result: Option<QuantumMode> = None;
        let mut remove_idx: Option<usize> = None;

        for (i, (mode, write_time)) in self.buffer.iter().enumerate() {
            if timestamp_ns >= *write_time + self.latency_ns {
                let mut out_mode = mode.clone();
                self.apply_loss(&mut out_mode);
                result = Some(out_mode);
                remove_idx = Some(i);
                break;
            }
        }

        if let Some(i) = remove_idx {
            self.buffer.remove(i);
        }

        Ok(result)
    }

    fn lifetime_ns(&self) -> u64 {
        self.latency_ns
    }

    fn is_valid(&self, write_time_ns: u64, read_time_ns: u64) -> bool {
        let elapsed = read_time_ns.saturating_sub(write_time_ns);
        elapsed >= self.latency_ns && elapsed < (self.latency_ns + self.coherence_time_ns)
    }
}

/// Photonic cache (microring resonator)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResonatorStore {
    pub id: String,
    pub lifetime_ns: u64, // Exponential decay τ
    pub coupling_q: f64,  // Quality factor
    pub bandwidth_ghz: f64,
    pub read_efficiency: f64,
    pub write_efficiency: f64,

    // Internal state
    #[serde(skip)]
    stored: Option<(QuantumMode, u64)>, // (mode, write_time)
}

impl ResonatorStore {
    pub fn new(id: String, lifetime_ns: u64) -> Self {
        Self {
            id,
            lifetime_ns,
            coupling_q: 10_000.0,
            bandwidth_ghz: 10.0,
            read_efficiency: 0.9,
            write_efficiency: 0.95,
            stored: None,
        }
    }

    fn apply_decay(&self, mode: &mut QuantumMode, elapsed_ns: u64) {
        let decay_factor = (-(elapsed_ns as f64) / self.lifetime_ns as f64).exp();
        if let Some(ref mut amps) = mode.amplitudes {
            for a in amps.iter_mut() {
                *a *= decay_factor;
            }
        }
    }
}

impl MemoryPrimitive for ResonatorStore {
    fn write(&mut self, mut mode: QuantumMode, timestamp_ns: u64) -> Result<()> {
        // Apply write efficiency loss
        if let Some(ref mut amps) = mode.amplitudes {
            for a in amps.iter_mut() {
                *a *= self.write_efficiency.sqrt();
            }
        }
        self.stored = Some((mode, timestamp_ns));
        Ok(())
    }

    fn read(&mut self, timestamp_ns: u64) -> Result<Option<QuantumMode>> {
        if let Some((mode, write_time)) = &self.stored {
            let elapsed = timestamp_ns.saturating_sub(*write_time);
            let mut out_mode = mode.clone();

            // Apply exponential decay
            self.apply_decay(&mut out_mode, elapsed);

            // Apply read efficiency
            if let Some(ref mut amps) = out_mode.amplitudes {
                for a in amps.iter_mut() {
                    *a *= self.read_efficiency.sqrt();
                }
            }

            // Partial readout: mode stays in resonator (reduced amplitude)
            if let Some((ref mut stored_mode, _)) = &mut self.stored {
                if let Some(ref mut amps) = stored_mode.amplitudes {
                    for a in amps.iter_mut() {
                        *a *= (1.0 - self.read_efficiency).sqrt();
                    }
                }
            }

            Ok(Some(out_mode))
        } else {
            Ok(None)
        }
    }

    fn lifetime_ns(&self) -> u64 {
        self.lifetime_ns
    }

    fn is_valid(&self, write_time_ns: u64, read_time_ns: u64) -> bool {
        let elapsed = read_time_ns.saturating_sub(write_time_ns);
        // Valid if within ~5τ (decay to ~1% amplitude)
        elapsed < (5 * self.lifetime_ns)
    }
}

/// Electronic/Photonic hybrid register
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HybridRegister {
    pub id: String,
    pub capacity_bits: usize,
    pub read_latency_ns: u64,
    pub write_latency_ns: u64,
    pub fidelity: f64,

    // Internal state (classical storage)
    #[serde(skip)]
    register: Vec<f64>, // Classical values
    #[serde(skip)]
    last_write_time: Option<u64>,
}

impl HybridRegister {
    pub fn new(id: String, capacity_bits: usize) -> Self {
        Self {
            id,
            capacity_bits,
            read_latency_ns: 1000, // 1μs E→P conversion
            write_latency_ns: 500, // 500ns P→E conversion
            fidelity: 0.99,
            register: vec![0.0; capacity_bits],
            last_write_time: None,
        }
    }

    /// Write classical value to register
    pub fn write_classical(&mut self, address: usize, value: f64, timestamp_ns: u64) -> Result<()> {
        if address >= self.capacity_bits {
            bail!(
                "Address {} out of bounds (capacity: {})",
                address,
                self.capacity_bits
            );
        }
        self.register[address] = value;
        self.last_write_time = Some(timestamp_ns);
        Ok(())
    }

    /// Read classical value from register
    pub fn read_classical(&self, address: usize) -> Result<f64> {
        if address >= self.capacity_bits {
            bail!(
                "Address {} out of bounds (capacity: {})",
                address,
                self.capacity_bits
            );
        }
        Ok(self.register[address])
    }
}

impl MemoryPrimitive for HybridRegister {
    fn write(&mut self, mode: QuantumMode, timestamp_ns: u64) -> Result<()> {
        // Photonic → Electronic: measure and store classical result
        let classical_value = if let Some(amps) = &mode.amplitudes {
            // Store first amplitude magnitude as classical value
            amps.first().map(|a| a.abs()).unwrap_or(0.0)
        } else {
            0.0
        };

        // Apply fidelity noise
        let noisy_value = classical_value * self.fidelity;

        // Store in first available address
        if !self.register.is_empty() {
            self.register[0] = noisy_value;
        }
        self.last_write_time = Some(timestamp_ns);
        Ok(())
    }

    fn read(&mut self, _timestamp_ns: u64) -> Result<Option<QuantumMode>> {
        // Electronic → Photonic: modulate based on stored value
        if self.register.is_empty() {
            return Ok(None);
        }

        let classical_value = self.register[0];

        // Create photonic mode from classical value
        let mode = QuantumMode {
            mode_id: format!("{}_output", self.id),
            mode_type: "classical".to_string(),
            photon_numbers: Some(vec![0, 1]),
            amplitudes: Some(vec![classical_value * self.fidelity]),
            phases: Some(vec![0.0]),
        };

        Ok(Some(mode))
    }

    fn lifetime_ns(&self) -> u64 {
        u64::MAX // Persistent electronic storage
    }

    fn is_valid(&self, _write_time_ns: u64, _read_time_ns: u64) -> bool {
        true // Always valid (persistent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_buffer_fifo() {
        let mut buffer = DelayBuffer::new("delay_0".to_string(), 1000, 0.5, 10_000);

        let mode = QuantumMode {
            mode_id: "test_mode".to_string(),
            mode_type: "classical".to_string(),
            photon_numbers: None,
            amplitudes: Some(vec![1.0]),
            phases: Some(vec![0.0]),
        };

        // Write at t=0
        buffer.write(mode.clone(), 0).unwrap();

        // Try read before delay completes
        assert!(buffer.read(500).unwrap().is_none());

        // Read after delay
        let result = buffer.read(1000).unwrap();
        assert!(result.is_some());

        // Check loss applied
        let out_mode = result.unwrap();
        let out_amp = out_mode.amplitudes.unwrap()[0];
        let expected_amp = 1.0 * 10.0_f64.powf(-0.5 / 10.0).sqrt();
        assert!((out_amp - expected_amp).abs() < 0.01);
    }

    #[test]
    fn test_resonator_exponential_decay() {
        let mut resonator = ResonatorStore::new("res_0".to_string(), 500);

        let mode = QuantumMode {
            mode_id: "test_mode".to_string(),
            mode_type: "quantum_fock".to_string(),
            photon_numbers: Some(vec![0, 1]),
            amplitudes: Some(vec![1.0]),
            phases: Some(vec![0.0]),
        };

        // Write at t=0
        resonator.write(mode.clone(), 0).unwrap();

        // Read at t=500 (1τ)
        let result = resonator.read(500).unwrap().unwrap();
        let out_amp = result.amplitudes.unwrap()[0];

        // Expected: write_eff * exp(-500/500) * read_eff
        let expected = 0.95_f64.sqrt() * (-1.0_f64).exp() * 0.9_f64.sqrt();
        assert!((out_amp - expected).abs() < 0.01);
    }

    #[test]
    fn test_hybrid_register_persistence() {
        let mut register = HybridRegister::new("hybrid_0".to_string(), 256);

        // Write classical value
        register.write_classical(0, 42.5, 0).unwrap();

        // Read after long time (should persist)
        let value = register.read_classical(0).unwrap();
        assert!((value - 42.5).abs() < 0.01);

        // Check lifetime
        assert_eq!(register.lifetime_ns(), u64::MAX);
        assert!(register.is_valid(0, 1_000_000_000)); // 1 second later
    }

    #[test]
    fn test_delay_buffer_capacity() {
        let mut buffer = DelayBuffer::new("delay_0".to_string(), 1000, 0.5, 10_000);
        buffer.capacity_modes = 2;

        let mode = QuantumMode {
            mode_id: "test_mode".to_string(),
            mode_type: "classical".to_string(),
            photon_numbers: None,
            amplitudes: Some(vec![1.0]),
            phases: Some(vec![0.0]),
        };

        // Fill capacity
        buffer.write(mode.clone(), 0).unwrap();
        buffer.write(mode.clone(), 100).unwrap();

        // Should fail (full)
        assert!(buffer.write(mode.clone(), 200).is_err());
    }
}
