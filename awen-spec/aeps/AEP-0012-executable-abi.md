# AEP-0012: AWEN executable ABI 1.0

Status: Accepted and implemented

## Purpose

`AWENEXE` is the stable compiler/runtime boundary. It carries a compact device
command table that the runtime consumes directly and embeds the versioned MLIR
Device IR bytecode for diagnostics, provenance, inspection, and future
recompilation. Runtime loading never parses compiler JSON and never shells out
to an MLIR tool.

## Little-endian binary layout

```text
offset  field
0       8 bytes: "AWENEXE\0"
8       u16 ABI major
10      u16 ABI minor
12      u16 backend identifier byte length
14      UTF-8 backend identifier
...     u32 command count
...     repeated command records
...     u32 MLIR bytecode byte length
...     MLIR bytecode
```

The v1 `execute_gemm` command record is:

```text
u8   command kind = 1
u32  tile M
u32  tile N
u32  tile K
u16  minimum effective bits
u16  calibration-handle byte length
...  UTF-8 calibration handle
u16  tensor-layout byte length
...  UTF-8 tensor layout (`row_major` or `column_major`)
u8   result rank
...  rank signed i64 dimensions; -1 means dynamic
```

## Validation

The runtime rejects invalid magic, unsupported major versions, truncated
fields, invalid UTF-8, unknown commands, zero tile dimensions, zero precision,
invalid result dimensions, missing commands, invalid MLIR bytecode magic, and
trailing bytes. ABI 1 readers accept compatible 1.x minor versions. Major
versions require an explicit runtime decoder.
