# Observability & Profiling Model (schemas v0.1)

This document specifies JSON schemas for AWEN observability exports: spans (`traces.jsonl`), timeline (`timeline.json`), and metrics (`metrics.json`). These schemas are deliberately minimal but versioned to allow conformance tests.

## Span schema (traces.jsonl - newline-delimited JSON)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "AWEN Span v0.1",
  "type": "object",
  "required": ["id","name","start_iso","end_iso"],
  "properties": {
    "id": {"type":"string"},
    "parent": {"type":["string","null"]},
    "name": {"type":"string"},
    "start_iso": {"type":"string","format":"date-time"},
    "end_iso": {"type":"string","format":"date-time"},
    "attributes": {"type":"object","additionalProperties":{"type":"string"}}
  }
}
```

## Timeline schema (timeline.json)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "AWEN Timeline v0.1",
  "type": "array",
  "items": {
    "type":"object",
    "required":["lane","name","start_ms","end_ms"],
    "properties":{
      "lane":{"type":"string"},
      "name":{"type":"string"},
      "start_ms":{"type":"integer"},
      "end_ms":{"type":"integer"},
      "attributes":{"type":"object","additionalProperties":{"type":"string"}}
    }
  }
}
```

## Metrics schema (metrics.json)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "AWEN Metrics v0.1",
  "type": "object",
  "properties": {
    "counters": {"type":"object","additionalProperties":{"type":"number"}},
    "gauges": {"type":"object","additionalProperties":{"type":"number"}}
  }
}
```

## Correlation IDs
All spans and timeline events should include attributes that reference stable `correlation_id`s where applicable (IR node ids, kernel ids, parameter ids, artifact ids). This allows deterministic linking between artifacts.

## Versioning
This schema is `observability.v0.1`. Future versions must follow AEP revision process.
