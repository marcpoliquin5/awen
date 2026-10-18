//! Vendor-neutral hardware descriptions (`awen.hardware-description.v1`) and
//! reference *simulator* drivers for three component classes.
//!
//! The schema lives at `awen-spec/schemas/awen_hardware_description.v1.json`.
//! The drivers below implement the existing [`Device`] trait on top of
//! textbook component models so the HAL can be exercised end to end without
//! hardware. They are not vendor drivers and make no claim about any product;
//! real drivers are written by hardware owners following
//! `docs/VENDOR_INTEGRATION.md`.

use super::{Capability, ChannelType, Device};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

pub const HARDWARE_DESCRIPTION_VERSION: &str = "awen.hardware-description.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HardwareDescription {
    pub version: String,
    pub device_id: String,
    pub provenance: Value,
    pub components: Vec<Value>,
}

impl HardwareDescription {
    pub fn from_json(text: &str) -> Result<Self> {
        let description: Self = serde_json::from_str(text)?;
        if description.version != HARDWARE_DESCRIPTION_VERSION {
            bail!("unsupported hardware description {}", description.version);
        }
        Ok(description)
    }

    /// `reference_simulator`, `vendor_datasheet` or `measured`.
    pub fn provenance_source(&self) -> Option<&str> {
        self.provenance.get("source").and_then(Value::as_str)
    }

    pub fn component(&self, class: &str, id: &str) -> Option<&Value> {
        self.components.iter().find(|component| {
            component.get("class").and_then(Value::as_str) == Some(class)
                && component.get("id").and_then(Value::as_str) == Some(id)
        })
    }

    pub fn classes(&self) -> Vec<String> {
        let mut classes: Vec<String> = self
            .components
            .iter()
            .filter_map(|component| component.get("class").and_then(Value::as_str))
            .map(str::to_owned)
            .collect();
        classes.sort();
        classes.dedup();
        classes
    }
}

fn number(component: &Value, key: &str) -> Result<f64> {
    component
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("component field {key} missing or not a number"))
}

fn count(component: &Value, key: &str) -> Result<usize> {
    component
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .ok_or_else(|| anyhow!("component field {key} missing or not an integer"))
}

fn expect_class(component: &Value, class: &str) -> Result<String> {
    if component.get("class").and_then(Value::as_str) != Some(class) {
        bail!("component is not a {class}");
    }
    Ok(component
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(class)
        .to_owned())
}

