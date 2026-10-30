#include <cassert>
#include <array>
#include <vector>
#include <algorithm>
#include <utility>

#include "spawn.h"
#include "util.h"

namespace pvz_emulator::system {

using namespace pvz_emulator::object;

namespace {

constexpr std::array<zombie_type, 24> SPAWN_FLAG_TYPES = {
    zombie_type::zombie,
    zombie_type::flag,
    zombie_type::conehead,
    zombie_type::pole_vaulting,
    zombie_type::buckethead,
    zombie_type::newspaper,
    zombie_type::screendoor,
    zombie_type::football,
    zombie_type::dancing,
    zombie_type::backup_dancer,
    zombie_type::snorkel,
    zombie_type::zomboni,
    zombie_type::dolphin_rider,
    zombie_type::jack_in_the_box,
    zombie_type::balloon,
    zombie_type::digger,
    zombie_type::pogo,
    zombie_type::yeti,
    zombie_type::bungee,
    zombie_type::ladder,
    zombie_type::catapult,
    zombie_type::gargantuar,
    zombie_type::imp,
    zombie_type::giga_gargantuar,
};

class battle_rng_override_guard {
    object::scene& scene;

public:
    battle_rng_override_guard(object::scene& scene_value, random_stream& override_stream) :
        scene(scene_value)
    {
        assert(scene.battle_rng_override == nullptr);
        scene.battle_rng_override = &override_stream;
        scene.battle_rng_override_active = false;
    }

    battle_rng_override_guard(const battle_rng_override_guard&) = delete;
    battle_rng_override_guard& operator=(const battle_rng_override_guard&) = delete;

