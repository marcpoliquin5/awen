"""Benchmark harness: statistics, evidence sealing, write-once files, schema."""

import json
from pathlib import Path

import numpy as np
import pytest

from awen_models import bench

REPO = Path(__file__).resolve().parents[2]


def test_percentiles_match_numpy():
    data = list(range(1, 101))
    assert bench.percentile(data, 50) == pytest.approx(50.5)
    assert bench.percentile(data, 99) == pytest.approx(np.percentile(data, 99))


def test_bootstrap_ci_brackets_statistic_and_is_seeded():
    rng = np.random.default_rng(3)
    data = rng.normal(10.0, 1.0, size=200)
    lo, hi = bench.bootstrap_ci(data, np.mean, seed=1)
    assert lo < data.mean() < hi
    assert hi - lo < 0.5
    assert (lo, hi) == bench.bootstrap_ci(data, np.mean, seed=1)
    assert bench.bootstrap_ci([2.0]) == (2.0, 2.0)
    with pytest.raises(ValueError):
        bench.bootstrap_ci([])


def test_latency_summary_units_and_ordering():
    runs = [[0.001, 0.002, 0.003], [0.002, 0.004, 0.010]]
    s = bench.latency_summary(runs)
    assert s["samples"] == 6 and s["runs"] == 2
    assert s["p50_ms"] <= s["p95_ms"] <= s["p99_ms"] <= 10.0
    assert s["median_ci95_ms"][0] <= s["p50_ms"] <= s["median_ci95_ms"][1]


def test_seal_verify_detects_tampering():
    record = bench.seal({"version": bench.EVIDENCE_VERSION, "value": 1.0})
    assert bench.verify(record)
    record["value"] = 2.0
    assert not bench.verify(record)


def test_write_once_refuses_overwrite(tmp_path):
    record = bench.seal({"created_at": "2026-10-18T10:00:00Z", "x": 1})
    path = bench.write_once(record, tmp_path)
    assert path.name.startswith("evidence-2026-10-18-")
    with pytest.raises(FileExistsError):
        bench.write_once(record, tmp_path)


def _validator():
    pytest.importorskip("jsonschema")
    from jsonschema import validators
    from referencing import Registry, Resource

    schemas = REPO / "awen-spec/schemas"
    registry = Registry()
    for name in ("awen_benchmark_claims.v1.json", "awen_benchmark_evidence.v1.json"):
        doc = json.loads((schemas / name).read_text())
        registry = registry.with_resource(name, Resource.from_contents(doc))
        registry = registry.with_resource(doc["$id"], Resource.from_contents(doc))
    schema = json.loads((schemas / "awen_benchmark_evidence.v1.json").read_text())
    return validators.validator_for(schema)(schema, registry=registry)


def test_small_run_all_backends_matches_schema(corpus):
    pytest.importorskip("awen_native")
    record = bench.run(runs=2, repeats=2, warmup=0, new_tokens=2, windows=2, shapes=((8, 16, 16),),
                       corpus=corpus)
    assert bench.verify(record)
    _validator().validate(record)
    rows = {(r["workload"], r["backend"]): r for r in record["results"]}
    assert ("gemm", "hybrid") not in rows
    for workload in ("gemm", "awen-mini-inference"):
        assert rows[(workload, "cpu")]["fidelity"] == "measured"
        if rows[(workload, "cuda")]["status"] == "skipped":
            assert "CUDA" in rows[(workload, "cuda")]["reason"]
    gemm_ref = rows[("gemm", "awen-reference")]
    assert gemm_ref["relative_rms_error_vs_fp64"]["max"] < 1e-6
    assert rows[("gemm", "photonic-sim")]["relative_rms_error_vs_fp64"]["mean"] > 0
    assert "compile" in rows[("gemm", "photonic-sim")]
    cpu_ppl = rows[("awen-mini-inference", "cpu")]["perplexity"]["mean"]
    ref_ppl = rows[("awen-mini-inference", "awen-reference")]["perplexity"]["mean"]
    assert ref_ppl == pytest.approx(cpu_ppl, rel=1e-4)
    assert rows[("awen-mini-inference", "hybrid")]["placement"].startswith("ffn")


def test_schema_rejects_measured_photonic_row():
    record = {"workload": "gemm", "backend": "photonic-sim", "status": "ok", "fidelity": "measured"}
    schema = json.loads((REPO / "awen-spec/schemas/awen_benchmark_evidence.v1.json").read_text())
    jsonschema = pytest.importorskip("jsonschema")
    Draft202012Validator = jsonschema.Draft202012Validator

    v = Draft202012Validator({"$ref": "#/$defs/result", "$defs": schema["$defs"]})
    assert not v.is_valid(record)


def test_committed_evidence_verifies():
    files = sorted((REPO / "benchmarks/results").glob("evidence-*.json"))
    for path in files:
        record = json.loads(path.read_text())
        assert bench.verify(record), path
        _validator().validate(record)
