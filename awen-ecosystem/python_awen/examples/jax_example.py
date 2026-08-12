import jax
import jax.numpy as jnp

from awen_py import export_jax_value_and_grad


def loss(left, right):
    return jnp.sum((left @ right) ** 2)


left = jnp.arange(6.0, dtype=jnp.float32).reshape(2, 3)
right = jnp.arange(12.0, dtype=jnp.float32).reshape(3, 4)
program = export_jax_value_and_grad(
    loss,
    left,
    right,
    argnums=(0, 1),
    polymorphic_shapes=("(batch, 3)", "(3, 4)"),
)
value, gradients = program(left, right)
print(value)
print(jax.tree.map(jnp.shape, gradients))
print(program.report.to_json())
