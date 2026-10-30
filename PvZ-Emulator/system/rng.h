#pragma once
#include <cassert>
#include <cstdint>
#include <type_traits>

#include "object/scene.h"

namespace pvz_emulator::system {

class rng {
	object::scene* scene;
	random_stream* direct_stream;

	random_stream& stream() {
		return scene == nullptr ? *direct_stream : scene->active_battle_rng();
	}

	float unit_float() {
		return static_cast<float>(
			static_cast<double>(stream().next()) / 0x7fffffffu);
	}

public:
	rng(object::scene& s) : scene(&s), direct_stream(nullptr) {}
	explicit rng(random_stream& s) : scene(nullptr), direct_stream(&s) {}

	unsigned int randint(unsigned int n) {
		assert(n != 0);
		return stream().next() % n;
	}

	template<typename A>
	size_t random_weighted_sample(const A& v) {
		using weight_type = std::remove_cv_t<std::remove_reference_t<decltype(*v.begin())>>;
		if constexpr (std::is_integral_v<weight_type>) {
			std::uint64_t total = 0;
			for (const auto weight : v) {
				total += weight;
			}
			assert(total > 0 && total <= UINT32_MAX);

			auto chosen = randint(static_cast<unsigned int>(total));
			for (size_t i = 0; i < v.size(); ++i) {
				if (chosen < v[i]) {
					return i;
				}
				chosen -= v[i];
			}
		} else {
			float total = 0;
			for (const auto weight : v) {
				total += weight;
			}
			assert(total > 0);

			const auto chosen = unit_float() * total;
			float accumulated = 0;
			for (size_t i = 0; i + 1 < v.size(); ++i) {
				accumulated += v[i];
				if (chosen <= accumulated) {
					return i;
				}
			}
			return v.size() - 1;
		}
		assert(false);
		return 0;
	}

	float randfloat(float a, float b) {
		const float range = b - a;
		const float scaled = unit_float() * range;
		return a + scaled;
	}
};

};
