//! Integration tests for observability system
//!
//! Tests end-to-end observability artifact generation from runtime execution.

use awen_runtime::observability::{ObservabilityContext, Level};
use tempfile::TempDir;

#[test]
fn test_complete_observability_flow() {
    // Create observability context
    let ctx = ObservabilityContext::new();
    
    // Simulate a complete AWEN run with instrumentation
    
    // Top-level run span
    {
        let mut run_span = ctx.tracer.start_span("Run");
        run_span.set_attribute("correlation_id", "run_test_001");
        run_span.set_attribute("subsystem", "engine");
        run_span.set_attribute("layer", "IR");
        
        // IR compilation
        {
            let mut ir_span = ctx.tracer.start_span("IR.compile");
            ir_span.set_attribute("correlation_id", "ir_node_test");
            ir_span.set_attribute("subsystem", "engine");
            ir_span.set_attribute("layer", "IR");
            
            ctx.metrics.counter("engine.operations_executed", 1.0, "operations", {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("subsystem".to_string(), "engine".to_string());
                attrs
            });
        }
        
        // Engine execution
        {
            let mut engine_span = ctx.tracer.start_span("Engine.execute");
            engine_span.set_attribute("correlation_id", "engine_exec_001");
            engine_span.set_attribute("subsystem", "engine");
            engine_span.set_attribute("layer", "Schedule");
            
            // Simulate HAL operations
            {
                let mut hal_span = ctx.tracer.start_span("HAL.configure_channel");
                hal_span.set_attribute("correlation_id", "hal_channel_0");
                hal_span.set_attribute("subsystem", "hal");
                hal_span.set_attribute("layer", "Device");
                
                // Simulate a safety clamp event
                hal_span.add_event("safety_clamp_triggered", {
                    let mut attrs = std::collections::HashMap::new();
                    attrs.insert("channel".to_string(), "mzi_phase_0".to_string());
                    attrs.insert("original_value".to_string(), "2.5".to_string());
                    attrs.insert("clamped_value".to_string(), "1.8".to_string());
                    attrs
                });
                
                ctx.events.warning("hal", "Safety clamp applied", {
                    let mut attrs = std::collections::HashMap::new();
                    attrs.insert("channel".to_string(), "mzi_phase_0".to_string());
                    attrs
                });
                
                ctx.metrics.counter("hal.safety_clamps", 1.0, "events", {
                    let mut attrs = std::collections::HashMap::new();
                    attrs.insert("subsystem".to_string(), "hal".to_string());
                    attrs
                });
                
                ctx.metrics.histogram("hal.command_latency_us", 125.5, "microseconds", {
                    let mut attrs = std::collections::HashMap::new();
                    attrs.insert("subsystem".to_string(), "hal".to_string());
                    attrs
                });
            }
            
            // Simulate control operation
            {
                let mut control_span = ctx.tracer.start_span("Control.calibrate");
                control_span.set_attribute("correlation_id", "calib_v1");
                control_span.set_attribute("subsystem", "control");
                control_span.set_attribute("layer", "Device");
                
                ctx.events.info("control", "Drift detected, triggering recalibration", {
                    let mut attrs = std::collections::HashMap::new();
                    attrs.insert("drift_magnitude".to_string(), "0.05".to_string());
                    attrs
                });
                
                ctx.metrics.counter("control.calibrations_triggered", 1.0, "events", {
                    let mut attrs = std::collections::HashMap::new();
                    attrs.insert("subsystem".to_string(), "control".to_string());
                    attrs
                });
            }
        }
        
        // Storage operation
        {
            let mut storage_span = ctx.tracer.start_span("Storage.write_artifact");
            storage_span.set_attribute("correlation_id", "artifact_test_001");
            storage_span.set_attribute("subsystem", "storage");
            
            ctx.metrics.counter("storage.artifacts_written", 1.0, "artifacts", {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("subsystem".to_string(), "storage".to_string());
                attrs
            });
            
            ctx.metrics.histogram("storage.artifact_size_bytes", 4096.0, "bytes", {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("subsystem".to_string(), "storage".to_string());
                attrs
            });
        }
    }
    
    // Build timeline from spans
    ctx.timeline.add_entry("Engine", "Run", 0, 1000, Default::default());
    ctx.timeline.add_entry("Engine", "IR.compile", 0, 100, Default::default());
    ctx.timeline.add_entry("Engine", "Engine.execute", 100, 900, Default::default());
    ctx.timeline.add_entry("HAL.Channel.0", "configure_channel", 150, 200, Default::default());
    ctx.timeline.add_entry("Control", "calibrate", 300, 500, Default::default());
    ctx.timeline.add_entry("Storage", "write_artifact", 900, 1000, Default::default());
    
    // Export all artifacts
    let temp_dir = TempDir::new().unwrap();
    let artifacts = ctx.export(temp_dir.path()).unwrap();
    
    // Verify all artifacts exist
    assert!(artifacts.traces.exists(), "traces.jsonl should exist");
    assert!(artifacts.timeline.exists(), "timeline.json should exist");
    assert!(artifacts.metrics.exists(), "metrics.json should exist");
    assert!(artifacts.events.exists(), "events.jsonl should exist");
    assert!(artifacts.metadata.exists(), "observability_metadata.json should exist");
    
    // Verify traces content
    let traces_content = std::fs::read_to_string(&artifacts.traces).unwrap();
    assert!(traces_content.contains("Run"), "Should contain Run span");
    assert!(traces_content.contains("IR.compile"), "Should contain IR.compile span");
    assert!(traces_content.contains("safety_clamp_triggered"), "Should contain safety clamp event");
    assert!(traces_content.contains("correlation_id"), "Should contain correlation IDs");
    
    // Verify timeline content
    let timeline_content = std::fs::read_to_string(&artifacts.timeline).unwrap();
    assert!(timeline_content.contains("Engine"), "Should contain Engine lane");
    assert!(timeline_content.contains("HAL.Channel.0"), "Should contain HAL lane");
    assert!(timeline_content.contains("Control"), "Should contain Control lane");
    assert!(timeline_content.contains("Storage"), "Should contain Storage lane");
    
    // Verify metrics content
    let metrics_content = std::fs::read_to_string(&artifacts.metrics).unwrap();
    assert!(metrics_content.contains("engine.operations_executed"), "Should contain engine counter");
    assert!(metrics_content.contains("hal.safety_clamps"), "Should contain HAL counter");
    assert!(metrics_content.contains("control.calibrations_triggered"), "Should contain control counter");
    assert!(metrics_content.contains("hal.command_latency_us"), "Should contain HAL histogram");
    assert!(metrics_content.contains("storage.artifact_size_bytes"), "Should contain storage histogram");
    
    // Verify events content
    let events_content = std::fs::read_to_string(&artifacts.events).unwrap();
    assert!(events_content.contains("warning"), "Should contain warning level");
    assert!(events_content.contains("info"), "Should contain info level");
    assert!(events_content.contains("Safety clamp applied"), "Should contain safety clamp message");
    assert!(events_content.contains("Drift detected"), "Should contain drift message");
    
    // Verify metadata
    let metadata_content = std::fs::read_to_string(&artifacts.metadata).unwrap();
    assert!(metadata_content.contains("observability.v0.1"), "Should contain schema version");
    assert!(metadata_content.contains("conformance_level"), "Should contain conformance level");
    
    println!("✓ All observability artifacts generated and validated");
    println!("  - Traces: {} spans", traces_content.lines().count());
    println!("  - Events: {} events", events_content.lines().count());
    println!("  - Timeline: verified");
    println!("  - Metrics: verified");
    println!("  - Metadata: verified");
}

