//! Loader for `awen-spec/hardware_assumptions.yaml` (Master Prompt v2, section 9).
//!
//! The noise model and cost estimates can read device values from the versioned assumption set
//! instead of their built-in defaults. This is **opt-in**: `PhotonicNoiseContract::default()` and
//! `CostModelInputs::from_capabilities` are unchanged, and only
//! [`PhotonicNoiseContract::with_assumptions`] and [`CostModelInputs::with_assumptions`] apply
//! the file. A field is overwritten only when the assumption set stores a value for it at the
//! chosen [`Corner`]; a null (no checked source) keeps the built-in value, and the returned
//! [`AssumptionReport`] lists both cases so no value is applied silently.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use yaml_rust2::{Yaml, YamlLoader};

use crate::cost::{CostModelInputs, ParameterProvenance, ParameterSource};
use crate::noise::PhotonicNoiseContract;

/// Assumption-set version this loader understands.
pub const HARDWARE_ASSUMPTIONS_VERSION: &str = "awen.hardware-assumptions.v0";
/// Report schema version.
pub const ASSUMPTION_REPORT_VERSION: &str = "awen.assumption-report.v0";
/// The assumption file at the commit this crate was built from.
pub const BUNDLED_HARDWARE_ASSUMPTIONS: &str =
    include_str!("../../awen-spec/hardware_assumptions.yaml");

/// Which value of a low / typical / high range to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    Low,
    Typical,
    High,
}

impl Corner {
    pub const ALL: [Corner; 3] = [Corner::Low, Corner::Typical, Corner::High];

    fn key(self) -> &'static str {
        match self {
            Corner::Low => "low",
            Corner::Typical => "typical",
            Corner::High => "high",
        }
    }
}

/// One range parameter (`kind: sweep`, `by_device_length_mm` and `points` entries are kept out:
/// they are not low / typical / high ranges).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RangeParameter {
    pub unit: String,
    pub low: Option<f64>,
    pub typical: Option<f64>,
    pub high: Option<f64>,
    pub source: Option<String>,
}

impl RangeParameter {
    pub fn at(&self, corner: Corner) -> Option<f64> {
        match corner {
            Corner::Low => self.low,
            Corner::Typical => self.typical,
            Corner::High => self.high,
        }
    }
}

/// The parsed assumption set: version, date and every range parameter by dotted name
/// (for example `dac.weight_dac_bits`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareAssumptions {
    pub version: String,
    pub date: String,
    pub parameters: BTreeMap<String, RangeParameter>,
}

fn number(node: &Yaml) -> Option<f64> {
    match node {
        Yaml::Real(text) => text.parse().ok(),
        Yaml::Integer(value) => Some(*value as f64),
        _ => None,
    }
}

fn text(node: &Yaml) -> Option<String> {
    match node {
        Yaml::String(value) => Some(value.clone()),
        Yaml::Real(value) => Some(value.clone()),
        Yaml::Integer(value) => Some(value.to_string()),
        _ => None,
    }
}

fn corner_value(entry: &Yaml, corner: Corner, name: &str) -> Result<Option<f64>> {
    let node = &entry[corner.key()];
    if node.is_null() {
        return Ok(None);
    }
    number(node)
        .map(Some)
        .ok_or_else(|| anyhow!("{name}.{} is not a number or null", corner.key()))
}

