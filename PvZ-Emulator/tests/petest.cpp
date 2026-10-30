#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
#include <tuple>
#include <vector>

#include "world.h"
#include "system/damage.h"
#include "system/plant/plant.h"
#include "system/projectile/projectile_factory.h"
#include "system/util.h"
#include "system/zombie/zombie.h"

using namespace pvz_emulator;
using namespace pvz_emulator::object;
using namespace pvz_emulator::system;

namespace {

void check(bool condition, const char* expression, int line) {
    if (condition) {
        return;
    }

    std::cerr << "CHECK failed at petest.cpp:" << line << ": "
              << expression << std::endl;
    std::exit(1);
}

#define CHECK(expr) check((expr), #expr, __LINE__)

template <typename F>
void expect_invalid_argument(F&& f) {
    bool rejected = false;
    try {
        f();
    } catch (const std::invalid_argument&) {
        rejected = true;
    }
    CHECK(rejected);
}

struct pool_probe {
    int value = 0;
    bool dead = false;

    bool is_freeable() {
        return dead;
    }
};

zombie& add_zombie(world& w, zombie_type type, int row, int from_wave = 0) {
    auto* result = w.zombie_factory.add_zombie_in_row(type, row, from_wave);
    CHECK(result != nullptr);
    return *result;
}

struct event_probe {
    int begins = 0;
    int finishes = 0;
    int homes = 0;
    bool suppress = false;
    std::uint8_t source = 0;
    std::uint8_t effect = 0;
    std::uint8_t outcome = 0;
    std::uint32_t actor_id = 0;
    std::uint32_t plant_id = 0;
    std::uint32_t home_id = 0;
    int raw_kind = 0;
    int effective_kind = 0;
    int row = 0;
    int col = 0;
    int hp = 0;
    int max_hp = 0;
    int requested = 0;
    int main_counter = 0;
    int applied = 0;
    int home_kind = 0;
    int home_row = 0;
    int home_counter = 0;
};

event_probe events;

std::uint32_t begin_event(
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
    std::int32_t requested,
    std::int32_t main_counter,
    std::uint8_t* out_suppress)
{
    ++events.begins;
    events.source = source;
    events.actor_id = actor_id;
    events.plant_id = plant_id;
    events.raw_kind = raw_kind;
    events.effective_kind = effective_kind;
    events.row = row;
    events.col = col;
    events.hp = hp;
    events.max_hp = max_hp;
    events.effect = effect;
    events.requested = requested;
    events.main_counter = main_counter;
    *out_suppress = events.suppress ? 1 : 0;
    return static_cast<std::uint32_t>(events.begins);
}

void finish_event(std::uint32_t, std::uint8_t outcome, std::int32_t applied) {
    ++events.finishes;
    events.outcome = outcome;
    events.applied = applied;
}

void home_event(
    std::uint32_t zombie_id,
    std::int32_t zombie_kind,
    std::int32_t row,
    std::int32_t main_counter)
{
    ++events.homes;
    events.home_id = zombie_id;
    events.home_kind = zombie_kind;
    events.home_row = row;
    events.home_counter = main_counter;
}

void observe(scene& scene, std::uint32_t interest, bool suppress = false) {
    events = {};
    events.suppress = suppress;
    scene.events.interest = interest;
    scene.events.begin_plant_effect = begin_event;
    scene.events.finish_plant_effect = finish_event;
    scene.events.emit_home_entry = home_event;
}


void test_scene_geometry_and_main_counter() {
    world garden(scene_type::mushroom_garden);
    garden.reset();
    CHECK(garden.scene.rows == 5);
    CHECK(garden.scene.grid_to_pixel_y(8, 4) == 480);
    CHECK(get_row_by_x_and_y(scene_type::mushroom_garden, 400, 530) == 4);
    CHECK(garden.zombie_factory.add_zombie_in_row(zombie_type::dancing, 2, 0) != nullptr);
    for (int i=0;i<20;++i) {
        auto& dancer=garden.zombie_factory.create(zombie_type::dancing);
        CHECK(dancer.row>=1 && dancer.row<=3);
    }
    CHECK(garden.zombie_factory.add_zombie_in_row(zombie_type::zomboni, 4, 0) != nullptr);
    CHECK(!garden.plant_factory.create(plant_type::puffshroom,0,0).is_sleeping);
    garden.scene.sun.sun=0;
    garden.scene.sun.natural_sun_countdown=1;
    garden.sun.update();
    CHECK(garden.scene.sun.sun==0);
    garden.scene.spawn.countdown.lurking_squad=1;
    garden.spawn.update();
    CHECK(garden.scene.griditems.size()==0);
    scene pool(scene_type::pool);
    CHECK(pool.is_pool_square(0, 2));
    CHECK(pool.is_pool_square(8, 3));
    CHECK(!pool.is_pool_square(0, 1));
    CHECK(!pool.is_pool_square(9, 2));
    CHECK(pool.is_water_grid(2, 0));
    CHECK(pool.row_can_have_zombies(5));
    CHECK(!pool.row_can_have_zombies(6));
    CHECK(pool.grid_to_pixel_x(3, 2) == get_x_by_col(3));
    CHECK(pool.grid_to_pixel_y(3, 2) ==
          get_y_by_row_and_col(scene_type::pool, 2, 3));

    scene roof(scene_type::roof);
    CHECK(roof.get_pos_y_based_on_row(400.0f, 2) ==
          get_y_by_row_and_x(scene_type::roof, 2, 400.0f));

    world regular(scene_type::day);
    regular.scene.stop_spawn = true;
    CHECK(regular.scene.main_counter == 0);
    CHECK(!regular.update());
    CHECK(regular.scene.main_counter == 1);
    CHECK(!regular.update(std::make_tuple(-3, 0, 0)));
    CHECK(regular.scene.main_counter == 2);
    regular.scene.set_type(scene_type::pool);
    CHECK(regular.scene.type == scene_type::pool);
    CHECK(regular.scene.rows == 6);
    CHECK(regular.scene.main_counter == 2);
    CHECK(regular.scene.stop_spawn);
    regular.reset(scene_type::pool);
    CHECK(regular.scene.main_counter == 0);

    world batched(scene_type::day);
    batched.scene.stop_spawn = true;
    std::vector<world*> worlds = {&batched};
    world::action_vector all_actions;
    world::action_vector actions = {{-3, 0, 0}};
    world::batch_action_masks masks;
    world::check_list checks;
    std::vector<int> check_result;
    std::vector<int> done;
    world::update_all(
        worlds, all_actions, actions, masks, checks, check_result, done, 3);
    CHECK(batched.scene.main_counter == 3);

    world terminal(scene_type::day);
    terminal.scene.sun.sun = 0;
    terminal.scene.sun.natural_sun_countdown = 1;
    terminal.scene.ice_path.countdown[0] = 2;
    terminal.scene.cards[0].type = plant_type::pea_shooter;
    terminal.scene.cards[0].cold_down = 1;
    auto& pending_effect = terminal.plant_factory.create(
        plant_type::cherry_bomb, 0, 0);
    pending_effect.countdown.effect = 2;
    auto& projectile = projectile_factory(terminal.scene).create(
        projectile_type::pea, 0, 801, 80);
    auto& intruder = add_zombie(terminal, zombie_type::zombie, 0);
    intruder.x = -100.0f;
    intruder.int_x = -100;
    intruder.dx = 0.0f;
    intruder.reanim.fps = 12.0f;
    intruder.reanim.n_frames = 100;
    intruder.reanim.progress = 0.25f;
    const float intruder_progress = intruder.reanim.progress;
    auto& later_zombie = add_zombie(terminal, zombie_type::zombie, 0);
    later_zombie.countdown.action = 10;

    CHECK(terminal.update());
    CHECK(terminal.scene.is_game_over);
    CHECK(!projectile.is_disappeared);
    CHECK(projectile.time_since_created == 1);
    CHECK(projectile.x == 801.0f);
    CHECK(pending_effect.countdown.effect == 1);
    CHECK(terminal.scene.cards[0].cold_down == 1);
    CHECK(terminal.scene.main_counter == 0);
    CHECK(intruder.reanim.progress > intruder_progress);
    CHECK(later_zombie.time_since_spawn == 1);
    CHECK(later_zombie.countdown.action == 10);
    CHECK(terminal.scene.sun.natural_sun_countdown == 1);
    CHECK(terminal.scene.ice_path.countdown[0] == 2);
    CHECK(terminal.update());
    CHECK(terminal.scene.main_counter == 0);
    CHECK(projectile.time_since_created == 1);
    CHECK(terminal.scene.cards[0].cold_down == 1);
}

void test_spawn_and_sun_atoms() {
    world w(scene_type::day);
    w.scene.rng.seed(1234);
    auto& normal = add_zombie(w, zombie_type::zombie, 0, 4);
    const int expected_hp = normal.hp + normal.accessory_1.hp +
        static_cast<int>(normal.accessory_2.hp * 0.2000000029802322) +
        (normal.has_balloon ? 20 : 0);
    CHECK(w.spawn.total_zombies_health_in_wave(4) == expected_hp);
    CHECK(w.spawn.total_zombies_health_in_wave(3) == 0);
    w.scene.spawn.wave = 5;
    CHECK(w.spawn.get_current_hp() == static_cast<unsigned int>(expected_hp));
    normal.is_hypno = true;
    CHECK(w.spawn.total_zombies_health_in_wave(4) == 0);

    w.spawn.reset();
    w.scene.spawn.wave = 7;
    w.scene.spawn.countdown.next_wave = 321;
    const auto flags = w.scene.spawn.spawn_flags;
    CHECK(w.spawn.pick_spawn_list());
    CHECK(w.scene.spawn.wave == 7);
    CHECK(w.scene.spawn.countdown.next_wave == 321);
    CHECK(w.scene.spawn.spawn_flags == flags);

    w.scene.spawn.spawn_flags.fill(false);
    CHECK(!w.spawn.pick_spawn_list());
    w.scene.spawn.spawn_flags[static_cast<int>(zombie_type::snorkel)] = true;
    CHECK(!w.spawn.pick_spawn_list());
    w.scene.spawn.spawn_flags[static_cast<int>(zombie_type::zombie)] = true;
    CHECK(w.spawn.pick_spawn_list());
    for (const auto& wave : w.scene.spawn.spawn_list) {
        for (const auto type : wave) {
            CHECK(type != zombie_type::snorkel);
        }
    }
    w.scene.reset(scene_type::roof, 1, 2, 3);
    w.scene.spawn.spawn_flags[static_cast<int>(zombie_type::dancing)] = true;
    CHECK(!w.spawn.pick_spawn_list());

    w.scene.sun.sun = 100;
    CHECK(w.sun.can_take_sun_money(100));
    CHECK(!w.sun.can_take_sun_money(101));
    CHECK(!w.sun.can_take_sun_money(-1));
    CHECK(w.sun.take_sun_money(25));
    CHECK(w.scene.sun.sun == 75);
    CHECK(!w.sun.take_sun_money(76));
    w.scene.ignore_sun_cost = true;
    CHECK(w.sun.can_take_sun_money(9990));
    CHECK(w.sun.take_sun_money(9990));
    CHECK(w.scene.sun.sun == 75);
    CHECK(!w.sun.take_sun_money(-1));
    w.scene.ignore_sun_cost = false;
    w.scene.sun.sun = 9980;
    w.sun.add_sun(50);
    CHECK(w.scene.sun.sun == 9990);
    w.scene.sun.limit = 0x7fffffffU;
    w.scene.sun.sun = 100000;
    w.sun.add_sun(50);
    CHECK(w.scene.sun.sun == 100050);
    CHECK(w.sun.take_sun_money(125));
    CHECK(w.scene.sun.sun == 99925);
    w.scene.type = scene_type::day;
    w.scene.sun.natural_sun_countdown = 1;
    w.sun.update();
    CHECK(w.scene.sun.sun == 99950);
    w.sun.add_sun(0xffffffffU);
    CHECK(w.scene.sun.sun == 0x7fffffffU);
    world independent(scene_type::day);
    CHECK(independent.scene.sun.limit == 9990);
    w.scene.reset(scene_type::day, 1, 2, 3);
    CHECK(w.scene.sun.limit == 9990);
}

void test_seed_cost_card_and_plant_atoms() {
    world w(scene_type::day);
    w.scene.sun.sun = 0;
    CHECK(w.plant_factory.can_plant_at(0, 0, plant_type::pea_shooter) ==
          planting_reason::ok);
    CHECK(!w.plant_factory.can_plant(0, 0, plant_type::pea_shooter));

    auto* repeater = w.plant_factory.new_plant(
        0, 0, plant_type::repeater);
    CHECK(repeater != nullptr);
    CHECK(w.scene.plant_map[0][0].content == repeater);
    CHECK(w.plant_factory.can_plant_at(0, 0, plant_type::gatling_pea) ==
          planting_reason::ok);
    auto* direct = w.plant_factory.add_plant(
        1, 0, plant_type::sunflower);
    CHECK(direct != nullptr);
    CHECK(w.scene.plant_map[0][1].content == direct);
    CHECK(w.plant_factory.new_plant(-1, 0, plant_type::pea_shooter) == nullptr);
    CHECK(w.plant_factory.add_plant(9, 0, plant_type::pea_shooter) == nullptr);

    const int gatling_base = static_cast<int>(
        plant::COST_TABLE[static_cast<int>(plant_type::gatling_pea)]);
    auto& existing_gatling = w.plant_factory.create(
        plant_type::gatling_pea, 1, 0);
    (void)existing_gatling;
    CHECK(w.plant_factory.get_current_plant_cost(
              plant_type::gatling_pea, plant_type::none) == gatling_base + 50);
    CHECK(w.plant_factory.get_current_plant_cost(
              plant_type::imitater, plant_type::melonpult) ==
          static_cast<int>(plant::COST_TABLE[
              static_cast<int>(plant_type::melonpult)]));
    expect_invalid_argument([&]() {
        (void)w.plant_factory.get_current_plant_cost(
            plant_type::imitater, plant_type::cob_cannon);
    });

    scene::card_data card;
    card.type = plant_type::pea_shooter;
    card.cold_down = 5;
    w.scene.sun.sun = 9990;
    CHECK(!w.plant_factory.can_pick_up(card));
    w.scene.seed_recharge_ignored = true;
    CHECK(w.plant_factory.can_pick_up(card));
    w.plant_factory.was_planted(card);
    CHECK(card.cold_down == 0);
    w.scene.seed_recharge_ignored = false;
    w.plant_factory.was_planted(card);
    CHECK(card.cold_down == plant::CD_TABLE[
              static_cast<int>(plant_type::pea_shooter)] + 1);

    world requirements(scene_type::day);
    requirements.scene.sun.sun = 9990;
    scene::card_data upgrade_card;
    upgrade_card.type = plant_type::gatling_pea;
    CHECK(!requirements.plant_factory.can_pick_up(upgrade_card));
    requirements.plant_factory.create(plant_type::repeater, 0, 0);
    CHECK(requirements.plant_factory.can_pick_up(upgrade_card));

    rect legacy_primary;
    rect strict_primary;
    rect legacy_secondary;
    rect strict_secondary;
    repeater->get_attack_box(legacy_primary, false);
    repeater->get_plant_attack_rect(plant_weapon::primary, strict_primary);
    repeater->get_attack_box(legacy_secondary, true);
    repeater->get_plant_attack_rect(plant_weapon::secondary, strict_secondary);
    CHECK(legacy_primary.x == strict_primary.x);
    CHECK(legacy_primary.y == strict_primary.y);
    CHECK(legacy_primary.width == strict_primary.width);
    CHECK(legacy_primary.height == strict_primary.height);
    CHECK(legacy_secondary.x == strict_secondary.x);
    CHECK(legacy_secondary.y == strict_secondary.y);
    CHECK(legacy_secondary.width == strict_secondary.width);
    CHECK(legacy_secondary.height == strict_secondary.height);
    CHECK(repeater->get_damage_range_flags(plant_weapon::primary) ==
          static_cast<int>(repeater->get_attack_flags(false)));
    CHECK(repeater->get_damage_range_flags(plant_weapon::secondary) ==
          static_cast<int>(repeater->get_attack_flags(true)));
}

void test_easy_planting_and_restriction_rules() {
    world pool_world(scene_type::pool);
    pool_world.scene.sun.sun = 0;
    scene::card_data& card = pool_world.scene.cards[0];
    card.type = plant_type::pea_shooter;
    card.cold_down = 100;

    pool_world.scene.easy_planting_cheat = true;
    CHECK(pool_world.plant_factory.can_pick_up(card));
    CHECK(pool_world.plant_factory.can_plant_at(
              0, 2, plant_type::pea_shooter) == planting_reason::not_on_water);
    CHECK(pool_world.plant_factory.can_plant_at(
              0, 2, plant_type::flower_pot) == planting_reason::not_on_water);
    CHECK(pool_world.plant_factory.can_plant_at(
              0, 2, plant_type::cattail) == planting_reason::ok);
    CHECK(pool_world.plant_factory.plant(0, 2, 0) == nullptr);

    pool_world.scene.ignore_planting_restrictions = true;
    CHECK(pool_world.plant_factory.can_plant_at(
              0, 2, plant_type::pea_shooter) == planting_reason::ok);
    auto* planted = pool_world.plant_factory.plant(0, 2, 0);
    CHECK(planted != nullptr);
    CHECK(pool_world.scene.sun.sun == 0);

    world upgrade(scene_type::day);
    upgrade.scene.easy_planting_cheat = true;
    CHECK(upgrade.plant_factory.can_plant_at(
              0, 0, plant_type::gatling_pea) == planting_reason::ok);
    CHECK(upgrade.plant_factory.can_plant_at(
              0, 0, plant_type::cattail) == planting_reason::not_here);
    upgrade.scene.easy_planting_cheat = false;
    CHECK(upgrade.plant_factory.can_plant_at(
              0, 0, plant_type::gatling_pea) == planting_reason::needs_upgrade);

    world easy_cob(scene_type::day);
    easy_cob.scene.easy_planting_cheat = true;
    CHECK(easy_cob.plant_factory.can_plant_at(
              0, 0, plant_type::cob_cannon) == planting_reason::ok);
}

void test_imitater_map_copy_and_cooldown_remap() {
    world pool_world(scene_type::pool);
    auto& imitated_lily = pool_world.plant_factory.create(
        plant_type::imitater, 2, 0, plant_type::lily_pad);
    auto& imitated_pumpkin = pool_world.plant_factory.create(
        plant_type::imitater, 0, 1, plant_type::pumpkin);
    CHECK(pool_world.scene.plant_map[2][0].base == &imitated_lily);
    CHECK(pool_world.scene.plant_map[0][1].pumpkin == &imitated_pumpkin);

    const auto lily_id = pool_world.scene.plants.get_id(imitated_lily);
    world copied(pool_world);
    auto* copied_lily = copied.scene.plants.try_to_get(lily_id);
    CHECK(copied_lily != nullptr);
    CHECK(copied_lily != &imitated_lily);
    CHECK(copied.scene.plant_map[2][0].base == copied_lily);

    world morph(scene_type::day);
    auto& placeholder = morph.plant_factory.create(
        plant_type::imitater, 0, 2, plant_type::melonpult);
    placeholder.status = plant_status::imitater_explode;
    placeholder.reanim.n_repeated = 1;
    plant_imitater(morph.scene).update(placeholder);
    auto* transformed = morph.scene.plant_map[0][2].content;
    CHECK(placeholder.is_dead);
    CHECK(transformed != nullptr);
    CHECK(transformed != &placeholder);
    CHECK(transformed->type == plant_type::melonpult);
    CHECK(transformed->imitater_target == plant_type::imitater);

    world cards(scene_type::day);
    CHECK(cards.select_plants(
        {plant_type::pea_shooter, plant_type::imitater},
        plant_type::wallnut));
    cards.scene.cards[0].cold_down = 123;
    const auto old_total =
        plant::CD_TABLE[static_cast<int>(plant_type::wallnut)] + 1;
    cards.scene.cards[1].cold_down = old_total - 100;
    CHECK(cards.select_plants(
        {plant_type::imitater, plant_type::pea_shooter},
        plant_type::cherry_bomb));
    CHECK(cards.scene.cards[1].cold_down == 123);
    const auto new_total =
        plant::CD_TABLE[static_cast<int>(plant_type::cherry_bomb)] + 1;
    CHECK(cards.scene.cards[0].cold_down ==
          (new_total > 100 ? new_total - 100 : 0));
}

void test_plant_effect_and_seed_modifiers() {
    world normal(scene_type::day);
    auto& normal_bomb = normal.plant_factory.create(
        plant_type::cherry_bomb, 0, 0);
    normal_bomb.countdown.effect = 10;
    normal.plant_system.update();
    CHECK(normal_bomb.countdown.effect == 9);
    CHECK(!normal_bomb.is_dead);

    world instant(scene_type::day);
    instant.scene.instant_special_effects = true;
    auto& instant_bomb = instant.plant_factory.create(
        plant_type::cherry_bomb, 0, 0);
    instant_bomb.countdown.effect = 10;
    instant.plant_system.update();
    CHECK(instant_bomb.countdown.effect == 0);
    CHECK(instant_bomb.is_dead);

    world sleeping(scene_type::day);
    sleeping.scene.instant_special_effects = true;
    auto& sleeping_bomb = sleeping.plant_factory.create(
        plant_type::cherry_bomb, 0, 0);
    sleeping_bomb.countdown.effect = 10;
    sleeping_bomb.is_sleeping = true;
    sleeping.plant_system.update();
    CHECK(sleeping_bomb.countdown.effect == 10);
    CHECK(!sleeping_bomb.is_dead);

    world mushroom(scene_type::day);
    auto& asleep = mushroom.plant_factory.create(plant_type::puffshroom, 0, 0);
    CHECK(asleep.is_sleeping);
    world awake(scene_type::day);
    awake.scene.mushrooms_awake = true;
    auto& awake_puff = awake.plant_factory.create(plant_type::puffshroom, 0, 0);
    CHECK(!awake_puff.is_sleeping);

    world recharge(scene_type::day);
    recharge.scene.cards[0].type = plant_type::pea_shooter;
    recharge.scene.cards[0].cold_down = 20;
    recharge.scene.seed_recharge_ignored = true;
    recharge.scene.stop_spawn = true;
    recharge.update();
    CHECK(recharge.scene.cards[0].cold_down == 0);
}

projectile& make_cob_probe(world& w, int x = 400) {
    auto& projectile = projectile_factory(w.scene).create(
        projectile_type::cob_cannon, 0, x, 100);
    projectile.motion_type = projectile_motion_type::parabola;
    projectile.dy1 = -100.0f;
    projectile.ddy = 0.0f;
    projectile.dddy = 0.0f;
    projectile.dy2 = 0.0f;
    return projectile;
}

plant& ready_cob(world& w, unsigned int row = 0, unsigned int col = 0) {
    auto& cannon = w.plant_factory.create(plant_type::cob_cannon, row, col);
    cannon.status = plant_status::cob_cannon_armed_idle;
    cannon.countdown.status = 0;
    cannon.countdown.launch = 0;
    return cannon;
}

void launch_cob(world& w, plant& cannon, unsigned int target_row) {
    constexpr int target_x = 400;
    const int target_y = w.scene.grid_to_pixel_y(4, target_row);
    CHECK(plant_cob_cannon(w.scene).launch(cannon, target_x, target_y));
}

projectile* live_cob(world& w) {
    for (auto& projectile : w.scene.projectiles) {
        if (projectile.type == projectile_type::cob_cannon) {
            return &projectile;
        }
    }
    return nullptr;
}

int cob_impact_time(
    scene_type type,
    unsigned int row,
    bool fixed_delay,
    bool add_target_on_first_land_frame)
{
    world w(type);
    w.scene.stop_spawn = true;
    w.scene.cob_fixed_delay = fixed_delay;
    auto& cannon = ready_cob(w, row);
    launch_cob(w, cannon, row);

    projectile* cob = nullptr;
    for (int elapsed = 1; elapsed <= 400; ++elapsed) {
        if (add_target_on_first_land_frame && elapsed == 373) {
            auto& target = add_zombie(w, zombie_type::zombie, static_cast<int>(row));
            target.x = static_cast<float>(get_x_by_col(4) - 30);
            target.int_x = static_cast<int>(target.x);
            target.y = zombie_init_y(type, target, row);
            target.int_y = static_cast<int>(target.y);
            target.countdown.freeze = 1000;
        }

        w.update();
        if (cob == nullptr) {
            cob = live_cob(w);
        }
        if (cob != nullptr && cob->is_disappeared) {
            return elapsed;
        }
    }
    return -1;
}

projectile& roof_cob_before_slope_transition(world& w, bool fixed_drift) {
    w.scene.stop_spawn = true;
    w.scene.cob_drift_fixed = fixed_drift;
    auto& cannon = ready_cob(w);
    launch_cob(w, cannon, 0);

    projectile* cob = nullptr;
    for (int elapsed = 1;
         elapsed <= 300 && (cob == nullptr || cob->time_since_created < 88);
         ++elapsed)
    {
        w.update();
        if (cob == nullptr) {
            cob = live_cob(w);
        }
    }
    CHECK(cob != nullptr);
    CHECK(cob->time_since_created == 88);
    CHECK(cob->dy1 < -700.0f);
    return *cob;
}

void test_cob_modifiers() {
    world separation(scene_type::day);
    separation.scene.stop_spawn = true;
    auto& separating_cannon = ready_cob(separation);
    launch_cob(separation, separating_cannon, 0);
    for (int elapsed = 1; elapsed < 205; ++elapsed) {
        separation.update();
        CHECK(live_cob(separation) == nullptr);
    }
    separation.update();
    CHECK(live_cob(separation) != nullptr);
    CHECK(live_cob(separation)->time_since_created == 1);

    CHECK(cob_impact_time(scene_type::day, 0, false, true) == 373);
    CHECK(cob_impact_time(scene_type::day, 0, false, false) == 376);
    CHECK(cob_impact_time(scene_type::day, 0, true, true) == 373);
    CHECK(cob_impact_time(scene_type::day, 0, true, false) == 373);
    CHECK(cob_impact_time(scene_type::pool, 2, false, false) == 378);
    CHECK(cob_impact_time(scene_type::pool, 2, true, false) == 378);

    world normal_drift(scene_type::roof);
    auto& drifting = roof_cob_before_slope_transition(normal_drift, false);
    const int normal_row = drifting.row;
    const float normal_x = drifting.x;
    const float normal_dy = drifting.dy1;
    const float normal_base_y = static_cast<float>(get_y_by_row_and_col(
        scene_type::roof,
        drifting.cannon_row,
        get_col_by_x(static_cast<int>(drifting.cannon_x))));
    const float slope_diff =
        get_y_by_row_and_x(scene_type::roof, normal_row, drifting.cannon_x) -
        get_y_by_row_and_x(scene_type::roof, normal_row, normal_x);
    CHECK(std::fabs(slope_diff) > 1.0f);
    normal_drift.update();
    CHECK(std::fabs(drifting.y - (normal_base_y + slope_diff)) < 0.001f);
    CHECK(std::fabs(drifting.dy1 - (normal_dy + 8.0f - slope_diff)) < 0.001f);
    CHECK(std::fabs(drifting.shadow_y - (normal_base_y + 67.0f + slope_diff)) < 0.001f);

    world fixed_drift(scene_type::roof);
    auto& stable = roof_cob_before_slope_transition(fixed_drift, true);
    const float stable_dy = stable.dy1;
    const float stable_base_y = static_cast<float>(get_y_by_row_and_col(
        scene_type::roof,
        stable.cannon_row,
        get_col_by_x(static_cast<int>(stable.cannon_x))));
    fixed_drift.update();
    CHECK(std::fabs(stable.y - stable_base_y) < 0.001f);
    CHECK(std::fabs(stable.dy1 - (stable_dy + 8.0f)) < 0.001f);
    CHECK(std::fabs(stable.shadow_y - (stable_base_y + 67.0f)) < 0.001f);

    world normal_other(scene_type::roof);
    world fixed_other(scene_type::roof);
    fixed_other.scene.cob_drift_fixed = true;
    auto& normal_cabbage = projectile_factory(normal_other.scene).create(
        projectile_type::cabbage, 0, 400, 100);
    auto& fixed_cabbage = projectile_factory(fixed_other.scene).create(
        projectile_type::cabbage, 0, 400, 100);
    normal_cabbage.motion_type = fixed_cabbage.motion_type =
        projectile_motion_type::parabola;
    normal_cabbage.time_since_created = fixed_cabbage.time_since_created = 21;
    normal_cabbage.dx = fixed_cabbage.dx = 4.0f;
    const float cabbage_y = normal_cabbage.y;
    normal_other.projectile.update();
    fixed_other.projectile.update();
    CHECK(normal_cabbage.y != cabbage_y);
    CHECK(normal_cabbage.x == fixed_cabbage.x);
    CHECK(normal_cabbage.y == fixed_cabbage.y);
    CHECK(normal_cabbage.dy1 == fixed_cabbage.dy1);
    CHECK(normal_cabbage.shadow_y == fixed_cabbage.shadow_y);

    world combined(scene_type::roof);
    combined.scene.cob_fixed_delay = true;
    combined.scene.cob_drift_fixed = true;
    auto& combined_projectile = make_cob_probe(combined);
    combined_projectile.dx = 4.0f;
    const float combined_y = combined_projectile.y;
    const float combined_dy = combined_projectile.dy1;
    const float combined_shadow = combined_projectile.shadow_y;
    combined.projectile.update();
    CHECK(combined_projectile.is_disappeared);
    CHECK(combined_projectile.y == combined_y);
    CHECK(combined_projectile.dy1 == combined_dy);
    CHECK(combined_projectile.shadow_y == combined_shadow);

    world normal_charge(scene_type::day);
    auto& unarmed = normal_charge.plant_factory.create(
        plant_type::cob_cannon, 0, 0);
    unarmed.status = plant_status::cob_cannon_unarmed_idle;
    unarmed.countdown.status = 500;
    plant_cob_cannon(normal_charge.scene).update(unarmed);
    CHECK(unarmed.status == plant_status::cob_cannon_unarmed_idle);

    world short_charge(scene_type::day);
    short_charge.scene.cob_recharge_shortened = true;
    auto& shortened = short_charge.plant_factory.create(
        plant_type::cob_cannon, 0, 0);
    shortened.status = plant_status::cob_cannon_unarmed_idle;
    shortened.countdown.status = 500;
    plant_cob_cannon(short_charge.scene).update(shortened);
    CHECK(shortened.status == plant_status::cob_cannon_charge);
    CHECK(shortened.countdown.status == 0);
}

void test_drop_and_special_event_modifiers() {
    world natural(scene_type::day);
    natural.scene.sun.sun = 0;
    natural.scene.sun.natural_sun_countdown = 1;
    natural.sun.update();
    CHECK(natural.scene.sun.sun == 25);

    world disabled_sun(scene_type::day);
    disabled_sun.scene.natural_sun_drop_disabled = true;
    disabled_sun.scene.sun.sun = 0;
    disabled_sun.scene.sun.natural_sun_countdown = 1;
    disabled_sun.sun.update();
    CHECK(disabled_sun.scene.sun.sun == 0);
    CHECK(disabled_sun.scene.sun.natural_sun_countdown == 1);

    world jack_disabled(scene_type::day);
    jack_disabled.scene.jack_explosions_disabled = true;
    auto& protected_plant = jack_disabled.plant_factory.create(
        plant_type::sunflower, 0, 3);
    auto& jack = add_zombie(jack_disabled, zombie_type::jack_in_the_box, 0);
    jack.status = zombie_status::jackbox_pop;
    jack.countdown.action = 0;
    jack.int_x = protected_plant.x;
    jack.int_y = protected_plant.y;
    jack.hit_box.offset_x = 0;
    jack.hit_box.offset_y = 0;
    zombie_jack_in_the_box(jack_disabled.scene).update(jack);
    CHECK(jack.is_dead);
    CHECK(!protected_plant.is_dead);

    world special(scene_type::night);
    special.scene.special_events_disabled = true;
    special.scene.spawn.wave = 0;
    special.scene.spawn.countdown.next_wave = 1;
    special.scene.spawn.countdown.next_wave_initial = 1;
    for (auto& type : special.scene.spawn.spawn_list[0]) {
        type = zombie_type::none;
    }
    special.scene.spawn.spawn_list[0][0] = zombie_type::bungee;
    special.spawn.update();
    CHECK(special.scene.spawn.wave == 1);
    CHECK(special.scene.zombies.active_count() == 0);

    special.scene.spawn.countdown.lurking_squad = 1;
    special.spawn.update();
    CHECK(special.scene.spawn.countdown.lurking_squad == 0);
    CHECK(special.scene.zombies.active_count() == 0);

    special.scene.spawn.wave = 19;
    special.scene.spawn.countdown.next_wave = 1;
    special.scene.spawn.countdown.next_wave_initial = 1;
    for (auto& type : special.scene.spawn.spawn_list[19]) {
        type = zombie_type::none;
    }
    special.spawn.update();
    CHECK(special.scene.griditems.active_count() == 0);

    world stopped(scene_type::day);
    stopped.scene.stop_spawn = true;
    stopped.scene.spawn.countdown.next_wave = 10;
    stopped.update();
    CHECK(stopped.scene.spawn.countdown.next_wave == 10);
}

void test_zombie_and_plant_damage_modifiers() {
    world normal(scene_type::day);
    auto& normal_plant = normal.plant_factory.create(
        plant_type::pea_shooter, 0, 0);
    const int normal_hp = normal_plant.hp;
    damage(normal.scene).take_plant(normal_plant, 25);
    CHECK(normal_plant.hp == normal_hp - 25);

    world invincible(scene_type::day);
    invincible.scene.plant_damage = plant_damage_rule::invincible;
    auto& invincible_plant = invincible.plant_factory.create(
        plant_type::pea_shooter, 0, 0);
    const int invincible_hp = invincible_plant.hp;
    damage(invincible.scene).take_plant(invincible_plant, 25);
    damage(invincible.scene).set_smashed(invincible_plant);
    CHECK(invincible_plant.hp == invincible_hp);
    CHECK(!invincible_plant.is_smashed);
    CHECK(!invincible_plant.is_dead);

    auto& basket = projectile_factory(invincible.scene).create(
        projectile_type::basketball,
        0,
        invincible_plant.x,
        invincible_plant.y);
    basket.motion_type = projectile_motion_type::parabola;
    basket.time_since_created = 21;
    basket.dy1 = 61.0f;
    invincible.projectile.update();
    CHECK(basket.is_disappeared);
    CHECK(invincible_plant.hp == invincible_hp);

    auto& garg = add_zombie(invincible, zombie_type::gargantuar, 0);
    garg.x = static_cast<float>(invincible_plant.x + 20);
    garg.int_x = invincible_plant.x + 20;
    garg.status = zombie_status::gargantuar_smash;
    garg.reanim.prev_progress = 0.6f;
    garg.reanim.progress = 0.7f;
    zombie_gargantuar(invincible.scene).update(garg);
    CHECK(!invincible_plant.is_smashed);
    CHECK(!invincible_plant.is_dead);

    auto& jack = add_zombie(invincible, zombie_type::jack_in_the_box, 0);
    jack.status = zombie_status::jackbox_pop;
    jack.countdown.action = 0;
    jack.int_x = invincible_plant.x;
    jack.int_y = invincible_plant.y;
    jack.hit_box.offset_x = 0;
    jack.hit_box.offset_y = 0;
    zombie_jack_in_the_box(invincible.scene).update(jack);
    CHECK(jack.is_dead);
    CHECK(!invincible_plant.is_dead);

    auto& eater = add_zombie(invincible, zombie_type::zombie, 0);
    eater.x = static_cast<float>(invincible_plant.x - 20);
    eater.int_x = invincible_plant.x - 20;
    eater.y = static_cast<float>(invincible_plant.y);
    eater.int_y = invincible_plant.y;
    damage(invincible.scene).set_is_eating(eater);
    eater.reanim.prev_progress = 0.1f;
    eater.reanim.progress = 0.15f;
    invincible.zombie.update();
    CHECK(invincible_plant.hp == invincible_hp);

    world weak(scene_type::day);
    weak.scene.plant_damage = plant_damage_rule::weak;
    auto& weak_plant = weak.plant_factory.create(
        plant_type::pea_shooter, 0, 0);
    damage(weak.scene).take_plant(weak_plant, 1);
    CHECK(weak_plant.is_dead);
    CHECK(weak.scene.plant_map[0][0].content == nullptr);

    world weak_smash(scene_type::day);
    weak_smash.scene.plant_damage = plant_damage_rule::weak;
    auto& weak_smashed = weak_smash.plant_factory.create(
        plant_type::sunflower, 0, 0);
    damage(weak_smash.scene).set_smashed(weak_smashed);
    CHECK(weak_smashed.is_dead);

    world lifecycle(scene_type::day);
    lifecycle.scene.plant_damage = plant_damage_rule::invincible;
    auto& cherry = lifecycle.plant_factory.create(
        plant_type::cherry_bomb, 0, 0);
    damage(lifecycle.scene).activate_plant(cherry);
    CHECK(cherry.is_dead);

    world house(scene_type::day);
    house.scene.stop_spawn = true;
    house.scene.zombies_die_at_house = true;
    auto& intruder = add_zombie(house, zombie_type::zombie, 0);
    intruder.x = -100.0f;
    intruder.int_x = -100;
    CHECK(!house.update());
    CHECK(!house.scene.is_game_over);
    CHECK(intruder.is_dead);
    CHECK(house.scene.main_counter == 1);
}

void test_gargantuar_motion_invalidation() {
    static unsigned calls = 0;
    static unsigned reasons = 0;
    static std::uint32_t last_id = 0;
    world w(scene_type::day);
    auto& giant = add_zombie(w, zombie_type::giga_gargantuar, 0);
    w.scene.events.interest = static_cast<std::uint32_t>(event_interest::gargantuar_motion);
    w.scene.events.frame_open = true;
    w.scene.events.frame_counter = 77;
    w.scene.events.emit_gargantuar_motion = [](std::uint32_t id, std::int32_t frame, std::uint32_t reason) {
        CHECK(frame == 77);
        ++calls;
        reasons |= reason;
        last_id = id;
    };
    pvz_emulator::system::reanim animation(w.scene);
    calls = reasons = 0;
    animation.update_dx(giant);
    CHECK(calls > 0 && (reasons & 1) != 0);
    CHECK(last_id == w.scene.zombies.get_id(giant));
    giant.countdown.freeze = 300;
    animation.update_fps(giant);
    calls = reasons = 0;
    animation.update_dx(giant);
    CHECK(calls == 0); // No notification of the hidden redraw while frozen.
    giant.countdown.freeze = 0;
    animation.update_fps(giant);
    CHECK(calls > 0 && (reasons & 4) != 0);
    calls = reasons = 0;
    animation.set(giant, zombie_reanim_name::anim_smash, reanim_type::once, 16);
    CHECK((reasons & 2) != 0);
    w.scene.events.clear();
    calls = 0;
    animation.update_dx(giant);
    CHECK(calls == 0);
}

void test_event_instrumentation() {
    const auto interest = [](event_interest value) {
        return static_cast<std::uint32_t>(value);
    };

    world unobserved(scene_type::day);
    auto& unobserved_plant = unobserved.plant_factory.create(
        plant_type::sunflower, 0, 3);
    auto& unobserved_jack = add_zombie(
        unobserved, zombie_type::jack_in_the_box, 0);
    unobserved_jack.status = zombie_status::jackbox_pop;
    unobserved_jack.countdown.action = 0;
    unobserved_jack.int_x = unobserved_plant.x;
    unobserved_jack.int_y = unobserved_plant.y;
    unobserved_jack.hit_box.offset_x = 0;
    unobserved_jack.hit_box.offset_y = 0;
    observe(unobserved.scene, 0);
    zombie_jack_in_the_box(unobserved.scene).update(unobserved_jack);
    CHECK(unobserved_plant.is_dead);
    CHECK(events.begins == 0 && events.finishes == 0);

    world jack_world(scene_type::day);
    auto& jack_target = jack_world.plant_factory.create(
        plant_type::imitater, 0, 3, plant_type::sunflower);
    auto& jack = add_zombie(jack_world, zombie_type::jack_in_the_box, 0);
    jack.status = zombie_status::jackbox_pop;
    jack.countdown.action = 0;
    jack.int_x = jack_target.x;
    jack.int_y = jack_target.y;
    jack.hit_box.offset_x = 0;
    jack.hit_box.offset_y = 0;
    observe(jack_world.scene, interest(event_interest::jack), true);
    zombie_jack_in_the_box(jack_world.scene).update(jack);
    CHECK(!jack_target.is_dead);
    CHECK(events.begins == 1 && events.finishes == 1);
    CHECK(events.source == static_cast<std::uint8_t>(plant_effect_source::jack));
    CHECK(events.effect == static_cast<std::uint8_t>(plant_effect::instant_kill));
    CHECK(events.outcome == static_cast<std::uint8_t>(plant_effect_outcome::suppressed));
    CHECK(events.actor_id == jack_world.scene.zombies.get_id(jack));
    CHECK(events.plant_id == jack_world.scene.plants.get_id(jack_target));
    CHECK(events.raw_kind == static_cast<int>(plant_type::imitater));
    CHECK(events.effective_kind == static_cast<int>(plant_type::sunflower));
    CHECK(events.row == 0 && events.col == 3 && events.requested == 0);
    CHECK(events.hp == jack_target.hp && events.max_hp == jack_target.max_hp);

    world bite_world(scene_type::day);
    auto& bite_target = bite_world.plant_factory.create(
        plant_type::wallnut, 0, 0);
    const int bite_hp = bite_target.hp;
    auto& eater = add_zombie(bite_world, zombie_type::zombie, 0);
    eater.x = static_cast<float>(bite_target.x - 20);
    eater.int_x = bite_target.x - 20;
    eater.y = static_cast<float>(bite_target.y);
    eater.int_y = bite_target.y;
    eater.time_since_spawn = 3;
    damage(bite_world.scene).set_is_eating(eater);
    eater.reanim.prev_progress = 0.1f;
    eater.reanim.progress = 0.15f;
    observe(bite_world.scene, interest(event_interest::bite));
    bite_world.zombie.update();
    CHECK(bite_target.hp == bite_hp - 4);
    CHECK(events.source == static_cast<std::uint8_t>(plant_effect_source::bite));
    CHECK(events.effect == static_cast<std::uint8_t>(plant_effect::hp_damage));
    CHECK(events.requested == 4 && events.applied == 4);
    CHECK(events.outcome == static_cast<std::uint8_t>(plant_effect_outcome::hp_delta));

    world lethal_bite_world(scene_type::day);
    auto& lethal_bite_target = lethal_bite_world.plant_factory.create(
        plant_type::sunflower, 0, 0);
    lethal_bite_target.hp = 4;
    auto& lethal_eater = add_zombie(
        lethal_bite_world, zombie_type::zombie, 0);
    lethal_eater.x = static_cast<float>(lethal_bite_target.x - 20);
    lethal_eater.int_x = lethal_bite_target.x - 20;
    lethal_eater.y = static_cast<float>(lethal_bite_target.y);
    lethal_eater.int_y = lethal_bite_target.y;
    lethal_eater.time_since_spawn = 3;
    damage(lethal_bite_world.scene).set_is_eating(lethal_eater);
    lethal_eater.reanim.prev_progress = 0.1f;
    lethal_eater.reanim.progress = 0.15f;
    observe(lethal_bite_world.scene, interest(event_interest::bite));
    lethal_bite_world.zombie.update();
    CHECK(lethal_bite_target.is_dead);
    CHECK(events.outcome == static_cast<std::uint8_t>(plant_effect_outcome::killed));

    world basket_world(scene_type::day);
    auto& basket_target = basket_world.plant_factory.create(
        plant_type::wallnut, 0, 0);
    const int basket_hp = basket_target.hp;
    auto& basket = projectile_factory(basket_world.scene).create(
        projectile_type::basketball,
        0,
        basket_target.x,
        basket_target.y);
    const auto basket_id = basket_world.scene.projectiles.get_id(basket);
    basket.motion_type = projectile_motion_type::parabola;
    basket.time_since_created = 21;
    basket.dy1 = 61.0f;
    observe(basket_world.scene, interest(event_interest::basketball));
    basket_world.projectile.update();
    CHECK(basket_target.hp == basket_hp - 75);
    CHECK(events.source == static_cast<std::uint8_t>(plant_effect_source::basketball));
    CHECK(events.actor_id == basket_id);
    CHECK(events.effect == static_cast<std::uint8_t>(plant_effect::hp_damage));
    CHECK(events.requested == 75 && events.applied == 75);
    CHECK(events.outcome == static_cast<std::uint8_t>(plant_effect_outcome::hp_delta));

    // Native Plant::Update@0x463ed7 uses hp<0, not hp<=0. A fourth
    // basketball leaves a300HP instant plant alive at zero health.
    basket_target.hp = 75;
    auto& zero_ball = projectile_factory(basket_world.scene).create(
        projectile_type::basketball, 0, basket_target.x, basket_target.y);
    zero_ball.motion_type = projectile_motion_type::parabola;
    zero_ball.time_since_created = 21;
    zero_ball.dy1 = 61.0f;
    basket_world.projectile.update();
    CHECK(basket_target.hp == 0 && !basket_target.is_dead);
    CHECK(events.outcome == static_cast<std::uint8_t>(plant_effect_outcome::hp_delta));
    CHECK(events.applied == 75);
    basket_world.plant_system.update();
    CHECK(!basket_target.is_dead);
    auto& lethal_ball = projectile_factory(basket_world.scene).create(
        projectile_type::basketball, 0, basket_target.x, basket_target.y);
    lethal_ball.motion_type = projectile_motion_type::parabola;
    lethal_ball.time_since_created = 21;
    lethal_ball.dy1 = 61.0f;
    basket_world.projectile.update();
    CHECK(basket_target.hp == -75);
    CHECK(events.outcome == static_cast<std::uint8_t>(plant_effect_outcome::killed));

    world spike_world(scene_type::day);
    auto& spike = spike_world.plant_factory.create(plant_type::spikerock, 0, 0);
    const int spike_hp = spike.hp;
    auto& spike_garg = add_zombie(spike_world, zombie_type::gargantuar, 0);
    spike_garg.x = static_cast<float>(spike.x + 20);
    spike_garg.int_x = spike.x + 20;
    spike_garg.status = zombie_status::gargantuar_smash;
    spike_garg.reanim.prev_progress = 0.6f;
    spike_garg.reanim.progress = 0.7f;
    observe(spike_world.scene, interest(event_interest::gargantuar));
    zombie_gargantuar(spike_world.scene).update(spike_garg);
    CHECK(spike.hp == spike_hp - 50);
    CHECK(events.source == static_cast<std::uint8_t>(plant_effect_source::gargantuar));
    CHECK(events.effect == static_cast<std::uint8_t>(plant_effect::hp_damage));
    CHECK(events.requested == 50 && events.applied == 50);

    world smash_world(scene_type::day);
    auto& smashed = smash_world.plant_factory.create(
        plant_type::sunflower, 0, 0);
    auto& smash_garg = add_zombie(smash_world, zombie_type::gargantuar, 0);
    smash_garg.x = static_cast<float>(smashed.x + 20);
    smash_garg.int_x = smashed.x + 20;
    smash_garg.status = zombie_status::gargantuar_smash;
    smash_garg.reanim.prev_progress = 0.6f;
    smash_garg.reanim.progress = 0.7f;
    observe(smash_world.scene, interest(event_interest::gargantuar));
    zombie_gargantuar(smash_world.scene).update(smash_garg);
    CHECK(smashed.is_smashed);
    CHECK(events.effect == static_cast<std::uint8_t>(plant_effect::squish));
    CHECK(events.outcome == static_cast<std::uint8_t>(plant_effect_outcome::squished));

    world house(scene_type::day);
    house.scene.stop_spawn = true;
    house.scene.zombies_die_at_house = true;
    auto& intruder = add_zombie(house, zombie_type::pogo, 2);
    const auto intruder_id = house.scene.zombies.get_id(intruder);
    intruder.x = -100.0f;
    intruder.int_x = -100;
    observe(house.scene, interest(event_interest::home_entry));
    CHECK(!house.update());
    CHECK(events.homes == 1);
    CHECK(events.home_id == intruder_id);
    CHECK(events.home_kind == static_cast<int>(zombie_type::pogo));
    CHECK(events.home_row == 2 && events.home_counter == 0);
    CHECK(house.scene.main_counter == 1);
}

void test_plant_loss_event_outcomes() {
    for (auto rule : {plant_damage_rule::normal, plant_damage_rule::invincible, plant_damage_rule::weak}) {
        world w(scene_type::day);
        w.scene.plant_damage = rule;
        auto& ice = w.plant_factory.create(plant_type::iceshroom, 0, 3);
        ice.is_sleeping = false;
        observe(w.scene, static_cast<std::uint32_t>(event_interest::vehicle_crush));
        const auto attempt = begin_effect(w.scene, plant_effect_source::zomboni_crush, 1,
            ice, plant_effect::squish, 0);
        damage(w.scene).set_smashed(ice);
        finish_effect(w.scene, ice, attempt);
        const auto expected = rule == plant_damage_rule::normal ? plant_effect_outcome::activated :
            rule == plant_damage_rule::invincible ? plant_effect_outcome::prevented : plant_effect_outcome::killed;
        CHECK(events.outcome == static_cast<std::uint8_t>(expected));
        CHECK(events.begins == 1 && events.finishes == 1);
    }

    world w(scene_type::day);
    auto& target = w.plant_factory.create(plant_type::imitater, 0, 3, plant_type::iceshroom);
    auto& bungee = add_zombie(w, zombie_type::bungee, 0);
    bungee.status = zombie_status::bungee_grab;
    bungee.bungee_target = static_cast<int>(w.scene.plants.get_id(target));
    target.edible = plant_edible_status::invisible_and_edible;
    observe(w.scene, static_cast<std::uint32_t>(event_interest::bungee));
    bungee.reanim.n_repeated = 0;
    zombie_bungee(w.scene).update(bungee);
    CHECK(events.begins == 0 && events.finishes == 0);
    bungee.reanim.n_repeated = 1;
    zombie_bungee(w.scene).update(bungee);
    CHECK(events.begins == 1 && events.finishes == 1);
    CHECK(events.source == static_cast<std::uint8_t>(plant_effect_source::bungee));
    CHECK(events.effect == static_cast<std::uint8_t>(plant_effect::steal));
    CHECK(events.outcome == static_cast<std::uint8_t>(plant_effect_outcome::stolen));
    CHECK(events.raw_kind == static_cast<int>(plant_type::imitater));
    CHECK(events.effective_kind == static_cast<int>(plant_type::iceshroom));
}

void test_kernel_and_maid_modifiers() {
    auto run_kernel = [](kernel_pult_projectile_rule rule) {
        world w(scene_type::day);
        w.scene.rng.seed(777);
        w.scene.kernel_pult_projectiles = rule;
        auto& kernel = w.plant_factory.create(plant_type::kernelpult, 0, 0);
        auto& target = add_zombie(w, zombie_type::zombie, 0);
        target.x = static_cast<float>(kernel.x + 200);
        target.int_x = static_cast<int>(target.x);
        plant_base(w.scene).set_launch_countdown(kernel, false);
        return std::make_tuple(kernel.status, w.scene.rng);
    };

    const auto [butter_status, butter_rng] =
        run_kernel(kernel_pult_projectile_rule::always_butter);
    const auto [kernel_status, kernel_rng] =
        run_kernel(kernel_pult_projectile_rule::always_kernel);
    CHECK(butter_status == plant_status::kernelpult_launch_butter);
    CHECK(kernel_status == plant_status::idle);
    CHECK(butter_rng == kernel_rng);

    auto run_dancer = [](maid_cheat_state state, int clock) {
        world w(scene_type::day);
        w.scene.zombie_dancing_clock = clock;
        w.scene.maid_cheat = state;
        auto& dancer = add_zombie(w, zombie_type::dancing, 2);
        dancer.x = 600.0f;
        dancer.int_x = 600;
        dancer.status = zombie_status::dancing_walking;
        dancer.catapult_or_jackson.summon_countdown = 1;
        zombie_dancing(w.scene).update(dancer);
        return dancer.status;
    };

    CHECK(run_dancer(maid_cheat_state::stop, 0) ==
          zombie_status::dancing_walking);
    CHECK(run_dancer(maid_cheat_state::call_partner, 0) ==
          zombie_status::dancing_wait_summoning);
    CHECK(run_dancer(maid_cheat_state::dancing, 0) ==
          zombie_status::dancing_armrise2);
    CHECK(run_dancer(maid_cheat_state::move, 0x140) ==
          zombie_status::dancing_walking);
    CHECK(run_dancer(
              maid_cheat_state::stop,
              std::numeric_limits<int>::min()) ==
          zombie_status::dancing_walking);

    world formation(scene_type::day);
    auto& dancer = add_zombie(formation, zombie_type::dancing, 2);
    dancer.x = 533.75f;
    dancer.int_x = 533;
    dancer.status = zombie_status::dancing_point;
    dancer.reanim.n_repeated = 1;
    zombie_dancing(formation.scene).update(dancer);
    auto* upper = formation.scene.zombies.try_to_get(
        static_cast<std::uint32_t>(dancer.partners[0]));
    auto* lower = formation.scene.zombies.try_to_get(
        static_cast<std::uint32_t>(dancer.partners[1]));
    auto* left = formation.scene.zombies.try_to_get(
        static_cast<std::uint32_t>(dancer.partners[2]));
    auto* right = formation.scene.zombies.try_to_get(
        static_cast<std::uint32_t>(dancer.partners[3]));
    CHECK(upper && upper->x == 533.0f);
    CHECK(lower && lower->x == 533.0f);
    CHECK(left && left->x == 433.0f);
    CHECK(right && right->x == 633.0f);
    CHECK(upper->reanim.begin_frame == 0 && upper->reanim.n_frames == 21);
    CHECK(lower->reanim.begin_frame == 0 && lower->reanim.n_frames == 21);
    CHECK(left->reanim.begin_frame == 0 && left->reanim.n_frames == 21);
    CHECK(right->reanim.begin_frame == 0 && right->reanim.n_frames == 21);
}

void test_zombie_creation_and_motion_regressions() {
    world wave8(scene_type::day);
    world wave9(scene_type::day);
    wave8.scene.rng.seed(123);
    wave9.scene.rng.seed(123);
    auto& ordinary = add_zombie(wave8, zombie_type::zombie, 0, 8);
    auto& huge = add_zombie(wave9, zombie_type::zombie, 0, 9);
    CHECK(ordinary.spawn_wave == 8);
    CHECK(huge.spawn_wave == 9);
    CHECK(huge.x == ordinary.x + 40.0f);
    CHECK(huge.int_x == static_cast<int>(huge.x));
    CHECK(huge.int_y == static_cast<int>(huge.y));

    world explicit_wave(scene_type::day);
    const zombie_type types[] = {
        zombie_type::backup_dancer,
        zombie_type::imp,
        zombie_type::bungee,
        zombie_type::gargantuar,
    };
    for (const auto type : types) {
        auto& zombie = add_zombie(explicit_wave, type, 0, 123456);
        CHECK(zombie.spawn_wave == 123456u);
    }
    CHECK(explicit_wave.zombie_factory.add_zombie_in_row(
              zombie_type::none, 0, 0) == nullptr);
    CHECK(explicit_wave.zombie_factory.add_zombie_in_row(
              zombie_type::zombie, -1, 0) == nullptr);
    CHECK(explicit_wave.zombie_factory.add_zombie_in_row(
              zombie_type::zombie, 0, -1) == nullptr);

    world placed(scene_type::day);
    placed.zombie_factory.place_zombie(zombie_type::zombie, 3, 2);
    auto* placed_normal = placed.scene.zombies.get(0);
    CHECK(placed_normal != nullptr);
    CHECK(placed_normal->row == 2);
    CHECK(placed_normal->spawn_wave == 0);
    CHECK(placed_normal->x == static_cast<float>(placed.scene.grid_to_pixel_x(3, 2) - 30));
    CHECK(placed_normal->int_x != static_cast<int>(placed_normal->x));
    placed.update();
    CHECK(placed_normal->int_x == static_cast<int>(placed_normal->x));

    world placed_bungee(scene_type::day);
    // Keep the initial random column distinct from the requested placement.
    placed_bungee.scene.rng.seed(123);
    placed_bungee.zombie_factory.place_zombie(zombie_type::bungee, 0, 1);
    auto* bungee = placed_bungee.scene.zombies.get(0);
    CHECK(bungee != nullptr);
    CHECK(bungee->row == 1);
    CHECK(bungee->bungee_col == 0);
    CHECK(bungee->x == static_cast<float>(
              placed_bungee.scene.grid_to_pixel_x(0, 1)));
    CHECK(bungee->int_x != static_cast<int>(bungee->x));
    placed_bungee.update();
    CHECK(bungee->int_x == static_cast<int>(bungee->x));

    world pool(scene_type::pool);
    pool.scene.rng.seed(99);
    pool.zombie_factory.create(zombie_type::bungee, 0);
    CHECK(pool.scene.spawn.row_random[2].b == 0.0f);
    CHECK(pool.scene.spawn.row_random[3].b == 0.0f);

    world dolphin_world(scene_type::pool);
    auto& dolphin = add_zombie(
        dolphin_world, zombie_type::dolphin_rider, 2);
    dolphin.status = zombie_status::dolphin_jump;
    dolphin.reanim.progress = 0.0f;
    dolphin.reanim.prev_progress = 0.0f;
    dolphin.countdown.action = 120;
    zombie_dolphin_rider(dolphin_world.scene).update(dolphin);
    CHECK(dolphin.dy == 0.0f);
    dolphin.countdown.action = 60;
    zombie_dolphin_rider(dolphin_world.scene).update(dolphin);
    CHECK(dolphin.dy == 5.0f);
    dolphin.countdown.action = 0;
    zombie_dolphin_rider(dolphin_world.scene).update(dolphin);
    CHECK(dolphin.dy == 10.0f);

    world digger_world(scene_type::day);
    auto& digger = add_zombie(digger_world, zombie_type::digger, 0);
    CHECK(digger.reanim.type == reanim_type::loop_full_last_frame);
    digger.status = zombie_status::digger_drill;
    digger.countdown.action = -5;
    zombie_digger(digger_world.scene).update(digger);
    CHECK(digger.dy == 0.0f);
    digger.countdown.action = 30;
    zombie_digger(digger_world.scene).update(digger);
    CHECK(digger.dy == 20.0f);
    digger.countdown.action = 40;
    zombie_digger(digger_world.scene).update(digger);
    CHECK(digger.dy == 20.0f);
}

void test_grid_atoms_and_debug_json_flags() {
    world w(scene_type::day);
    auto& strict_grave_plant = w.plant_factory.create(plant_type::pea_shooter, 4, 3);
    auto* ladder = w.griditem_factory.add_ladder(1, 2);
    auto* crater = w.griditem_factory.add_crater(2, 3);
    auto* grave = w.griditem_factory.add_grave_stone(3, 4);
    CHECK(ladder != nullptr && ladder->type == griditem_type::ladder);
    CHECK(ladder->col == 1 && ladder->row == 2);
    CHECK(crater != nullptr && crater->type == griditem_type::crater);
    CHECK(crater->countdown == 0);
    CHECK(grave != nullptr && grave->type == griditem_type::grave);
    CHECK(!strict_grave_plant.is_dead);
    CHECK(grave->countdown <= 0 && grave->countdown > -50);
    w.griditem.update();
    CHECK(crater->is_disappeared);

    auto& legacy_grave_plant = w.plant_factory.create(plant_type::pea_shooter, 0, 0);
    auto& legacy_crater = w.griditem_factory.create(griditem_type::crater, 0, 1);
    w.griditem_factory.create(griditem_type::grave, 0, 0);
    CHECK(legacy_crater.countdown == 18000);
    CHECK(legacy_grave_plant.is_dead);
    CHECK(w.griditem_factory.add_ladder(-1, 0) == nullptr);
    CHECK(w.griditem_factory.add_crater(9, 0) == nullptr);
    CHECK(w.griditem_factory.add_grave_stone(0, 5) == nullptr);
    w.griditem_factory.destroy(*ladder);
    CHECK(ladder->is_disappeared);

    auto& projectile = projectile_factory(w.scene).create(
        projectile_type::pea, 0, 40, 80);
    projectile.flags = static_cast<unsigned int>(attack_flags::ground) |
        static_cast<unsigned int>(attack_flags::dog) |
        static_cast<unsigned int>(attack_flags::hypno_zombies);
    std::string json;
    w.to_json(json);
    CHECK(json.find("\"dog\":true") != std::string::npos);
    CHECK(json.find("\"hypno_zombies\":true") != std::string::npos);
    CHECK(json.find("\"flying_balloon\":false") != std::string::npos);
}

void test_modifier_defaults_copy_and_reset() {
    CHECK(static_cast<int>(reanim_type::loop) == 0);
    CHECK(static_cast<int>(reanim_type::loop_full_last_frame) == 1);
    CHECK(static_cast<int>(reanim_type::play_once) == 2);
    CHECK(static_cast<int>(reanim_type::play_once_and_hold) == 3);
    CHECK(static_cast<int>(reanim_type::play_once_full_last_frame) == 4);
    CHECK(static_cast<int>(reanim_type::play_once_full_last_frame_and_hold) == 5);

    scene original(scene_type::day);
    CHECK(!original.seed_recharge_ignored);
    CHECK(!original.instant_special_effects);
    CHECK(!original.cob_fixed_delay);
    CHECK(!original.cob_recharge_shortened);
    CHECK(!original.cob_drift_fixed);
    CHECK(!original.jack_explosions_disabled);
    CHECK(!original.special_events_disabled);
    CHECK(original.kernel_pult_projectiles ==
          kernel_pult_projectile_rule::normal);
    CHECK(original.plant_damage == plant_damage_rule::normal);

    original.seed_recharge_ignored = true;
    original.instant_special_effects = true;
    original.cob_fixed_delay = true;
    original.cob_recharge_shortened = true;
    original.cob_drift_fixed = true;
    original.jack_explosions_disabled = true;
    original.special_events_disabled = true;
    original.kernel_pult_projectiles =
        kernel_pult_projectile_rule::always_butter;
    original.plant_damage = plant_damage_rule::weak;
    scene copied(original);
    CHECK(copied.seed_recharge_ignored);
    CHECK(copied.instant_special_effects);
    CHECK(copied.cob_fixed_delay);
    CHECK(copied.cob_recharge_shortened);
    CHECK(copied.cob_drift_fixed);
    CHECK(copied.jack_explosions_disabled);
    CHECK(copied.special_events_disabled);
    CHECK(copied.kernel_pult_projectiles ==
          kernel_pult_projectile_rule::always_butter);
    CHECK(copied.plant_damage == plant_damage_rule::weak);

    copied.reset();
    CHECK(!copied.seed_recharge_ignored);
    CHECK(!copied.instant_special_effects);
    CHECK(!copied.cob_fixed_delay);
    CHECK(!copied.cob_recharge_shortened);
    CHECK(!copied.cob_drift_fixed);
    CHECK(!copied.jack_explosions_disabled);
    CHECK(!copied.special_events_disabled);
    CHECK(copied.kernel_pult_projectiles ==
          kernel_pult_projectile_rule::normal);
    CHECK(copied.plant_damage == plant_damage_rule::normal);

    copied.reset(scene_type::roof, 11, 22, 33);
    CHECK(copied.type == scene_type::roof);
    CHECK(copied.rows == 5);
    CHECK(copied.rng.seed_value() == 11);
    CHECK(copied.level_rng.seed_value() == 22);
    CHECK(copied.zombie_dancing_clock == 33);
}

#undef CHECK

} // namespace

int main() {
    test_scene_geometry_and_main_counter();
    test_spawn_and_sun_atoms();
    test_seed_cost_card_and_plant_atoms();
    test_easy_planting_and_restriction_rules();
    test_imitater_map_copy_and_cooldown_remap();
    test_plant_effect_and_seed_modifiers();
    test_cob_modifiers();
    test_drop_and_special_event_modifiers();
    test_zombie_and_plant_damage_modifiers();
    test_event_instrumentation();
    test_gargantuar_motion_invalidation();
    test_plant_loss_event_outcomes();
    test_kernel_and_maid_modifiers();
    test_zombie_creation_and_motion_regressions();
    test_grid_atoms_and_debug_json_flags();
    test_modifier_defaults_copy_and_reset();
    return 0;
}
