#!/usr/bin/env python3
import argparse
import pathlib
import struct
import subprocess
import sys


def run(command, *, input_bytes=None):
    result = subprocess.run(
        command,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        sys.stderr.buffer.write(result.stdout)
        sys.stderr.buffer.write(result.stderr)
        raise SystemExit(
            f"command failed with exit code {result.returncode}: {' '.join(map(str, command))}"
        )
    return result


def filecheck(tool, check_file, input_bytes):
    run([tool, str(check_file)], input_bytes=input_bytes)


def executable_result_shapes(artifact):
    if not artifact.startswith(b"AWENEXE\0\x01\x00\x00\x00"):
        raise SystemExit("awen-compile emitted an invalid executable header")
    offset = 12

    def read(fmt):
        nonlocal offset
        size = struct.calcsize(fmt)
        value = struct.unpack_from(fmt, artifact, offset)[0]
        offset += size
        return value

    def skip_string():
        nonlocal offset
        length = read("<H")
        offset += length

    skip_string()  # backend
    command_count = read("<I")
    shapes = []
    for _ in range(command_count):
        if read("<B") != 1:
            raise SystemExit("awen-compile emitted an unknown command kind")
        read("<I")  # tile_m
        read("<I")  # tile_n
        read("<I")  # tile_k
        read("<H")  # minimum_effective_bits
        skip_string()  # calibration
        skip_string()  # layout
        rank = read("<B")
        shapes.append(tuple(read("<q") for _ in range(rank)))
    return shapes


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--awen-opt", required=True)
    parser.add_argument("--awen-compile", required=True)
    parser.add_argument("--filecheck", required=True)
    parser.add_argument("--source-dir", type=pathlib.Path, required=True)
    parser.add_argument("--output-dir", type=pathlib.Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)

    stablehlo = args.source_dir / "stablehlo_gemm.mlir"
    lowered = run(
        [args.awen_opt, str(stablehlo), "--awen-lower-stablehlo-to-device"]
    ).stdout
    lowered_again = run(
        [args.awen_opt, str(stablehlo), "--awen-lower-stablehlo-to-device"]
    ).stdout
    if lowered != lowered_again:
        raise SystemExit("StableHLO-to-Device lowering is not deterministic")
    filecheck(args.filecheck, stablehlo, lowered)

    dynamic = args.source_dir / "dynamic_gemm.mlir"
    imported = run(
        [args.awen_opt, str(dynamic), "--awen-import-stablehlo"]
    ).stdout
    filecheck(args.filecheck, dynamic, imported)

    complex_gemm = args.source_dir / "complex_gemm.mlir"
    complex_imported = run(
        [args.awen_opt, str(complex_gemm), "--awen-import-stablehlo"]
    ).stdout
    filecheck(args.filecheck, complex_gemm, complex_imported)

    source_location = args.source_dir / "source_location.mlir"
    located = run(
        [
            args.awen_opt,
            str(source_location),
            "--awen-lower-stablehlo-to-device",
            "--mlir-print-debuginfo",
        ]
    ).stdout
    filecheck(args.filecheck, source_location, located)

    batched = args.source_dir / "batched_gemm.mlir"
    batched_lowered = run(
        [args.awen_opt, str(batched), "--awen-lower-stablehlo-to-device"]
    ).stdout
    batched_lowered_again = run(
        [args.awen_opt, str(batched), "--awen-lower-stablehlo-to-device"]
    ).stdout
    if batched_lowered != batched_lowered_again:
        raise SystemExit("batched StableHLO-to-Device lowering is not deterministic")
    filecheck(args.filecheck, batched, batched_lowered)

    for invalid_batch in (
        "invalid_batch_dimensions.mlir",
        "invalid_batch_contracting.mlir",
        "invalid_batch_shape.mlir",
    ):
        run(
            [
                args.awen_opt,
                str(args.source_dir / invalid_batch),
                "--awen-import-stablehlo",
                "--verify-diagnostics",
            ]
        )
    run(
        [
            args.awen_opt,
            str(args.source_dir / "invalid_tensor.mlir"),
            "--verify-diagnostics",
        ]
    )
    run(
        [
            args.awen_opt,
            str(args.source_dir / "unsupported_operation.mlir"),
            "--awen-import-stablehlo",
            "--verify-diagnostics",
        ]
    )
    run(
        [
            args.awen_opt,
            str(args.source_dir / "dialect_roundtrip.mlir"),
            "--verify-roundtrip",
        ]
    )
    run(
        [
            args.awen_opt,
            str(args.source_dir / "invalid_qphotonic_state.mlir"),
            "--verify-diagnostics",
        ]
    )
    run(
        [
            args.awen_opt,
            str(args.source_dir / "invalid_qphotonic_contract.mlir"),
            "--verify-diagnostics",
        ]
    )

    all_dialects_bytecode = args.output_dir / "all_dialects.mlirbc"
    run(
        [
            args.awen_opt,
            str(args.source_dir / "dialect_roundtrip.mlir"),
            "--emit-bytecode",
            "-o",
            str(all_dialects_bytecode),
        ]
    )
    all_dialects_output = run([args.awen_opt, str(all_dialects_bytecode)]).stdout
    for expected in (
        b"!awen_tensor.handle",
        b"!awen_photonic.optical_tile",
        b"awen_photonic.modulate",
        b"!awen_qphotonic.state",
        b"!awen_qphotonic.fock_state",
        b"!awen_qphotonic.gaussian_state",
        b"awen_qphotonic.photon_count",
        b"awen_qphotonic.homodyne_p",
        b"awen_qphotonic.feed_forward_phase",
        b"awen_qphotonic.feed_forward_displacement_q",
        b"awen_qphotonic.feed_forward_displacement_p",
        b"awen_qphotonic.feed_forward_squeezing",
        b"!awen_device.command_buffer",
        b"awen_tensor.gemm",
    ):
        if expected not in all_dialects_output:
            raise SystemExit(f"bytecode round trip lost {expected.decode()}")

    bytecode = args.output_dir / "device.mlirbc"
    run(
        [
            args.awen_opt,
            str(stablehlo),
            "--awen-lower-stablehlo-to-device",
            "--emit-bytecode",
            "-o",
            str(bytecode),
        ]
    )
    bytecode_output = run([args.awen_opt, str(bytecode)]).stdout
    if b"awen_device.execute_gemm" not in bytecode_output:
        raise SystemExit("device operation was not preserved by bytecode round trip")

    executable = args.output_dir / "stablehlo_gemm.awenx"
    run([args.awen_compile, str(stablehlo), "-o", str(executable)])
    artifact = executable.read_bytes()
    if executable_result_shapes(artifact) != [(256, 64)]:
        raise SystemExit("rank-two executable lost its result shape")
    second_executable = args.output_dir / "stablehlo_gemm_second.awenx"
    run([args.awen_compile, str(stablehlo), "-o", str(second_executable)])
    if artifact != second_executable.read_bytes():
        raise SystemExit("awen-compile output is not deterministic")

    batched_executable = args.output_dir / "batched_gemm.awenx"
    run([args.awen_compile, str(batched), "-o", str(batched_executable)])
    batched_artifact = batched_executable.read_bytes()
    if executable_result_shapes(batched_artifact) != [
        (2, 16, 8),
        (-1, -1, -1),
    ]:
        raise SystemExit("batched executable lost its rank-three result shapes")
    batched_executable_again = args.output_dir / "batched_gemm_second.awenx"
    run([args.awen_compile, str(batched), "-o", str(batched_executable_again)])
    if batched_artifact != batched_executable_again.read_bytes():
        raise SystemExit("batched awen-compile output is not deterministic")


if __name__ == "__main__":
    main()
