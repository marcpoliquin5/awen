use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
    pub fn new(out_dir: &str) -> Self {
        Self {
            out_dir: out_dir.to_string(),
        }
    }

    fn write_traces(&self, spans: &[Span]) -> Result<()> {
        let path = Path::new(&self.out_dir).join("traces.jsonl");
        let mut s = String::new();
        for sp in spans {
            s.push_str(&serde_json::to_string(sp)?);
            s.push('\n');
        }
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
    pub fn write_all(
        &self,
        spans: &[Span],
        events: &[TimelineEvent],
        metrics: &Metrics,
    ) -> Result<()> {
        self.write_traces(spans)?;
        self.write_timeline(events)?;
        self.write_metrics(metrics)?;
        Ok(())
    }
}

impl Tracer for FileExporter {
    fn start_span(&self, name: &str, parent: Option<&str>, attrs: HashMap<String, String>) -> Span {
        let now = Utc::now().to_rfc3339();
        Span {
            id: format!("span-{}-{}", name, now),
            parent: parent.map(|s| s.to_string()),
            name: name.to_string(),
            start_iso: now.clone(),
            end_iso: now,
            attributes: attrs,
        }
    }

    fn end_span(&self, span: &mut Span) {
        span.end_iso = Utc::now().to_rfc3339();
    }
}

impl MetricsSink for FileExporter {
    fn record_counter(&self, _key: &str, _value: f64) { /* noop for minimal exporter */
    }
    fn record_gauge(&self, _key: &str, _value: f64) { /* noop for minimal exporter */
    }
}

impl TimelineBuilder for FileExporter {
    fn add_event(&self, _ev: TimelineEvent) { /* noop: builder writes on write_all */
    }
}

/// Write newline-delimited spans to traces.jsonl under out_dir (standalone helper for non-exporter use)
pub fn write_traces(out_dir: &Path, spans: &[Span]) -> Result<()> {
    let path = out_dir.join("traces.jsonl");
    let mut s = String::new();
    for sp in spans {
        s.push_str(&serde_json::to_string(sp)?);
        s.push('\n');
    }
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
pub fn build_basic_observability(
    run_id: &str,
    node_ids: &[String],
    seed: Option<u64>,
) -> (Vec<Span>, Vec<TimelineEvent>, Metrics) {
    let mut spans = Vec::new();
    let mut events = Vec::new();
    let mut metrics = Metrics::default();

    let now = Utc::now();
    let t0 = now.timestamp_millis() as u128;
    for (idx, nid) in node_ids.iter().enumerate() {
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
    }

    metrics
        .counters
        .insert("nodes_executed".to_string(), node_ids.len() as f64);
    metrics
        .gauges
        .insert("seed_used".to_string(), seed.unwrap_or(0) as f64);

    (spans, events, metrics)
}
// Observability primitives (v0.1)

pub fn emit_trace(_trace: &str) {
    // TODO: emit structured trace/span JSON
}

pub fn emit_metric(_name: &str, _value: f64) {
    // TODO: integrate with metrics exporter
}

// Compatibility layer: a higher-level ObservabilityContext used by older tests
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRecord {
    pub name: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub unit: String,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub level: Level,
    pub source: String,
    pub message: String,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub lane: String,
    pub name: String,
    pub start_ms: u128,
    pub end_ms: u128,
    pub attributes: HashMap<String, String>,
}

pub mod timeline {
    pub mod lanes {
        pub const ENGINE: &str = "Engine";
        pub const SCHEDULER: &str = "Scheduler";
        pub const CONTROL: &str = "Control";
        pub const STORAGE: &str = "Storage";

        pub fn hal_channel(i: u32) -> String {
            format!("HAL.Channel.{}", i)
        }
    }
}

/// Simple thread-safe tracer compatible with tests
#[derive(Clone)]
pub struct TracerHandle {
    inner: Arc<Mutex<Vec<Span>>>,
}

pub struct SpanHandle {
    inner: Arc<Mutex<Vec<Span>>>,
    idx: usize,
}

impl TracerHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub fn start_span(&self, name: &str) -> SpanHandle {
        let mut guard = self.inner.lock().unwrap();
        let sp = Span {
            id: format!("span-{}", name),
            parent: None,
            name: name.to_string(),
            start_iso: Utc::now().to_rfc3339(),
            end_iso: Utc::now().to_rfc3339(),
            attributes: HashMap::new(),
        };
        guard.push(sp);
        SpanHandle {
            inner: Arc::clone(&self.inner),
            idx: guard.len() - 1,
        }
    }
    pub fn spans(&self) -> Vec<Span> {
        self.inner.lock().unwrap().clone()
    }
}

