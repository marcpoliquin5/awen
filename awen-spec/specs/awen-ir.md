# AWEN legacy graph IR

The bootstrap graph contract is defined by `../schemas/awen_ir.proto`. A graph
has an ID, typed nodes, named inputs, parameter maps, and metadata. Runtimes must
reject unknown node kinds or malformed references instead of inventing
semantics.

```json
{
  "id": "example",
  "nodes": [
    {
      "id": "mzi_0",
      "kind": "Mzi",
      "inputs": ["input_0"],
      "parameters": {"phase": 0.25}
    }
  ],
  "metadata": {"author": "AWEN contributors"}
}
```

The complete checked fixture is `../../awen-runtime/example_ir.json`.
Production compiler lowering uses the registered MLIR dialects from AEP-0010;
the JSON graph remains a legacy runtime and semantic-reference surface.
