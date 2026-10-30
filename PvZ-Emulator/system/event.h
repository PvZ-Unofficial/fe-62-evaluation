#pragma once

#include <cstdint>

namespace pvz_emulator::object {
class plant;
class scene;
class zombie;
}

namespace pvz_emulator::system {

enum class event_interest : std::uint32_t {
    jack = 1u << 0,
    bite = 1u << 1,
    gargantuar = 1u << 2,
    basketball = 1u << 3,
    home_entry = 1u << 4,
    imp_diagnostic = 1u << 5,
    vehicle_crush = 1u << 6,
    bungee = 1u << 7,
    gargantuar_motion = 1u << 9,
};

enum class plant_effect_source : std::uint8_t {
    jack,
    bite,
    gargantuar,
    basketball,
    zomboni_crush,
    catapult_crush,
    bungee,
};

enum class plant_effect : std::uint8_t {
    hp_damage,
    instant_kill,
    squish,
    steal,
};

enum class plant_effect_outcome : std::uint8_t {
    suppressed,
    prevented,
    hp_delta,
    killed,
    squished,
    activated,
    no_effect,
    stolen,
};

using begin_plant_effect_fn = std::uint32_t (*)(
    std::uint8_t source,
    std::uint32_t actor_id,
    std::uint32_t plant_id,
    std::int32_t raw_kind,
    std::int32_t effective_kind,
    std::int32_t row,
    std::int32_t col,
    std::int32_t hp,
    std::int32_t max_hp,
    std::uint8_t effect,
    std::int32_t native_requested,
    std::int32_t main_counter,
    std::uint8_t* out_suppress);
using finish_plant_effect_fn = void (*)(std::uint32_t token, std::uint8_t outcome, std::int32_t applied);
using emit_home_entry_fn = void (*)(
    std::uint32_t zombie_id,
    std::int32_t zombie_kind,
    std::int32_t row,
    std::int32_t main_counter);
using emit_gargantuar_spawned_fn = void (*)(
    std::uint32_t parent_id,
    std::int32_t parent_kind,
    std::int32_t from_wave,
    std::int32_t row,
    std::int32_t main_counter);
using emit_imp_thrown_fn = void (*)(
    std::uint32_t parent_id,
    std::uint32_t imp_id,
    std::int32_t parent_kind,
    std::int32_t from_wave,
    std::int32_t row,
    std::int32_t main_counter,
    float parent_x,
    std::int32_t parent_hp,
    std::int32_t parent_max_hp,
    std::int32_t parent_phase,
    float parent_speed_x,
    std::int32_t parent_frozen,
    std::int32_t parent_chilled,
    std::int32_t parent_buttered);
using emit_gargantuar_ash_hit_fn = void (*)(
    std::uint32_t parent_id,
    std::int32_t main_counter,
    float x,
    std::int32_t hp_before,
    std::int32_t phase,
    std::int32_t frozen,
    std::int32_t chilled,
    std::int32_t buttered);
// Invalidation only: never exposes a speed drawn while the actor is frozen.
using emit_gargantuar_motion_fn = void (*)(
    std::uint32_t zombie_id, std::int32_t main_counter, std::uint32_t reason);

struct event_sink {
    std::uint32_t interest = 0;
    bool frame_open = false;
    std::int32_t frame_counter = 0;
    begin_plant_effect_fn begin_plant_effect = nullptr;
    finish_plant_effect_fn finish_plant_effect = nullptr;
    emit_home_entry_fn emit_home_entry = nullptr;
    emit_gargantuar_spawned_fn emit_gargantuar_spawned = nullptr;
    emit_imp_thrown_fn emit_imp_thrown = nullptr;
    emit_gargantuar_ash_hit_fn emit_gargantuar_ash_hit = nullptr;
    emit_gargantuar_motion_fn emit_gargantuar_motion = nullptr;

    [[nodiscard]] bool interested(event_interest value) const noexcept {
        return (interest & static_cast<std::uint32_t>(value)) != 0;
    }

    void clear() noexcept { *this = {}; }
};

struct plant_effect_attempt {
    std::uint32_t token = 0;
    std::int32_t hp = 0;
    std::int32_t status = 0;
    bool dead = false;
    bool smashed = false;
    bool suppressed = false;
    bool activates = false;
    plant_effect effect = plant_effect::hp_damage;
    bool zero_hp_survives = false;
};

plant_effect_attempt begin_effect(
    object::scene& scene,
    plant_effect_source source,
    std::uint32_t actor_id,
    object::plant& plant,
    plant_effect effect,
    std::int32_t native_requested) noexcept;
void finish_effect(object::scene& scene, object::plant& plant, const plant_effect_attempt& attempt) noexcept;
void emit_home_entry(object::scene& scene, object::zombie& zombie) noexcept;
void emit_gargantuar_spawned(object::scene& scene, object::zombie& zombie) noexcept;
void emit_imp_thrown(object::scene& scene, object::zombie& parent, object::zombie& imp) noexcept;
void emit_gargantuar_ash_hit(object::scene& scene, object::zombie& zombie) noexcept;
void emit_gargantuar_motion(object::scene& scene, object::zombie& zombie, std::uint32_t reason) noexcept;

}
