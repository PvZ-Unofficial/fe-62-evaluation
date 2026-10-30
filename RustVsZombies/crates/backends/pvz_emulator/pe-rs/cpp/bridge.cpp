#include "bridge.h"

#include "system/damage.h"
#include "system/rng.h"
#include "system/util.h"
#include "system/zombie/zombie.h"
#include "world.h"

#include <cstdint>
#include <exception>
#include <new>
#include <utility>

using namespace pvz_emulator;
using namespace pvz_emulator::object;

namespace {

using plant_pool = decltype(scene::plants);
using zombie_pool = decltype(scene::zombies);
using griditem_pool = decltype(scene::griditems);
using projectile_pool = decltype(scene::projectiles);

constexpr pe_rs_status ok() noexcept {
    return {PE_RS_STATUS_OK, 0};
}

constexpr pe_rs_status null_pointer() noexcept {
    return {PE_RS_STATUS_NULL_POINTER, 0};
}

constexpr pe_rs_status invalid(std::int32_t detail) noexcept {
    return {PE_RS_STATUS_INVALID_ARGUMENT, detail};
}

constexpr pe_rs_status not_found() noexcept {
    return {PE_RS_STATUS_NOT_FOUND, 0};
}

template <typename F>
pe_rs_status guarded(F&& f) noexcept {
    try {
        std::forward<F>(f)();
        return ok();
    } catch (const std::bad_alloc&) {
        return {PE_RS_STATUS_BAD_ALLOC, 0};
    } catch (const std::exception&) {
        return {PE_RS_STATUS_STD_EXCEPTION, 0};
    } catch (...) {
        return {PE_RS_STATUS_UNKNOWN_EXCEPTION, 0};
    }
}

bool valid_scene_type(std::int32_t value) noexcept {
    return value >= static_cast<std::int32_t>(scene_type::day) &&
        value <= static_cast<std::int32_t>(scene_type::mushroom_garden);
}

bool valid_plant_type(std::int32_t value) noexcept {
    return value >= static_cast<std::int32_t>(plant_type::pea_shooter) &&
        value <= static_cast<std::int32_t>(plant_type::imitater);
}

bool valid_imitater_type(std::int32_t value) noexcept {
    return value == static_cast<std::int32_t>(plant_type::none) ||
        (value >= static_cast<std::int32_t>(plant_type::pea_shooter) &&
            value <= static_cast<std::int32_t>(plant_type::melonpult));
}

bool valid_zombie_type(std::int32_t value) noexcept {
    switch (static_cast<zombie_type>(value)) {
    case zombie_type::zombie:
    case zombie_type::flag:
    case zombie_type::conehead:
    case zombie_type::pole_vaulting:
    case zombie_type::buckethead:
    case zombie_type::newspaper:
    case zombie_type::screendoor:
    case zombie_type::football:
    case zombie_type::dancing:
    case zombie_type::backup_dancer:
    case zombie_type::ducky_tube:
    case zombie_type::snorkel:
    case zombie_type::zomboni:
    case zombie_type::dolphin_rider:
    case zombie_type::jack_in_the_box:
    case zombie_type::balloon:
    case zombie_type::digger:
    case zombie_type::pogo:
    case zombie_type::yeti:
    case zombie_type::bungee:
    case zombie_type::ladder:
    case zombie_type::catapult:
    case zombie_type::gargantuar:
    case zombie_type::imp:
    case zombie_type::giga_gargantuar:
        return true;
    default:
        return false;
    }
}

bool valid_grid(const scene& scene, std::int32_t grid_x, std::int32_t grid_y) noexcept {
    return grid_x >= 0 && grid_x < 9 && scene.row_can_have_zombies(grid_y);
}

scene* native(pe_rs_scene* value) noexcept {
    return reinterpret_cast<scene*>(value);
}

const scene* native(const pe_rs_scene* value) noexcept {
    return reinterpret_cast<const scene*>(value);
}

plant_pool* native(pe_rs_plant_pool* value) noexcept {
    return reinterpret_cast<plant_pool*>(value);
}

const plant_pool* native(const pe_rs_plant_pool* value) noexcept {
    return reinterpret_cast<const plant_pool*>(value);
}

zombie_pool* native(pe_rs_zombie_pool* value) noexcept {
    return reinterpret_cast<zombie_pool*>(value);
}

const zombie_pool* native(const pe_rs_zombie_pool* value) noexcept {
    return reinterpret_cast<const zombie_pool*>(value);
}

griditem_pool* native(pe_rs_griditem_pool* value) noexcept {
    return reinterpret_cast<griditem_pool*>(value);
}

const griditem_pool* native(const pe_rs_griditem_pool* value) noexcept {
    return reinterpret_cast<const griditem_pool*>(value);
}

projectile_pool* native(pe_rs_projectile_pool* value) noexcept {
    return reinterpret_cast<projectile_pool*>(value);
}

const projectile_pool* native(const pe_rs_projectile_pool* value) noexcept {
    return reinterpret_cast<const projectile_pool*>(value);
}

bool card_belongs_to(const world& world, const pe_rs_card* card) noexcept {
    const auto address = reinterpret_cast<std::uintptr_t>(card);
    const auto begin = reinterpret_cast<std::uintptr_t>(world.scene.cards.data());
    const auto end = begin + sizeof(world.scene.cards);
    return address >= begin && address < end && (address - begin) % sizeof(pe_rs_card) == 0;
}

plant_weapon to_weapon(std::int32_t weapon) {
    return static_cast<plant_weapon>(weapon);
}

} // namespace

struct pe_rs_world {
    pe_rs_world(scene_type type, std::uint32_t battle_seed,
        std::uint32_t level_seed, std::uint32_t dancer_clock) :
        inner(type, battle_seed, level_seed, dancer_clock) {}
    world inner;
};

