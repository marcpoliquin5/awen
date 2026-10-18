//! Lightweight wall-clock profiler with Chrome trace export.
//!
//! Spans are host wall-clock times measured with `std::time::Instant` around
//! whatever ran (on this project today: CPU reference or the simulator). They
//! are not device timings.

use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Span {
    pub name: String,
    pub device: String,
    pub start_ns: u128,
    pub duration_ns: u128,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SpanSummary {
    pub count: usize,
    pub total_ns: u128,
    pub mean_ns: f64,
    pub median_ns: u128,
    pub max_ns: u128,
}

#[derive(Debug)]
pub struct Profiler {
    origin: Instant,
    spans: Vec<Span>,
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            spans: Vec::new(),
        }
    }

    pub fn time<T>(&mut self, name: &str, device: &str, work: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let value = work();
        let end = Instant::now();
        self.spans.push(Span {
            name: name.into(),
            device: device.into(),
            start_ns: start.duration_since(self.origin).as_nanos(),
            duration_ns: end.duration_since(start).as_nanos(),
        });
        value
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub fn summary(&self) -> BTreeMap<String, SpanSummary> {
        let mut grouped: BTreeMap<String, Vec<u128>> = BTreeMap::new();
        for span in &self.spans {
            grouped
                .entry(span.name.clone())
                .or_default()
                .push(span.duration_ns);
        }
        grouped
            .into_iter()
            .map(|(name, mut durations)| {
                durations.sort_unstable();
                let total: u128 = durations.iter().sum();
                let count = durations.len();
                let median = durations[(count - 1) / 2];
                (
                    name,
                    SpanSummary {
                        count,
                        total_ns: total,
                        mean_ns: total as f64 / count as f64,
                        median_ns: median,
                        max_ns: *durations.last().expect("non-empty"),
                    },
                )
            })
            .collect()
    }

    /// Chrome / Perfetto trace-event JSON ("X" complete events, microseconds).
    pub fn chrome_trace(&self) -> serde_json::Value {
        let events: Vec<_> = self
            .spans
            .iter()
            .map(|span| {
                json!({
                    "name": span.name,
                    "cat": "awen",
                    "ph": "X",
                    "ts": span.start_ns as f64 / 1000.0,
                    "dur": span.duration_ns as f64 / 1000.0,
                    "pid": 1,
                    "tid": span.device,
                })
            })
            .collect();
        json!({ "traceEvents": events, "displayTimeUnit": "ns" })
    }
}
