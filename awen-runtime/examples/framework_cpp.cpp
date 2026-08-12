#include <awen/framework.hpp>

#include <array>
#include <iostream>

int main() {
    const std::array<double, 4> lhs{1.0, 2.0, 3.0, 4.0};
    const std::array<double, 4> rhs{5.0, 6.0, 7.0, 8.0};
    std::array<double, 4> output{};
    awen::gemm<double>(lhs, rhs, output, 2, 2, 2);
    std::cout << output[0] << " " << output[1] << " " << output[2] << " " << output[3]
              << '\n';
}
