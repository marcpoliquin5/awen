//! JSON Schema validation shared by the runtime and its conformance tests.
//!
//! Wraps the `jsonschema` crate so every caller gets the same policy:
//! formats are asserted (not just annotated) for every draft, retrieval is
//! offline (only the documents passed in can be referenced), and errors are
//! reported as a flat list of messages.

use anyhow::{anyhow, Result};
use serde_json::Value;

pub use jsonschema::Validator;

/// Compile `schema`, resolving `$ref`s only against `documents` (`(id, schema)` pairs).
pub fn compile_with(schema: &Value, documents: &[(&str, &Value)]) -> Result<Validator> {
    let mut builder = jsonschema::Registry::new();
    for (id, document) in documents {
        builder = builder
            .add(id, *document)
            .map_err(|error| anyhow!("register schema {id}: {error}"))?;
    }
    let registry = builder
        .prepare()
        .map_err(|error| anyhow!("prepare schema registry: {error}"))?;
    jsonschema::options()
        .should_validate_formats(true)
        .with_registry(&registry)
        .build(schema)
        .map_err(|error| anyhow!("compile schema: {error}"))
}

/// Compile a self-contained schema.
pub fn compile(schema: &Value) -> Result<Validator> {
    compile_with(schema, &[])
}

/// Every validation error message for `instance` (empty when valid).
pub fn errors(validator: &Validator, instance: &Value) -> Vec<String> {
    validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect()
}

/// `Ok(())` when valid, otherwise all messages joined with `"; "`.
pub fn check(validator: &Validator, instance: &Value) -> std::result::Result<(), String> {
    let messages = errors(validator, instance);
    if messages.is_empty() {
        Ok(())
    } else {
        Err(messages.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn formats_are_asserted_under_2020_12() {
        let v = compile(&json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "string", "format": "date-time"
        }))
        .unwrap();
        assert!(v.is_valid(&json!("2026-10-18T07:00:00Z")));
        assert!(!v.is_valid(&json!("not a date")));
    }

    #[test]
    fn references_resolve_only_against_supplied_documents() {
        let dep = json!({"$id": "https://awen.dev/schemas/dep.json", "type": "integer"});
        let root = json!({"$ref": "https://awen.dev/schemas/dep.json"});
        let v = compile_with(&root, &[("https://awen.dev/schemas/dep.json", &dep)]).unwrap();
        assert!(v.is_valid(&json!(3)));
        assert_eq!(errors(&v, &json!("x")).len(), 1);
        assert!(check(&v, &json!("x")).is_err());
        assert!(
            compile(&root).is_err(),
            "unknown remote refs must not be fetched"
        );
    }
}
