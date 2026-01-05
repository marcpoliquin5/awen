//! Event logging for AWEN runtime
//!
//! Captures discrete events with severity levels.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// EventLogger records discrete events
#[derive(Clone)]
pub struct EventLogger {
    events: Arc<Mutex<Vec<Event>>>,
}

impl EventLogger {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Log an event
    pub fn log(&self, level: Level, subsystem: &str, message: &str, attributes: HashMap<String, String>) {
        self.record(Event {
            timestamp_iso: Self::now_iso8601(),
            level,
            subsystem: subsystem.to_string(),
            message: message.to_string(),
            attributes,
        });
    }

    /// Convenience methods for each level
    pub fn trace(&self, subsystem: &str, message: &str, attributes: HashMap<String, String>) {
        self.log(Level::Trace, subsystem, message, attributes);
    }

    pub fn debug(&self, subsystem: &str, message: &str, attributes: HashMap<String, String>) {
        self.log(Level::Debug, subsystem, message, attributes);
    }

    pub fn info(&self, subsystem: &str, message: &str, attributes: HashMap<String, String>) {
        self.log(Level::Info, subsystem, message, attributes);
    }

    pub fn warning(&self, subsystem: &str, message: &str, attributes: HashMap<String, String>) {
        self.log(Level::Warning, subsystem, message, attributes);
    }

    pub fn error(&self, subsystem: &str, message: &str, attributes: HashMap<String, String>) {
        self.log(Level::Error, subsystem, message, attributes);
    }

    pub fn fatal(&self, subsystem: &str, message: &str, attributes: HashMap<String, String>) {
        self.log(Level::Fatal, subsystem, message, attributes);
    }

    fn record(&self, event: Event) {
        self.events.lock().unwrap().push(event);
    }

    /// Get all recorded events
    pub fn events(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
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

impl Default for EventLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// Discrete event
#[derive(Clone, Debug)]
pub struct Event {
    pub timestamp_iso: String,
    pub level: Level,
    pub subsystem: String,
    pub message: String,
    pub attributes: HashMap<String, String>,
}

/// Event severity levels
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Level::Trace => write!(f, "trace"),
            Level::Debug => write!(f, "debug"),
            Level::Info => write!(f, "info"),
            Level::Warning => write!(f, "warning"),
            Level::Error => write!(f, "error"),
            Level::Fatal => write!(f, "fatal"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_logging() {
        let logger = EventLogger::new();
        
        logger.info("engine", "Execution started", HashMap::new());
        logger.warning("control", "Drift detected", {
            let mut attrs = HashMap::new();
            attrs.insert("magnitude".to_string(), "0.05".to_string());
            attrs
        });
        
        let events = logger.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].level, Level::Info);
        assert_eq!(events[1].level, Level::Warning);
        assert_eq!(events[1].attributes.get("magnitude").unwrap(), "0.05");
    }

    #[test]
    fn test_level_ordering() {
        assert!(Level::Trace < Level::Debug);
        assert!(Level::Debug < Level::Info);
        assert!(Level::Info < Level::Warning);
        assert!(Level::Warning < Level::Error);
        assert!(Level::Error < Level::Fatal);
    }
}