extern "C" pe_rs_status pe_rs_world_new_deterministic(
    std::int32_t scene_value,
    std::uint32_t battle_seed,
    std::uint32_t level_seed,
    std::uint32_t dancer_clock,
    pe_rs_world** out_world) noexcept
{
    if (out_world == nullptr) {
        return null_pointer();
    }
    *out_world = nullptr;
    if (!valid_scene_type(scene_value)) {
        return invalid(scene_value);
    }
    return guarded([&] {
        *out_world = new pe_rs_world(
            static_cast<scene_type>(scene_value), battle_seed, level_seed, dancer_clock);
    });
}

extern "C" pe_rs_status pe_rs_world_free(pe_rs_world* world) noexcept {
    delete world;
    return ok();
}

extern "C" pe_rs_status pe_rs_world_set_scene_type(pe_rs_world* world, std::int32_t scene_value) noexcept {
    if (world == nullptr) {
        return null_pointer();
    }
    if (!valid_scene_type(scene_value)) {
        return invalid(scene_value);
    }
    return guarded([&] { world->inner.scene.set_type(static_cast<scene_type>(scene_value)); });
}
extern "C" pe_rs_status pe_rs_world_reset_deterministic(
    pe_rs_world* world,
    std::int32_t scene_value,
    std::uint32_t battle_seed,
    std::uint32_t level_seed,
    std::uint32_t dancer_clock) noexcept
{
    if (world == nullptr) {
        return null_pointer();
    }
    if (!valid_scene_type(scene_value)) {
        return invalid(scene_value);
    }
    return guarded([&] {
        world->inner.scene.reset(
            static_cast<scene_type>(scene_value), battle_seed, level_seed, dancer_clock);
    });
}

extern "C" pe_rs_status pe_rs_world_update(pe_rs_world* world, std::uint8_t* out_terminal) noexcept {
    if (world == nullptr || out_terminal == nullptr) {
        return null_pointer();
    }
    return guarded([&] {
        world->inner.scene.events.frame_counter = world->inner.scene.main_counter;
        world->inner.scene.events.frame_open = true;
        try {
            *out_terminal = static_cast<std::uint8_t>(world->inner.update());
            world->inner.scene.events.frame_open = false;
        } catch (...) {
            world->inner.scene.events.frame_open = false;
            throw;
        }
    });
}

extern "C" pe_rs_status pe_rs_world_set_event_sink(
    pe_rs_world* world,
    std::uint32_t interest,
    pe_rs_begin_plant_effect_fn begin_plant_effect,
    pe_rs_finish_plant_effect_fn finish_plant_effect,
    pe_rs_emit_home_entry_fn emit_home_entry,
    pe_rs_emit_gargantuar_spawned_fn emit_gargantuar_spawned,
    pe_rs_emit_imp_thrown_fn emit_imp_thrown,
    pe_rs_emit_gargantuar_ash_hit_fn emit_gargantuar_ash_hit,
    pe_rs_emit_gargantuar_motion_fn emit_gargantuar_motion) noexcept
{
    if (world == nullptr || interest == 0 || begin_plant_effect == nullptr ||
        finish_plant_effect == nullptr || emit_home_entry == nullptr ||
        emit_gargantuar_spawned == nullptr || emit_imp_thrown == nullptr ||
        emit_gargantuar_ash_hit == nullptr || emit_gargantuar_motion == nullptr)
    {
        return null_pointer();
    }
    world->inner.scene.events = {
        interest,
        false,
        0,
        begin_plant_effect,
        finish_plant_effect,
        emit_home_entry,
        emit_gargantuar_spawned,
        emit_imp_thrown,
        emit_gargantuar_ash_hit,
        emit_gargantuar_motion,
    };
    return ok();
}

extern "C" pe_rs_status pe_rs_world_clear_event_sink(pe_rs_world* world) noexcept {
    if (world == nullptr) {
        return null_pointer();
    }
    world->inner.scene.events.clear();
    return ok();
}





extern "C" pe_rs_status pe_rs_world_restore_dancer_clock(pe_rs_world* world, std::uint32_t clock) noexcept {
    if (world == nullptr) {
        return null_pointer();
    }
    world->inner.scene.zombie_dancing_clock = clock;
    return ok();
}

extern "C" pe_rs_status pe_rs_world_scene(pe_rs_world* world, pe_rs_scene** out_scene) noexcept {
    if (world == nullptr || out_scene == nullptr) {
        return null_pointer();
    }
    *out_scene = reinterpret_cast<pe_rs_scene*>(&world->inner.scene);
    return ok();
}

extern "C" std::uint32_t pe_rs_world_imitater_morph_successor(
    const pe_rs_world* world, std::uint32_t placeholder_id) noexcept
{
    const auto* placeholder = world->inner.scene.plants.try_to_get(placeholder_id);
    return placeholder == nullptr ? 0 : placeholder->imitater_morph_successor_id;
}

extern "C" pe_rs_status pe_rs_world_reset_sun(pe_rs_world* world) noexcept {
    if (world == nullptr) {
        return null_pointer();
    }
    return guarded([&] { world->inner.sun.reset(); });
}

extern "C" pe_rs_status pe_rs_world_reset_spawn(pe_rs_world* world) noexcept {
    if (world == nullptr) {
        return null_pointer();
    }
    return guarded([&] { world->inner.spawn.reset(); });
}

extern "C" pe_rs_status pe_rs_scene_seed_rngs(
    pe_rs_scene* scene_value,
    std::uint32_t battle_seed,
    std::uint32_t level_seed) noexcept {
    if (scene_value == nullptr) {
        return null_pointer();
    }
    native(scene_value)->rng.seed(battle_seed);
    native(scene_value)->level_rng.seed(level_seed);
    return ok();
}

