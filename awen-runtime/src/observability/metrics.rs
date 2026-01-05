//! Metrics collection for AWEN runtime
//!
//! Supports counters, gauges, and histograms with units and attributes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// MetricsSink records metrics
#[derive(Clone)]
pub struct MetricsSink {
    metrics: Arc<Mutex<Vec<Metric>>>,
}

impl MetricsSink {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record a counter (monotonically increasing value)
    pub fn counter(&self, name: &str, value: f64, unit: &str, attributes: HashMap<String, String>) {
        self.record(Metric {
            name: name.to_string(),
            metric_type: MetricType::Counter,
            value,
            unit: unit.to_string(),
            timestamp_iso: Self::now_iso8601(),
            attributes,
        });
    }

    /// Record a gauge (point-in-time value)
    pub fn gauge(&self, name: &str, value: f64, unit: &str, attributes: HashMap<String, String>) {
        self.record(Metric {
            name: name.to_string(),
            metric_type: MetricType::Gauge,
            value,
            unit: unit.to_string(),
            timestamp_iso: Self::now_iso8601(),
            attributes,
        });
    }

    /// Record a histogram value
    pub fn histogram(&self, name: &str, value: f64, unit: &str, attributes: HashMap<String, String>) {
        self.record(Metric {
            name: name.to_string(),
            metric_type: MetricType::Histogram,
            value,
            unit: unit.to_string(),
            timestamp_iso: Self::now_iso8601(),
            attributes,
        });
    }

    fn record(&self, metric: Metric) {
        self.metrics.lock().unwrap().push(metric);
    }

    /// Get all recorded metrics
    pub fn metrics(&self) -> Vec<Metric> {
        self.metrics.lock().unwrap().clone()
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

impl Default for MetricsSink {
    fn default() -> Self {
        Self::new()
    }
}

/// Recorded metric
#[derive(Clone, Debug)]
pub struct Metric {
    pub name: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub unit: String,
    pub timestamp_iso: String,
    pub attributes: HashMap<String, String>,
}

/// Metric types
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter() {
        let sink = MetricsSink::new();
        sink.counter("test.counter", 42.0, "operations", HashMap::new());
        
        let metrics = sink.metrics();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "test.counter");
        assert_eq!(metrics[0].metric_type, MetricType::Counter);
        assert_eq!(metrics[0].value, 42.0);
    }

    #[test]
    fn test_gauge() {
        let sink = MetricsSink::new();
        sink.gauge("test.gauge", 3.14, "celsius", HashMap::new());
        
        let metrics = sink.metrics();
        assert_eq!(metrics[0].metric_type, MetricType::Gauge);
        assert_eq!(metrics[0].unit, "celsius");
    }

    #[test]
    fn test_histogram() {
        let sink = MetricsSink::new();
        sink.histogram("test.histogram", 125.5, "microseconds", HashMap::new());
        
        let metrics = sink.metrics();
        assert_eq!(metrics[0].metric_type, MetricType::Histogram);
    }
}
