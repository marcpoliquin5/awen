#ifndef AWEN_FRAMEWORK_HPP
#define AWEN_FRAMEWORK_HPP

#include "framework.h"

#include <span>
#include <stdexcept>
#include <string>
#include <type_traits>

namespace awen {

inline std::string last_error() {
    const auto required = awen_last_error_message(nullptr, 0);
    std::string message(required, '\0');
    if (required > 1) {
        awen_last_error_message(message.data(), required);
        message.pop_back();
    }
    return message;
}

template <typename T>
void gemm(
    std::span<const T> lhs,
    std::span<const T> rhs,
    std::span<T> output,
    std::size_t m,
    std::size_t n,
    std::size_t k) {
    awen_status status;
    if constexpr (std::is_same_v<T, double>) {
        status = awen_gemm_f64(
            lhs.data(), lhs.size(), rhs.data(), rhs.size(), output.data(), output.size(), m, n, k);
    } else if constexpr (std::is_same_v<T, float>) {
        status = awen_gemm_f32(
            lhs.data(), lhs.size(), rhs.data(), rhs.size(), output.data(), output.size(), m, n, k);
    } else {
        static_assert(std::is_same_v<T, float> || std::is_same_v<T, double>);
    }
    if (status != AWEN_STATUS_OK) {
        throw std::runtime_error(last_error());
    }
}

}  // namespace awen

#endif