extern "C" pe_rs_status pe_rs_scene_lock_rngs(pe_rs_scene* scene_value, std::uint32_t value) noexcept {
    if (scene_value == nullptr) {
        return null_pointer();
    }
    native(scene_value)->rng.lock(value);
    native(scene_value)->level_rng.lock(value);
    return ok();
}

extern "C" pe_rs_status pe_rs_scene_set_wave_spawn_random_seed(
    pe_rs_scene* scene_value,
    std::uint8_t enabled,
    std::uint32_t base_seed) noexcept {
    if (scene_value == nullptr) {
        return null_pointer();
    }
    native(scene_value)->wave_spawn_random_enabled = enabled != 0;
    native(scene_value)->wave_spawn_random_seed = base_seed;
    return ok();
}

extern "C" pe_rs_status pe_rs_scene_battle_randint(
    pe_rs_scene* scene_value,
    std::uint32_t upper,
    std::uint32_t* out_value) noexcept {
    if (scene_value == nullptr || out_value == nullptr) {
        return null_pointer();
    }
    if (upper == 0) {
        return invalid(0);
    }
    *out_value = system::rng(native(scene_value)->rng).randint(upper);
    return ok();
}

#define PE_RS_DEFINE_RNG_SCALAR(name, type, expression) \
    extern "C" type pe_rs_scene_rng_##name(const pe_rs_scene* scene_value, bool level_stream) noexcept { \
        const auto& rng = level_stream ? native(scene_value)->level_rng : native(scene_value)->rng; \
        return (expression); \
    }


PE_RS_DEFINE_RNG_SCALAR(seed, std::uint32_t, rng.seed_value())
PE_RS_DEFINE_RNG_SCALAR(locked, std::uint8_t, static_cast<std::uint8_t>(rng.locked()))
PE_RS_DEFINE_RNG_SCALAR(fixed, std::uint32_t, rng.fixed_value())

#undef PE_RS_DEFINE_RNG_SCALAR


extern "C" std::uint32_t pe_rs_scene_main_counter(
    const pe_rs_scene* scene_value) noexcept {
    // The Rust caller holds a live borrowed scene.
    return native(scene_value)->main_counter;
}

extern "C" pe_rs_status pe_rs_scene_set_main_counter(pe_rs_scene* scene_value, std::uint32_t counter) noexcept {
    if (scene_value == nullptr) {
        return null_pointer();
    }
    native(scene_value)->main_counter = counter;
    return ok();
}

extern "C" std::uint8_t pe_rs_scene_game_over(const pe_rs_scene* scene_value) noexcept { return static_cast<std::uint8_t>(native(scene_value)->is_game_over); }

extern "C" std::uint32_t pe_rs_scene_dancer_clock(
    const pe_rs_scene* scene_value) noexcept {
    // The Rust caller holds a live borrowed scene.
    return native(scene_value)->zombie_dancing_clock;
}

extern "C" pe_rs_spawn_data* pe_rs_scene_spawn_data(
    pe_rs_scene* scene_value) noexcept {
    // The Rust caller holds a live borrowed scene.
    return &native(scene_value)->spawn;
}

extern "C" pe_rs_sun_data* pe_rs_scene_sun_data(pe_rs_scene* scene_value) noexcept {
    // The Rust caller holds a live borrowed scene.
    return &native(scene_value)->sun;
}

extern "C" pe_rs_ice_path_data* pe_rs_scene_ice_path_data(
    pe_rs_scene* scene_value) noexcept {
    // The Rust caller holds a live borrowed scene.
    return &native(scene_value)->ice_path;
}

extern "C" pe_rs_status pe_rs_scene_card_at(
    pe_rs_scene* scene_value,
    std::uint32_t slot,
    pe_rs_card** out_card) noexcept {
    if (scene_value == nullptr || out_card == nullptr) {
        return null_pointer();
    }
    *out_card = nullptr;
    if (slot >= native(scene_value)->cards.size()) {
        return invalid(static_cast<std::int32_t>(slot));
    }
    *out_card = &native(scene_value)->cards[slot];
    return ok();
}

extern "C" pe_rs_status pe_rs_scene_plant_map_incomplete(
    pe_rs_scene* scene_value, std::int32_t row, std::uint8_t* out_incomplete) noexcept {
    if (scene_value == nullptr || out_incomplete == nullptr) {
        return null_pointer();
    }
    if (row < 0 || static_cast<std::size_t>(row) >= native(scene_value)->plant_map_incomplete.size()) {
        return invalid(row);
    }
    *out_incomplete = native(scene_value)->plant_map_incomplete[static_cast<std::size_t>(row)];
    return ok();
}

extern "C" pe_rs_status pe_rs_scene_grid_plant_status_at(
    pe_rs_scene* scene_value,
    std::int32_t grid_x,
    std::int32_t grid_y,
    pe_rs_grid_plant_status** out_status) noexcept {
    if (scene_value == nullptr || out_status == nullptr) {
        return null_pointer();
    }
    *out_status = nullptr;
    if (!valid_grid(*native(scene_value), grid_x, grid_y)) {
        return invalid(grid_x);
    }
    *out_status = &native(scene_value)->plant_map[static_cast<std::size_t>(grid_y)][static_cast<std::size_t>(grid_x)];
    return ok();
}

extern "C" pe_rs_plant_pool* pe_rs_scene_plant_pool(
    pe_rs_scene* scene_value) noexcept {
    // The Rust caller holds a live borrowed scene.
    return reinterpret_cast<pe_rs_plant_pool*>(&native(scene_value)->plants);
}

