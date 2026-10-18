//! Library side of the `awenctl caps` (capability explorer) and
//! `awenctl profile` (kernel profiler) subcommands (directive item C10).

use crate::devrt::profile::Profiler;
use anyhow::{bail, Context, Result};
use awen_compiler::{
    execute_kernel_reference, execute_kernel_simulator, KernelRequest, KernelSimulatorOptions,
    TargetBackend,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Summarise any capability-like AWEN document: a device capability
/// (`awen.device-capability.v1`), a plugin manifest (its embedded backend
/// capability), a hardware description (`awen.hardware-description.v1`) or
/// a list of awenBLAS backend profiles.
pub fn explore_capabilities(document: &Value) -> Result<Value> {
    if let Some(profiles) = document.as_array() {
        let backends: Vec<Value> = profiles
            .iter()
            .map(|profile| {
                json!({
                    "backend_id": profile.get("backend_id"),
                    "target": profile.get("target"),
                    "kernel_kinds": profile.get("supported_kinds").and_then(Value::as_array).map(Vec::len),
                    "dtypes": profile.get("supported_dtypes"),
                    "effective_bits": profile.get("effective_bits"),
                    "parameter_source": profile.get("source"),
                })
            })
            .collect();
        return Ok(json!({"document": "awen.blas-backend-profiles", "backends": backends}));
    }
    if document.get("capability_version").is_some() {
        return Ok(device_capability_summary(document));
    }
    if document.get("manifest_version").is_some() {
        let capability = document
            .pointer("/backend/capabilities")
            .map(device_capability_summary);
        return Ok(json!({
            "document": document.get("manifest_version"),
            "plugin_id": document.get("id"),
            "plugin_version": document.get("version"),
            "signed": document.get("signature").is_some_and(|s| !s.is_null()),
            "capabilities": document.get("capabilities"),
            "backend": capability,
        }));
    }
    if document.get("version").and_then(Value::as_str) == Some("awen.hardware-description.v1") {
        let mut classes: BTreeMap<String, u64> = BTreeMap::new();
        for component in document
            .get("components")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(class) = component.get("class").and_then(Value::as_str) {
                *classes.entry(class.to_owned()).or_default() += 1;
            }
        }
        return Ok(json!({
            "document": "awen.hardware-description.v1",
            "device_id": document.get("device_id"),
            "provenance": document.pointer("/provenance/source"),
            "component_classes": classes,
        }));
    }
    bail!("not a recognised AWEN capability document")
}

fn device_capability_summary(capability: &Value) -> Value {
    let operations: Vec<Value> = capability
        .get("supported_operations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|op| op.get("operation").cloned())
        .collect();
    json!({
        "document": capability.get("capability_version"),
        "backend_id": capability.get("backend_id"),
        "matrix_core": capability.get("matrix_core"),
        "operations": operations,
        "dtypes": capability.get("supported_dtypes"),
        "effective_bits": capability.get("effective_bits"),
        "adc_bits": capability.get("adc_bits"),
        "dac_bits": capability.get("dac_bits"),
        "wavelengths_nm": capability.get("supported_wavelengths_nm"),
        "coherence_mode": capability.get("coherence_mode"),
        "supports_complex": capability.get("supports_complex"),
        "analog_noise": capability.get("analog_noise"),
        "calibration_required": capability.pointer("/calibration_requirements/required"),
    })
}

/// Render [`explore_capabilities`] output as aligned `key: value` text.
pub fn render_text(summary: &Value) -> String {
    let mut out = String::new();
    fn walk(value: &Value, prefix: &str, out: &mut String) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map {
                    let name = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    walk(inner, &name, out);
                }
            }
            Value::Null => {}
            other => out.push_str(&format!("{prefix}: {other}\n")),
        }
    }
    walk(summary, "", &mut out);
    out
}

/// Run one awenBLAS request `repeat` times and return (summary, chrome trace).
/// Timings are host wall-clock times of the CPU reference or the simulator.
pub fn profile_kernel(
    request: &KernelRequest,
    repeat: usize,
    simulator: Option<KernelSimulatorOptions>,
) -> Result<(Value, Value)> {
    if repeat == 0 {
        bail!("repeat must be at least 1");
    }
    let mut profiler = Profiler::new();
    let (name, device) = match simulator {
        Some(_) => (format!("{:?}", request.kind), "photonic-simulator"),
        None => (format!("{:?}", request.kind), "cpu-reference"),
    };
    let mut simulated = false;
    for index in 0..repeat {
        let result = profiler.time(&name, device, || match simulator {
            Some(options) => execute_kernel_simulator(
                request,
                KernelSimulatorOptions {
                    seed: options.seed.wrapping_add(index as u64),
                    ..options
                },
            ),
            None => execute_kernel_reference(request),
        });
        simulated = result.with_context(|| format!("run {index}"))?.simulated;
    }
    let summary = json!({
        "request_id": request.id,
        "kind": request.kind,
        "device": device,
        "simulated": simulated,
        "timing_source": "host wall clock (std::time::Instant); not a device measurement",
        "spans": profiler.summary(),
    });
    Ok((summary, profiler.chrome_trace()))
}

pub fn parse_simulator(
    mode: &str,
    effective_bits: u8,
    noise_fraction: f64,
    seed: u64,
) -> Result<Option<KernelSimulatorOptions>> {
    match mode {
        "reference" => Ok(None),
        "simulator" => Ok(Some(KernelSimulatorOptions {
            target: TargetBackend::Photonic,
            effective_bits,
            noise_fraction,
            seed,
        })),
        other => bail!("unknown profile mode {other}; use reference or simulator"),
    }
}