#[test]
fn test_span_hierarchy() {
    let ctx = ObservabilityContext::new();
    
    {
        let mut parent = ctx.tracer.start_span("parent");
        parent.set_attribute("correlation_id", "parent_001");
        parent.set_attribute("subsystem", "engine");
        
        {
            let mut child1 = ctx.tracer.start_span("child1");
            child1.set_attribute("correlation_id", "child1_001");
            child1.set_attribute("subsystem", "engine");
        }
        
        {
            let mut child2 = ctx.tracer.start_span("child2");
            child2.set_attribute("correlation_id", "child2_001");
            child2.set_attribute("subsystem", "hal");
        }
    }
    
    let spans = ctx.tracer.spans();
    assert_eq!(spans.len(), 3, "Should have 3 spans");
    
    // Verify all spans have required attributes
    for span in &spans {
        assert!(span.attributes.contains_key("correlation_id"), "Span {} should have correlation_id", span.name);
        assert!(span.attributes.contains_key("subsystem"), "Span {} should have subsystem", span.name);
    }
}

#[test]
fn test_metrics_types() {
    let ctx = ObservabilityContext::new();
    
    // Counter
    ctx.metrics.counter("test.counter", 42.0, "operations", Default::default());
    
    // Gauge
    ctx.metrics.gauge("test.gauge", 98.6, "celsius", Default::default());
    
    // Histogram
    ctx.metrics.histogram("test.histogram", 250.5, "milliseconds", Default::default());
    
    let metrics = ctx.metrics.metrics();
    assert_eq!(metrics.len(), 3, "Should have 3 metrics");
    
    // Verify types
    let counter = metrics.iter().find(|m| m.name == "test.counter").unwrap();
    assert_eq!(counter.metric_type, awen_runtime::observability::MetricType::Counter);
    
    let gauge = metrics.iter().find(|m| m.name == "test.gauge").unwrap();
    assert_eq!(gauge.metric_type, awen_runtime::observability::MetricType::Gauge);
    
    let histogram = metrics.iter().find(|m| m.name == "test.histogram").unwrap();
    assert_eq!(histogram.metric_type, awen_runtime::observability::MetricType::Histogram);
}