extern "C" pe_rs_zombie_pool* pe_rs_scene_zombie_pool(
    pe_rs_scene* scene_value) noexcept {
    // The Rust caller holds a live borrowed scene.
    return reinterpret_cast<pe_rs_zombie_pool*>(&native(scene_value)->zombies);
}

extern "C" pe_rs_griditem_pool* pe_rs_scene_griditem_pool(
    pe_rs_scene* scene_value) noexcept {
    // The Rust caller holds a live borrowed scene.
    return reinterpret_cast<pe_rs_griditem_pool*>(&native(scene_value)->griditems);
}

extern "C" pe_rs_projectile_pool* pe_rs_scene_projectile_pool(
    pe_rs_scene* scene_value) noexcept {
    // The Rust caller holds a live borrowed scene.
    return reinterpret_cast<pe_rs_projectile_pool*>(&native(scene_value)->projectiles);
}

extern "C" std::uint8_t pe_rs_scene_is_pool_square(
    const pe_rs_scene* scene_value,
    std::int32_t grid_x,
    std::int32_t grid_y) noexcept { return static_cast<std::uint8_t>(native(scene_value)->is_pool_square(grid_x, grid_y)); }

extern "C" std::uint8_t pe_rs_scene_row_can_have_zombies(
    const pe_rs_scene* scene_value,
    std::int32_t row) noexcept { return static_cast<std::uint8_t>(native(scene_value)->row_can_have_zombies(row)); }

extern "C" pe_rs_status pe_rs_scene_grid_to_pixel_x(
    const pe_rs_scene* scene_value,
    std::int32_t grid_x,
    std::int32_t grid_y,
    std::int32_t* out_x) noexcept {
    if (scene_value == nullptr || out_x == nullptr) {
        return null_pointer();
    }
    if (!valid_grid(*native(scene_value), grid_x, grid_y)) {
        return invalid(grid_y);
    }
    *out_x = native(scene_value)->grid_to_pixel_x(grid_x, grid_y);
    return ok();
}

extern "C" pe_rs_status pe_rs_scene_grid_to_pixel_y(
    const pe_rs_scene* scene_value,
    std::int32_t grid_x,
    std::int32_t grid_y,
    std::int32_t* out_y) noexcept {
    if (scene_value == nullptr || out_y == nullptr) {
        return null_pointer();
    }
    if (!valid_grid(*native(scene_value), grid_x, grid_y)) {
        return invalid(grid_y);
    }
    *out_y = native(scene_value)->grid_to_pixel_y(grid_x, grid_y);
    return ok();
}

extern "C" pe_rs_status pe_rs_scene_pos_y_based_on_row(
    const pe_rs_scene* scene_value,
    float pos_x,
    std::int32_t row,
    float* out_y) noexcept {
    if (scene_value == nullptr || out_y == nullptr) {
        return null_pointer();
    }
    if (!native(scene_value)->row_can_have_zombies(row)) {
        return invalid(row);
    }
    *out_y = native(scene_value)->get_pos_y_based_on_row(pos_x, row);
    return ok();
}

#define PE_RS_DEFINE_POOL_API(prefix, c_pool_type, c_object_type, native_pool_type) \
    extern "C" std::uint32_t pe_rs_##prefix##_id_unchecked(const c_object_type* object) noexcept { \
        return native_pool_type::get_id_unchecked(*object); \
    } \
    extern "C" std::uint32_t pe_rs_##prefix##_pool_max_used_count(const c_pool_type* pool) noexcept { \
        return native(pool)->max_used_count(); \
    } \
    extern "C" std::uint32_t pe_rs_##prefix##_pool_active_count(const c_pool_type* pool) noexcept { \
        return native(pool)->active_count(); \
    } \
    extern "C" std::uint32_t pe_rs_##prefix##_pool_capacity(const c_pool_type*) noexcept { \
        return native_pool_type::capacity(); \
    } \
    extern "C" c_object_type* pe_rs_##prefix##_pool_get( \
        c_pool_type* pool, std::int32_t native_index) noexcept { \
        return native(pool)->get(native_index); \
    } \
    extern "C" c_object_type* pe_rs_##prefix##_pool_try_to_get(c_pool_type* pool, std::uint32_t id) noexcept \
    { return native(pool)->try_to_get(id); } \
    extern "C" std::uint32_t pe_rs_##prefix##_pool_get_id( \
        const c_pool_type* pool, const c_object_type* object) noexcept { \
        return native(pool)->get_id(object); \
    }

PE_RS_DEFINE_POOL_API(plant, pe_rs_plant_pool, pe_rs_plant, plant_pool)
PE_RS_DEFINE_POOL_API(zombie, pe_rs_zombie_pool, pe_rs_zombie, zombie_pool)
PE_RS_DEFINE_POOL_API(griditem, pe_rs_griditem_pool, pe_rs_griditem, griditem_pool)
PE_RS_DEFINE_POOL_API(projectile, pe_rs_projectile_pool, pe_rs_projectile, projectile_pool)

#undef PE_RS_DEFINE_POOL_API

extern "C" std::int32_t pe_rs_spawn_total_zombies_health_in_wave(
    const pe_rs_world* world,
    std::uint32_t wave) noexcept { return world->inner.spawn.total_zombies_health_in_wave(wave); }

extern "C" std::uint32_t pe_rs_spawn_current_health(
    const pe_rs_world* world) noexcept { return world->inner.spawn.get_current_hp(); }

extern "C" pe_rs_status pe_rs_spawn_pick_list(pe_rs_world* world, std::uint8_t* out_picked) noexcept {
    if (world == nullptr || out_picked == nullptr) {
        return null_pointer();
    }
    return guarded([&] {
        *out_picked = static_cast<std::uint8_t>(world->inner.spawn.pick_spawn_list());
    });
}