fn parse_indexed<'a>(name: &'a str, prefix: &str) -> Option<(usize, &'a str)> {
    let rest = name.strip_prefix(prefix)?;
    let (index, field) = rest.split_once(':')?;
    Some((index.parse().ok()?, field))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct C64 {
    pub re: f64,
    pub im: f64,
}

impl C64 {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }
    pub fn polar(magnitude: f64, angle: f64) -> Self {
        Self::new(magnitude * angle.cos(), magnitude * angle.sin())
    }
    pub fn times(self, other: Self) -> Self {
        Self::new(
            self.re * other.re - self.im * other.im,
            self.re * other.im + self.im * other.re,
        )
    }
    pub fn plus(self, other: Self) -> Self {
        Self::new(self.re + other.re, self.im + other.im)
    }
    pub fn conj(self) -> Self {
        Self::new(self.re, -self.im)
    }
    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

/// Clements-arranged MZI mesh. Each MZI on modes (i, i+1) applies
/// `[[e^{iφ} cos(θ/2), -sin(θ/2)], [e^{iφ} sin(θ/2), cos(θ/2)]]` scaled by the
/// per-MZI insertion-loss amplitude. Parameters: `mzi_<k>:theta`,
/// `mzi_<k>:phi`, `in_<j>:re`, `in_<j>:im`; sensors: `out_<j>:power`.
pub struct MziMeshDriver {
    id: String,
    modes: usize,
    pairs: Vec<usize>,
    loss_amplitude: f64,
    state: Mutex<MeshState>,
}

struct MeshState {
    theta: Vec<f64>,
    phi: Vec<f64>,
    input: Vec<C64>,
}

impl MziMeshDriver {
    pub fn from_component(component: &Value) -> Result<Self> {
        let id = expect_class(component, "mzi_mesh")?;
        let modes = count(component, "modes")?;
        if modes < 2 {
            bail!("an MZI mesh needs at least two modes");
        }
        let topology = component.get("topology").and_then(Value::as_str);
        if topology != Some("clements") {
            bail!("the reference driver implements the clements topology only");
        }
        let loss_db = number(component, "insertion_loss_db_per_mzi")?;
        let mut pairs = Vec::new();
        for layer in 0..modes {
            let mut top = layer % 2;
            while top + 1 < modes {
                pairs.push(top);
                top += 2;
            }
        }
        let n = pairs.len();
        Ok(Self {
            id,
            modes,
            pairs,
            loss_amplitude: 10f64.powf(-loss_db / 20.0),
            state: Mutex::new(MeshState {
                theta: vec![0.0; n],
                phi: vec![0.0; n],
                input: vec![C64::ZERO; modes],
            }),
        })
    }

    pub fn modes(&self) -> usize {
        self.modes
    }

    pub fn mzi_count(&self) -> usize {
        self.pairs.len()
    }

    /// Full N×N field transfer matrix, row-major (out × in).
    pub fn transfer_matrix(&self) -> Vec<C64> {
        let state = self.state.lock().expect("mesh lock");
        let n = self.modes;
        let mut u = vec![C64::ZERO; n * n];
        for i in 0..n {
            u[i * n + i] = C64::new(1.0, 0.0);
        }
        for (k, &top) in self.pairs.iter().enumerate() {
            let (c, s) = ((state.theta[k] / 2.0).cos(), (state.theta[k] / 2.0).sin());
            let e = C64::polar(1.0, state.phi[k]);
            let a = self.loss_amplitude;
            let t00 = e.times(C64::new(a * c, 0.0));
            let t01 = C64::new(-a * s, 0.0);
            let t10 = e.times(C64::new(a * s, 0.0));
            let t11 = C64::new(a * c, 0.0);
            for column in 0..n {
                let x = u[top * n + column];
                let y = u[(top + 1) * n + column];
                u[top * n + column] = t00.times(x).plus(t01.times(y));
                u[(top + 1) * n + column] = t10.times(x).plus(t11.times(y));
            }
        }
        u
    }

    pub fn output_fields(&self) -> Vec<C64> {
        let u = self.transfer_matrix();
        let input = self.state.lock().expect("mesh lock").input.clone();
        (0..self.modes)
            .map(|row| {
                (0..self.modes).fold(C64::ZERO, |acc, column| {
                    acc.plus(u[row * self.modes + column].times(input[column]))
                })
            })
            .collect()
    }
}

impl Device for MziMeshDriver {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability {
            name: "mzi_mesh".into(),
            channel: ChannelType::Optical,
            metadata: Some(HashMap::from([
                ("modes".to_string(), self.modes.to_string()),
                ("mzis".to_string(), self.pairs.len().to_string()),
                ("driver".to_string(), "reference_simulator".to_string()),
            ])),
        }]
    }

    fn set_param(&self, name: &str, value: f64) -> Result<(), String> {
        if !value.is_finite() {
            return Err(format!("{name}: value must be finite"));
        }
        let mut state = self.state.lock().map_err(|_| "mesh lock poisoned")?;
        if let Some((k, field)) = parse_indexed(name, "mzi_") {
            if k >= self.pairs.len() {
                return Err(format!("{name}: no such MZI"));
            }
            match field {
                "theta" => state.theta[k] = value,
                "phi" => state.phi[k] = value,
                _ => return Err(format!("{name}: unknown MZI parameter")),
            }
            return Ok(());
        }
        if let Some((j, field)) = parse_indexed(name, "in_") {
            if j >= self.modes {
                return Err(format!("{name}: no such input mode"));
            }
            match field {
                "re" => state.input[j].re = value,
                "im" => state.input[j].im = value,
                _ => return Err(format!("{name}: unknown input field")),
            }
            return Ok(());
        }
        Err(format!("{name}: unknown parameter"))
    }

    fn read_sensor(&self, name: &str) -> Result<f64, String> {
        match parse_indexed(name, "out_") {
            Some((j, "power")) if j < self.modes => Ok(self.output_fields()[j].norm_sqr()),
            _ => Err(format!("{name}: unknown sensor")),
        }
    }
}

/// All-pass microring bank. Per ring `ring_<i>:phase` sets the round-trip
/// phase detuning φ; `ring_<i>:transmission` reads the through-port power
/// transmission `(a² − 2ar cos φ + r²) / (1 − 2ar cos φ + (ar)²)`.
pub struct MicroringBankDriver {
    id: String,
    r: f64,
    a: f64,
    phase: Mutex<Vec<f64>>,
}

impl MicroringBankDriver {
    pub fn from_component(component: &Value) -> Result<Self> {
        let id = expect_class(component, "microring_bank")?;
        if component.get("configuration").and_then(Value::as_str) != Some("all_pass") {
            bail!("the reference driver implements all-pass rings only");
        }
        let rings = count(component, "rings")?;
        Ok(Self {
            id,
            r: number(component, "self_coupling_r")?,
            a: number(component, "round_trip_amplitude_a")?,
            phase: Mutex::new(vec![0.0; rings]),
        })
    }

