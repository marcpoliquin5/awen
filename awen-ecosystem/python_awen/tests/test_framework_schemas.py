import json
from pathlib import Path

import numpy as np
import pytest

from awen_py import InProcessRuntime

jsonschema = pytest.importorskip("jsonschema")


SCHEMAS = Path(__file__).parents[3] / "awen-spec" / "schemas"


def test_serialized_plan_and_trace_validate_against_normative_schemas():
    runtime = InProcessRuntime()
    value = np.eye(2)
    result = runtime.execute_with_trace("gemm", value, value)
    plan_schema = json.loads((SCHEMAS / "awen_framework_plan.v1.json").read_text())
    trace_schema = json.loads((SCHEMAS / "awen_framework_trace.v1.json").read_text())
    jsonschema.Draft202012Validator.check_schema(plan_schema)
    jsonschema.Draft202012Validator.check_schema(trace_schema)
    jsonschema.validate(json.loads(result.plan.to_json()), plan_schema)
    jsonschema.validate(json.loads(result.trace.to_json()), trace_schema)