extern "C" pe_rs_status pe_rs_spawn_entry(
    const pe_rs_scene* scene_value,
    std::uint32_t wave,
    std::uint32_t slot,
    std::int32_t* out_zombie_type) noexcept {
    if (scene_value == nullptr || out_zombie_type == nullptr) {
        return null_pointer();
    }
    const auto& list = native(scene_value)->spawn.spawn_list;
    if (wave >= list.size() || slot >= list[wave].size()) {
        return invalid(static_cast<std::int32_t>(wave));
    }
    *out_zombie_type = static_cast<std::int32_t>(list[wave][slot]);
    return ok();
}

extern "C" pe_rs_status pe_rs_spawn_set_entry(
    pe_rs_scene* scene_value,
    std::uint32_t wave,
    std::uint32_t slot,
    std::int32_t zombie_value) noexcept {
    if (scene_value == nullptr) {
        return null_pointer();
    }
    auto& list = native(scene_value)->spawn.spawn_list;
    if (wave >= list.size() || slot >= list[wave].size() ||
        (zombie_value != static_cast<std::int32_t>(zombie_type::none) && !valid_zombie_type(zombie_value))) {
        return invalid(zombie_value);
    }
    list[wave][slot] = static_cast<zombie_type>(zombie_value);
    return ok();
}

extern "C" pe_rs_status pe_rs_spawn_flag(
    const pe_rs_scene* scene_value,
    std::uint32_t zombie_value,
    std::uint8_t* out_enabled) noexcept {
    if (scene_value == nullptr || out_enabled == nullptr) {
        return null_pointer();
    }
    const auto& flags = native(scene_value)->spawn.spawn_flags;
    if (zombie_value >= flags.size()) {
        return invalid(static_cast<std::int32_t>(zombie_value));
    }
    *out_enabled = static_cast<std::uint8_t>(flags[zombie_value]);
    return ok();
}

extern "C" pe_rs_status pe_rs_spawn_set_flag(
    pe_rs_scene* scene_value,
    std::uint32_t zombie_value,
    std::uint8_t enabled) noexcept {
    if (scene_value == nullptr) {
        return null_pointer();
    }
    auto& flags = native(scene_value)->spawn.spawn_flags;
    if (zombie_value >= flags.size() ||
        (enabled != 0 && !valid_zombie_type(static_cast<std::int32_t>(zombie_value)))) {
        return invalid(static_cast<std::int32_t>(zombie_value));
    }
    flags[zombie_value] = enabled != 0;
    return ok();
}

extern "C" std::int32_t pe_rs_seed_base_cost(std::int32_t kind) noexcept {
    return kind >= 0 && kind < 48 ? plant::COST_TABLE[kind] : -1;
}

extern "C" std::int32_t pe_rs_seed_base_recharge(std::int32_t kind) noexcept {
    return kind >= 0 && kind < 48 ? plant::CD_TABLE[kind] : -1;
}

extern "C" pe_rs_status pe_rs_current_plant_cost(
    const pe_rs_world* world,
    std::int32_t seed_value,
    std::int32_t imitater_value,
    std::int32_t* out_cost) noexcept {
    if (world == nullptr || out_cost == nullptr) {
        return null_pointer();
    }
    const auto seed = static_cast<plant_type>(seed_value);
    const auto imitater = static_cast<plant_type>(imitater_value);
    const bool valid_combination = valid_plant_type(seed_value) &&
        (seed == plant_type::imitater ?
            (valid_imitater_type(imitater_value) && imitater != plant_type::none) :
            imitater == plant_type::none);
    if (!valid_combination) {
        return invalid(seed_value);
    }
    return guarded([&] {
        *out_cost = world->inner.plant_factory.get_current_plant_cost(seed, imitater);
    });
}

extern "C" pe_rs_status pe_rs_can_take_sun_money(
    const pe_rs_world* world,
    std::int32_t amount,
    std::uint8_t* out_allowed) noexcept {
    if (world == nullptr || out_allowed == nullptr) {
        return null_pointer();
    }
    if (amount < 0) {
        return invalid(amount);
    }
    *out_allowed = static_cast<std::uint8_t>(world->inner.sun.can_take_sun_money(amount));
    return ok();
}

extern "C" pe_rs_status pe_rs_take_sun_money(
    pe_rs_world* world,
    std::int32_t amount,
    std::uint8_t* out_taken) noexcept {
    if (world == nullptr || out_taken == nullptr) {
        return null_pointer();
    }
    if (amount < 0) {
        return invalid(amount);
    }
    *out_taken = static_cast<std::uint8_t>(world->inner.sun.take_sun_money(amount));
    return ok();
}

extern "C" pe_rs_status pe_rs_seed_can_pick_up(
    const pe_rs_world* world,
    const pe_rs_card* card,
    std::uint8_t* out_can_pick_up) noexcept {
    if (world == nullptr || card == nullptr || out_can_pick_up == nullptr) {
        return null_pointer();
    }
    if (!card_belongs_to(world->inner, card)) {
        return invalid(0);
    }
    *out_can_pick_up = static_cast<std::uint8_t>(world->inner.plant_factory.can_pick_up(*card));
    return ok();
}

extern "C" pe_rs_status pe_rs_seed_was_planted(pe_rs_world* world, pe_rs_card* card) noexcept {
    if (world == nullptr || card == nullptr) {
        return null_pointer();
    }
    if (!card_belongs_to(world->inner, card)) {
        return invalid(0);
    }
    world->inner.plant_factory.was_planted(*card);
    return ok();
}

