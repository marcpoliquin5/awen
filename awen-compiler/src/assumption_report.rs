//! Opt-in use of the hardware assumption set in the noise model and cost estimates
//! (Master Prompt v2, section 9).
//!
//! `PhotonicNoiseContract::default()` and `CostModelInputs::from_capabilities` are unchanged; only
//! [`PhotonicNoiseContract::with_assumptions`] and [`CostModelInputs::with_assumptions`] apply the
//! file. A field is overwritten only when the set stores a value for it at the chosen [`Corner`];
//! a null keeps the built-in value, and the returned [`AssumptionReport`] lists both cases.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

use crate::assumptions::{Corner, HardwareAssumptions, SourceType};
use crate::cost::{CostModelInputs, ParameterProvenance, ParameterSource};
use crate::noise::PhotonicNoiseContract;

/// Report schema version.
pub const ASSUMPTION_REPORT_VERSION: &str = "awen.assumption-report.v0";

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
    /// Citation key and source type of the applied value; `None` when the built-in value was kept
    /// (built-in values are listed in `awen-spec/hardcoded_device_defaults.yaml`).
    pub source: Option<String>,
    pub source_type: Option<SourceType>,
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

    /// Provenance class of the applied fields: the weakest of their source types (an unlabeled
    /// value or an estimate counts as `Assumed`), and `Assumed` when nothing was applied.
    pub fn provenance_source(&self) -> ParameterSource {
        let rank = |source: Option<SourceType>| match source {
            Some(SourceType::Measured) => (0, ParameterSource::Measured),
            Some(SourceType::Vendor) => (1, ParameterSource::VendorSpecified),
            Some(SourceType::Estimate) | None => (2, ParameterSource::Assumed),
        };
        self.applied()
            .map(|field| rank(field.source_type))
            .max_by_key(|(rank, _)| *rank)
            .map_or(ParameterSource::Assumed, |(_, source)| source)
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
        let label = found.and(assumptions.parameters.get(name));
        self.fields.push(AssumptionField {
            field: field.to_string(),
            assumption: name.to_string(),
            unit: unit.to_string(),
            value,
            from_assumptions: found.is_some(),
            source: label.and_then(|parameter| parameter.source.clone()),
            source_type: label.and_then(|parameter| parameter.source_type),
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
    /// set has a value, plus a provenance entry naming the assumption-set version and corner, whose
    /// source is [`AssumptionReport::provenance_source`] of the applied fields.
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
            source: report.provenance_source(),
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
