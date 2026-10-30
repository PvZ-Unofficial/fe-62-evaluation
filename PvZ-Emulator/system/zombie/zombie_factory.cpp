#include <cassert>
#include <algorithm>
#include <array>
#include <new>
#include <stdexcept>

#include "system/zombie/zombie_factory.h"
#include "system/plant/plant_factory.h"
#include "system/zombie/zombie.h"
#include "system/util.h"
#include "system/event.h"

namespace pvz_emulator::system {

using namespace pvz_emulator::object;

namespace {

bool is_supported_zombie_type(zombie_type type) {
    const auto raw = static_cast<int>(type);
    return (raw >= 0 && raw <= 0x18 && raw != 0x0d) ||
        type == zombie_type::giga_gargantuar;
}

float smooth_weight(float weight, float last_picked, float second_last_picked) {
    if (weight < 1e-6f) {
        return 0.0f;
    }
    const float expected_1 = 1.0f / weight;
    const float expected_2 = expected_1 * 2.0f;
    const float advanced_1 = last_picked + 1.0f - expected_1;
    const float advanced_2 = second_last_picked + 1.0f - expected_2;
    const float factor_1 = 1.0f + advanced_1 / expected_1 * 2.0f;
    const float factor_2 = 1.0f + advanced_2 / expected_2 * 2.0f;
    const float factor = std::clamp(factor_1 * 0.75f + factor_2 * 0.25f, 0.01f, 100.0f);
    return weight * factor;
}

class battle_rng_override_use_guard {
    object::scene& scene;
    bool enabled;

public:
    explicit battle_rng_override_use_guard(object::scene& scene_value) :
        scene(scene_value), enabled(scene.battle_rng_override != nullptr)
    {
        assert(!scene.battle_rng_override_active);
        scene.battle_rng_override_active = enabled;
    }

