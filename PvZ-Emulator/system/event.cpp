#include "system/event.h"

#include "object/plant.h"
#include "object/scene.h"
#include "object/zombie.h"

namespace pvz_emulator::system {

plant_effect_attempt begin_effect(
    object::scene& scene,
    plant_effect_source source,
    std::uint32_t actor_id,
    object::plant& plant,
    plant_effect effect,
    std::int32_t native_requested) noexcept
{
    plant_effect_attempt attempt{
        0,
        plant.hp,
        static_cast<std::int32_t>(plant.status),
        plant.is_dead,
        plant.is_smashed,
        false,
    };
    auto callback = scene.events.begin_plant_effect;
    attempt.effect = effect;
    attempt.zero_hp_survives = source == plant_effect_source::basketball;
    attempt.activates = effect == plant_effect::squish && !plant.is_sleeping &&
        scene.plant_damage == object::plant_damage_rule::normal &&
        (plant.type == object::plant_type::cherry_bomb || plant.type == object::plant_type::jalapeno ||
         plant.type == object::plant_type::doomshroom || plant.type == object::plant_type::iceshroom ||
         (plant.type == object::plant_type::potato_mine && plant.status != object::plant_status::idle));
    if (callback == nullptr) {
        return attempt;
    }

    std::uint8_t suppress = 0;
    const auto effective_kind = plant.type == object::plant_type::imitater ?
        plant.imitater_target : plant.type;
    attempt.token = callback(
        static_cast<std::uint8_t>(source),
        actor_id,
        scene.plants.get_id(plant),
        static_cast<std::int32_t>(plant.type),
        static_cast<std::int32_t>(effective_kind),
        static_cast<std::int32_t>(plant.row),
        static_cast<std::int32_t>(plant.col),
        plant.hp,
        plant.max_hp,
        static_cast<std::uint8_t>(effect),
        native_requested,
        static_cast<std::int32_t>(scene.main_counter),
        &suppress);
    attempt.suppressed = suppress != 0;
    return attempt;
}

void finish_effect(object::scene& scene, object::plant& plant, const plant_effect_attempt& attempt) noexcept {
    auto callback = scene.events.finish_plant_effect;
    if (attempt.token == 0 || callback == nullptr) {
        return;
    }

    plant_effect_outcome outcome;
    std::int32_t applied = 0;
    if (attempt.suppressed) {
        outcome = plant_effect_outcome::suppressed;
    } else if (attempt.effect == plant_effect::steal && plant.edible == object::plant_edible_status::invisible_and_not_edible) {
        outcome = plant_effect_outcome::stolen;
    } else if (attempt.activates && !attempt.dead && (plant.is_dead || static_cast<std::int32_t>(plant.status) != attempt.status)) {
        outcome = plant_effect_outcome::activated;
    } else if (!attempt.dead && (plant.is_dead || plant.hp < 0 ||
        (plant.hp == 0 && !attempt.zero_hp_survives))) {
        outcome = plant_effect_outcome::killed;
    } else if (!attempt.smashed && plant.is_smashed) {
        outcome = plant_effect_outcome::squished;
    } else if (plant.hp != attempt.hp) {
        outcome = plant_effect_outcome::hp_delta;
        applied = attempt.hp - plant.hp;
    } else if (static_cast<std::int32_t>(plant.status) != attempt.status) {
        outcome = plant_effect_outcome::activated;
    } else if (scene.plant_damage == object::plant_damage_rule::invincible) {
        outcome = plant_effect_outcome::prevented;
    } else {
        outcome = plant_effect_outcome::no_effect;
    }
    callback(attempt.token, static_cast<std::uint8_t>(outcome), applied);
}

void emit_gargantuar_motion(object::scene& scene, object::zombie& zombie, std::uint32_t reason) noexcept {
    if (!scene.events.frame_open || !scene.events.interested(event_interest::gargantuar_motion) ||
        scene.events.emit_gargantuar_motion == nullptr ||
        (zombie.type != object::zombie_type::gargantuar && zombie.type != object::zombie_type::giga_gargantuar)) {
        return;
    }
    scene.events.emit_gargantuar_motion(scene.zombies.get_id(zombie), scene.events.frame_counter, reason);
}

void emit_home_entry(object::scene& scene, object::zombie& zombie) noexcept {
    auto callback = scene.events.emit_home_entry;
    if (callback != nullptr) {
        callback(
            scene.zombies.get_id(zombie),
            static_cast<std::int32_t>(zombie.type),
            static_cast<std::int32_t>(zombie.row),
            static_cast<std::int32_t>(scene.main_counter));
    }
}

void emit_gargantuar_spawned(object::scene& scene, object::zombie& zombie) noexcept {
    auto callback = scene.events.emit_gargantuar_spawned;
    if (callback != nullptr && scene.events.frame_open && scene.events.interested(event_interest::imp_diagnostic)) {
        callback(
            scene.zombies.get_id(zombie),
            static_cast<std::int32_t>(zombie.type),
            static_cast<std::int32_t>(zombie.spawn_wave),
            static_cast<std::int32_t>(zombie.row),
            scene.events.frame_counter);
    }
}

void emit_imp_thrown(object::scene& scene, object::zombie& parent, object::zombie& imp) noexcept {
    auto callback = scene.events.emit_imp_thrown;
    if (callback != nullptr && scene.events.frame_open && scene.events.interested(event_interest::imp_diagnostic)) {
        callback(
            scene.zombies.get_id(parent),
            scene.zombies.get_id(imp),
            static_cast<std::int32_t>(parent.type),
            static_cast<std::int32_t>(parent.spawn_wave),
            static_cast<std::int32_t>(parent.row),
            static_cast<std::int32_t>(scene.main_counter),
            parent.x,
            parent.hp,
            static_cast<std::int32_t>(parent.max_hp),
            static_cast<std::int32_t>(parent.status),
            parent.dx,
            static_cast<std::int32_t>(parent.countdown.freeze),
            static_cast<std::int32_t>(parent.countdown.slow),
            static_cast<std::int32_t>(parent.countdown.butter));
    }
}

void emit_gargantuar_ash_hit(object::scene& scene, object::zombie& zombie) noexcept {
    auto callback = scene.events.emit_gargantuar_ash_hit;
    if (callback != nullptr && scene.events.frame_open && scene.events.interested(event_interest::imp_diagnostic)) {
        callback(
            scene.zombies.get_id(zombie),
            static_cast<std::int32_t>(scene.main_counter),
            zombie.x,
            zombie.hp,
            static_cast<std::int32_t>(zombie.status),
            static_cast<std::int32_t>(zombie.countdown.freeze),
            static_cast<std::int32_t>(zombie.countdown.slow),
            static_cast<std::int32_t>(zombie.countdown.butter));
    }
}

}
