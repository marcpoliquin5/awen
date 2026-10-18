//! Loader for `awen-spec/hardware_assumptions.yaml` (Master Prompt v2, section 9).
//!
//! Parses the versioned assumption set (bundled at build time, or from a path), checks its version,
//! and returns each low / typical / high range parameter with its unit. Nothing reads it by default:
//! the noise model and cost estimates keep their built-in values until a caller opts in.
//! A null value means the set has no checked source; it is returned as `None`, never filled in.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use yaml_rust2::{Yaml, YamlLoader};

/// Assumption-set version this loader understands.
pub const HARDWARE_ASSUMPTIONS_VERSION: &str = "awen.hardware-assumptions.v0";
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