impl HardwareAssumptions {
    /// Parse the YAML text of an assumption set.
    pub fn parse(source: &str) -> Result<Self> {
        let docs =
            YamlLoader::load_from_str(source).context("hardware assumptions are not valid YAML")?;
        let doc = docs
            .first()
            .ok_or_else(|| anyhow!("hardware assumptions file is empty"))?;
        let version = text(&doc["version"]).ok_or_else(|| anyhow!("missing version"))?;
        if version != HARDWARE_ASSUMPTIONS_VERSION {
            bail!("unsupported hardware assumptions version '{version}'; expected '{HARDWARE_ASSUMPTIONS_VERSION}'");
        }
        let date = text(&doc["date"]).ok_or_else(|| anyhow!("missing date"))?;
        let Yaml::Hash(groups) = &doc["parameters"] else {
            bail!("missing parameters mapping");
        };
        let mut parameters = BTreeMap::new();
        for (group, entries) in groups {
            let group = text(group).ok_or_else(|| anyhow!("parameter group name is not text"))?;
            let Yaml::Hash(entries) = entries else {
                bail!("parameter group {group} is not a mapping");
            };
            for (name, entry) in entries {
                let name = format!(
                    "{group}.{}",
                    text(name).ok_or_else(|| anyhow!("bad name in {group}"))?
                );
                if !matches!(entry, Yaml::Hash(_)) || !entry["kind"].is_badvalue() {
                    continue; // a rule, a null count, or a sweep / keyed / points entry: not a range
                }
                let unit = text(&entry["unit"]).ok_or_else(|| anyhow!("{name} has no unit"))?;
                parameters.insert(
                    name.clone(),
                    RangeParameter {
                        unit,
                        low: corner_value(entry, Corner::Low, &name)?,
                        typical: corner_value(entry, Corner::Typical, &name)?,
                        high: corner_value(entry, Corner::High, &name)?,
                        source: text(&entry["source"]),
                    },
                );
            }
        }
        Ok(Self {
            version,
            date,
            parameters,
        })
    }

    /// The assumption set bundled at build time (`awen-spec/hardware_assumptions.yaml`).
    pub fn bundled() -> Result<Self> {
        Self::parse(BUNDLED_HARDWARE_ASSUMPTIONS)
    }

    /// Read an assumption set from a file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        Self::parse(&source)
    }

    /// The value of a range parameter at a corner, after checking its unit.
    pub fn value(&self, name: &str, unit: &str, corner: Corner) -> Result<Option<f64>> {
        let parameter = self
            .parameters
            .get(name)
            .ok_or_else(|| anyhow!("{name} is not a range parameter in {}", self.version))?;
        if parameter.unit != unit {
            bail!("{name} is in {}, expected {unit}", parameter.unit);
        }
        Ok(parameter.at(corner))
    }
}

/// Where one field's value came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssumptionField {
    pub field: String,
    pub assumption: String,
    pub unit: String,
    /// The value now in the field.
    pub value: f64,
    /// True when the assumption set supplied the value; false when it is null at this corner
    /// and the built-in value was kept.
    pub from_assumptions: bool,
}

/// Which fields an assumption set changed, for one corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssumptionReport {
    pub version: String,
    pub assumption_set: String,
    pub assumption_date: String,
    pub corner: Corner,
    pub fields: Vec<AssumptionField>,
}

impl AssumptionReport {
    fn new(assumptions: &HardwareAssumptions, corner: Corner) -> Self {
        Self {
            version: ASSUMPTION_REPORT_VERSION.to_string(),
            assumption_set: assumptions.version.clone(),
            assumption_date: assumptions.date.clone(),
            corner,
            fields: Vec::new(),
        }
    }

    /// Fields whose value came from the assumption set.
    pub fn applied(&self) -> impl Iterator<Item = &AssumptionField> {
        self.fields.iter().filter(|field| field.from_assumptions)
    }

    /// Look up `name` (in `unit`), convert it with `scale` into the field's unit, and record it.
    fn resolve(
        &mut self,
        assumptions: &HardwareAssumptions,
        field: &str,
        name: &str,
        unit: &str,
        scale: f64,
        current: f64,
    ) -> Result<f64> {
        let found = assumptions
            .value(name, unit, self.corner)?
            .map(|value| value * scale);
        let value = found.unwrap_or(current);
        self.fields.push(AssumptionField {
            field: field.to_string(),
            assumption: name.to_string(),
            unit: unit.to_string(),
            value,
            from_assumptions: found.is_some(),
        });
        Ok(value)
    }
}