extern "C" pe_rs_status pe_rs_can_plant_at(
    const pe_rs_world* world,
    std::int32_t grid_x,
    std::int32_t grid_y,
    std::int32_t planting_value,
    std::int32_t* out_reason) noexcept {
    if (world == nullptr || out_reason == nullptr) {
        return null_pointer();
    }
    if (!valid_plant_type(planting_value)) {
        return invalid(planting_value);
    }
    *out_reason = static_cast<std::int32_t>(world->inner.plant_factory.can_plant_at(
        grid_x,
        grid_y,
        static_cast<plant_type>(planting_value)));
    return ok();
}

template <typename Create>
pe_rs_status create_plant(
    pe_rs_world* world,
    std::int32_t grid_x,
    std::int32_t grid_y,
    std::int32_t seed_value,
    std::int32_t imitater_value,
    pe_rs_plant** out_plant,
    Create&& create) noexcept {
    if (world == nullptr || out_plant == nullptr) {
        return null_pointer();
    }
    *out_plant = nullptr;
    if (!valid_plant_type(seed_value) || !valid_imitater_type(imitater_value)) {
        return invalid(seed_value);
    }
    return guarded([&] {
        *out_plant = create(
            grid_x,
            grid_y,
            static_cast<plant_type>(seed_value),
            static_cast<plant_type>(imitater_value));
    });
}

extern "C" pe_rs_status pe_rs_new_plant(
    pe_rs_world* world,
    std::int32_t grid_x,
    std::int32_t grid_y,
    std::int32_t seed_value,
    std::int32_t imitater_value,
    pe_rs_plant** out_plant) noexcept {
    return create_plant(world, grid_x, grid_y, seed_value, imitater_value, out_plant,
        [&](auto x, auto y, auto seed, auto imitater) {
            return world->inner.plant_factory.new_plant(x, y, seed, imitater);
        });
}

extern "C" pe_rs_status pe_rs_add_plant(
    pe_rs_world* world,
    std::int32_t grid_x,
    std::int32_t grid_y,
    std::int32_t seed_value,
    std::int32_t imitater_value,
    pe_rs_plant** out_plant) noexcept {
    return create_plant(world, grid_x, grid_y, seed_value, imitater_value, out_plant,
        [&](auto x, auto y, auto seed, auto imitater) {
            return world->inner.plant_factory.add_plant(x, y, seed, imitater);
        });
}

extern "C" pe_rs_status pe_rs_plant_die(pe_rs_world* world, pe_rs_plant* plant) noexcept {
    if (world == nullptr || plant == nullptr) {
        return null_pointer();
    }
    if (world->inner.scene.plants.get_id(plant) == 0) {
        return not_found();
    }
    world->inner.plant_factory.destroy(*plant);
    return ok();
}

extern "C" pe_rs_status pe_rs_plant_imitater_morph(pe_rs_world* world, pe_rs_plant* plant, pe_rs_plant** out_plant) noexcept {
    if (!world || !plant || !out_plant) return null_pointer();
    *out_plant = nullptr;
    if (world->inner.scene.plants.get_id(plant) == 0) return not_found();
    return guarded([&] {
        pvz_emulator::system::plant_imitater imitater(world->inner.scene);
        *out_plant = &imitater.morph(*plant);
    });
}

extern "C" pe_rs_status pe_rs_plant_set_sleep(pe_rs_plant* plant, std::uint8_t asleep) noexcept {
    if (plant == nullptr) {
        return null_pointer();
    }
    plant->set_sleep(asleep != 0);
    return ok();
}

extern "C" pe_rs_status pe_rs_plant_play_idle(pe_rs_plant* plant, float fps) noexcept {
    if (plant == nullptr) {
        return null_pointer();
    }
    plant->set_reanim(plant_reanim_name::anim_idle, reanim_type::repeat, fps);
    return ok();
}

extern "C" pe_rs_status pe_rs_plant_fire_cob(
    pe_rs_world* world,
    pe_rs_plant* plant,
    std::int32_t target_x,
    std::int32_t target_y,
    std::uint8_t* out_fired) noexcept {
    if (world == nullptr || plant == nullptr || out_fired == nullptr) {
        return null_pointer();
    }
    if (world->inner.scene.plants.get_id(plant) == 0) {
        return not_found();
    }
    *out_fired = static_cast<std::uint8_t>(
        system::plant_cob_cannon(world->inner.scene).launch(*plant, target_x, target_y));
    return ok();
}

extern "C" void pe_rs_plant_hit_box(pe_rs_plant* plant, pe_rs_rect* out_rect) noexcept {
    // Live input and writable output are guaranteed by the Rust borrow; weapon is validated.
    plant->get_hit_box(*out_rect);
}

extern "C" void pe_rs_plant_attack_rect(
    const pe_rs_plant* plant,
    std::int32_t weapon,
    pe_rs_rect* out_rect) noexcept {
    // Live input and writable output are guaranteed by the Rust borrow; weapon is validated.
    plant->get_plant_attack_rect(to_weapon(weapon), *out_rect);
}

extern "C" std::int32_t pe_rs_plant_damage_range_flags(const pe_rs_plant* plant, std::int32_t weapon) noexcept
{
    // Caller provides a live plant and a validated primary/secondary weapon.
    return plant->get_damage_range_flags(to_weapon(weapon));
}

extern "C" pe_rs_status pe_rs_add_zombie_in_row(
    pe_rs_world* world,
    std::int32_t zombie_value,
    std::int32_t row,
    std::int32_t from_wave,
    pe_rs_zombie** out_zombie) noexcept {
    if (world == nullptr || out_zombie == nullptr) {
        return null_pointer();
    }
    *out_zombie = nullptr;
    if (!valid_zombie_type(zombie_value)) {
        return invalid(zombie_value);
    }
    return guarded([&] {
        *out_zombie = world->inner.zombie_factory.add_zombie_in_row(
            static_cast<zombie_type>(zombie_value), row, from_wave);
    });
}

