import pytest

from awen_py import NumericalContract, awen
from awen_py.torch_backend import get_last_compile_report, get_last_execution_traces
from awen_py.torch_wrapper import awen_backward, awen_forward

torch = pytest.importorskip("torch")


class MixedModel(torch.nn.Module):
    def __init__(self):
        super().__init__()
        self.linear = torch.nn.Linear(4, 5)

    def forward(self, left, right):
        supported = left @ right
        projected = self.linear(supported)
        return torch.relu(projected)  # Relu is the eager fallback region.


def test_installed_backend_entry_point_works_by_name():
    assert "awen" in torch._dynamo.list_backends()
    compiled = torch.compile(lambda left, right: left @ right, backend="awen")
    torch.testing.assert_close(compiled(torch.eye(2), torch.eye(2)), torch.eye(2))


def test_torch_compile_mixed_regions_outputs_and_analytic_gradients():
    torch.manual_seed(7)
    eager = MixedModel()
    compiled_model = MixedModel()
    compiled_model.load_state_dict(eager.state_dict())
    compiled = torch.compile(compiled_model, backend=awen)

    left_eager = torch.randn(3, 2, requires_grad=True)
    right_eager = torch.randn(2, 4, requires_grad=True)
    left_compiled = left_eager.detach().clone().requires_grad_()
    right_compiled = right_eager.detach().clone().requires_grad_()
    expected = eager(left_eager, right_eager)
    actual = compiled(left_compiled, right_compiled)
    torch.testing.assert_close(actual, expected, atol=1.0e-5, rtol=1.0e-5)
    expected.sum().backward()
    actual.sum().backward()
    torch.testing.assert_close(left_compiled.grad, left_eager.grad)
    torch.testing.assert_close(right_compiled.grad, right_eager.grad)
    torch.testing.assert_close(compiled_model.linear.weight.grad, eager.linear.weight.grad)

    report = get_last_compile_report()
    assert report is not None
    assert report.supported_nodes
    assert report.fallback_nodes
    assert report.shape_guard_owner == "torch_dynamo"
    assert any(item.action == "framework_fallback" for item in report.diagnostics)
    assert get_last_execution_traces()


def test_dynamic_shapes_noncontiguous_and_batched_gemm():
    def function(left, right):
        return torch.bmm(left, right).sin()

    compiled = torch.compile(function, backend=awen, dynamic=True)
    for batch in (2, 5):
        raw = torch.randn(batch, 4, 6)
        left = raw.transpose(-2, -1)  # [batch, 6, 4], non-contiguous
        right = torch.randn(batch, 4, 3)
        torch.testing.assert_close(compiled(left, right), function(left, right))


def test_actionable_strict_fallback_error_and_exception_propagation():
    def unsupported(value):
        return torch.sin(value)

    def strict_backend(graph, inputs, **kwargs):
        return awen(graph, inputs, options={"allow_fallback": False})

    compiled = torch.compile(unsupported, backend=strict_backend)
    # TorchDynamo wraps backend exceptions while retaining the actionable cause.
    with pytest.raises(Exception, match="sin"):
        compiled(torch.ones(2))

    compiled_matmul = torch.compile(lambda left, right: left @ right, backend=awen)
    with pytest.raises(RuntimeError):
        compiled_matmul(torch.ones(2, 3), torch.ones(4, 2))


def test_standard_tensor_serialization_and_contract_object():
    compiled = torch.compile(lambda left, right: left @ right, backend=awen)
    output = compiled(torch.eye(2), torch.arange(4.0).reshape(2, 2))
    buffer = __import__("io").BytesIO()
    torch.save(output, buffer)
    buffer.seek(0)
    torch.testing.assert_close(torch.load(buffer), output)
    NumericalContract(max_abs_error=1.0e-5, max_rel_error=1.0e-5).validate()


def test_compatibility_wrapper_uses_live_analytic_autograd():
    left = torch.randn(2, 3, requires_grad=True)
    right = torch.randn(3, 4, requires_grad=True)
    output = awen_forward(left, right)
    gradients = awen_backward(output, (left, right))
    expected_left, expected_right = torch.autograd.grad(
        left @ right, (left, right), grad_outputs=torch.ones_like(output)
    )
    torch.testing.assert_close(gradients[0], expected_left)
    torch.testing.assert_close(gradients[1], expected_right)
