#ifndef AWEN_AWEN_H
#define AWEN_AWEN_H

/*
 * AWEN C SDK header: the dense framework ABI (framework.h) plus the awenBLAS
 * kernel ABI. Link against libawen_runtime (cdylib or staticlib).
 *
 * Conventions:
 *  - every buffer is caller-owned and only borrowed for the call;
 *  - functions return awen_status; on failure awen_last_error_message()
 *    describes the error for the calling thread;
 *  - variable-length outputs take (output, output_length, required): call
 *    once with output = NULL to learn the byte count (including NUL), which
 *    returns AWEN_STATUS_BUFFER_TOO_SMALL, then call again.
 *
 * Execution through this ABI is the CPU reference (mode 0) or the photonic
 * noise-model simulator (mode 1). Neither is a hardware measurement.
 */

#include <stddef.h>
#include <stdint.h>

#include "framework.h"

#ifdef __cplusplus
extern "C" {
#endif

#define AWEN_KERNEL_MODE_REFERENCE 0u
#define AWEN_KERNEL_MODE_SIMULATOR 1u

/* Returns "awen.kernel-c.v1". */
AWEN_FRAMEWORK_API const char *awen_kernel_abi_version(void);

/* Comma-separated awenBLAS kernel kind names (snake_case). */
AWEN_FRAMEWORK_API awen_status awen_kernel_kinds(
    char *output,
    size_t output_length,
    size_t *required);

/* Row-major complex GEMM on interleaved (re, im) doubles.
 * lhs_length == 2*m*k, rhs_length == 2*k*n, output_length >= 2*m*n. */
AWEN_FRAMEWORK_API awen_status awen_cgemm_f64(
    const double *lhs,
    size_t lhs_length,
    const double *rhs,
    size_t rhs_length,
    double *output,
    size_t output_length,
    size_t m,
    size_t n,
    size_t k);

/* Execute an awen.blas.v1 request (JSON, NUL-terminated) and write the
 * KernelResult JSON. effective_bits/noise_fraction/seed apply to
 * AWEN_KERNEL_MODE_SIMULATOR only. */
AWEN_FRAMEWORK_API awen_status awen_kernel_execute_json(
    const char *request_json,
    uint32_t mode,
    uint8_t effective_bits,
    double noise_fraction,
    uint64_t seed,
    char *output,
    size_t output_length,
    size_t *required);

#ifdef __cplusplus
}
#endif

#endif