extern "C" pe_rs_status pe_rs_place_zombie(
    pe_rs_world* world,
    std::int32_t zombie_value,
    std::int32_t grid_x,
    std::int32_t grid_y,
    pe_rs_zombie** out_zombie) noexcept {
    if (world == nullptr || out_zombie == nullptr) {
        return null_pointer();
    }
    *out_zombie = nullptr;
    if (!valid_zombie_type(zombie_value)) {
        return invalid(zombie_value);
    }
    return guarded([&] {
        *out_zombie = world->inner.zombie_factory.place_zombie(
            static_cast<zombie_type>(zombie_value), grid_x, grid_y);
    });
}

extern "C" pe_rs_status pe_rs_zombie_die_no_loot(pe_rs_world* world, pe_rs_zombie* zombie) noexcept {
    if (world == nullptr || zombie == nullptr) {
        return null_pointer();
    }
    if (world->inner.scene.zombies.get_id(zombie) == 0) {
        return not_found();
    }
    world->inner.zombie_factory.destroy(*zombie);
    return ok();
}

extern "C" pe_rs_status pe_rs_zombie_can_be_attacked(
    const pe_rs_world* world,
    const pe_rs_zombie* zombie,
    std::uint8_t flags,
    std::uint8_t* out_can_be_attacked) noexcept {
    if (world == nullptr || zombie == nullptr || out_can_be_attacked == nullptr) {
        return null_pointer();
    }
    if (world->inner.scene.zombies.get_id(zombie) == 0) {
        return not_found();
    }
    *out_can_be_attacked = static_cast<std::uint8_t>(
        system::damage(const_cast<scene&>(world->inner.scene)).can_be_attacked(*zombie, flags));
    return ok();
}

extern "C" pe_rs_status pe_rs_zombie_can_attack_plant(
    pe_rs_world* world,
    pe_rs_zombie* zombie,
    pe_rs_plant* plant,
    std::int32_t attack_type,
    std::uint8_t* out_can_attack) noexcept {
    if (world == nullptr || zombie == nullptr || plant == nullptr || out_can_attack == nullptr) {
        return null_pointer();
    }
    if (world->inner.scene.zombies.get_id(zombie) == 0 ||
        world->inner.scene.plants.get_id(plant) == 0) {
        return not_found();
    }
    if (attack_type < 0 || attack_type > 3) {
        return invalid(attack_type);
    }
    *out_can_attack = static_cast<std::uint8_t>(
        system::zombie_base(world->inner.scene).can_attack_plant(
            *zombie, *plant, static_cast<system::zombie_attack_type>(attack_type)));
    return ok();
}

extern "C" void pe_rs_zombie_hit_box(
    const pe_rs_zombie* zombie,
    pe_rs_rect* out_rect) noexcept {
    // Live input and writable output are guaranteed by the Rust borrow; weapon is validated.
    zombie->get_hit_box(*out_rect);
}

extern "C" void pe_rs_projectile_attack_box(
    const pe_rs_projectile* projectile,
    pe_rs_rect* out_rect) noexcept {
    // Live input and writable output are guaranteed by the Rust borrow; weapon is validated.
    projectile->get_attack_box(*out_rect);
}

extern "C" pe_rs_status pe_rs_add_ladder(
    pe_rs_world* world,
    std::int32_t grid_x,
    std::int32_t grid_y,
    pe_rs_griditem** out_item) noexcept {
    if (world == nullptr || out_item == nullptr) {
        return null_pointer();
    }
    *out_item = nullptr;
    return guarded([&] { *out_item = world->inner.griditem_factory.add_ladder(grid_x, grid_y); });
}

extern "C" pe_rs_status pe_rs_add_crater(
    pe_rs_world* world,
    std::int32_t grid_x,
    std::int32_t grid_y,
    pe_rs_griditem** out_item) noexcept {
    if (world == nullptr || out_item == nullptr) {
        return null_pointer();
    }
    *out_item = nullptr;
    return guarded([&] { *out_item = world->inner.griditem_factory.add_crater(grid_x, grid_y); });
}

extern "C" pe_rs_status pe_rs_add_gravestone(
    pe_rs_world* world,
    std::int32_t grid_x,
    std::int32_t grid_y,
    pe_rs_griditem** out_item) noexcept {
    if (world == nullptr || out_item == nullptr) {
        return null_pointer();
    }
    *out_item = nullptr;
    return guarded([&] { *out_item = world->inner.griditem_factory.add_grave_stone(grid_x, grid_y); });
}

extern "C" pe_rs_status pe_rs_griditem_die(pe_rs_world* world, pe_rs_griditem* item) noexcept {
    if (world == nullptr || item == nullptr) {
        return null_pointer();
    }
    if (world->inner.scene.griditems.get_id(item) == 0) {
        return not_found();
    }
    world->inner.griditem_factory.destroy(*item);
    return ok();
}

#define PE_RS_DEFINE_BOOL_MODIFIER(api_name, member_name) \
    extern "C" std::uint8_t pe_rs_scene_##api_name(const pe_rs_scene* scene_value) noexcept { \
        return static_cast<std::uint8_t>(native(scene_value)->member_name); \
    } \
    extern "C" pe_rs_status pe_rs_scene_set_##api_name( \
        pe_rs_scene* scene_value, std::uint8_t enabled) noexcept { \
        if (scene_value == nullptr) return null_pointer(); \
        native(scene_value)->member_name = enabled != 0; \
        return ok(); \
    }

