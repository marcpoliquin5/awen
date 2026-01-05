//! Observability data exporters
//!
//! Exports traces, metrics, events, and timeline to artifact bundles.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use serde_json;

use super::{ObservabilityContext, Span, Metric, MetricType, Event, Timeline};

/// Exporter trait for observability data
pub trait Exporter {
    fn export(&self, ctx: &ObservabilityContext) -> io::Result<ObservabilityArtifacts>;
}

/// File-based exporter (required for conformance)
pub struct FileExporter {
    artifact_dir: PathBuf,
}

impl FileExporter {
    pub fn new(artifact_dir: &Path) -> Self {
        Self {
            artifact_dir: artifact_dir.to_path_buf(),
        }
    }

    fn write_traces(&self, spans: &[Span]) -> io::Result<PathBuf> {
        let path = self.artifact_dir.join("traces.jsonl");
        let mut file = File::create(&path)?;
        
        for span in spans {
            let json = serde_json::to_string(&SpanExport {
                id: &span.id,
                parent: span.parent.as_deref(),
                name: &span.name,
                start_iso: &span.start_iso,
                end_iso: &span.end_iso,
                attributes: &span.attributes,
                events: &span.events,
                status: &format!("{:?}", span.status).to_lowercase(),
            })?;
            writeln!(file, "{}", json)?;
        }
        
        Ok(path)
    }

    fn write_timeline(&self, timeline: &Timeline) -> io::Result<PathBuf> {
        let path = self.artifact_dir.join("timeline.json");
        
        let entries: Vec<TimelineEntryExport> = timeline
            .entries
            .iter()
            .map(|e| TimelineEntryExport {
                lane: &e.lane,
                name: &e.name,
                start_ms: e.start_ms,
                end_ms: e.end_ms,
                color: e.color.as_deref(),
                attributes: &e.attributes,
            })
            .collect();
        
        let json = serde_json::to_string_pretty(&entries)?;
        fs::write(&path, json)?;
        
        Ok(path)
    }

    fn write_metrics(&self, metrics: &[Metric]) -> io::Result<PathBuf> {
        let path = self.artifact_dir.join("metrics.json");
        
        let mut counters = Vec::new();
        let mut gauges = Vec::new();
        let mut histograms = Vec::new();
        
        for metric in metrics {
            let export = MetricExport {
                name: &metric.name,
                value: metric.value,
                unit: &metric.unit,
                timestamp_iso: &metric.timestamp_iso,
                attributes: &metric.attributes,
            };
            
            match metric.metric_type {
                MetricType::Counter => counters.push(export),
                MetricType::Gauge => gauges.push(export),
                MetricType::Histogram => histograms.push(export),
            }
        }
        
        let export = MetricsExport {
            counters,
            gauges,
            histograms,
        };
        
        let json = serde_json::to_string_pretty(&export)?;
        fs::write(&path, json)?;
        
        Ok(path)
    }

    fn write_events(&self, events: &[Event]) -> io::Result<PathBuf> {
        let path = self.artifact_dir.join("events.jsonl");
        let mut file = File::create(&path)?;
        
        for event in events {
            let json = serde_json::to_string(&EventExport {
                timestamp_iso: &event.timestamp_iso,
                level: &event.level.to_string(),
                subsystem: &event.subsystem,
                message: &event.message,
                attributes: &event.attributes,
            })?;
            writeln!(file, "{}", json)?;
        }
        
        Ok(path)
    }

    fn write_metadata(&self) -> io::Result<PathBuf> {
        let path = self.artifact_dir.join("observability_metadata.json");
        
        let metadata = MetadataExport {
            schema_version: "observability.v0.1",
            runtime_version: env!("CARGO_PKG_VERSION"),
            export_timestamp_iso: &Self::now_iso8601(),
            conformance_level: "full",
        };
        
        let json = serde_json::to_string_pretty(&metadata)?;
        fs::write(&path, json)?;
        
        Ok(path)
    }

    fn now_iso8601() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        
        let secs = now.as_secs();
        let micros = now.subsec_micros();
        
        let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, micros * 1000)
            .expect("Invalid timestamp");
        
        datetime.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
    }
}

