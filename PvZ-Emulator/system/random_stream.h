#pragma once

#include <cstdint>
#include <random>

namespace pvz_emulator::system {

class random_stream {
    std::mt19937 engine;
    std::uint32_t source_seed;
    std::uint32_t initial_seed;
    std::uint32_t locked_value;
    bool is_locked;

public:
    using result_type = std::uint32_t;

    explicit random_stream(std::uint32_t seed_value = 4357) {
        seed(seed_value);
    }

    static constexpr result_type min() { return 0; }
    static constexpr result_type max() { return 0x7fffffffu; }

    void seed(std::uint32_t value) {
        source_seed = value;
        initial_seed = value == 0 ? 4357 : value;
        engine.seed(initial_seed);
        locked_value = 0;
        is_locked = false;
    }

    void reseed_engine(std::uint32_t value) {
        source_seed = value;
        initial_seed = value == 0 ? 4357 : value;
        engine.seed(initial_seed);
        locked_value = 0;
        is_locked = false;
    }

    void lock(std::uint32_t value) {
        locked_value = value & 0x7fffffffu;
        is_locked = true;
    }

    std::uint32_t next() {
        return is_locked ? locked_value : engine() & 0x7fffffffu;
    }

    result_type operator()() { return next(); }

    [[nodiscard]] std::uint32_t seed_value() const { return initial_seed; }
    [[nodiscard]] std::uint32_t source_seed_value() const { return source_seed; }
    [[nodiscard]] std::uint32_t fixed_value() const { return locked_value; }
    [[nodiscard]] bool locked() const { return is_locked; }

    friend bool operator==(const random_stream& lhs, const random_stream& rhs) {
        return lhs.engine == rhs.engine &&
            lhs.source_seed == rhs.source_seed &&
            lhs.initial_seed == rhs.initial_seed &&
            lhs.locked_value == rhs.locked_value &&
            lhs.is_locked == rhs.is_locked;
    }
};

}
