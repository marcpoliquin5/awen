#!/usr/bin/env python3
"""Parse repository JSON, meta-validate schemas, and validate checked instances."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

from jsonschema import validators
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[1]
listed = subprocess.run(
    ["git", "ls-files", "--cached", "--others", "--exclude-standard", "*.json"],
    cwd=ROOT,
    check=True,
    capture_output=True,
    text=True,
).stdout.splitlines()
json_paths = sorted(ROOT / name for name in listed if (ROOT / name).is_file())
documents = {
    path.relative_to(ROOT).as_posix(): json.loads(path.read_text(encoding="utf-8"))
    for path in json_paths
}

schemas = {
    name: document
    for name, document in documents.items()
    if name.startswith("awen-spec/schemas/")
}
registry = Registry()
for name, schema in schemas.items():
    validator_type = validators.validator_for(schema)
    validator_type.check_schema(schema)
    resource = Resource.from_contents(schema)
    if "$id" in schema:
        registry = registry.with_resource(schema["$id"], resource)
    filename = Path(name).name
    registry = registry.with_resource(filename, resource)
    registry = registry.with_resource(f"https://awen.dev/schemas/{filename}", resource)

instances = {
    "awen-compiler/capabilities/pace_like_128.json": "awen_device_capability.v1.json",
    "awen-compiler/capabilities/reference_2x2.json": "awen_device_capability.v1.json",
    "awen-compiler/capabilities/pace_like_128.health.json": "awen_backend_health.v1.json",
    "awen-compiler/capabilities/reference_2x2.health.json": "awen_backend_health.v1.json",
    "awen-compiler/cost_models/reference_full_system.json": "awen_cost_model.v1.json",
    "awen-compiler/cost_models/reference_observations.json": "awen_cost_observations.v1.json",
    "awen-compiler/kernels/transformer_qkv.json": "awen_blas.v1.json",
    "awen-compiler/kernels/reference_kernel_backends.json": "awen_blas_backend.v1.json",
    "awen-runtime/plugins/reference_sim/backend-manifest.json": "awen_plugin_manifest.v1.json",
    "awen-runtime/plugins/reference_sim/health.json": "awen_backend_health.v1.json",
    "awen-ecosystem/pdks/example_silicon_pdk.json": "awen_physical_design.v1.json",
    "awen-spec/fixtures/classical_photonic_program.json": "awen_photonic_program.v1.json",
    "awen-spec/fixtures/photonic_interop_program.json": "awen_photonic_interop.v1.json",
    "awen-spec/fixtures/physical_design_mapping_request.v1.json": "awen_physical_design.v1.json",
    "awen-spec/fixtures/quantum_photonic_program.json": "awen_qphotonic_program.v1.json",
    "awen-spec/fixtures/quantum_photonic_result.json": "awen_qphotonic_result.v1.json",
    "benchmarks/reference_hil_suite.json": "awen_hil_suite.v1.json",
}

for instance_name, schema_file in instances.items():
    schema_name = f"awen-spec/schemas/{schema_file}"
    schema = schemas[schema_name]
    validator_type = validators.validator_for(schema)
    validator = validator_type(schema, registry=registry)
    instance = documents[instance_name]
    if isinstance(instance, list) and schema.get("type") == "object":
        for item in instance:
            validator.validate(item)
    else:
        validator.validate(instance)

print(
    f"parsed {len(documents)} JSON documents, meta-validated {len(schemas)} schemas, "
    f"and validated {len(instances)} checked instances"
)