impl Exporter for FileExporter {
    fn export(&self, ctx: &ObservabilityContext) -> io::Result<ObservabilityArtifacts> {
        // Ensure artifact directory exists
        fs::create_dir_all(&self.artifact_dir)?;
        
        // Get data from context
        let spans = ctx.tracer.spans();
        let timeline = ctx.timeline.build();
        let metrics = ctx.metrics.metrics();
        let events = ctx.events.events();
        
        // Write all artifacts
        let traces_path = self.write_traces(&spans)?;
        let timeline_path = self.write_timeline(&timeline)?;
        let metrics_path = self.write_metrics(&metrics)?;
        let events_path = self.write_events(&events)?;
        let metadata_path = self.write_metadata()?;
        
        Ok(ObservabilityArtifacts {
            traces: traces_path,
            timeline: timeline_path,
            metrics: metrics_path,
            events: events_path,
            metadata: metadata_path,
        })
    }
}

/// Paths to exported observability artifacts
#[derive(Debug)]
pub struct ObservabilityArtifacts {
    pub traces: PathBuf,
    pub timeline: PathBuf,
    pub metrics: PathBuf,
    pub events: PathBuf,
    pub metadata: PathBuf,
}

// Export structures for JSON serialization

#[derive(serde::Serialize)]
struct SpanExport<'a> {
    id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<&'a str>,
    name: &'a str,
    start_iso: &'a str,
    end_iso: &'a str,
    attributes: &'a std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    events: &'a [super::tracer::SpanEvent],
    status: &'a str,
}

#[derive(serde::Serialize)]
struct TimelineEntryExport<'a> {
    lane: &'a str,
    name: &'a str,
    start_ms: i64,
    end_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<&'a str>,
    attributes: &'a std::collections::HashMap<String, String>,
}

#[derive(serde::Serialize)]
struct MetricsExport<'a> {
    counters: Vec<MetricExport<'a>>,
    gauges: Vec<MetricExport<'a>>,
    histograms: Vec<MetricExport<'a>>,
}

#[derive(serde::Serialize)]
struct MetricExport<'a> {
    name: &'a str,
    value: f64,
    unit: &'a str,
    timestamp_iso: &'a str,
    attributes: &'a std::collections::HashMap<String, String>,
}

#[derive(serde::Serialize)]
struct EventExport<'a> {
    timestamp_iso: &'a str,
    level: &'a str,
    subsystem: &'a str,
    message: &'a str,
    attributes: &'a std::collections::HashMap<String, String>,
}

#[derive(serde::Serialize)]
struct MetadataExport<'a> {
    schema_version: &'a str,
    runtime_version: &'a str,
    export_timestamp_iso: &'a str,
    conformance_level: &'a str,
}

/// OTLP Exporter interface (TODO: implement in v0.2)
pub struct OTLPExporter {
    endpoint: String,
}

impl OTLPExporter {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }
}

impl Exporter for OTLPExporter {
    fn export(&self, _ctx: &ObservabilityContext) -> io::Result<ObservabilityArtifacts> {
        // TODO: Implement OTLP export in v0.2
        // This will send data to an OpenTelemetry collector endpoint
        todo!("OTLP exporter not yet implemented - planned for v0.2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_export() {
        let ctx = ObservabilityContext::new();
        
        // Add some data
        {
            let mut span = ctx.tracer.start_span("test");
            span.set_attribute("key", "value");
        }
        
        ctx.metrics.counter("test.counter", 1.0, "ops", Default::default());
        ctx.events.info("test", "test message", Default::default());
        ctx.timeline.add_entry("Engine", "test", 0, 100, Default::default());
        
        // Export
        let temp_dir = TempDir::new().unwrap();
        let exporter = FileExporter::new(temp_dir.path());
        let artifacts = exporter.export(&ctx).unwrap();
        
        // Verify files exist
        assert!(artifacts.traces.exists());
        assert!(artifacts.timeline.exists());
        assert!(artifacts.metrics.exists());
        assert!(artifacts.events.exists());
        assert!(artifacts.metadata.exists());
        
        // Verify content
        let traces_content = fs::read_to_string(&artifacts.traces).unwrap();
        assert!(traces_content.contains("test"));
        
        let timeline_content = fs::read_to_string(&artifacts.timeline).unwrap();
        assert!(timeline_content.contains("Engine"));
    }
}