    pub fn transmission(&self, phase: f64) -> f64 {
        let (a, r, c) = (self.a, self.r, phase.cos());
        (a * a - 2.0 * a * r * c + r * r) / (1.0 - 2.0 * a * r * c + (a * r) * (a * r))
    }
}

impl Device for MicroringBankDriver {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn capabilities(&self) -> Vec<Capability> {
        let rings = self.phase.lock().map(|p| p.len()).unwrap_or(0);
        vec![Capability {
            name: "microring_bank".into(),
            channel: ChannelType::Optical,
            metadata: Some(HashMap::from([
                ("rings".to_string(), rings.to_string()),
                ("driver".to_string(), "reference_simulator".to_string()),
            ])),
        }]
    }

    fn set_param(&self, name: &str, value: f64) -> Result<(), String> {
        let mut phase = self.phase.lock().map_err(|_| "ring lock poisoned")?;
        match parse_indexed(name, "ring_") {
            Some((i, "phase")) if i < phase.len() && value.is_finite() => {
                phase[i] = value;
                Ok(())
            }
            _ => Err(format!("{name}: unknown parameter or invalid value")),
        }
    }

    fn read_sensor(&self, name: &str) -> Result<f64, String> {
        let phase = self.phase.lock().map_err(|_| "ring lock poisoned")?;
        match parse_indexed(name, "ring_") {
            Some((i, "transmission")) if i < phase.len() => Ok(self.transmission(phase[i])),
            _ => Err(format!("{name}: unknown sensor")),
        }
    }
}

/// Photodetector + ADC readout chain: `I = R·P + I_dark`, `V = I·R_tia`,
/// code = clamp(round(V / V_fs · (2^bits − 1))). Noise-free by design; noise
/// models live in the simulator. Parameters: `pd_<i>:power_w`, `tia:ohm`;
/// sensors: `pd_<i>:current_a`, `pd_<i>:code`.
pub struct DetectorAdcDriver {
    id: String,
    responsivity: f64,
    dark_current_a: f64,
    bits: u32,
    full_scale_v: f64,
    state: Mutex<(Vec<f64>, f64)>,
}

impl DetectorAdcDriver {
    pub fn from_components(detector: &Value, adc: &Value) -> Result<Self> {
        let id = expect_class(detector, "detector")?;
        expect_class(adc, "adc")?;
        let bits = count(adc, "bits")? as u32;
        if !(1..=24).contains(&bits) {
            bail!("ADC bits must be in 1..=24 for the reference driver");
        }
        Ok(Self {
            id,
            responsivity: number(detector, "responsivity_a_per_w")?,
            dark_current_a: number(detector, "dark_current_na")? * 1e-9,
            bits,
            full_scale_v: number(adc, "full_scale_v")?,
            state: Mutex::new((vec![0.0; count(detector, "count")?], 1000.0)),
        })
    }

    pub fn max_code(&self) -> f64 {
        ((1u64 << self.bits) - 1) as f64
    }
}

impl Device for DetectorAdcDriver {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability {
            name: "detector_adc".into(),
            channel: ChannelType::Electrical,
            metadata: Some(HashMap::from([
                ("adc_bits".to_string(), self.bits.to_string()),
                ("driver".to_string(), "reference_simulator".to_string()),
            ])),
        }]
    }

    fn set_param(&self, name: &str, value: f64) -> Result<(), String> {
        if !(value.is_finite() && value >= 0.0) {
            return Err(format!("{name}: value must be finite and non-negative"));
        }
        let mut state = self.state.lock().map_err(|_| "detector lock poisoned")?;
        if name == "tia:ohm" {
            state.1 = value;
            return Ok(());
        }
        match parse_indexed(name, "pd_") {
            Some((i, "power_w")) if i < state.0.len() => {
                state.0[i] = value;
                Ok(())
            }
            _ => Err(format!("{name}: unknown parameter")),
        }
    }

    fn read_sensor(&self, name: &str) -> Result<f64, String> {
        let state = self.state.lock().map_err(|_| "detector lock poisoned")?;
        let Some((i, field)) = parse_indexed(name, "pd_") else {
            return Err(format!("{name}: unknown sensor"));
        };
        if i >= state.0.len() {
            return Err(format!("{name}: no such detector"));
        }
        let current = self.responsivity * state.0[i] + self.dark_current_a;
        match field {
            "current_a" => Ok(current),
            "code" => {
                let volts = current * state.1;
                Ok((volts / self.full_scale_v * self.max_code())
                    .round()
                    .clamp(0.0, self.max_code()))
            }
            _ => Err(format!("{name}: unknown sensor")),
        }
    }
}