    ~battle_rng_override_use_guard() {
        scene.battle_rng_override_active = false;
    }
};

}

zombie& zombie_factory::create(zombie_type type, std::optional<unsigned int> spawn_wave)
{
    if (!is_supported_zombie_type(type) || !has_spawn_row(type)) {
        throw std::invalid_argument("zombie type has no valid spawn row");
    }
    const auto row = get_spawn_row(type);
    if (auto* zombie = try_create_in_row(type, row, spawn_wave)) {
        return *zombie;
    }
    throw std::bad_alloc();
}

zombie* zombie_factory::try_create(
    zombie_type type,
    std::optional<unsigned int> spawn_wave)
{
    if (!is_supported_zombie_type(type) || !has_spawn_row(type)) {
        return nullptr;
    }
    const auto row = get_spawn_row(type);
    return try_create_in_row(type, row, spawn_wave);
}

zombie* zombie_factory::add_zombie_in_row(
    zombie_type type,
    int row,
    int from_wave)
{
    if (!is_supported_zombie_type(type) ||
        !scene.row_can_have_zombies(row) || from_wave < 0)
    {
        return nullptr;
    }

    return try_create_in_row(
        type, static_cast<unsigned int>(row), static_cast<unsigned int>(from_wave));
}

zombie* zombie_factory::place_zombie(zombie_type type, int grid_x, int grid_y) {
    if (!is_supported_zombie_type(type) || grid_x < 0 || grid_x >= 9 ||
        !scene.row_can_have_zombies(grid_y))
    {
        return nullptr;
    }

    return try_create_in_grid(
        type,
        static_cast<unsigned int>(grid_y),
        static_cast<unsigned int>(grid_x),
        0);
}

zombie* zombie_factory::try_create_in_row(
    zombie_type type,
    unsigned int row,
    std::optional<unsigned int> spawn_wave)
{
    if (scene.zombies.active_count() >= scene.zombies.capacity() - 1) {
        return nullptr;
    }
    return &create_in_row_impl(type, row, spawn_wave);
}

zombie* zombie_factory::try_create_in_grid(
    zombie_type type,
    unsigned int row,
    unsigned int col,
    std::optional<unsigned int> spawn_wave)
{
    auto* z = try_create_in_row(type, row, spawn_wave);
    if (z == nullptr) {
        return nullptr;
    }

    z->row = row;
    z->x = static_cast<float>(
        type == zombie_type::bungee ? get_x_by_col(col) : get_x_by_col(col) - 30);
    z->y = zombie_init_y(scene.type, *z, row);
    if (type == zombie_type::bungee) {
        z->bungee_col = static_cast<int>(col);
    }

    return z;
}

bool zombie_factory::has_spawn_row(zombie_type type) {
	for (unsigned int row = 0; row < scene.rows; ++row) {
		if (can_spawn_at_row(type, row)) {
			return true;
		}
	}
	return false;
}

zombie& zombie_factory::create_in_row_impl(
    zombie_type type,
    unsigned int row,
    std::optional<unsigned int> spawn_wave)
{

    auto& z = scene.zombies.alloc();

    auto wave = spawn_wave ? spawn_wave.value() : scene.spawn.wave;

    switch (type) {
    case zombie_type::zombie:
    case zombie_type::conehead:
    case zombie_type::buckethead:
    case zombie_type::screendoor:
    case zombie_type::flag:
    case zombie_type::ducky_tube:
        subsystems.zombie.init(z, type, row);
        break;

    case zombie_type::pole_vaulting:
        subsystems.pole_vaulting.init(z, row);
        break;

    case zombie_type::newspaper:
        subsystems.newspaper.init(z, row);
        break;

    case zombie_type::football:
        subsystems.football.init(z, row);
        break;

    case zombie_type::dancing:
        subsystems.dancing.init(z, row);
        break;

    case zombie_type::backup_dancer:
        subsystems.backup_dancer.init(z, row, wave);
        break;

    case zombie_type::snorkel:
        subsystems.snorkel.init(z, row);
        break;

    case zombie_type::zomboni:
        subsystems.zomboni.init(z, row);
        break;

    case zombie_type::dolphin_rider:
        subsystems.dolphin_rider.init(z, row);
        break;

    case zombie_type::jack_in_the_box:
        subsystems.jack_in_the_box.init(z, row);
        break;

    case zombie_type::balloon:
        subsystems.balloon.init(z, row);
        break;

    case zombie_type::digger:
        subsystems.digger.init(z, row);
        break;

    case zombie_type::pogo:
        subsystems.pogo.init(z, row);
        break;

    case zombie_type::yeti:
        subsystems.yeti.init(z, row);
        break;

    case zombie_type::bungee:
        subsystems.bungee.init(z, row);
        break;

    case zombie_type::ladder:
        subsystems.ladder.init(z, row);
        break;

    case zombie_type::catapult:
        subsystems.catapult.init(z, row);
        break;

    case zombie_type::gargantuar:
        subsystems.gargantuar.init(z, zombie_type::gargantuar, row);
        break;

    case zombie_type::giga_gargantuar:
        subsystems.gargantuar.init(z, zombie_type::giga_gargantuar, row);
        break;

    case zombie_type::imp:
        subsystems.imp.init(z, row, wave);
        break;

    default:
        assert(false);
    }

    const bool old_huge_wave = z.spawn_wave == 9 || z.spawn_wave == 19;
    const bool new_huge_wave = wave == 9 || wave == 19;
    bool uses_base_spawn_position = true;
    switch (type) {
    case zombie_type::flag:
    case zombie_type::pole_vaulting:
    case zombie_type::zomboni:
    case zombie_type::bungee:
    case zombie_type::catapult:
    case zombie_type::gargantuar:
    case zombie_type::giga_gargantuar:
        uses_base_spawn_position = false;
        break;
    default:
        break;
    }
    if (uses_base_spawn_position && old_huge_wave != new_huge_wave) {
        z.x += new_huge_wave ? 40.0f : -40.0f;
        z.y = zombie_init_y(scene.type, z, row);
        z.int_x = static_cast<int>(z.x);
        z.int_y = static_cast<int>(z.y);
    }
    z.spawn_wave = wave;

    if (type == zombie_type::gargantuar || type == zombie_type::giga_gargantuar) {
        emit_gargantuar_spawned(scene, z);
    }

    return z;
}

bool zombie_factory::create_roof_lurking(
	zombie_type type,
	unsigned int row,
	unsigned int col)
{
	auto* b = try_create(zombie_type::bungee);
	if (b == nullptr) {
		return false;
	}
	auto* z = try_create(type);
	if (z == nullptr) {
		b->is_dead = true;
		return false;
	}

	b->bungee_col = col;
	b->row = row;
	b->x = static_cast<float>(80 * col + 40);
	b->y = zombie_init_y(scene.type, *b, row);
	b->master_id = static_cast<int>(scene.zombies.get_id(z));
	reanim.set(*b, zombie_reanim_name::anim_raise, reanim_type::once, 36);

	z->x = b->x - 15;
	z->row = row;
	z->y = zombie_init_y(scene.type, *z, row);
	z->action = zombie_action::fall_from_sky;
	reanim.set(*z, zombie_reanim_name::anim_idle, reanim_type::repeat, 0);
	return true;
}

bool zombie_factory::create_pool_or_night_lurking(
	zombie_type type,
	unsigned int row,
	unsigned int col)
{
	auto* z = scene.type == scene_type::night ?
		try_create(type) :
		try_create_in_row(type, row);
	if (z == nullptr) {
		return false;
	}
	
	z->x = static_cast<float>(80 * col + 15);
	z->y = zombie_init_y(scene.type, *z, row);
	z->int_x = static_cast<int>(z->x);
	z->int_y = static_cast<int>(z->y);
	z->row = row;
	z->dy = static_cast<float>(scene.type == scene_type::night ? -200 : -150);
	z->status = zombie_status::rising_from_ground;
	z->is_in_water = scene.type != scene_type::night;
	z->countdown.action = scene.type == scene_type::night ? 150 : 50;

	if (scene.type != scene_type::night) {
		z->action = zombie_action::none;
	}
	reanim.update_status(*z);
	return true;
}

bool zombie_factory::create_lurking(
	zombie_type type,
	unsigned int row,
	unsigned int col)
{
	if (!is_supported_zombie_type(type) ||
		!has_spawn_row(type) ||
		row >= scene.rows || col >= 9)
	{
		return false;
	}

	switch (scene.type) {
	case scene_type::pool:
	case scene_type::fog:
		return create_pool_or_night_lurking(type, row, col);

	case scene_type::roof:
	case scene_type::moon_night:
		return create_roof_lurking(type, row, col);

    case scene_type::night:
		return create_pool_or_night_lurking(type, row, col);

	default:
		return false;
	}
}

void zombie_factory::destroy(object::zombie& z) {
	if (!scene.zombies.is_active(&z)) {
		throw std::invalid_argument("zombie does not belong to this scene");
	}

	z.is_dead = true;

	if (z.type == zombie_type::bungee) {
		zombie_bungee(scene).drop_plant(z);

		if (auto p = scene.plants.try_to_get(static_cast<std::uint32_t>(z.bungee_target))) {
			plant_factory(scene).destroy(*p);
		}

		if (auto partner = scene.zombies.try_to_get(static_cast<std::uint32_t>(z.master_id))) {
			if (!partner->is_dead) {
				destroy(*partner);
			}
		}
	}
}

bool zombie_factory::can_spawn_at_row(zombie_type type, unsigned int row) {
	if (row >= scene.rows) {
		return false;
	}

	switch (type) {
	case zombie_type::zombie:
	case zombie_type::flag:
	case zombie_type::conehead:
	case zombie_type::buckethead:
	case zombie_type::balloon:
		switch (scene.type) {
		case scene_type::pool:
		case scene_type::fog:
			return row == 0 || row == 1 || row == 4 || row == 5 || scene.spawn.wave >= 5;
		default:
			return true;
		}

	case zombie_type::pole_vaulting:
	case zombie_type::newspaper:
	case zombie_type::screendoor:
	case zombie_type::football:
	case zombie_type::jack_in_the_box:
	case zombie_type::backup_dancer:
	case zombie_type::digger:
	case zombie_type::zomboni:
	case zombie_type::pogo:
	case zombie_type::yeti:
	case zombie_type::ladder:
	case zombie_type::catapult:
	case zombie_type::gargantuar:
	case zombie_type::imp:
	case zombie_type::giga_gargantuar:
	case zombie_type::bungee:
		switch (scene.type) {
		case scene_type::fog:
		case scene_type::pool:
			return row == 0 || row == 1 || row == 4 || row == 5;
		default:
			return true;
		}

	case zombie_type::dancing:
		switch (scene.type) {
		case scene_type::day:
		case scene_type::night:
		case scene_type::mushroom_garden:
			return row == 1 || row == 2 || row == 3;

		case scene_type::pool:
		case scene_type::fog:
            return row == 0 || row == 1 || row == 4 || row == 5;

		default:
			return false;
		}

	case zombie_type::ducky_tube:
	case zombie_type::snorkel:
	case zombie_type::dolphin_rider:
		return (scene.type == scene_type::pool || scene.type == scene_type::fog) &&
			(row == 2 || row == 3);

	default:
		assert(false);
		return false;
	}
}

unsigned int zombie_factory::get_spawn_row(zombie_type type) {
	battle_rng_override_use_guard rng_scope(scene);
	float sigma_b = 0;

	for (int i = 0; i < 6; i++) {
		if (can_spawn_at_row(type, i)) {
			sigma_b += data.row_random[i].b = 1;
		} else {
			data.row_random[i].b = 0;
		}
	}

	std::array<float, 6> f = { 0 };
	for (int i = 0; i < 6; i++) {
		if (data.row_random[i].b == 0) {
			f[i] = 0;
			continue;
		}

		float weight = data.row_random[i].b / sigma_b;

		f[i] = smooth_weight(weight, data.row_random[i].c, data.row_random[i].d);
	}

	auto row = rng.random_weighted_sample(f);

	for (int i = 0; i < 6; i++) {
		if (data.row_random[i].b > 0) {
			data.row_random[i].c++;
			data.row_random[i].d++;
		}
	}

	data.row_random[row].d = data.row_random[row].c;
	data.row_random[row].c = 0;

	return static_cast<unsigned int>(row);
}

}