#[test]
fn test_event_levels() {
    let ctx = ObservabilityContext::new();
    
    ctx.events.trace("test", "trace message", Default::default());
    ctx.events.debug("test", "debug message", Default::default());
    ctx.events.info("test", "info message", Default::default());
    ctx.events.warning("test", "warning message", Default::default());
    ctx.events.error("test", "error message", Default::default());
    ctx.events.fatal("test", "fatal message", Default::default());
    
    let events = ctx.events.events();
    assert_eq!(events.len(), 6, "Should have 6 events");
    
    // Verify levels
    assert_eq!(events[0].level, Level::Trace);
    assert_eq!(events[1].level, Level::Debug);
    assert_eq!(events[2].level, Level::Info);
    assert_eq!(events[3].level, Level::Warning);
    assert_eq!(events[4].level, Level::Error);
    assert_eq!(events[5].level, Level::Fatal);
}

#[test]
fn test_timeline_lanes() {
    let ctx = ObservabilityContext::new();
    
    use awen_runtime::observability::timeline::lanes;
    
    ctx.timeline.add_entry(&lanes::ENGINE, "operation1", 0, 100, Default::default());
    ctx.timeline.add_entry(&lanes::SCHEDULER, "schedule", 0, 50, Default::default());
    ctx.timeline.add_entry(&lanes::hal_channel(0), "configure", 50, 75, Default::default());
    ctx.timeline.add_entry(&lanes::hal_channel(1), "apply", 75, 100, Default::default());
    ctx.timeline.add_entry(&lanes::CONTROL, "calibrate", 100, 200, Default::default());
    ctx.timeline.add_entry(&lanes::STORAGE, "write", 200, 250, Default::default());
    
    let timeline = ctx.timeline.build();
    assert_eq!(timeline.entries.len(), 6, "Should have 6 timeline entries");
    
    // Verify lanes
    let lane_names: Vec<&str> = timeline.entries.iter().map(|e| e.lane.as_str()).collect();
    assert!(lane_names.contains(&"Engine"));
    assert!(lane_names.contains(&"Scheduler"));
    assert!(lane_names.contains(&"HAL.Channel.0"));
    assert!(lane_names.contains(&"HAL.Channel.1"));
    assert!(lane_names.contains(&"Control"));
    assert!(lane_names.contains(&"Storage"));
}
