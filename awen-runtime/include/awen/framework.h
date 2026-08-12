#ifndef AWEN_FRAMEWORK_H
#define AWEN_FRAMEWORK_H

#include <stddef.h>

#if defined(_WIN32)
#  if defined(AWEN_FRAMEWORK_BUILD)
#    define AWEN_FRAMEWORK_API __declspec(dllexport)
#  else
#    define AWEN_FRAMEWORK_API __declspec(dllimport)
#  endif
#else
#  define AWEN_FRAMEWORK_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef enum awen_status {
    AWEN_STATUS_OK = 0,
    AWEN_STATUS_INVALID_ARGUMENT = 1,
    AWEN_STATUS_BUFFER_TOO_SMALL = 2,
    AWEN_STATUS_UNSUPPORTED = 3,
    AWEN_STATUS_INTERNAL_ERROR = 4
} awen_status;

AWEN_FRAMEWORK_API const char *awen_framework_abi_version(void);

/* Returns the required byte count including NUL and copies as much as fits. */
AWEN_FRAMEWORK_API size_t awen_last_error_message(char *output, size_t output_length);

/* Row-major contiguous buffers remain caller-owned for the entire call. */
AWEN_FRAMEWORK_API awen_status awen_gemm_f64(
    const double *lhs,
    size_t lhs_length,
    const double *rhs,
    size_t rhs_length,
    double *output,
    size_t output_length,
    size_t m,
    size_t n,
    size_t k);

AWEN_FRAMEWORK_API awen_status awen_gemm_f32(
    const float *lhs,
    size_t lhs_length,
    const float *rhs,
    size_t rhs_length,
    float *output,
    size_t output_length,
    size_t m,
    size_t n,
    size_t k);

#ifdef __cplusplus
}
#endif

#endif
