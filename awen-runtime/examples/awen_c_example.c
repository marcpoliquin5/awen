/* Compiled and run in CI (see the "C SDK example" step). Prints three lines
 * that the workflow checks: the ABI version, a complex GEMM result and a
 * kernel executed from JSON through the reference and simulator paths. */
#include <awen/awen.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int fail(const char *what) {
    char message[512];
    awen_last_error_message(message, sizeof message);
    fprintf(stderr, "%s failed: %s\n", what, message);
    return 1;
}

static char *execute(const char *request, uint32_t mode) {
    size_t required = 0;
    if (awen_kernel_execute_json(request, mode, 12, 0.0, 7, NULL, 0, &required) !=
        AWEN_STATUS_BUFFER_TOO_SMALL) {
        return NULL;
    }
    char *json = malloc(required);
    if (json == NULL ||
        awen_kernel_execute_json(request, mode, 12, 0.0, 7, json, required, &required) !=
            AWEN_STATUS_OK) {
        free(json);
        return NULL;
    }
    return json;
}

int main(void) {
    printf("abi %s %s\n", awen_framework_abi_version(), awen_kernel_abi_version());

    /* (1+2i)(3-1i) = 5+5i ; (1+2i)(0+1i) = -2+1i */
    const double lhs[2] = {1.0, 2.0};
    const double rhs[4] = {3.0, -1.0, 0.0, 1.0};
    double out[4] = {0};
    if (awen_cgemm_f64(lhs, 2, rhs, 4, out, 4, 1, 2, 1) != AWEN_STATUS_OK) {
        return fail("awen_cgemm_f64");
    }
    printf("cgemm %g %g %g %g\n", out[0], out[1], out[2], out[3]);

    if (awen_cgemm_f64(lhs, 3, rhs, 4, out, 4, 1, 2, 1) != AWEN_STATUS_INVALID_ARGUMENT) {
        fprintf(stderr, "length validation did not trigger\n");
        return 1;
    }

    size_t required = 0;
    awen_kernel_kinds(NULL, 0, &required);
    char *kinds = malloc(required);
    if (kinds == NULL || awen_kernel_kinds(kinds, required, NULL) != AWEN_STATUS_OK) {
        return fail("awen_kernel_kinds");
    }
    printf("kinds-has-fft %d\n", strstr(kinds, "fft") != NULL);
    free(kinds);

    const char *request =
        "{\"version\":\"awen.blas.v1\",\"id\":\"c-example\",\"kind\":\"gemm\","
        "\"inputs\":["
        "{\"id\":\"a\",\"shape\":[2,2],\"dtype\":\"f32\",\"layout\":\"row_major\","
        "\"data\":{\"representation\":\"real\",\"values\":[1,2,3,4]}},"
        "{\"id\":\"b\",\"shape\":[2,2],\"dtype\":\"f32\",\"layout\":\"row_major\","
        "\"data\":{\"representation\":\"real\",\"values\":[5,6,7,8]}}]}";
    char *reference = execute(request, AWEN_KERNEL_MODE_REFERENCE);
    if (reference == NULL) {
        return fail("reference execution");
    }
    char *simulated = execute(request, AWEN_KERNEL_MODE_SIMULATOR);
    if (simulated == NULL) {
        return fail("simulator execution");
    }
    printf("reference-has-19-22-43-50 %d simulated-flag %d\n",
           strstr(reference, "[19.0,22.0,43.0,50.0]") != NULL,
           strstr(simulated, "\"simulated\":true") != NULL);
    free(reference);
    free(simulated);

    if (awen_kernel_execute_json("{not json", 0, 8, 0.0, 0, NULL, 0, NULL) !=
        AWEN_STATUS_INVALID_ARGUMENT) {
        fprintf(stderr, "bad JSON was accepted\n");
        return 1;
    }
    return 0;
}
