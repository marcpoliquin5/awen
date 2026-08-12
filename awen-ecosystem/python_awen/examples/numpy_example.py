import numpy as np

from awen_py import ExecutionOptions, InProcessRuntime, gemm


left = np.arange(24.0).reshape(4, 6)[:, ::2]
right = np.arange(15.0).reshape(3, 5)
print(gemm(left, right))

with InProcessRuntime() as runtime:
    future = runtime.execute(
        "gemm",
        left,
        right,
        options=ExecutionOptions(asynchronous=True, seed=42),
    )
    result = future.wait()
    replay = runtime.replay(result.plan, left, right)
    print(result.trace.to_dict())
    print(replay.outputs)
