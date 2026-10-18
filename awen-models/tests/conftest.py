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


@pytest.fixture
def tiny_llama_dir(tmp_path):
    from transformers import LlamaConfig, LlamaForCausalLM, PreTrainedTokenizerFast
    from tokenizers import Tokenizer, models, pre_tokenizers

    config = LlamaConfig(vocab_size=64, hidden_size=32, intermediate_size=64, num_hidden_layers=2,
                         num_attention_heads=4, num_key_value_heads=2, max_position_embeddings=64,
                         tie_word_embeddings=False)
    model = LlamaForCausalLM(config)
    model.save_pretrained(tmp_path, safe_serialization=True)
    vocab = {chr(97 + i): i for i in range(26)}
    vocab.update({"<unk>": 26, "<s>": 27, "</s>": 28, " ": 29})
    backend = Tokenizer(models.WordLevel(vocab=vocab, unk_token="<unk>"))
    backend.pre_tokenizer = pre_tokenizers.Split("", "isolated")
    tokenizer = PreTrainedTokenizerFast(tokenizer_object=backend, unk_token="<unk>", bos_token="<s>", eos_token="</s>")
    tokenizer.save_pretrained(tmp_path)
    return tmp_path