impl Default for TracerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl SpanHandle {
    pub fn set_attribute(&mut self, key: &str, value: &str) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(sp) = guard.get_mut(self.idx) {
                sp.attributes.insert(key.to_string(), value.to_string());
            }
        }
    }
    pub fn add_event(&mut self, _name: &str, _attrs: HashMap<String, String>) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(sp) = guard.get_mut(self.idx) {
                // record the event name and attrs into span attributes for test visibility
                let key = format!("event::{}", _name);
                if let Ok(s) = serde_json::to_string(&_attrs) {
                    sp.attributes.insert(key, s);
                } else {
                    sp.attributes.insert(key, "{}".to_string());
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct MetricsCollector {
    inner: Arc<Mutex<Vec<MetricRecord>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub fn counter(&self, name: &str, value: f64, unit: &str, attrs: HashMap<String, String>) {
        let mut guard = self.inner.lock().unwrap();
        guard.push(MetricRecord {
            name: name.to_string(),
            metric_type: MetricType::Counter,
            value,
            unit: unit.to_string(),
            attributes: attrs,
        });
    }
    pub fn gauge(&self, name: &str, value: f64, unit: &str, attrs: HashMap<String, String>) {
        let mut guard = self.inner.lock().unwrap();
        guard.push(MetricRecord {
            name: name.to_string(),
            metric_type: MetricType::Gauge,
            value,
            unit: unit.to_string(),
            attributes: attrs,
        });
    }
    pub fn histogram(&self, name: &str, value: f64, unit: &str, attrs: HashMap<String, String>) {
        let mut guard = self.inner.lock().unwrap();
        guard.push(MetricRecord {
            name: name.to_string(),
            metric_type: MetricType::Histogram,
            value,
            unit: unit.to_string(),
            attributes: attrs,
        });
    }
    pub fn metrics(&self) -> Vec<MetricRecord> {
        self.inner.lock().unwrap().clone()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct EventSink {
    inner: Arc<Mutex<Vec<EventRecord>>>,
}

impl EventSink {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }
    fn push(&self, level: Level, source: &str, message: &str, attrs: HashMap<String, String>) {
        let mut guard = self.inner.lock().unwrap();
        guard.push(EventRecord {
            level,
            source: source.to_string(),
            message: message.to_string(),
            attributes: attrs,
        });
    }
    pub fn trace(&self, source: &str, message: &str, attrs: HashMap<String, String>) {
        self.push(Level::Trace, source, message, attrs)
    }
    pub fn debug(&self, source: &str, message: &str, attrs: HashMap<String, String>) {
        self.push(Level::Debug, source, message, attrs)
    }
    pub fn info(&self, source: &str, message: &str, attrs: HashMap<String, String>) {
        self.push(Level::Info, source, message, attrs)
    }
    pub fn warning(&self, source: &str, message: &str, attrs: HashMap<String, String>) {
        self.push(Level::Warning, source, message, attrs)
    }
    pub fn error(&self, source: &str, message: &str, attrs: HashMap<String, String>) {
        self.push(Level::Error, source, message, attrs)
    }
    pub fn fatal(&self, source: &str, message: &str, attrs: HashMap<String, String>) {
        self.push(Level::Fatal, source, message, attrs)
    }
    pub fn events(&self) -> Vec<EventRecord> {
        self.inner.lock().unwrap().clone()
    }
}

impl Default for EventSink {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct TimelineBuilderCompat {
    inner: Arc<Mutex<Vec<TimelineEntry>>>,
}

impl TimelineBuilderCompat {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub fn add_entry(
        &self,
        lane: &str,
        name: &str,
        start_ms: u128,
        end_ms: u128,
        attrs: HashMap<String, String>,
    ) {
        let mut guard = self.inner.lock().unwrap();
        guard.push(TimelineEntry {
            lane: lane.to_string(),
            name: name.to_string(),
            start_ms,
            end_ms,
            attributes: attrs,
        });
    }
    pub fn build(&self) -> Timeline {
        Timeline {
            entries: self.inner.lock().unwrap().clone(),
        }
    }
}

impl Default for TimelineBuilderCompat {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct Timeline {
    pub entries: Vec<TimelineEntry>,
}

pub struct ObservabilityArtifacts {
    pub traces: PathBuf,
    pub timeline: PathBuf,
    pub metrics: PathBuf,
    pub events: PathBuf,
    pub metadata: PathBuf,
}

impl ObservabilityArtifacts {
    #[allow(dead_code)]
    fn exists(&self) -> bool {
        self.traces.exists() && self.timeline.exists() && self.metrics.exists()
    }
}

#[derive(Clone)]
pub struct ObservabilityContext {
    pub tracer: TracerHandle,
    pub metrics: MetricsCollector,
    pub events: EventSink,
    pub timeline: TimelineBuilderCompat,
}

impl ObservabilityContext {
    pub fn new() -> Self {
        ObservabilityContext {
            tracer: TracerHandle::new(),
            metrics: MetricsCollector::new(),
            events: EventSink::new(),
            timeline: TimelineBuilderCompat::new(),
        }
    }

    pub fn export(&self, out_dir: &std::path::Path) -> Result<ObservabilityArtifacts> {
        // write traces
        let traces = out_dir.join("traces.jsonl");
        let spans = self.tracer.spans();
        let mut f = std::fs::File::create(&traces)?;
        for sp in &spans {
            writeln!(f, "{}", serde_json::to_string(sp)?)?;
        }

        // timeline
        let timeline = out_dir.join("timeline.json");
        let tl = self.timeline.build();
        std::fs::write(&timeline, serde_json::to_string_pretty(&tl.entries)?)?;

        // metrics
        let metrics = out_dir.join("metrics.json");
        let m = self.metrics.metrics();
        std::fs::write(&metrics, serde_json::to_string_pretty(&m)?)?;

        // events
        let events = out_dir.join("events.jsonl");
        let evs = self.events.events();
        let mut ef = std::fs::File::create(&events)?;
        for e in &evs {
            writeln!(ef, "{}", serde_json::to_string(e)?)?;
        }

        // metadata
        let metadata = out_dir.join("observability_metadata.json");
        let meta =
            serde_json::json!({ "schema": "observability.v0.1", "conformance_level": "basic" });
        std::fs::write(&metadata, serde_json::to_string_pretty(&meta)?)?;

        Ok(ObservabilityArtifacts {
            traces,
            timeline,
            metrics,
            events,
            metadata,
        })
    }
}

impl Default for ObservabilityContext {
    fn default() -> Self {
        Self::new()
    }
}