fn whole_bits(value: f64, field: &str) -> Result<u32> {
    if value.fract() != 0.0 || value < 1.0 || value > f64::from(u32::MAX) {
        bail!("{field} must be a positive whole number of bits, got {value}");
    }
    Ok(value as u32)
}

impl PhotonicNoiseContract {
    /// A copy of this contract with every mapped field taken from `assumptions` at `corner`
    /// where the set has a value. Mapped fields: `dac_bits` (`dac.weight_dac_bits`),
    /// `rin_db_per_hz` (`laser.rin`), `detection_bandwidth_hz` (`photodetector.bandwidth`, GHz),
    /// `crosstalk_db` (`mzi.crosstalk`). A field that is `None` (switched off) stays off.
    pub fn with_assumptions(
        &self,
        assumptions: &HardwareAssumptions,
        corner: Corner,
    ) -> Result<(Self, AssumptionReport)> {
        let mut report = AssumptionReport::new(assumptions, corner);
        let mut contract = self.clone();
        if let Some(bits) = contract.dac_bits {
            let value = report.resolve(
                assumptions,
                "dac_bits",
                "dac.weight_dac_bits",
                "bits",
                1.0,
                f64::from(bits),
            )?;
            contract.dac_bits = Some(whole_bits(value, "dac_bits")?);
        }
        if let Some(rin) = contract.rin_db_per_hz {
            contract.rin_db_per_hz = Some(report.resolve(
                assumptions,
                "rin_db_per_hz",
                "laser.rin",
                "dB/Hz",
                1.0,
                rin,
            )?);
        }
        contract.detection_bandwidth_hz = report.resolve(
            assumptions,
            "detection_bandwidth_hz",
            "photodetector.bandwidth",
            "GHz",
            1.0e9,
            contract.detection_bandwidth_hz,
        )?;
        if let Some(crosstalk) = contract.crosstalk_db {
            contract.crosstalk_db = Some(report.resolve(
                assumptions,
                "crosstalk_db",
                "mzi.crosstalk",
                "dB",
                1.0,
                crosstalk,
            )?);
        }
        contract.validate()?;
        Ok((contract, report))
    }
}

impl CostModelInputs {
    /// This cost model with every mapped field taken from `assumptions` at `corner` where the
    /// set has a value, plus a provenance entry naming the assumption-set version and corner.
    /// Mapped fields: `modulator_energy_pj_per_sample` (`modulator.energy_per_symbol`, fJ).
    /// The added provenance entry reuses the largest existing uncertainty, so the estimate's
    /// uncertainty is unchanged.
    pub fn with_assumptions(
        mut self,
        assumptions: &HardwareAssumptions,
        corner: Corner,
    ) -> Result<(Self, AssumptionReport)> {
        let mut report = AssumptionReport::new(assumptions, corner);
        self.modulator_energy_pj_per_sample = report.resolve(
            assumptions,
            "modulator_energy_pj_per_sample",
            "modulator.energy_per_symbol",
            "fJ",
            1.0e-3,
            self.modulator_energy_pj_per_sample,
        )?;
        let largest = self
            .provenance
            .iter()
            .map(|entry| entry.uncertainty_fraction)
            .reduce(f64::max)
            .ok_or_else(|| anyhow!("cost model has no provenance entries"))?;
        let applied: Vec<&str> = report.applied().map(|field| field.field.as_str()).collect();
        self.provenance.push(ParameterProvenance {
            parameter: "hardware assumption set".to_string(),
            source: ParameterSource::Assumed,
            reference: format!(
                "{} ({}, {:?} corner); applied: [{}]",
                assumptions.version,
                assumptions.date,
                corner,
                applied.join(", ")
            ),
            uncertainty_fraction: largest,
        });
        self.validate()?;
        Ok((self, report))
    }
}
