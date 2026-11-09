#include <yaff/yaff.h>

#include <algorithm>
#include <chrono>
#include <fstream>
#include <iostream>

#include "schema.yaff.h"

template <class F>
void Measure(const char* name, F read) {
    constexpr size_t count = 5000000;
    std::vector<double> samples;
    uint64_t sum = 0;
    for (size_t i = 0; i < 1000000; ++i) sum += read();
    for (size_t sample = 0; sample < 7; ++sample) {
        const auto start = std::chrono::steady_clock::now();
        for (size_t i = 0; i < count; ++i) sum += read();
        const auto elapsed = std::chrono::steady_clock::now() - start;
        samples.push_back(std::chrono::duration<double, std::nano>(elapsed).count() / count);
    }
    std::sort(samples.begin(), samples.end());
    std::cout << "cpp," << name << ',' << samples[3] << '\n';
    // Observable use; the memory barrier below prevents loop-invariant reads.
    if (sum == 123456789) std::cerr << sum;
}

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    std::ifstream in(argv[1], std::ios::binary);
    if (!in) return 2;
    std::string data{std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>()};
    const auto* view = &yaff::ReadMessage<protoyaff::interop::Everything>(data.data());
    const auto barrier = [&] {
#if defined(__clang__) || defined(__GNUC__)
        const auto* pointer = view;
        asm volatile("" : "+r"(pointer) : : "memory");
        return pointer;
#endif
        return view;
    };
    Measure("empty_register", [&] {
        barrier();
        return uint64_t{1};
    });
    Measure("open_scalar", [&] {
        const auto* ptr = data.data();
#if defined(__clang__) || defined(__GNUC__)
        asm volatile("" : "+r"(ptr) : : "memory");
#endif
        return static_cast<uint64_t>(yaff::ReadMessage<protoyaff::interop::Everything>(ptr).id());
    });
    Measure("scalar", [&] { return static_cast<uint64_t>(barrier()->id()); });
    Measure("nested", [&] { return barrier()->child().counter(); });
    Measure("array", [&] { return static_cast<uint64_t>(barrier()->numbers(2)); });
}
