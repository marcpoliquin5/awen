//! Timeline builder for Nsight-like visualization
//!
//! Aggregates spans into lane-based timeline format.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// TimelineBuilder constructs timeline visualization data
#[derive(Clone)]
pub struct TimelineBuilder {
    entries: Arc<Mutex<Vec<TimelineEntry>>>,
    start_time_ms: Arc<Mutex<Option<i64>>>,
}

impl TimelineBuilder {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            start_time_ms: Arc::new(Mutex::new(None)),
        }
    }

    /// Add a timeline entry
    pub fn add_entry(&self, lane: &str, name: &str, start_ms: i64, end_ms: i64, attributes: HashMap<String, String>) {
        let entry = TimelineEntry {
            lane: lane.to_string(),
            name: name.to_string(),
            start_ms,
            end_ms,
            color: None,
            attributes,
        };
        
        self.entries.lock().unwrap().push(entry);
    }

    /// Add entry with color hint
    pub fn add_entry_with_color(
        &self,
        lane: &str,
        name: &str,
        start_ms: i64,
        end_ms: i64,
        color: &str,
        attributes: HashMap<String, String>,
    ) {
        let entry = TimelineEntry {
            lane: lane.to_string(),
            name: name.to_string(),
            start_ms,
            end_ms,
            color: Some(color.to_string()),
            attributes,
        };
        
        self.entries.lock().unwrap().push(entry);
    }

    /// Build the complete timeline
    pub fn build(&self) -> Timeline {
        Timeline {
            entries: self.entries.lock().unwrap().clone(),
        }
    }

    /// Record the absolute start time for relative timestamp conversion
    pub fn set_start_time_ms(&self, start_ms: i64) {
        *self.start_time_ms.lock().unwrap() = Some(start_ms);
    }

    /// Get the start time if set
    pub fn start_time_ms(&self) -> Option<i64> {
        *self.start_time_ms.lock().unwrap()
    }
}

impl Default for TimelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete timeline with all entries
#[derive(Clone, Debug)]
pub struct Timeline {
    pub entries: Vec<TimelineEntry>,
}

/// Timeline entry (appears in a lane)
#[derive(Clone, Debug)]
pub struct TimelineEntry {
    pub lane: String,
    pub name: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub color: Option<String>,
    pub attributes: HashMap<String, String>,
}

/// Standard lane names
pub mod lanes {
    pub const ENGINE: &str = "Engine";
    pub const SCHEDULER: &str = "Scheduler";
    pub const CONTROL: &str = "Control";
    pub const STORAGE: &str = "Storage";
    
    /// Generate HAL channel lane name
    pub fn hal_channel(channel_id: usize) -> String {
        format!("HAL.Channel.{}", channel_id)
    }
    
    /// Generate plugin lane name
    pub fn plugin(name: &str) -> String {
        format!("Plugin.{}", name)
    }
    
    /// Generate quantum backend lane name
    pub fn quantum(backend: &str) -> String {
        format!("Quantum.{}", backend)
    }
}

/// Default colors per subsystem
pub mod colors {
    pub const ENGINE: &str = "#3498db";
    pub const SCHEDULER: &str = "#9b59b6";
    pub const HAL: &str = "#e67e22";
    pub const CONTROL: &str = "#f39c12";
    pub const STORAGE: &str = "#16a085";
    pub const PLUGIN: &str = "#95a5a6";
    pub const QUANTUM: &str = "#e74c3c";
    pub const ERROR: &str = "#c0392b";
    pub const WARNING: &str = "#f39c12";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeline_builder() {
        let builder = TimelineBuilder::new();
        
        builder.add_entry("Engine", "execute", 0, 100, HashMap::new());
        builder.add_entry("HAL.Channel.0", "apply_waveform", 50, 75, HashMap::new());
        
        let timeline = builder.build();
        assert_eq!(timeline.entries.len(), 2);
        assert_eq!(timeline.entries[0].lane, "Engine");
        assert_eq!(timeline.entries[1].lane, "HAL.Channel.0");
    }

    #[test]
    fn test_lane_helpers() {
        assert_eq!(lanes::hal_channel(0), "HAL.Channel.0");
        assert_eq!(lanes::plugin("reference_sim"), "Plugin.reference_sim");
        assert_eq!(lanes::quantum("CV"), "Quantum.CV");
    }
}