    ~battle_rng_override_guard() {
        assert(!scene.battle_rng_override_active);
        scene.battle_rng_override = nullptr;
    }
};

}

const unsigned int spawn::MAX_WAVES = 20;
const unsigned int spawn::MAX_ZOMBIES = 50;
const unsigned int spawn::MAX_ZOMBIE_TYPES = 9;

const unsigned int spawn::ZOMBIE_SPAWN_WEIGHT[33] = {
	4000,0,4000,2000,3000,
	1000,3500,2000,1000,0,
	0,2000,2000,2000,1500,
	1000,2000,1000,1000,1,
	1000,1000,1500,1500,0,
	0,4000,3000,1000,2000,
	2000,2000,6000
};

const unsigned int spawn::N_LURKERS = 3;
const unsigned int spawn::N_HUGEWAVE_SQUAD = 8;

const unsigned int spawn::NIGHT_MAX_GRAVES = 12;

spawn::spawn(object::scene& s):
	scene(s),
	data(s.spawn),
	zombie_factory(s),
	griditem_factory(s),
	rng(s)
{}

unsigned int spawn::get_spawn_weight(zombie_type type, unsigned wave) {
	switch (type) {
	case zombie_type::zombie:
		return ZOMBIE_SPAWN_WEIGHT[static_cast<int>(type)] / 10;
	case zombie_type::conehead:
		return ZOMBIE_SPAWN_WEIGHT[static_cast<int>(type)] / 4;
	case zombie_type::giga_gargantuar:
		return is_wave_9_or_19(wave) ?
            ZOMBIE_SPAWN_WEIGHT[static_cast<int>(type)] :
            1000;
	default:
		return ZOMBIE_SPAWN_WEIGHT[static_cast<int>(type)];
		break;
	}
}

bool spawn::can_spawn(zombie_type type) {
	switch (type) {
	case zombie_type::zombie:
	case zombie_type::conehead:
	case zombie_type::pole_vaulting:
	case zombie_type::buckethead:
	case zombie_type::newspaper:
	case zombie_type::screendoor:
	case zombie_type::football:
	case zombie_type::jack_in_the_box:
	case zombie_type::balloon:
	case zombie_type::pogo:
	case zombie_type::yeti:
	case zombie_type::bungee:
	case zombie_type::ladder:
	case zombie_type::catapult:
	case zombie_type::gargantuar:
	case zombie_type::giga_gargantuar:
		return true;

	case zombie_type::snorkel:
	case zombie_type::dolphin_rider:
		return scene.type == scene_type::pool ||
			scene.type == scene_type::fog;

	case zombie_type::zomboni:
		return scene.type != scene_type::night;

	case zombie_type::digger:
	case zombie_type::dancing:
		return scene.type != scene_type::moon_night &&
			scene.type != scene_type::roof;

	default:
		return false;
	}
}

bool spawn::can_spawn(zombie_type type, unsigned wave) {
	if (!data.spawn_flags[static_cast<int>(type)] || !can_spawn(type)) {
		return false;
	}

	bool is_critical_wave = is_wave_9_or_19(wave);
	int i = static_cast<int>(type);

	switch (type) {
	case zombie_type::bungee:
		return is_critical_wave;

	case zombie_type::zomboni:
	case zombie_type::gargantuar:
		return curr_spawn_count[i] < MAX_ZOMBIES;

	case zombie_type::giga_gargantuar:
		if (is_critical_wave) {
			return curr_spawn_count[i] < MAX_ZOMBIES;
		} else {
			return total_spawn_count[i] < MAX_ZOMBIES;
		}
	default:
		return true;
	}
}

zombie_type spawn::gen_lurking_zombie_type() {
	static const std::array<zombie_type, 3> LURKING_TYPES = {
        zombie_type::zombie,
        zombie_type::conehead,
        zombie_type::buckethead,
	};

	std::array<unsigned int, 3> weight = {
        ZOMBIE_SPAWN_WEIGHT[static_cast<int>(zombie_type::zombie)],

        data.spawn_flags[static_cast<int>(zombie_type::conehead)] ?
            ZOMBIE_SPAWN_WEIGHT[static_cast<int>(zombie_type::conehead)] :
            0,

        scene.type != scene_type::night &&
        data.spawn_flags[static_cast<int>(zombie_type::buckethead)] ?
            ZOMBIE_SPAWN_WEIGHT[static_cast<int>(zombie_type::buckethead)] :
            0
	};

	return LURKING_TYPES[rng.random_weighted_sample(weight)];
}

void spawn::gen_spawn_list() {
	memset(total_spawn_count, 0, sizeof(total_spawn_count));

	for (int wave = 0; wave < MAX_WAVES; wave++) {
		memset(curr_spawn_count, 0, sizeof(curr_spawn_count));

		unsigned int i = 0;

		if (is_wave_9_or_19(wave)) {
			for (; i < N_HUGEWAVE_SQUAD; i++) {
				spawn_list_append(wave, i, zombie_type::zombie);
			}

            spawn_list_append(wave, i++, zombie_type::flag);
		}

		std::vector<zombie_type> types;
		std::vector<unsigned int> weights;
		types.reserve(SPAWN_FLAG_TYPES.size());
		weights.reserve(SPAWN_FLAG_TYPES.size());

		for (; i < MAX_ZOMBIES; i++) {
			types.clear();
			weights.clear();

			for (auto type : SPAWN_FLAG_TYPES) {
				if (!can_spawn(type, wave)) {
					continue;
				}

				types.push_back(type);
				weights.push_back(get_spawn_weight(type, wave));
			}

			spawn_list_append(wave, i, types[rng.random_weighted_sample(weights)]);
		}
	}
}

void spawn::gen_spawn_flags() {
	system::rng level_rng(scene.level_rng);
	memset(data.spawn_flags.data(), 0, sizeof(data.spawn_flags));

	data.spawn_flags[0] = true;

	if (level_rng.randint(5)) {
		data.spawn_flags[static_cast<int>(zombie_type::conehead)] = true;
	} else {
		data.spawn_flags[static_cast<int>(zombie_type::newspaper)] = true;
	}

	std::array<unsigned int, 33> weights = { 0 };
	for (int i = 0; i < weights.size(); i++) {
		if (i != static_cast<int>(zombie_type::yeti) &&
			!data.spawn_flags[i] && can_spawn(static_cast<zombie_type>(i))) {
			weights[i] = 1;
		}
	}

	for (int i = 0; i < MAX_ZOMBIE_TYPES; i++) {
		auto index = level_rng.random_weighted_sample(weights);
		data.spawn_flags[index] = true;
		weights[index] = 0;
	}

    // PE models an account where the Yeti unlock is already available. Native
    // survival selection adds it after the rejection-sampling loop as well.
    data.spawn_flags[static_cast<int>(zombie_type::yeti)] = true;
}

bool spawn::has_wave_filling_candidate() {
	// Bungee cannot fill ordinary waves, and Giga exhausts its 50-per-level cap.
	return std::any_of(SPAWN_FLAG_TYPES.begin(), SPAWN_FLAG_TYPES.end(), [&](auto type) {
		return type != zombie_type::bungee &&
			type != zombie_type::giga_gargantuar &&
			data.spawn_flags[static_cast<int>(type)] &&
			can_spawn(type) &&
			get_spawn_weight(type, 0) != 0;
	});
}

bool spawn::pick_spawn_list() {
	if (!has_wave_filling_candidate()) {
		return false;
	}
	gen_spawn_list();
	return true;
}

void spawn::pool_lurking_spawn() {
	static const std::pair<unsigned int, unsigned int> GRIDS[] = {
		{2, 5}, {3, 5}, {2, 6}, {3, 6},
		{2, 7}, {3, 7}, {2, 8}, {3, 8}
	};

	std::array<unsigned int, 8> weights = { 10000, 10000, 10000, 10000, 10000, 10000, 10000, 10000 };

	for (int i = 0; i < N_LURKERS; i++) {
		auto k = rng.random_weighted_sample(weights);
		weights[k] = 0;

		if (!zombie_factory.create_lurking(
			gen_lurking_zombie_type(),
			GRIDS[k].first,
			GRIDS[k].second))
		{
			return;
		}
	}
}

void spawn::roof_lurking_spawn() {
	static const std::pair<unsigned int, unsigned int> GRIDS[] = {
		{0, 4}, {0, 5}, {0, 6}, {0, 7}, {0, 8},
		{1, 4}, {1, 5}, {1, 6}, {1, 7}, {1, 8},
		{2, 4}, {2, 5}, {2, 6}, {2, 7}, {2, 8},
		{3, 4}, {3, 5}, {3, 6}, {3, 7}, {3, 8},
		{4, 4}, {4, 5}, {4, 6}, {4, 7}, {4, 8},
		{5, 4}, {5, 5}, {5, 6}, {5, 7}, {5, 8},
	};

	std::vector<unsigned int> weights(25, 10000);

	for (int n = 0; n < N_LURKERS; n++) {
		auto j = rng.random_weighted_sample(weights);

		weights[j] = 1;

		if (!zombie_factory.create_lurking(
			gen_lurking_zombie_type(),
			GRIDS[j].first,
			GRIDS[j].second))
		{
			return;
		}
	}
}

void spawn::night_lurking_spawn() {
	for (auto& grave : scene.griditems) {
		if (grave.type != griditem_type::grave ||
			grave.countdown < 100)
		{
			continue;
		}

		if (!zombie_factory.create_lurking(
			gen_lurking_zombie_type(),
			grave.row,
			grave.col))
		{
			return;
		}
	}
}

void spawn::lurking_squad_update() {
	if (data.countdown.lurking_squad == 0) {
		return;
	}

	--data.countdown.lurking_squad;
	if (scene.special_events_disabled || data.countdown.lurking_squad != 0) {
		return;
	}

	switch (scene.type) {
	case scene_type::pool:
	case scene_type::fog:
		if (data.countdown.pool <= 0) {
            pool_lurking_spawn();
		}
		return;

	case scene_type::night:
		night_lurking_spawn();
		return;

	case scene_type::roof:
	case scene_type::moon_night:
		if (data.countdown.pool <= 0) {
			roof_lurking_spawn();
		}
		return;

	default:
		break;
	}
}

void spawn::night_grave_spawn() {
	int n = 0;
	bool grids[5][5] = {{0}};

	for (auto& item : scene.griditems) {
		if ((item.type == griditem_type::grave ||
			item.type == griditem_type::crater ||
			item.type == griditem_type::ladder) &&
			item.col >= 4)	
		{
			grids[item.row][item.col - 4] = true;
		}

		if (item.type == griditem_type::grave && ++n >= NIGHT_MAX_GRAVES) {
			return;
		}
	}

	std::vector<std::pair<int, int>> targets;
	std::vector<unsigned int> weights;

	for (int row = 0; row < 5; row++) {
		for (int col = 4; col < 9; col++) {
			if (grids[row][col - 4]) {
				continue;
			}

			auto& s = scene.plant_map[row][col];

			if (s.base || s.coffee_bean || s.content || s.pumpkin) {
				weights.push_back(1);
			} else {
				weights.push_back(100000);
			}

			targets.emplace_back(row, col);
		}
    }

	if (targets.empty()) {
		return;
	}

	auto& p = targets[rng.random_weighted_sample(weights)];
	griditem_factory.create(griditem_type::grave, p.first, p.second);
}

void spawn::hugewave_warn_update() {
	if (data.countdown.hugewave_fade > 0 && --data.countdown.hugewave_fade == 0) {
		data.is_hugewave_shown = false;
		data.countdown.next_wave = 1;
	}
}

int spawn::total_zombies_health_in_wave(unsigned int wave) const {
	int hp = 0;

	for (auto& z : scene.zombies) {
		if (z.spawn_wave != wave ||
			z.is_hypno ||
			z.has_death_status() ||
			z.type == zombie_type::bungee ||
			z.master_id != -1)
		{
			continue;
		}

		hp += z.hp +
			z.accessory_1.hp +
			static_cast<int>(z.accessory_2.hp * 0.2000000029802322) +
			(z.has_balloon ? 20 : 0);
	}

	return hp;
}

unsigned int spawn::get_current_hp() const {
	if (data.wave == 0) {
		return 0;
	}

	return static_cast<unsigned int>(
		std::max(0, total_zombies_health_in_wave(data.wave - 1)));
}

bool spawn::are_enemy_zombies_on_screen() const {
	for (const auto& z : scene.zombies) {
		if (z.is_not_dying &&
			!z.is_dead &&
			!z.has_death_status() &&
			!z.is_hypno)
		{
			return true;
		}
	}

	return false;
}

void spawn::start_endgame_countdown() {
	if (data.countdown.endgame > 0) {
		return;
	}

	data.countdown.endgame = 500;
	for (auto& t : scene.ice_path.countdown) {
		t = 500;
	}
}

void spawn::try_start_endgame_for_clear() {
	if (data.countdown.endgame == 0 &&
		data.wave == MAX_WAVES &&
		!are_enemy_zombies_on_screen())
	{
		start_endgame_countdown();
	}
}

void spawn::spawn_current_wave_objects() {
    if (!scene.special_events_disabled &&
        scene.type == scene_type::night && data.wave == 19)
    {
        night_grave_spawn();
    }

    for (int i = 0; i < 50; i++) {
        auto type = data.spawn_list[data.wave][i];
        if (type == zombie_type::none ||
            (scene.special_events_disabled && type == zombie_type::bungee))
        {
            continue;
        }

        zombie_factory.try_create(type);
    }
}

void spawn::next_spawn_countdown_update() {
	if (data.countdown.endgame > 0) {
		return;
	}

	assert(data.countdown.next_wave <= 5500);

	if (data.wave == MAX_WAVES) {
		if (!are_enemy_zombies_on_screen()) {
			start_endgame_countdown();
			return;
		}

		--data.countdown.next_wave;
		if (data.countdown.next_wave == 0) {
			start_endgame_countdown();
		}

		return;
	} 

	--data.countdown.next_wave;
	
	if (data.countdown.next_wave > 5 &&
		data.countdown.next_wave_initial - data.countdown.next_wave > 400 &&
        data.countdown.next_wave > 200 &&
		static_cast<int>(get_current_hp()) <= data.hp.threshold)
	{
		data.countdown.next_wave = 200;
	}

	if (data.countdown.next_wave == 5 && is_wave_9_or_19()) {
		data.is_hugewave_shown = true;
		data.countdown.hugewave_fade = 750;
		return;
	}

	if (data.countdown.next_wave != 0) {
		return;
	}

	if (scene.wave_spawn_random_enabled) {
		random_stream wave_rng(scene.wave_spawn_random_seed + data.wave);
		battle_rng_override_guard guard(scene, wave_rng);
		spawn_current_wave_objects();
	} else {
		spawn_current_wave_objects();
	}

	if (data.wave == MAX_WAVES - 1) {
		data.countdown.lurking_squad = 200;
	}

	++data.wave;

	if (data.wave == 10) {
		data.total_flags++;
	}

	data.hp.initial = get_current_hp();

	if (data.wave == MAX_WAVES) {
		data.hp.threshold = 0;
		data.countdown.next_wave = 5500;
	} else if (is_wave_9_or_19()) {
		data.hp.threshold = 0;
		data.countdown.next_wave = 4500;
	} else {
		data.hp.threshold = static_cast<int>(
			rng.randfloat(0.5, 0.64999998) * data.hp.initial);
		data.countdown.next_wave = rng.randint(600) + 2500;
	}

	data.countdown.next_wave_initial = data.countdown.next_wave;
}

void spawn::update() {
	if (data.countdown.endgame > 0) {
		return;
	}

	lurking_squad_update();

	hugewave_warn_update();

	bool skip_spawn_update =
		data.countdown.hugewave_fade > 0 &&
		data.countdown.hugewave_fade != 725;

	if (!skip_spawn_update) {
		next_spawn_countdown_update();
	}
}

void spawn::reset() {
    data.wave = 0;
	data.hp = { 0 };

    data.countdown.next_wave_initial = data.countdown.next_wave = 600;
	data.countdown.lurking_squad =
		data.countdown.hugewave_fade =
		data.countdown.endgame = 0;

    gen_spawn_flags();
    gen_spawn_list();
}

};
