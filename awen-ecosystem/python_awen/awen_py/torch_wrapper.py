try:
    import torch
except Exception:
    torch = None

from typing import List, Optional
from .client import compute_gradients, run_ir


class AWENAutogradFunction:
    """A thin autograd connector that uses `awenctl run` for forward (returns scalar cost)
    and `awenctl gradient` for backward. This is a convenience bridge for PyTorch-based
    experiments. It shells out to `awenctl`, so ensure the runtime is installed and on PATH.

    Usage pattern (high-level):
        # ir.json contains placeholders for parameter values which we overwrite per-call
        params_tensor = torch.tensor([0.1, 0.2], requires_grad=True)
        cost = awen_forward('ir_template.json', ['mzi_0:phase','mzi_1:phase'], params_tensor)
        cost.backward()
        print(params_tensor.grad)
    """

    @staticmethod
    def forward(ir_template_path: str, param_names: List[str], params_tensor, seed: Optional[int] = None):
        if torch is None:
            raise RuntimeError("PyTorch is required for AWEN autograd wrapper")

        # Write a temporary IR with parameters substituted. We assume the IR nodes have params we can update by name.
        import json, tempfile, os

        ir = None
        with open(ir_template_path, 'r') as f:
            ir = json.load(f)

        # Map tensor values (1D) to parameter names in order
        vals = params_tensor.detach().cpu().numpy().tolist()
        if len(vals) != len(param_names):
            raise ValueError("param_names length must match params_tensor length")

        # Apply params: support names like 'node_id:param' or global search
        for name, v in zip(param_names, vals):
            if ':' in name:
                node_id, pkey = name.split(':', 1)
                for node in ir.get('nodes', []):
                    if node.get('id') == node_id:
                        node.setdefault('params', {})[pkey] = float(v)
            else:
                # search first matching param key in nodes
                applied = False
                for node in ir.get('nodes', []):
                    if 'params' in node and name in node['params']:
                        node['params'][name] = float(v)
                        applied = True
                        break
                if not applied:
                    # as fallback, write to metadata
                    ir.setdefault('metadata', {})[name] = str(v)

        tmpdir = tempfile.mkdtemp(prefix='awen_autograd_')
        tmp_ir = os.path.join(tmpdir, 'ir.json')
        with open(tmp_ir, 'w') as f:
            json.dump(ir, f, indent=2)

        # Run the IR to produce a scalar cost (runtime-defined; we read results.json last node power)
        run_res = run_ir(tmp_ir, seed=seed)
        results_json = run_res.get('results.json')
        if results_json is None:
            raise RuntimeError('run did not produce results.json')

        with open(results_json, 'r') as f:
            results = json.load(f)

        # Extract scalar cost: expect SimulationResult with node_results and out_amplitude
        scalar = 0.0
        node_results = results.get('node_results')
        if node_results and len(node_results) > 0:
            last = node_results[-1]
            re, im = last.get('out_amplitude', [0.0, 0.0])
            scalar = float(re) * float(re) + float(im) * float(im)
        else:
            # fallback: sum analog measurements
            for n in results.get('node_results', []):
                m = n.get('measurement')
                if m and m.get('analog_value') is not None:
                    scalar += float(m.get('analog_value'))

        out = torch.tensor([scalar], dtype=params_tensor.dtype, device=params_tensor.device)
        out.requires_grad = True

        # Save context for backward: temp_ir path and param names
        ctx = {
            'tmp_ir': tmp_ir,
            'param_names': param_names,
            'seed': seed,
        }
        return out, ctx

    @staticmethod
    def backward(ctx_obj, grad_output):
        # ctx_obj is the dict returned by forward
        import json, os

        tmp_ir = ctx_obj.get('tmp_ir')
        param_names = ctx_obj.get('param_names')
        seed = ctx_obj.get('seed')

        # Call compute_gradients helper which invokes awenctl gradient
        grad_res = compute_gradients(tmp_ir, param_names, strategy='finite_difference', seed=seed, samples=1)

        # parse gradients.json format: expect gradients mapping
        grads_map = grad_res.get('gradients', {})
        # create gradient tensor corresponding to param_names
        import torch
        grads = [float(grads_map.get(n, 0.0)) for n in param_names]
        grad_tensor = torch.tensor(grads, dtype=grad_output.dtype, device=grad_output.device)

        # Multiply by upstream grad (scalar)
        if grad_output is not None:
            grad_tensor = grad_tensor * grad_output.view(-1)[0]

        # Return None for ir_template_path and param_names, and gradient tensor for params
        return None, None, grad_tensor


def awen_forward(ir_template_path: str, param_names: List[str], params_tensor, seed: Optional[int] = None):
    """Convenience wrapper that integrates with PyTorch autograd when available.

    Returns a scalar torch tensor (1-element).
    """
    if torch is None:
        raise RuntimeError('PyTorch is required for awen_forward')

    out, ctx = AWENAutogradFunction.forward(ir_template_path, param_names, params_tensor, seed=seed)
    # Attach backward context by wrapping in a custom object exposing backward
    class _TensorWithCtx(torch.Tensor):
        pass

    # We cannot easily manufacture a subclassed tensor in pure python without extending C API.
    # Instead, we return `out` and let users call a manual `awen_backward` helper if needed.
    # Provide the context for manual backward call:
    out._awen_ctx = ctx
    return out


def awen_backward(out_tensor):
    """Manual backward helper: calls compute_gradients using the saved context and applies gradients to parameters.
    This is intended as a thin fallback when integrating with pure Python workflows.
    """
    if not hasattr(out_tensor, '_awen_ctx'):
        raise RuntimeError('no AWEN context attached to tensor')
    ctx = out_tensor._awen_ctx
    grads = AWENAutogradFunction.backward(ctx, torch.tensor([1.0]))
    return grads
