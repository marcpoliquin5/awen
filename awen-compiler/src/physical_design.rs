//! Immutable boundary contracts for external photonic physical-design tooling.
//!
//! AWEN owns logical mapping constraints and compilation decisions. PDKs,
//! components, layout, DRC/LVS, electromagnetic simulation, and circuit models
//! remain owned by mature external tools. These types intentionally cannot
//! represent polygons, GDS payloads, rule decks, or foundry source data.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

pub const PHYSICAL_DESIGN_VERSION: &str = "awen.physical-design.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    OpenReference,
    ProprietaryReference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReference {
    pub artifact_id: String,
    pub digest: String,
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PdkReference {
    pub name: String,
    pub version: String,
    pub manifest: ArtifactReference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProcessCorner {
    pub corner_id: String,
    pub fingerprint: String,
    pub temperature_c: f64,
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolIdentity {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    Gdsfactory,
    CircuitSimulator,
    ElectromagneticSimulator,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Connectivity,
    Drc,
    Lvs,
    CircuitSimulation,
    ElectromagneticSimulation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PhysicalDesignAdapter {
    pub kind: AdapterKind,
    pub tool: ToolIdentity,
    pub request_version: String,
    pub response_version: String,
    pub supported_evidence: Vec<EvidenceKind>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnit {
    Nanometer,
    Micrometer,
    Meter,
}

impl LengthUnit {
    pub fn to_micrometers(self, value: f64) -> f64 {
        match self {
            Self::Nanometer => value / 1_000.0,
            Self::Micrometer => value,
            Self::Meter => value * 1_000_000.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PortKind {
    Optical,
    Electrical,
    Placement,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WavelengthBand {
    pub minimum_nm: f64,
    pub maximum_nm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PortContract {
    pub name: String,
    pub kind: PortKind,
    pub center: [f64; 2],
    pub orientation_degrees: f64,
    pub width: f64,
    pub unit: LengthUnit,
    pub layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wavelength: Option<WavelengthBand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TopologyNode {
    pub instance_id: String,
    pub component: String,
    pub ports: Vec<PortContract>,
    #[serde(default)]
    pub settings: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct TopologyEndpoint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    pub port_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TopologyConnection {
    pub source: TopologyEndpoint,
    pub destination: TopologyEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TopologyContract {
    pub name: String,
    pub external_ports: Vec<PortContract>,
    pub nodes: Vec<TopologyNode>,
    pub connections: Vec<TopologyConnection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LayoutConstraints {
    pub unit: LengthUnit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_bend_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_path_length_imbalance: Option<f64>,
    pub maximum_crossings: usize,
    pub allowed_layers: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CircuitFramework {
    Circulax,
    Sax,
    Touchstone,
    Analytic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CircuitModelReference {
    pub name: String,
    pub framework: CircuitFramework,
    pub artifact: ArtifactReference,
    pub ports: Vec<String>,
    pub wavelength: WavelengthBand,
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerificationEvidence {
    pub kind: EvidenceKind,
    pub status: EvidenceStatus,
    pub tool: ToolIdentity,
    pub settings_fingerprint: String,
    pub report: ArtifactReference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PhysicalDesignBinding {
    pub contract_version: String,
    pub classification: DataClassification,
    pub pdk: PdkReference,
    pub process_corner: ProcessCorner,
    pub component_library: ArtifactReference,
    pub topology_artifact: ArtifactReference,
    pub topology: TopologyContract,
    pub layout_constraints: LayoutConstraints,
    pub circuit_models: Vec<CircuitModelReference>,
    pub adapters: Vec<PhysicalDesignAdapter>,
    pub verification: Vec<VerificationEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PhysicalDesignProvenance {
    pub contract_version: String,
    pub binding_fingerprint: String,
    pub classification: DataClassification,
    pub pdk_name: String,
    pub pdk_version: String,
    pub pdk_manifest: ArtifactReference,
    pub process_corner_id: String,
    pub process_corner_fingerprint: String,
    pub component_library: ArtifactReference,
    pub topology_artifact: ArtifactReference,
    pub circuit_model_artifacts: Vec<ArtifactReference>,
    pub verification_reports: Vec<ArtifactReference>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogicalOperationKind {
    MatrixMultiply,
    PhaseShift,
    Split,
    Combine,
    Detect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LogicalOperation {
    pub operation_id: String,
    pub kind: LogicalOperationKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MappingRequest {
    pub contract_version: String,
    pub request_id: String,
    pub logical_operations: Vec<LogicalOperation>,
    pub required_ports: Vec<PortContract>,
    pub constraints: LayoutConstraints,
    pub candidate_topologies: Vec<TopologyContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MappingResponse {
    pub contract_version: String,
    pub request_id: String,
    pub adapter: PhysicalDesignAdapter,
    pub binding: PhysicalDesignBinding,
}

impl ArtifactReference {
    fn validate(&self, field: &str) -> Result<()> {
        non_empty(&self.artifact_id, &format!("{field} artifact_id"))?;
        if !(self.artifact_id.starts_with("urn:") || self.artifact_id.starts_with("sha256:")) {
            bail!("{field} artifact_id must be an immutable urn or sha256 identity");
        }
        if self.artifact_id == "urn:" {
            bail!("{field} artifact_id urn must include an opaque identity");
        }
        if self.artifact_id.starts_with("sha256:") {
            sha256(&self.artifact_id, &format!("{field} artifact_id"))?;
        }
        sha256(&self.digest, &format!("{field} digest"))?;
        non_empty(&self.media_type, &format!("{field} media_type"))?;
        if self.uri.as_ref().is_some_and(|uri| {
            !((uri.starts_with("https://") && uri.len() > 8)
                || (uri.starts_with("urn:") && uri.len() > 4))
        }) {
            bail!("{field} uri must use https or urn");
        }
        Ok(())
    }

    fn redacted(&self) -> Self {
        let mut reference = self.clone();
        reference.uri = None;
        reference
    }
}

impl PortContract {
    fn validate(&self, field: &str) -> Result<()> {
        non_empty(&self.name, &format!("{field} name"))?;
        non_empty(&self.layer, &format!("{field} layer"))?;
        finite(self.center[0], &format!("{field} center x"))?;
        finite(self.center[1], &format!("{field} center y"))?;
        finite(self.orientation_degrees, &format!("{field} orientation"))?;
        if !(0.0..360.0).contains(&self.orientation_degrees) {
            bail!("{field} orientation must be in [0, 360)");
        }
        positive(self.width, &format!("{field} width"))?;
        if let Some(wavelength) = &self.wavelength {
            wavelength.validate(field)?;
        }
        if self.kind == PortKind::Optical && self.wavelength.is_none() {
            bail!("{field} optical port requires a wavelength band");
        }
        if self
            .mode
            .as_ref()
            .is_some_and(|mode| mode.trim().is_empty())
        {
            bail!("{field} mode must not be empty");
        }
        Ok(())
    }
}

impl WavelengthBand {
    fn validate(&self, field: &str) -> Result<()> {
        positive(self.minimum_nm, &format!("{field} minimum wavelength"))?;
        positive(self.maximum_nm, &format!("{field} maximum wavelength"))?;
        if self.minimum_nm > self.maximum_nm {
            bail!("{field} wavelength minimum must not exceed maximum");
        }
        Ok(())
    }
}

impl TopologyContract {
    pub fn validate(&self) -> Result<()> {
        non_empty(&self.name, "topology name")?;
        unique_ports(&self.external_ports, "external topology ports")?;
        let mut node_ids = HashSet::new();
        for node in &self.nodes {
            if !node_ids.insert(node.instance_id.as_str()) || node.instance_id.trim().is_empty() {
                bail!("topology node ids must be non-empty and unique");
            }
            non_empty(&node.component, "topology node component")?;
            unique_ports(&node.ports, &format!("node '{}' ports", node.instance_id))?;
            validate_parameters(
                &node.settings,
                &format!("node '{}' settings", node.instance_id),
            )?;
        }
        let mut connections = HashSet::new();
        for connection in &self.connections {
            validate_endpoint(self, &connection.source)?;
            validate_endpoint(self, &connection.destination)?;
            if connection.source == connection.destination {
                bail!("topology connection cannot connect a port to itself");
            }
            if !connections.insert((connection.source.clone(), connection.destination.clone())) {
                bail!("topology connections must be unique");
            }
        }
        Ok(())
    }
}

impl LayoutConstraints {
    fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("maximum_width", self.maximum_width),
            ("maximum_height", self.maximum_height),
            ("minimum_bend_radius", self.minimum_bend_radius),
            (
                "maximum_path_length_imbalance",
                self.maximum_path_length_imbalance,
            ),
        ] {
            if let Some(value) = value {
                positive(value, name)?;
            }
        }
        unique_non_empty(&self.allowed_layers, "allowed layers")
    }
}

impl PhysicalDesignAdapter {
    pub fn validate(&self) -> Result<()> {
        non_empty(&self.tool.name, "adapter tool name")?;
        non_empty(&self.tool.version, "adapter tool version")?;
        version(&self.request_version, "adapter request version")?;
        version(&self.response_version, "adapter response version")?;
        let mut evidence = HashSet::new();
        if self
            .supported_evidence
            .iter()
            .any(|kind| !evidence.insert(*kind))
        {
            bail!("adapter supported evidence kinds must be unique");
        }
        Ok(())
    }
}

impl PhysicalDesignBinding {
    pub fn validate(&self) -> Result<()> {
        version(&self.contract_version, "physical-design contract")?;
        non_empty(&self.pdk.name, "PDK name")?;
        non_empty(&self.pdk.version, "PDK version")?;
        self.pdk.manifest.validate("PDK manifest")?;
        non_empty(&self.process_corner.corner_id, "process corner id")?;
        sha256(
            &self.process_corner.fingerprint,
            "process corner fingerprint",
        )?;
        finite(
            self.process_corner.temperature_c,
            "process corner temperature",
        )?;
        validate_parameters(&self.process_corner.parameters, "process corner parameters")?;
        self.component_library.validate("component library")?;
        self.topology_artifact.validate("topology artifact")?;
        self.topology.validate()?;
        self.layout_constraints.validate()?;
        if self.topology_artifact.digest != sha256_json(&self.topology)? {
            bail!("topology artifact digest does not match the imported topology contract");
        }
        if self.circuit_models.is_empty() {
            bail!("physical-design binding requires at least one circuit model");
        }
        let mut model_names = HashSet::new();
        for model in &self.circuit_models {
            if !model_names.insert(model.name.as_str()) || model.name.trim().is_empty() {
                bail!("circuit model names must be non-empty and unique");
            }
            model.artifact.validate("circuit model artifact")?;
            unique_non_empty(&model.ports, "circuit model ports")?;
            model.wavelength.validate("circuit model")?;
            validate_parameters(&model.parameters, "circuit model parameters")?;
        }
        if self.adapters.is_empty() {
            bail!("physical-design binding requires at least one adapter");
        }
        let mut adapter_kinds = HashSet::new();
        for adapter in &self.adapters {
            adapter.validate()?;
            if !adapter_kinds.insert(adapter.kind) {
                bail!("physical-design adapter kinds must be unique");
            }
        }
        if !adapter_kinds.contains(&AdapterKind::Gdsfactory) {
            bail!("physical-design binding requires a gdsfactory adapter");
        }
        if self.verification.is_empty() {
            bail!("verified physical-design binding requires verification evidence");
        }
        let mut has_connectivity = false;
        for evidence in &self.verification {
            non_empty(&evidence.tool.name, "verification tool name")?;
            non_empty(&evidence.tool.version, "verification tool version")?;
            sha256(
                &evidence.settings_fingerprint,
                "verification settings fingerprint",
            )?;
            evidence.report.validate("verification report")?;
            if evidence.status != EvidenceStatus::Passed {
                bail!("verified physical-design binding cannot contain failed evidence");
            }
            has_connectivity |= evidence.kind == EvidenceKind::Connectivity;
            if !self
                .adapters
                .iter()
                .any(|adapter| adapter.supported_evidence.contains(&evidence.kind))
            {
                bail!("verification evidence kind is not supported by an imported adapter");
            }
        }
        if !has_connectivity {
            bail!("verified physical-design binding requires passed connectivity evidence");
        }
        if self
            .circuit_models
            .iter()
            .any(|model| model.framework == CircuitFramework::Circulax)
            && !adapter_kinds.contains(&AdapterKind::CircuitSimulator)
        {
            bail!("Circulax models require a circuit-simulator adapter boundary");
        }
        if self.classification == DataClassification::ProprietaryReference {
            self.validate_proprietary_boundary()?;
        }
        Ok(())
    }

    pub fn fingerprint(&self) -> Result<String> {
        self.validate()?;
        sha256_json(self)
    }

    pub(crate) fn content_fingerprint(&self) -> String {
        sha256_json(self).expect("physical-design contract structs serialize")
    }

    pub fn provenance(&self) -> Result<PhysicalDesignProvenance> {
        let binding_fingerprint = self.fingerprint()?;
        let redact = self.classification == DataClassification::ProprietaryReference;
        let safe = |reference: &ArtifactReference| {
            if redact {
                reference.redacted()
            } else {
                reference.clone()
            }
        };
        Ok(PhysicalDesignProvenance {
            contract_version: PHYSICAL_DESIGN_VERSION.to_string(),
            binding_fingerprint,
            classification: self.classification,
            pdk_name: self.pdk.name.clone(),
            pdk_version: self.pdk.version.clone(),
            pdk_manifest: safe(&self.pdk.manifest),
            process_corner_id: self.process_corner.corner_id.clone(),
            process_corner_fingerprint: self.process_corner.fingerprint.clone(),
            component_library: safe(&self.component_library),
            topology_artifact: safe(&self.topology_artifact),
            circuit_model_artifacts: self
                .circuit_models
                .iter()
                .map(|model| safe(&model.artifact))
                .collect(),
            verification_reports: self
                .verification
                .iter()
                .map(|evidence| safe(&evidence.report))
                .collect(),
        })
    }

    fn validate_proprietary_boundary(&self) -> Result<()> {
        let references = std::iter::once(&self.pdk.manifest)
            .chain(std::iter::once(&self.component_library))
            .chain(std::iter::once(&self.topology_artifact))
            .chain(self.circuit_models.iter().map(|model| &model.artifact))
            .chain(self.verification.iter().map(|evidence| &evidence.report));
        if references
            .into_iter()
            .any(|reference| reference.uri.is_some())
        {
            bail!("proprietary physical-design references must not expose source URIs");
        }
        if !self.process_corner.parameters.is_empty()
            || self
                .topology
                .nodes
                .iter()
                .any(|node| !node.settings.is_empty())
            || self
                .circuit_models
                .iter()
                .any(|model| !model.parameters.is_empty())
        {
            bail!("proprietary PDK, component, and model parameters must not be embedded");
        }
        if !self.topology.nodes.is_empty() || !self.topology.connections.is_empty() {
            bail!(
                "proprietary topology internals must be represented only by immutable references"
            );
        }
        Ok(())
    }

    pub fn reference_open_pdk() -> Self {
        let optical_port = |name: &str, x: f64, orientation_degrees: f64| PortContract {
            name: name.to_string(),
            kind: PortKind::Optical,
            center: [x, 0.0],
            orientation_degrees,
            width: 0.5,
            unit: LengthUnit::Micrometer,
            layer: "WG".to_string(),
            wavelength: Some(WavelengthBand {
                minimum_nm: 1_530.0,
                maximum_nm: 1_565.0,
            }),
            mode: Some("te0".to_string()),
        };
        let topology = TopologyContract {
            name: "reference_mzi_mesh_cell".to_string(),
            external_ports: vec![
                optical_port("o1", 0.0, 180.0),
                optical_port("o2", 100.0, 0.0),
            ],
            nodes: vec![TopologyNode {
                instance_id: "mzi_0".to_string(),
                component: "mzi".to_string(),
                ports: vec![
                    optical_port("o1", 0.0, 180.0),
                    optical_port("o2", 100.0, 0.0),
                ],
                settings: BTreeMap::from([
                    ("coupling".to_string(), 0.5),
                    ("delta_length_um".to_string(), 10.0),
                ]),
            }],
            connections: vec![
                TopologyConnection {
                    source: TopologyEndpoint {
                        instance_id: None,
                        port_name: "o1".to_string(),
                    },
                    destination: TopologyEndpoint {
                        instance_id: Some("mzi_0".to_string()),
                        port_name: "o1".to_string(),
                    },
                },
                TopologyConnection {
                    source: TopologyEndpoint {
                        instance_id: Some("mzi_0".to_string()),
                        port_name: "o2".to_string(),
                    },
                    destination: TopologyEndpoint {
                        instance_id: None,
                        port_name: "o2".to_string(),
                    },
                },
            ],
        };
        let topology_digest = sha256_json(&topology).expect("reference topology serializes");
        let artifact =
            |artifact_id: &str, seed: &str, media_type: &str, uri: &str| ArtifactReference {
                artifact_id: artifact_id.to_string(),
                digest: sha256_bytes(seed.as_bytes()),
                media_type: media_type.to_string(),
                uri: Some(uri.to_string()),
            };
        Self {
            contract_version: PHYSICAL_DESIGN_VERSION.to_string(),
            classification: DataClassification::OpenReference,
            pdk: PdkReference {
                name: "awen-example-silicon".to_string(),
                version: "1.0.0".to_string(),
                manifest: artifact(
                    "urn:awen:pdk:example-silicon:1.0.0",
                    "awen-example-silicon-pdk-1.0.0",
                    "application/vnd.awen.pdk-manifest+json",
                    "https://github.com/marcpoliquin5/awen/tree/main/awen-ecosystem/pdks",
                ),
            },
            process_corner: ProcessCorner {
                corner_id: "nominal-22c".to_string(),
                fingerprint: sha256_bytes(b"awen-example-silicon-nominal-22c"),
                temperature_c: 22.0,
                parameters: BTreeMap::from([("waveguide_width_um".to_string(), 0.5)]),
            },
            component_library: artifact(
                "urn:awen:component-library:example-silicon:1.0.0",
                "awen-example-silicon-components-1.0.0",
                "application/vnd.gdsfactory.components+json",
                "https://github.com/marcpoliquin5/awen/tree/main/awen-ecosystem/pdks",
            ),
            topology_artifact: ArtifactReference {
                artifact_id: "urn:awen:topology:reference-mzi-mesh-cell:1.0.0".to_string(),
                digest: topology_digest,
                media_type: "application/vnd.awen.photonic-topology+json".to_string(),
                uri: Some(
                    "https://github.com/marcpoliquin5/awen/tree/main/awen-ecosystem/pdks"
                        .to_string(),
                ),
            },
            topology,
            layout_constraints: LayoutConstraints {
                unit: LengthUnit::Micrometer,
                maximum_width: Some(250.0),
                maximum_height: Some(100.0),
                minimum_bend_radius: Some(5.0),
                maximum_path_length_imbalance: Some(20.0),
                maximum_crossings: 0,
                allowed_layers: vec!["WG".to_string()],
            },
            circuit_models: vec![CircuitModelReference {
                name: "mzi".to_string(),
                framework: CircuitFramework::Circulax,
                artifact: artifact(
                    "urn:awen:circuit-model:mzi:circulax:1.0.0",
                    "awen-example-circulax-mzi-1.0.0",
                    "application/vnd.circulax.model+python",
                    "https://github.com/gdsfactory/circulax",
                ),
                ports: vec!["o1".to_string(), "o2".to_string()],
                wavelength: WavelengthBand {
                    minimum_nm: 1_530.0,
                    maximum_nm: 1_565.0,
                },
                parameters: BTreeMap::from([
                    ("coupling".to_string(), 0.5),
                    ("loss_db".to_string(), 0.2),
                ]),
            }],
            adapters: vec![
                PhysicalDesignAdapter {
                    kind: AdapterKind::Gdsfactory,
                    tool: ToolIdentity {
                        name: "gdsfactory".to_string(),
                        version: "9.48.0".to_string(),
                    },
                    request_version: PHYSICAL_DESIGN_VERSION.to_string(),
                    response_version: PHYSICAL_DESIGN_VERSION.to_string(),
                    supported_evidence: vec![
                        EvidenceKind::Connectivity,
                        EvidenceKind::Drc,
                        EvidenceKind::Lvs,
                    ],
                },
                PhysicalDesignAdapter {
                    kind: AdapterKind::CircuitSimulator,
                    tool: ToolIdentity {
                        name: "circulax".to_string(),
                        version: "0.2.3".to_string(),
                    },
                    request_version: PHYSICAL_DESIGN_VERSION.to_string(),
                    response_version: PHYSICAL_DESIGN_VERSION.to_string(),
                    supported_evidence: vec![EvidenceKind::CircuitSimulation],
                },
            ],
            verification: vec![VerificationEvidence {
                kind: EvidenceKind::Connectivity,
                status: EvidenceStatus::Passed,
                tool: ToolIdentity {
                    name: "gdsfactory".to_string(),
                    version: "9.48.0".to_string(),
                },
                settings_fingerprint: sha256_bytes(b"reference-connectivity-settings-v1"),
                report: artifact(
                    "urn:awen:verification:reference-connectivity:1.0.0",
                    "awen-reference-connectivity-report-1.0.0",
                    "application/vnd.awen.verification-report+json",
                    "https://github.com/marcpoliquin5/awen/tree/main/awen-ecosystem/pdks",
                ),
            }],
        }
    }
}

impl MappingRequest {
    pub fn validate(&self) -> Result<()> {
        version(&self.contract_version, "mapping request")?;
        non_empty(&self.request_id, "mapping request id")?;
        if self.logical_operations.is_empty() {
            bail!("mapping request requires at least one logical operation");
        }
        let mut operation_ids = HashSet::new();
        for operation in &self.logical_operations {
            if !operation_ids.insert(operation.operation_id.as_str())
                || operation.operation_id.trim().is_empty()
            {
                bail!("logical operation ids must be non-empty and unique");
            }
        }
        unique_ports(&self.required_ports, "required mapping ports")?;
        self.constraints.validate()?;
        if self.candidate_topologies.is_empty() {
            bail!("mapping request requires at least one candidate topology");
        }
        for topology in &self.candidate_topologies {
            topology.validate()?;
        }
        Ok(())
    }
}

impl MappingResponse {
    pub fn validate(&self) -> Result<()> {
        version(&self.contract_version, "mapping response")?;
        non_empty(&self.request_id, "mapping response request id")?;
        self.adapter.validate()?;
        self.binding.validate()
    }
}

pub fn import_mapping_response(
    request: &MappingRequest,
    response: MappingResponse,
) -> Result<PhysicalDesignBinding> {
    request.validate()?;
    response.validate()?;
    if request.request_id != response.request_id {
        bail!("mapping response request_id does not match the exported mapping request");
    }
    if response.adapter.kind != AdapterKind::Gdsfactory {
        bail!("mapping response must be produced by a gdsfactory adapter");
    }
    if !response.binding.adapters.contains(&response.adapter) {
        bail!("mapping response adapter is not recorded in the imported binding");
    }
    if !request
        .candidate_topologies
        .iter()
        .any(|candidate| candidate.name == response.binding.topology.name)
    {
        bail!("imported topology was not one of the exported mapping candidates");
    }
    validate_imported_constraints(&request.constraints, &response.binding.layout_constraints)?;
    for required in &request.required_ports {
        let imported = response
            .binding
            .topology
            .external_ports
            .iter()
            .find(|port| port.name == required.name)
            .ok_or_else(|| {
                anyhow::anyhow!("imported topology omits required port '{}'", required.name)
            })?;
        if imported.kind != required.kind {
            bail!("imported port '{}' has an incompatible kind", required.name);
        }
        let required_width_um = required.unit.to_micrometers(required.width);
        let imported_width_um = imported.unit.to_micrometers(imported.width);
        if (required_width_um - imported_width_um).abs() > 1e-9 {
            bail!(
                "imported port '{}' has an incompatible width",
                required.name
            );
        }
    }
    Ok(response.binding)
}

fn validate_imported_constraints(
    requested: &LayoutConstraints,
    imported: &LayoutConstraints,
) -> Result<()> {
    let imported_value_um = |value: f64| imported.unit.to_micrometers(value);
    let requested_value_um = |value: f64| requested.unit.to_micrometers(value);
    for (name, requested_limit, imported_limit) in [
        ("width", requested.maximum_width, imported.maximum_width),
        ("height", requested.maximum_height, imported.maximum_height),
        (
            "path-length imbalance",
            requested.maximum_path_length_imbalance,
            imported.maximum_path_length_imbalance,
        ),
    ] {
        if let Some(limit) = requested_limit {
            let Some(actual) = imported_limit else {
                bail!("imported mapping omits the requested {name} constraint");
            };
            if imported_value_um(actual) > requested_value_um(limit) + 1e-9 {
                bail!("imported mapping exceeds the requested {name} constraint");
            }
        }
    }
    if let Some(minimum) = requested.minimum_bend_radius {
        let Some(actual) = imported.minimum_bend_radius else {
            bail!("imported mapping omits the requested bend-radius constraint");
        };
        if imported_value_um(actual) + 1e-9 < requested_value_um(minimum) {
            bail!("imported mapping violates the requested bend-radius constraint");
        }
    }
    if imported.maximum_crossings > requested.maximum_crossings {
        bail!("imported mapping exceeds the requested crossing constraint");
    }
    if imported
        .allowed_layers
        .iter()
        .any(|layer| !requested.allowed_layers.contains(layer))
    {
        bail!("imported mapping uses a layer outside the exported allowlist");
    }
    Ok(())
}

fn validate_endpoint(topology: &TopologyContract, endpoint: &TopologyEndpoint) -> Result<()> {
    non_empty(&endpoint.port_name, "topology endpoint port")?;
    let ports = match endpoint.instance_id.as_deref() {
        Some(instance_id) => topology
            .nodes
            .iter()
            .find(|node| node.instance_id == instance_id)
            .map(|node| node.ports.as_slice())
            .ok_or_else(|| {
                anyhow::anyhow!("topology endpoint references unknown node '{instance_id}'")
            })?,
        None => topology.external_ports.as_slice(),
    };
    if !ports.iter().any(|port| port.name == endpoint.port_name) {
        bail!(
            "topology endpoint references an unknown port '{}'",
            endpoint.port_name
        );
    }
    Ok(())
}

fn unique_ports(ports: &[PortContract], field: &str) -> Result<()> {
    if ports.is_empty() {
        bail!("{field} must not be empty");
    }
    let mut names = HashSet::new();
    for port in ports {
        port.validate(field)?;
        if !names.insert(port.name.as_str()) {
            bail!("{field} names must be unique");
        }
    }
    Ok(())
}

fn unique_non_empty(values: &[String], field: &str) -> Result<()> {
    if values.is_empty() {
        bail!("{field} must not be empty");
    }
    let mut unique = HashSet::new();
    if values
        .iter()
        .any(|value| value.trim().is_empty() || !unique.insert(value.as_str()))
    {
        bail!("{field} must be non-empty and unique");
    }
    Ok(())
}

fn validate_parameters(parameters: &BTreeMap<String, f64>, field: &str) -> Result<()> {
    for (name, value) in parameters {
        non_empty(name, &format!("{field} name"))?;
        finite(*value, &format!("{field} '{name}'"))?;
    }
    Ok(())
}

fn version(value: &str, field: &str) -> Result<()> {
    if value != PHYSICAL_DESIGN_VERSION {
        bail!("unsupported {field} version '{value}'; expected '{PHYSICAL_DESIGN_VERSION}'");
    }
    Ok(())
}

fn sha256(value: &str, field: &str) -> Result<()> {
    let digest = value.strip_prefix("sha256:").unwrap_or_default();
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("{field} must be a lowercase sha256 digest");
    }
    Ok(())
}

fn sha256_json(value: &impl Serialize) -> Result<String> {
    Ok(sha256_bytes(&serde_json::to_vec(value)?))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn non_empty(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{field} must not be empty");
    }
    Ok(())
}

fn finite(value: f64, field: &str) -> Result<()> {
    if !value.is_finite() {
        bail!("{field} must be finite");
    }
    Ok(())
}

fn positive(value: f64, field: &str) -> Result<()> {
    finite(value, field)?;
    if value <= 0.0 {
        bail!("{field} must be positive");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_binding_is_self_consistent() {
        let binding = PhysicalDesignBinding::reference_open_pdk();
        binding.validate().unwrap();
        assert!(binding.fingerprint().unwrap().starts_with("sha256:"));
        assert_eq!(binding.provenance().unwrap().pdk_version, "1.0.0");
    }

    #[test]
    fn proprietary_binding_rejects_embedded_parameters_and_uris() {
        let mut binding = PhysicalDesignBinding::reference_open_pdk();
        binding.classification = DataClassification::ProprietaryReference;
        let error = binding.validate().unwrap_err().to_string();
        assert!(error.contains("source URIs"));
    }

    #[test]
    fn topology_digest_is_verified() {
        let mut binding = PhysicalDesignBinding::reference_open_pdk();
        binding.topology.nodes[0]
            .settings
            .insert("coupling".to_string(), 0.4);
        assert!(binding
            .validate()
            .unwrap_err()
            .to_string()
            .contains("topology artifact digest"));
    }
}
