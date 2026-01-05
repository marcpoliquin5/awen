//! Distributed tracing implementation for AWEN
//!
//! Provides hierarchical span tracing with RAII guards and event capture.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Tracer creates and manages spans
#[derive(Clone)]
pub struct Tracer {
    spans: Arc<Mutex<Vec<Span>>>,
}

impl Tracer {
    pub fn new() -> Self {
        Self {
            spans: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Start a new span with the given name
    pub fn start_span(&self, name: &str) -> SpanGuard {
        let id = Self::generate_span_id();
        let parent = None; // TODO: Track active span for parent relationship
        let start = Self::now_iso8601();
        
        SpanGuard {
            id: id.clone(),
            name: name.to_string(),
            parent,
            start,
            attributes: HashMap::new(),
            events: Vec::new(),
            status: SpanStatus::Ok,
            tracer: self.clone(),
        }
    }

    /// Get all completed spans
    pub fn spans(&self) -> Vec<Span> {
        self.spans.lock().unwrap().clone()
    }

    fn record_span(&self, span: Span) {
        self.spans.lock().unwrap().push(span);
    }

    fn generate_span_id() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("span_{:016x}", id)
    }

    fn now_iso8601() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        
        let secs = now.as_secs();
        let micros = now.subsec_micros();
        
        // Format as ISO8601 with microsecond precision
        let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, micros * 1000)
            .expect("Invalid timestamp");
        
        datetime.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
    }
}

impl Default for Tracer {
    fn default() -> Self {
        Self::new()
    }
}

/// Completed span (recorded after SpanGuard drops)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Span {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub name: String,
    pub start_iso: String,
    pub end_iso: String,
    pub attributes: Attributes,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<SpanEvent>,
    #[serde(skip_serializing_if = "SpanStatus::is_ok")]
    pub status: SpanStatus,
}

/// RAII guard for active span - automatically closes on drop
pub struct SpanGuard {
    id: String,
    name: String,
    parent: Option<String>,
    start: String,
    attributes: HashMap<String, String>,
    events: Vec<SpanEvent>,
    status: SpanStatus,
    tracer: Tracer,
}

impl SpanGuard {
    /// Set an attribute on this span
    pub fn set_attribute(&mut self, key: &str, value: &str) {
        self.attributes.insert(key.to_string(), value.to_string());
    }

    /// Add an event within this span
    pub fn add_event(&mut self, name: &str, attributes: Attributes) {
        self.events.push(SpanEvent {
            timestamp_iso: Tracer::now_iso8601(),
            name: name.to_string(),
            attributes,
        });
    }

    /// Set the status of this span
    pub fn set_status(&mut self, status: SpanStatus) {
        self.status = status;
    }

    /// Get the span ID
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        let end = Tracer::now_iso8601();
        
        let span = Span {
            id: self.id.clone(),
            parent: self.parent.clone(),
            name: self.name.clone(),
            start_iso: self.start.clone(),
            end_iso: end,
            attributes: self.attributes.clone(),
            events: self.events.clone(),
            status: self.status.clone(),
        };
        
        self.tracer.record_span(span);
    }
}

/// Event within a span
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpanEvent {
    pub timestamp_iso: String,
    pub name: String,
    pub attributes: Attributes,
}

/// Span status
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpanStatus {
    Ok,
    Error,
    Cancelled,
}

impl SpanStatus {
    fn is_ok(&self) -> bool {
        *self == SpanStatus::Ok
    }
}

/// Attributes are string key-value pairs
pub type Attributes = HashMap<String, String>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_lifecycle() {
        let tracer = Tracer::new();
        
        {
            let mut span = tracer.start_span("test_span");
            span.set_attribute("key", "value");
            span.add_event("event1", HashMap::new());
        }
        
        let spans = tracer.spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].name, "test_span");
        assert_eq!(spans[0].attributes.get("key").unwrap(), "value");
        assert_eq!(spans[0].events.len(), 1);
    }

    #[test]
    fn test_span_status() {
        let tracer = Tracer::new();
        
        {
            let mut span = tracer.start_span("error_span");
            span.set_status(SpanStatus::Error);
        }
        
        let spans = tracer.spans();
        assert_eq!(spans[0].status, SpanStatus::Error);
    }

    #[test]
    fn test_span_serialization() {
        let span = Span {
            id: "span_123".to_string(),
            parent: None,
            name: "test".to_string(),
            start_iso: "2026-01-05T10:00:00.000000Z".to_string(),
            end_iso: "2026-01-05T10:00:01.000000Z".to_string(),
            attributes: HashMap::new(),
            events: vec![],
            status: SpanStatus::Ok,
        };
        
        let json = serde_json::to_string(&span).unwrap();
        assert!(json.contains("span_123"));
        assert!(json.contains("test"));
    }
}
