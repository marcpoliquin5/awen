import torch

from awen_py import awen
from awen_py.torch_backend import get_last_compile_report


class Model(torch.nn.Module):
    def __init__(self):
        super().__init__()
        self.projection = torch.nn.Linear(4, 3)

    def forward(self, left, right):
        return torch.relu(self.projection(left @ right))


model = torch.compile(Model(), backend=awen, dynamic=True)
left = torch.randn(8, 2, requires_grad=True)
right = torch.randn(2, 4, requires_grad=True)
output = model(left, right)
output.sum().backward()
print(output)
print(get_last_compile_report().to_json())
