use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use chrono::Utc;
use anyhow::Result;
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Span {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub start_iso: String,
    pub end_iso: String,
    pub attributes: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimelineEvent {
    pub lane: String,
    pub name: String,
    pub start_ms: u128,
    pub end_ms: u128,
    pub attributes: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Metrics {
    pub counters: HashMap<String, f64>,
    pub gauges: HashMap<String, f64>,
}

/// Core runtime-facing traits for observability. Implementations (exporters) must provide these
/// so the Engine and other subsystems can produce traces/metrics/timelines via a stable API.
pub trait Tracer: Send + Sync + Debug {
    fn start_span(&self, name: &str, parent: Option<&str>, attrs: HashMap<String, String>) -> Span;
    fn end_span(&self, span: &mut Span);
}

pub trait MetricsSink: Send + Sync + Debug {
    fn record_counter(&self, key: &str, value: f64);
    fn record_gauge(&self, key: &str, value: f64);
}

pub trait TimelineBuilder: Send + Sync + Debug {
    fn add_event(&self, ev: TimelineEvent);
}

/// Simple file exporter implementing Tracer/MetricsSink/TimelineBuilder by writing files into the run bundle.
#[derive(Debug)]
pub struct FileExporter {
    pub out_dir: String,
}

impl FileExporter {
    pub fn new(out_dir: &str) -> Self { Self { out_dir: out_dir.to_string() } }

    fn write_traces(&self, spans: &[Span]) -> Result<()> {
        let path = Path::new(&self.out_dir).join("traces.jsonl");
        let mut s = String::new();
        for sp in spans { s.push_str(&serde_json::to_string(sp)?); s.push('\n'); }
        fs::write(path, s)?;
        Ok(())
    }

    fn write_timeline(&self, events: &[TimelineEvent]) -> Result<()> {
        let path = Path::new(&self.out_dir).join("timeline.json");
        fs::write(path, serde_json::to_string_pretty(events)?)?;
        Ok(())
    }

    fn write_metrics(&self, metrics: &Metrics) -> Result<()> {
        let path = Path::new(&self.out_dir).join("metrics.json");
        fs::write(path, serde_json::to_string_pretty(metrics)?)?;
        Ok(())
    }

    /// Convenience: write all artifacts
    pub fn write_all(&self, spans: &[Span], events: &[TimelineEvent], metrics: &Metrics) -> Result<()> {
        self.write_traces(spans)?;
        self.write_timeline(events)?;
        self.write_metrics(metrics)?;
        Ok(())
    }
}

impl Tracer for FileExporter {
    fn start_span(&self, name: &str, parent: Option<&str>, attrs: HashMap<String, String>) -> Span {
        let now = Utc::now().to_rfc3339();
        Span { id: format!("span-{}-{}", name, now), parent: parent.map(|s| s.to_string()), name: name.to_string(), start_iso: now.clone(), end_iso: now, attributes: attrs }
    }

    fn end_span(&self, span: &mut Span) {
        span.end_iso = Utc::now().to_rfc3339();
    }
}

impl MetricsSink for FileExporter {
    fn record_counter(&self, _key: &str, _value: f64) { /* noop for minimal exporter */ }
    fn record_gauge(&self, _key: &str, _value: f64) { /* noop for minimal exporter */ }
}

impl TimelineBuilder for FileExporter {
    fn add_event(&self, _ev: TimelineEvent) { /* noop: builder writes on write_all */ }
}

/// Write newline-delimited spans to traces.jsonl under out_dir (standalone helper for non-exporter use)
pub fn write_traces(out_dir: &Path, spans: &[Span]) -> Result<()> {
    let path = out_dir.join("traces.jsonl");
    let mut s = String::new();
    for sp in spans { s.push_str(&serde_json::to_string(sp)?); s.push('\n'); }
    fs::write(path, s)?;
    Ok(())
}

/// Write timeline.json
pub fn write_timeline(out_dir: &Path, events: &[TimelineEvent]) -> Result<()> {
    let path = out_dir.join("timeline.json");
    fs::write(path, serde_json::to_string_pretty(events)?)?;
    Ok(())
}

/// Write metrics.json
pub fn write_metrics(out_dir: &Path, metrics: &Metrics) -> Result<()> {
    let path = out_dir.join("metrics.json");
    fs::write(path, serde_json::to_string_pretty(metrics)?)?;
    Ok(())
}

/// Helpers to create simple spans and timeline events from simulation results.
pub fn build_basic_observability(run_id: &str, node_ids: &[String], seed: Option<u64>) -> (Vec<Span>, Vec<TimelineEvent>, Metrics) {
    let mut spans = Vec::new();
    let mut events = Vec::new();
    let mut metrics = Metrics::default();

    let now = Utc::now();
    let t0 = now.timestamp_millis() as u128;
    let mut idx = 0u64;
    for nid in node_ids {
        let start = t0 + (idx as u128) * 5;
        let end = start + 4;
        let sp = Span {
            id: format!("{}-span-{}", run_id, idx),
            parent: None,
            name: format!("node:{}", nid),
            start_iso: Utc::now().to_rfc3339(),
            end_iso: Utc::now().to_rfc3339(),
            attributes: HashMap::new(),
        };
        spans.push(sp);

        let mut attrs = HashMap::new();
        attrs.insert("node_id".to_string(), nid.clone());
        let ev = TimelineEvent {
            lane: "kernel".to_string(),
            name: format!("exec:{}", nid),
            start_ms: start,
            end_ms: end,
            attributes: attrs,
        };
        events.push(ev);
        idx += 1;
    }

    metrics.counters.insert("nodes_executed".to_string(), node_ids.len() as f64);
    metrics.gauges.insert("seed_used".to_string(), seed.unwrap_or(0) as f64);

    (spans, events, metrics)
}
// Observability primitives (v0.1)

pub fn emit_trace(_trace: &str) {
    // TODO: emit structured trace/span JSON
}

pub fn emit_metric(_name: &str, _value: f64) {
    // TODO: integrate with metrics exporter
}