PE_RS_DEFINE_BOOL_MODIFIER(seed_recharge_ignored, seed_recharge_ignored)
PE_RS_DEFINE_BOOL_MODIFIER(sun_cost_ignored, ignore_sun_cost)
PE_RS_DEFINE_BOOL_MODIFIER(instant_special_effects, instant_special_effects)
PE_RS_DEFINE_BOOL_MODIFIER(easy_planting_cheat, easy_planting_cheat)
PE_RS_DEFINE_BOOL_MODIFIER(planting_restrictions_ignored, ignore_planting_restrictions)
PE_RS_DEFINE_BOOL_MODIFIER(mushrooms_awake, mushrooms_awake)
PE_RS_DEFINE_BOOL_MODIFIER(cob_delay_disabled, disable_cob_delay)
PE_RS_DEFINE_BOOL_MODIFIER(cob_fixed_delay, cob_fixed_delay)
PE_RS_DEFINE_BOOL_MODIFIER(cob_recharge_shortened, cob_recharge_shortened)
PE_RS_DEFINE_BOOL_MODIFIER(cob_drift_fixed, cob_drift_fixed)
PE_RS_DEFINE_BOOL_MODIFIER(natural_sun_drop_disabled, natural_sun_drop_disabled)
PE_RS_DEFINE_BOOL_MODIFIER(jack_explosions_disabled, jack_explosions_disabled)
PE_RS_DEFINE_BOOL_MODIFIER(special_events_disabled, special_events_disabled)
PE_RS_DEFINE_BOOL_MODIFIER(zombie_spawn_stopped, stop_spawn)
PE_RS_DEFINE_BOOL_MODIFIER(zombies_die_at_house, zombies_die_at_house)

#undef PE_RS_DEFINE_BOOL_MODIFIER

extern "C" std::int32_t pe_rs_scene_kernel_pult_rule(const pe_rs_scene* scene_value) noexcept { return static_cast<std::int32_t>(native(scene_value)->kernel_pult_projectiles); }

extern "C" pe_rs_status pe_rs_scene_set_kernel_pult_rule(
    pe_rs_scene* scene_value,
    std::int32_t rule) noexcept {
    if (scene_value == nullptr) {
        return null_pointer();
    }
    if (rule < 0 || rule > 2) {
        return invalid(rule);
    }
    native(scene_value)->kernel_pult_projectiles = static_cast<kernel_pult_projectile_rule>(rule);
    return ok();
}

extern "C" std::int32_t pe_rs_scene_plant_damage_rule(const pe_rs_scene* scene_value) noexcept { return static_cast<std::int32_t>(native(scene_value)->plant_damage); }

extern "C" pe_rs_status pe_rs_scene_set_plant_damage_rule(
    pe_rs_scene* scene_value,
    std::int32_t rule) noexcept {
    if (scene_value == nullptr) {
        return null_pointer();
    }
    if (rule < 0 || rule > 2) {
        return invalid(rule);
    }
    native(scene_value)->plant_damage = static_cast<plant_damage_rule>(rule);
    return ok();
}

extern "C" std::int32_t pe_rs_scene_maid_cheat(const pe_rs_scene* scene_value) noexcept { return static_cast<std::int32_t>(native(scene_value)->maid_cheat); }

extern "C" pe_rs_status pe_rs_scene_set_maid_cheat(
    pe_rs_scene* scene_value,
    std::int32_t state) noexcept {
    if (scene_value == nullptr) {
        return null_pointer();
    }
    if (state < 0 || state > 3) {
        return invalid(state);
    }
    native(scene_value)->maid_cheat = static_cast<maid_cheat_state>(state);
    return ok();
}

extern "C" pe_rs_status pe_rs_scene_set_dance_mode(pe_rs_scene* value, std::uint8_t enabled) noexcept {
    if (!value) return null_pointer();
    return guarded([&] { native(value)->set_dance_mode(enabled != 0); });
}
extern "C" std::uint8_t pe_rs_scene_dance_mode_enabled(pe_rs_scene* value) noexcept {
    return native(value)->is_zombie_dance;
}

extern "C" pe_rs_status pe_rs_world_select_plants(pe_rs_world* value, const std::int32_t* cards, std::uint32_t count, std::int32_t imitater) noexcept {
    if (!value || (!cards && count)) return null_pointer();
    if (count > 10) return invalid(count);
    return guarded([&] {
        std::vector<plant_type> selections;
        selections.reserve(count);
        for (std::uint32_t i = 0; i < count; ++i) selections.push_back(static_cast<plant_type>(cards[i]));
        if (!value->inner.select_plants(selections, static_cast<plant_type>(imitater)))
            throw std::invalid_argument("card selection");
    });
}

extern "C" pe_rs_status pe_rs_scene_set_common_zombie_dance(
    pe_rs_scene* scene_value,
    std::int32_t state) noexcept {
    if (scene_value == nullptr) {
        return null_pointer();
    }
    if (state < 0 || state > 2) {
        return invalid(state);
    }
    auto& scene = *native(scene_value);
    const auto dance = static_cast<zombie_dance_cheat>(state);
    scene.common_zombie_dance_cheat = dance;
    scene.is_zombie_dance = dance == zombie_dance_cheat::slow;
    for (auto& zombie : scene.zombies) {
        if (zombie.type == zombie_type::zombie ||
            zombie.type == zombie_type::conehead ||
            zombie.type == zombie_type::buckethead) {
            zombie.dance_cheat = dance;
        }
    }
    return ok();
}

extern "C" std::uint32_t pe_rs_plant_pool_free_list_head(const pe_rs_plant_pool* pool) noexcept {
    return native(pool)->free_list_head();
}
