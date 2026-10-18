import pytest
import torch

torch.set_num_threads(2)

CORPUS = (
    "To be, or not to be, that is the question:\n"
    "Whether 'tis nobler in the mind to suffer\n"
    "The slings and arrows of outrageous fortune,\n"
    "Or to take arms against a sea of troubles\n"
) * 40


@pytest.fixture
def corpus() -> str:
    return CORPUS
