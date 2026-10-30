#include <cstdlib>
#include <cmath>
#include <cstring>
#include <iostream>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <tuple>
#include <type_traits>
#include <vector>

#include "world.h"
#include "object/projectile.h"
#include "system/damage.h"
#include "system/debuff.h"
#include "system/util.h"
#include "system/projectile/projectile_factory.h"
#include "system/rng.h"
#include "system/zombie/zombie.h"
#include "learning/observation_factory.h"

using namespace pvz_emulator;
using namespace pvz_emulator::object;
using namespace pvz_emulator::system;

namespace {

void check(bool condition, const char* expression, int line) {
    if (condition) {
        return;
    }
    std::cerr << "CHECK failed at peparitytest.cpp:" << line << ": " << expression << std::endl;
    std::exit(1);
}

#define CHECK(expr) check((expr), #expr, __LINE__)

std::uint32_t float_bits(float value) {
    std::uint32_t bits;
    std::memcpy(&bits, &value, sizeof(bits));
    return bits;
}

float float_from_bits(std::uint32_t bits) {
    float value;
    std::memcpy(&value, &bits, sizeof(value));
    return value;
}

template <typename T, typename = void>
struct has_raw_observation_create : std::false_type {};

template <typename T>
struct has_raw_observation_create<T, std::void_t<decltype(static_cast<void (T::*)(
    world&,
    const world::action_masks&,
    float*)>(&T::create))>> : std::true_type {};

static_assert(
    !has_raw_observation_create<learning::observation_factory>::value,
    "observation_factory must not expose an unchecked raw-buffer create overload");

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

template <typename F>
void expect_bad_alloc(F&& f) {
    bool rejected = false;
    try {
        f();
    } catch (const std::bad_alloc&) {
        rejected = true;
    }
    CHECK(rejected);
}

zombie& make_zombie(world& w, zombie_type type, unsigned row, int x) {
    auto& z = w.zombie_factory.create(type, 0);
    z.row = row;
    z.x = static_cast<float>(x);
    z.int_x = x;
    z.y = 0;
    z.int_y = 0;
    return z;
}

bool is_valid_zombie_type(zombie_type type) {
    const auto raw = static_cast<int>(type);
    return (raw >= 0 && raw <= 0x18 && raw != 0x0d) ||
        type == zombie_type::giga_gargantuar;
}

int splash_damage_to_secondaries(projectile_type type) {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& main = make_zombie(w, zombie_type::zombie, 0, 80);
    std::vector<zombie*> secondaries;
    for (int i = 0; i < 8; ++i) {
        secondaries.push_back(&make_zombie(w, zombie_type::zombie, 0, 100));
    }

    auto& projectile = projectile_factory(w.scene).create(type, 0, 100, 0);
    projectile.flags = static_cast<unsigned int>(attack_flags::ground);
    w.projectile.update();
    CHECK(main.hp < static_cast<int>(main.max_hp));

    int total = 0;
    for (const auto zombie : secondaries) {
        total += static_cast<int>(zombie->max_hp) - zombie->hp;
    }
    return total;
}

void fill_graves_except(world& w, unsigned int keep_row, unsigned int keep_col) {
    for (unsigned int row = 0; row < w.scene.rows; ++row) {
        for (unsigned int col = 0; col < 9; ++col) {
            if (row != keep_row || col != keep_col) {
                w.griditem_factory.create(griditem_type::grave, row, col);
            }
        }
    }
}

void fill_graves_except_first_two_cols(world& w) {
    for (unsigned int row = 0; row < w.scene.rows; ++row) {
        for (unsigned int col = 0; col < 9; ++col) {
            if (row != 0 || col >= 2) {
                w.griditem_factory.create(griditem_type::grave, row, col);
            }
        }
    }
}

void set_single_zombie_wave(world& w, unsigned int wave, zombie_type type) {
    for (auto& entry : w.scene.spawn.spawn_list[wave]) {
        entry = zombie_type::none;
    }
    w.scene.spawn.spawn_list[wave][0] = type;
}

void arm_w20_countdown(world& w, unsigned int next_wave) {
    w.scene.spawn.wave = 20;
    w.scene.spawn.countdown.endgame = 0;
    w.scene.spawn.countdown.next_wave = next_wave;
    w.scene.spawn.countdown.next_wave_initial = 5500;
    w.scene.spawn.countdown.hugewave_fade = 0;
    w.scene.spawn.countdown.lurking_squad = 0;
}

void arm_night_w20_grave_spawn(world& w) {
    w.scene.spawn.wave = 19;
    w.scene.spawn.countdown.endgame = 0;
    w.scene.spawn.countdown.next_wave = 1;
    w.scene.spawn.countdown.next_wave_initial = 5500;
    w.scene.spawn.countdown.hugewave_fade = 0;
    w.scene.spawn.countdown.lurking_squad = 0;
}

unsigned int count_graves(const world& w) {
    unsigned int count = 0;
    for (const auto& item : w.scene.griditems) {
        if (item.type == griditem_type::grave) {
            ++count;
        }
    }
    return count;
}

void create_night_graves(world& w, unsigned int count) {
    unsigned int created = 0;
    for (unsigned int row = 0; row < 5 && created < count; ++row) {
        for (unsigned int col = 4; col < 9 && created < count; ++col) {
            w.griditem_factory.create(griditem_type::grave, row, col);
            ++created;
        }
    }
    CHECK(created == count);
}

void check_ice_timers_are_500(const world& w) {
    for (const auto countdown : w.scene.ice_path.countdown) {
        CHECK(countdown == 500);
    }
}

void test_upgrade_plant_placement_regressions() {
    auto w = std::make_unique<world>(scene_type::day);
    w->plant_factory.create(plant_type::pea_shooter, 0, 0);
    w->plant_factory.create(plant_type::repeater, 0, 1);
    w->plant_factory.create(plant_type::sunflower, 0, 2);
    w->plant_factory.create(plant_type::fumeshroom, 0, 3);
    w->plant_factory.create(plant_type::melonpult, 0, 4);
    w->plant_factory.create(plant_type::magnetshroom, 0, 5);

    CHECK(!w->plant_factory.can_plant(0, 0, plant_type::gatling_pea));
    CHECK(w->plant_factory.can_plant(0, 1, plant_type::gatling_pea));
    CHECK(w->plant_factory.can_plant(0, 2, plant_type::twin_sunflower));
    CHECK(w->plant_factory.can_plant(0, 3, plant_type::gloomshroom));
    CHECK(w->plant_factory.can_plant(0, 4, plant_type::winter_melon));
    CHECK(w->plant_factory.can_plant(0, 5, plant_type::gold_magnet));

    auto pool = std::make_unique<world>(scene_type::pool);
    pool->plant_factory.create(plant_type::lily_pad, 2, 0);
    CHECK(pool->plant_factory.can_plant(2, 0, plant_type::cattail));
}

void test_natural_sun_zero_countdown_waits_one_frame() {
    world w(scene_type::day);
    w.scene.sun.sun = 0;
    w.scene.sun.natural_sun_generated = 0;
    w.scene.sun.natural_sun_countdown = 0;

    w.sun.update();

    CHECK(w.scene.sun.sun == 0);
    CHECK(w.scene.sun.natural_sun_generated == 0);
    CHECK(w.scene.sun.natural_sun_countdown >= 424);
    CHECK(w.scene.sun.natural_sun_countdown <= 699);
}

void test_cob_cannon_rejects_double_square_pumpkins() {
    auto can_place_cob = [](bool left_pumpkin, bool right_pumpkin) {
        auto w = std::make_unique<world>(scene_type::day);
        w->plant_factory.create(plant_type::kernelpult, 0, 0);
        w->plant_factory.create(plant_type::kernelpult, 0, 1);
        if (left_pumpkin) {
            w->plant_factory.create(plant_type::pumpkin, 0, 0);
        }
        if (right_pumpkin) {
            w->plant_factory.create(plant_type::pumpkin, 0, 1);
        }

        return w->plant_factory.can_plant(0, 0, plant_type::cob_cannon);
    };

    CHECK(can_place_cob(false, false));
    CHECK(!can_place_cob(true, false));
    CHECK(!can_place_cob(false, true));
}

void test_magnetshroom_ladder_range_and_row_weight() {
    auto far_world = std::make_unique<world>(scene_type::night);
    far_world->plant_factory.create(plant_type::magnetshroom, 1, 1);
    auto& far_ladder = far_world->griditem_factory.create(griditem_type::ladder, 4, 8);
    far_world->plant_system.update();
    CHECK(!far_ladder.is_disappeared);

    auto weighted_world = std::make_unique<world>(scene_type::night);
    weighted_world->plant_factory.create(plant_type::magnetshroom, 1, 1);
    auto& same_row_ladder = weighted_world->griditem_factory.create(griditem_type::ladder, 1, 3);
    auto& diagonal_ladder = weighted_world->griditem_factory.create(griditem_type::ladder, 3, 3);
    weighted_world->plant_system.update();

    CHECK(same_row_ladder.is_disappeared);
    CHECK(!diagonal_ladder.is_disappeared);
}

void test_magnetshroom_float_distance_tie_keeps_first_target() {
    world w(scene_type::night);
    auto& magnet = w.plant_factory.create(plant_type::magnetshroom, 2, 4);
    auto& first = make_zombie(w, zombie_type::buckethead, 2, 38);
    first.y = 181.0f;
    first.int_y = 181;
    auto& second = make_zombie(w, zombie_type::buckethead, 3, 546);
    second.y = 297.0f;
    second.int_y = 297;

    rect first_box;
    rect second_box;
    first.get_hit_box(first_box);
    second.get_hit_box(second_box);
    CHECK(magnet.x == 360.0f);
    CHECK(magnet.y == 280.0f);
    CHECK(first_box.x == 74 && first_box.y == 181);
    CHECK(second_box.x == 582 && second_box.y == 297);

    w.plant_system.update();

    CHECK(first.accessory_1.type == zombie_accessories_type_1::none);
    CHECK(second.accessory_1.type == zombie_accessories_type_1::bucket);
}

void test_straight_projectile_edge_overlap_hits() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& z = make_zombie(w, zombie_type::zombie, 0, 104);
    auto& pea = projectile_factory(w.scene).create(projectile_type::snow_pea, 0, 100, 0);
    pea.flags = static_cast<unsigned int>(attack_flags::ground);

    w.projectile.update();

    CHECK(pea.is_disappeared);
    CHECK(z.hp == static_cast<int>(z.max_hp) - 20);
}

void test_basketball_hits_low_plants() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& potato = w.plant_factory.create(plant_type::potato_mine, 0, 3);
    auto& ball = projectile_factory(w.scene).create(projectile_type::basketball, 0, potato.x, potato.y);
    ball.motion_type = projectile_motion_type::parabola;
    ball.time_since_created = 21;
    ball.dy1 = 61.0f;

    w.projectile.update();

    CHECK(ball.is_disappeared);
    CHECK(potato.hp == 225);
}

void test_basketball_skips_not_on_ground_top_layer() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& content = w.plant_factory.create(plant_type::pea_shooter, 0, 3);
    auto& pumpkin = w.plant_factory.create(plant_type::pumpkin, 0, 3);
    content.edible = plant_edible_status::invisible_and_not_edible;
    pumpkin.x = content.x + 200;
    const auto content_hp = content.hp;
    const auto pumpkin_hp = pumpkin.hp;

    auto& ball = projectile_factory(w.scene).create(projectile_type::basketball, 0, content.x, content.y);
    ball.motion_type = projectile_motion_type::parabola;
    ball.time_since_created = 21;
    ball.dy1 = 61.0f;

    w.projectile.update();

    CHECK(ball.is_disappeared);
    CHECK(content.hp == content_hp);
    CHECK(pumpkin.hp == pumpkin_hp - static_cast<int>(projectile::DAMAGE[static_cast<int>(projectile_type::basketball)]));
}

void test_fire_pea_fire_resistant_target_no_splash() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& door = make_zombie(w, zombie_type::screendoor, 0, 64);
    door.accessory_2.type = zombie_accessories_type_2::screen_door;
    door.accessory_2.hp = 400;
    door.accessory_2.max_hp = 400;
    auto& secondary = make_zombie(w, zombie_type::zombie, 0, 90);

    auto& fire = projectile_factory(w.scene).create(projectile_type::fire_pea, 0, 100, 0);
    fire.flags = static_cast<unsigned int>(attack_flags::ground);

    w.projectile.update();

    CHECK(fire.is_disappeared);
    CHECK(door.hp == static_cast<int>(door.max_hp));
    CHECK(door.accessory_2.hp == 360);
    CHECK(secondary.hp == static_cast<int>(secondary.max_hp));
}

void test_iceshroom_water_freeze_fixed_300() {
    world w(scene_type::pool);
    w.scene.stop_spawn = true;

    auto& z = make_zombie(w, zombie_type::zombie, 2, 300);
    z.is_in_water = true;
    auto& ice = w.plant_factory.create(plant_type::iceshroom, 0, 0);
    ice.is_sleeping = false;

    damage(w.scene).activate_plant(ice);

    CHECK(z.countdown.freeze == 300);
    CHECK(z.countdown.slow > 0);
}

void test_opening_jackbox_immune_to_freeze_and_butter() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;
    w.scene.rng.seed(1);

    auto& popping = make_zombie(w, zombie_type::jack_in_the_box, 2, 300);
    popping.status = zombie_status::jackbox_pop;
    popping.countdown.action = 10;
    const int popping_hp = popping.hp;

    auto& running = make_zombie(w, zombie_type::jack_in_the_box, 2, 360);
    running.status = zombie_status::jackbox_walking;
    running.countdown.action = 100;

    auto& ice = w.plant_factory.create(plant_type::iceshroom, 0, 0);
    ice.is_sleeping = false;

    damage(w.scene).activate_plant(ice);

    CHECK(popping.countdown.freeze == 0);
    CHECK(popping.countdown.slow > 0);
    CHECK(popping.hp == popping_hp);
    CHECK(running.countdown.freeze > 0);
    CHECK(running.countdown.slow > 0);

    debuff(w.scene).set_butter(popping);
    CHECK(popping.countdown.butter == 0);

    debuff(w.scene).set_butter(running);
    CHECK(running.countdown.butter == 400);

    w.zombie.update();
    CHECK(popping.countdown.action == 9);
}

void test_jalapeno_shortens_ice_path() {
    world w(scene_type::pool);
    w.scene.stop_spawn = true;
    w.scene.ice_path.countdown[2] = 999;

    auto& jalapeno = w.plant_factory.create(plant_type::jalapeno, 2, 0);
    damage(w.scene).activate_plant(jalapeno);

    CHECK(w.scene.ice_path.countdown[2] == 20);
}

void test_bungee_grab_uses_content_before_pumpkin_and_base() {
    world w(scene_type::day);
    auto& base = w.plant_factory.create(plant_type::flower_pot, 0, 0);
    auto& content = w.plant_factory.create(plant_type::pea_shooter, 0, 0);
    auto& pumpkin = w.plant_factory.create(plant_type::pumpkin, 0, 0);
    auto content_id = static_cast<int>(w.scene.plants.get_id(content));

    auto& bungee = w.zombie_factory.create(zombie_type::bungee, 0);
    bungee.row = 0;
    bungee.bungee_col = 0;
    bungee.status = zombie_status::bungee_idle_after_drop;
    bungee.countdown.action = 0;

    zombie_bungee(w.scene).update(bungee);

    CHECK(bungee.status == zombie_status::bungee_grab);
    CHECK(bungee.bungee_target == content_id);
    CHECK(content.edible == plant_edible_status::invisible_and_edible);
    CHECK(pumpkin.edible == plant_edible_status::visible_and_edible);
    CHECK(base.edible == plant_edible_status::visible_and_edible);
}

void test_bungee_top_query_skips_not_on_ground_plants() {
    world smashed_world(scene_type::day);
    auto& smashed_content = smashed_world.plant_factory.create(plant_type::pea_shooter, 0, 0);
    auto& fallback_pumpkin = smashed_world.plant_factory.create(plant_type::pumpkin, 0, 0);
    smashed_content.is_smashed = true;

    auto& smashed_bungee = smashed_world.zombie_factory.create(zombie_type::bungee, 0);
    smashed_bungee.row = 0;
    smashed_bungee.bungee_col = 0;
    smashed_bungee.status = zombie_status::bungee_idle_after_drop;
    smashed_bungee.countdown.action = 0;

    zombie_bungee(smashed_world.scene).update(smashed_bungee);

    CHECK(smashed_bungee.status == zombie_status::bungee_grab);
    CHECK(smashed_bungee.bungee_target == static_cast<int>(smashed_world.scene.plants.get_id(fallback_pumpkin)));
    CHECK(smashed_content.edible == plant_edible_status::visible_and_edible);
    CHECK(fallback_pumpkin.edible == plant_edible_status::invisible_and_edible);

    world grabbed_world(scene_type::day);
    auto& grabbed_content = grabbed_world.plant_factory.create(plant_type::pea_shooter, 0, 0);
    auto& untouched_pumpkin = grabbed_world.plant_factory.create(plant_type::pumpkin, 0, 0);
    grabbed_content.edible = plant_edible_status::invisible_and_edible;

    auto& grabbed_bungee = grabbed_world.zombie_factory.create(zombie_type::bungee, 0);
    grabbed_bungee.row = 0;
    grabbed_bungee.bungee_col = 0;
    grabbed_bungee.status = zombie_status::bungee_idle_after_drop;
    grabbed_bungee.countdown.action = 0;

    zombie_bungee(grabbed_world.scene).update(grabbed_bungee);

    CHECK(grabbed_bungee.status == zombie_status::bungee_grab);
    CHECK(grabbed_bungee.bungee_target == static_cast<int>(grabbed_world.scene.plants.get_id(grabbed_content)));
    CHECK(grabbed_content.edible == plant_edible_status::invisible_and_edible);
    CHECK(untouched_pumpkin.edible == plant_edible_status::visible_and_edible);
}

void test_bungee_skips_unstealable_target_cells() {
    auto only_target_is_rejected = [](plant_type type) {
        auto w = std::make_unique<world>(scene_type::pool);
        fill_graves_except(*w, 0, 0);
        w->plant_factory.create(type, 0, 0);
        auto& bungee = w->zombie_factory.create(zombie_type::bungee, 0);
        return bungee.is_dead;
    };

    CHECK(only_target_is_rejected(plant_type::grave_buster));
    CHECK(only_target_is_rejected(plant_type::cob_cannon));
    CHECK(only_target_is_rejected(plant_type::sunflower));

    auto smashed_sun_with_pumpkin = std::make_unique<world>(scene_type::pool);
    fill_graves_except(*smashed_sun_with_pumpkin, 0, 0);
    auto& smashed_sun = smashed_sun_with_pumpkin->plant_factory.create(plant_type::sunflower, 0, 0);
    smashed_sun_with_pumpkin->plant_factory.create(plant_type::pumpkin, 0, 0);
    smashed_sun.is_smashed = true;
    auto& fallback_bungee = smashed_sun_with_pumpkin->zombie_factory.create(zombie_type::bungee, 0);
    CHECK(!fallback_bungee.is_dead);
    CHECK(fallback_bungee.row == 0);
    CHECK(fallback_bungee.bungee_col == 0);

    auto two_sunflowers = std::make_unique<world>(scene_type::pool);
    fill_graves_except_first_two_cols(*two_sunflowers);
    two_sunflowers->plant_factory.create(plant_type::sunflower, 0, 0);
    two_sunflowers->plant_factory.create(plant_type::sunflower, 0, 1);
    auto& allowed_bungee = two_sunflowers->zombie_factory.create(zombie_type::bungee, 0);
    CHECK(!allowed_bungee.is_dead);
    CHECK(allowed_bungee.row == 0);
    CHECK(allowed_bungee.bungee_col == 0 || allowed_bungee.bungee_col == 1);

    auto grab_world = std::make_unique<world>(scene_type::day);
    auto& grave_buster = grab_world->plant_factory.create(plant_type::grave_buster, 0, 0);
    auto& grab_bungee = grab_world->zombie_factory.create(zombie_type::bungee, 0);
    grab_bungee.row = 0;
    grab_bungee.bungee_col = 0;
    grab_bungee.status = zombie_status::bungee_idle_after_drop;
    grab_bungee.countdown.action = 0;

    zombie_bungee(grab_world->scene).update(grab_bungee);

    CHECK(grab_bungee.status == zombie_status::bungee_grab);
    CHECK(grab_bungee.bungee_target == -1);
    CHECK(grave_buster.edible == plant_edible_status::visible_and_edible);
}

void test_catapult_ignores_too_close_and_right_side_plants() {
    auto catapult_status_after_target_check = [](int catapult_x, unsigned int plant_col) {
        world w(scene_type::day);
        w.scene.stop_spawn = true;
        w.plant_factory.create(plant_type::sunflower, 0, plant_col);
        auto& catapult = make_zombie(w, zombie_type::catapult, 0, catapult_x);
        catapult.status = zombie_status::catapult_idle;
        catapult.countdown.action = 0;
        catapult.catapult_or_jackson.n_basketballs = 1;

        zombie_catapult(w.scene).update(catapult);
        return catapult.status;
    };

    CHECK(catapult_status_after_target_check(379, 3) == zombie_status::walking);
    CHECK(catapult_status_after_target_check(200, 3) == zombie_status::walking);
    CHECK(catapult_status_after_target_check(380, 3) == zombie_status::catapult_shoot);
}

void test_catapult_target_skips_not_on_ground_top_layer() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& squash = w.plant_factory.create(plant_type::squash, 0, 3);
    w.plant_factory.create(plant_type::pumpkin, 0, 3);
    squash.status = plant_status::squash_stop_in_the_air;
    squash.x = 40;

    auto& catapult = make_zombie(w, zombie_type::catapult, 0, 650);
    catapult.status = zombie_status::catapult_shoot;
    catapult.catapult_or_jackson.n_basketballs = 1;
    catapult.reanim.prev_progress = 0.5f;
    catapult.reanim.progress = 0.6f;

    zombie_catapult(w.scene).update(catapult);

    CHECK(w.scene.projectiles.size() == 1);
    auto& ball = *w.scene.projectiles.begin();
    CHECK(ball.type == projectile_type::basketball);
    CHECK(ball.dx > -4.1f && ball.dx < -3.7f);
}

void test_roof_gargantuar_imp_throw_uses_negative_min_d2y() {
    world w(scene_type::roof);
    w.scene.stop_spawn = true;

    auto& garg = make_zombie(w, zombie_type::gargantuar, 0, 400);
    garg.status = zombie_status::gargantuar_throw;
    garg.has_item_or_walk_left = true;
    garg.reanim.prev_progress = 0.7f;
    garg.reanim.progress = 0.8f;

    zombie_gargantuar(w.scene).update(garg);

    bool found = false;
    for (auto& z : w.scene.zombies) {
        if (&z == &garg || z.type != zombie_type::imp) {
            continue;
        }

        found = true;
        CHECK(z.status == zombie_status::imp_flying);
        CHECK(z.dy == 88.0f);
        CHECK(float_bits(z.d2y) == 0xBF955556u);
    }

    CHECK(found);
}

void test_backup_dancer_spawning_dy_uses_float_ratio() {
    world w(scene_type::day);
    auto& dancer = make_zombie(w, zombie_type::backup_dancer, 0, 300);
    dancer.status = zombie_status::backup_spawning;
    dancer.countdown.action = 3;

    zombie_backup_dancer(w.scene).update(dancer);

    CHECK(dancer.status == zombie_status::backup_spawning);
    CHECK(dancer.dy == -4.0f);

    dancer.countdown.action = 0;
    zombie_backup_dancer(w.scene).update(dancer);
    CHECK(float_bits(dancer.dy) == 0x00000000u);
}

void test_bungee_target_order_is_column_major() {
    world w(scene_type::pool);
    w.scene.rng.lock(7);
    for (unsigned row : {0u, 1u}) {
        for (unsigned col : {0u, 1u}) {
            w.plant_factory.create(plant_type::pumpkin, row, col);
        }
    }
    auto& first = w.zombie_factory.create(zombie_type::bungee, 0);
    auto& second = w.zombie_factory.create(zombie_type::bungee, 0);
    CHECK(first.row == 0 && first.bungee_col == 0);
    CHECK(second.row == 1 && second.bungee_col == 0);
}

void test_split_pea_second_shot_uses_stored_attack_flags() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& split = w.plant_factory.create(plant_type::split_pea, 0, 3);
    split.countdown.generate = 1000;
    split.countdown.launch = 2;
    split.split_pea_attack_flags.front = true;
    split.split_pea_attack_flags.back = true;

    w.plant_system.update();

    CHECK(w.scene.projectiles.size() == 2);
    CHECK(split.split_pea_attack_flags.front);
    CHECK(split.split_pea_attack_flags.back);

    w.plant_system.update();

    CHECK(!split.split_pea_attack_flags.front);
    CHECK(!split.split_pea_attack_flags.back);
}

void test_potato_mine_trigger_rect_special_cases_pole_only() {
    auto normal_world = std::make_unique<world>(scene_type::day);
    normal_world->scene.stop_spawn = true;
    auto& normal_potato = normal_world->plant_factory.create(plant_type::potato_mine, 0, 3);
    normal_potato.status = plant_status::potato_armed;
    auto& normal = make_zombie(*normal_world, zombie_type::zombie, 0, 202);

    normal_world->plant_system.update();

    CHECK(normal.hp == 0);
    CHECK(normal_potato.is_dead);

    auto pole_world = std::make_unique<world>(scene_type::day);
    pole_world->scene.stop_spawn = true;
    auto& pole_potato = pole_world->plant_factory.create(plant_type::potato_mine, 0, 3);
    pole_potato.status = plant_status::potato_armed;
    auto& pole = make_zombie(*pole_world, zombie_type::pole_vaulting, 0, 202);
    pole.status = zombie_status::pole_vaulting_walking;

    pole_world->plant_system.update();

    CHECK(pole.hp == static_cast<int>(pole.max_hp));
    CHECK(!pole_potato.is_dead);
}

void test_hypno_zombie_attacks_no_head_enemy() {
    world w(scene_type::day);
    w.scene.rng.seed(0);
    w.scene.stop_spawn = true;

    auto& hypno = make_zombie(w, zombie_type::zombie, 0, 300);
    hypno.is_hypno = true;
    hypno.time_since_spawn = 3;
    auto& enemy = make_zombie(w, zombie_type::zombie, 0, 310);
    enemy.is_not_dying = false;
    const auto enemy_hp = enemy.hp;
    w.scene.rng.lock(1);

    w.zombie.update();

    CHECK(enemy.hp == enemy_hp - 4);
    CHECK(hypno.is_eating);
}

void test_hypno_target_refreshes_between_updates() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;
    auto& first = make_zombie(w, zombie_type::zombie, 0, 300);
    auto& second = make_zombie(w, zombie_type::zombie, 0, 310);
    for (bool hypno : {false, true, false, true}) {
        first.is_hypno = hypno; // Public raw edits between updates remain supported.
        first.time_since_spawn = second.time_since_spawn = 3;
        const auto hp = second.hp;
        w.zombie.update();
        CHECK(second.hp == hp - (hypno ? 4 : 0));
    }

    // Loading a scene into an already-used subsystem must not retain its old gate.
    auto saved = std::make_unique<scene>(w.scene);
    w.reset();
    w.scene.stop_spawn = true;
    w.zombie.update();
    w.scene = *saved;
    auto* restored_first = w.scene.zombies.get(0);
    auto* restored_second = w.scene.zombies.get(1);
    CHECK(restored_first && restored_second);
    restored_first->time_since_spawn = restored_second->time_since_spawn = 3;
    const auto hp = restored_second->hp;
    auto copied = std::make_unique<world>(w);
    w.zombie.update();
    copied->zombie.update();
    CHECK(restored_second->hp == hp - 4);
    CHECK(copied->scene.zombies.get(1)->hp == hp - 4);
}

void test_hypno_conversion_is_visible_later_in_same_update() {
    for (auto type : {zombie_type::zombie, zombie_type::dancing}) {
        world w(scene_type::night);
        auto& mushroom = w.plant_factory.create(plant_type::hypnoshroom, 0, 3);
        auto& first = make_zombie(w, type, 0, mushroom.x + (type == zombie_type::dancing ? 0 : 20));
        first.y = static_cast<float>(mushroom.y);
        first.int_y = mushroom.y;
        if (type == zombie_type::dancing) {
            first.status = zombie_status::dancing_walking;
        }
        damage(w.scene).set_is_eating(first);
        first.time_since_spawn = 0;
        first.reanim.prev_progress = 0.10f;
        first.reanim.progress = 0.15f;
        auto& second = make_zombie(w, zombie_type::zombie, 0, first.int_x + 10);
        second.y = first.y;
        second.int_y = first.int_y;
        second.time_since_spawn = 3;
        const auto hp = first.hp;
        CHECK(!first.is_hypno && !second.is_hypno);

        w.zombie.update();

        CHECK(mushroom.is_dead);
        CHECK(first.is_hypno);
        CHECK(first.hp == hp - 4);
        CHECK(second.is_eating);
    }
}

void test_pool_spawn_never_selects_ducky_tube() {
    for (unsigned int seed = 0; seed < 200; ++seed) {
        world w(scene_type::pool);
        w.scene.rng.seed(seed);
        w.spawn.reset();

        CHECK(!w.scene.spawn.spawn_flags[static_cast<int>(zombie_type::ducky_tube)]);
        for (const auto& wave : w.scene.spawn.spawn_list) {
            for (const auto type : wave) {
                CHECK(type != zombie_type::ducky_tube);
            }
        }
    }
}

void test_hugewave_fade_zero_frame_spawns_wave() {
    world w(scene_type::pool);
    w.scene.spawn.wave = 9;
    w.scene.spawn.countdown.next_wave = 1;
    w.scene.spawn.countdown.next_wave_initial = 4500;
    w.scene.spawn.countdown.hugewave_fade = 1;
    w.scene.spawn.countdown.lurking_squad = 0;
    set_single_zombie_wave(w, 9, zombie_type::zombie);

    w.spawn.update();

    CHECK(w.scene.spawn.countdown.hugewave_fade == 0);
    CHECK(w.scene.spawn.wave == 10);
    CHECK(w.scene.spawn.total_flags == 1001);
    CHECK(w.scene.zombies.size() == 1);
    CHECK((*w.scene.zombies.begin()).type == zombie_type::zombie);
}

void test_spawn_giga_candidates_and_weight() {
    int normal_giga = 0;
    int flag_giga = 0;
    int allowed_samples = 0;

    for (unsigned seed = 0; seed < 2000; ++seed) {
        world w(scene_type::pool);
        w.scene.rng.seed(seed);
        w.spawn.reset();

        if (!w.scene.spawn.spawn_flags[static_cast<int>(zombie_type::giga_gargantuar)]) {
            continue;
        }

        ++allowed_samples;
        for (const auto& wave : w.scene.spawn.spawn_list) {
            for (const auto type : wave) {
                CHECK(is_valid_zombie_type(type));
            }
        }

        for (const auto type : w.scene.spawn.spawn_list[0]) {
            if (type == zombie_type::giga_gargantuar) {
                ++normal_giga;
            }
        }
        for (const auto type : w.scene.spawn.spawn_list[9]) {
            if (type == zombie_type::giga_gargantuar) {
                ++flag_giga;
            }
        }
    }

    CHECK(allowed_samples > 0);
    CHECK(normal_giga > 0);
    CHECK(flag_giga > normal_giga);
}

void test_spawn_current_type_counts_reset_each_wave() {
    int allowed_samples = 0;
    int late_wave_gargantuars = 0;

    for (unsigned seed = 0; seed < 2000; ++seed) {
        world w(scene_type::pool);
        w.scene.rng.seed(seed);
        w.spawn.reset();

        if (!w.scene.spawn.spawn_flags[static_cast<int>(zombie_type::gargantuar)]) {
            continue;
        }

        ++allowed_samples;
        for (const auto type : w.scene.spawn.spawn_list[19]) {
            if (type == zombie_type::gargantuar) {
                ++late_wave_gargantuars;
            }
        }
    }

    CHECK(allowed_samples > 0);
    CHECK(late_wave_gargantuars > 0);
}

void test_spawn_giga_cap_recomputed_within_wave() {
    int allowed_samples = 0;

    for (unsigned seed = 0; seed < 1000; ++seed) {
        world w(scene_type::pool);
        w.scene.rng.seed(seed);
        w.spawn.reset();

        if (!w.scene.spawn.spawn_flags[static_cast<int>(zombie_type::giga_gargantuar)]) {
            continue;
        }

        ++allowed_samples;
        unsigned int total_giga = 0;
        for (unsigned int wave = 0; wave < w.scene.spawn.spawn_list.size(); ++wave) {
            unsigned int wave_giga = 0;
            const bool critical_wave = wave == 9 || wave == 19;

            for (const auto type : w.scene.spawn.spawn_list[wave]) {
                if (type != zombie_type::giga_gargantuar) {
                    continue;
                }

                if (critical_wave) {
                    CHECK(wave_giga < 50);
                } else {
                    CHECK(total_giga < 50);
                }

                ++wave_giga;
                ++total_giga;
            }
        }
    }

    CHECK(allowed_samples > 0);
}

void test_splash_damage_caps() {
    CHECK(splash_damage_to_secondaries(projectile_type::melon) > 0);
    CHECK(splash_damage_to_secondaries(projectile_type::melon) <= 80 * 7);
    CHECK(splash_damage_to_secondaries(projectile_type::wintermelon) > 0);
    CHECK(splash_damage_to_secondaries(projectile_type::wintermelon) <= 80 * 7);
    CHECK(splash_damage_to_secondaries(projectile_type::fire_pea) <= 40);

    world dense(scene_type::day);
    dense.scene.stop_spawn = true;
    make_zombie(dense, zombie_type::zombie, 0, 80);
    std::vector<zombie*> secondaries;
    for (int i = 0; i < 22; ++i) {
        secondaries.push_back(&make_zombie(dense, zombie_type::zombie, 0, 100));
    }

    auto& melon = projectile_factory(dense.scene).create(projectile_type::melon, 0, 100, 0);
    melon.flags = static_cast<unsigned int>(attack_flags::ground);
    dense.projectile.update();

    int total = 0;
    for (auto* z : secondaries) {
        total += static_cast<int>(z->max_hp) - z->hp;
    }
    CHECK(total == 22 * 26);
}

void test_chill_on_helm_only() {
    world w(scene_type::day);

    auto& bucket = make_zombie(w, zombie_type::buckethead, 0, 300);
    bucket.accessory_1.type = zombie_accessories_type_1::bucket;
    bucket.accessory_1.hp = 400;
    bucket.accessory_1.max_hp = 400;
    auto bucket_hp = bucket.hp;

    damage(w.scene).take(bucket, 20, static_cast<unsigned int>(zombie_damage_flags::slow_effect));
    CHECK(bucket.hp == bucket_hp);
    CHECK(bucket.countdown.slow > 0);

    auto& door = make_zombie(w, zombie_type::screendoor, 0, 300);
    door.accessory_2.type = zombie_accessories_type_2::screen_door;
    door.accessory_2.hp = 400;
    door.accessory_2.max_hp = 400;

    damage(w.scene).take(door, 20, static_cast<unsigned int>(zombie_damage_flags::slow_effect));
    CHECK(door.countdown.slow == 0);
}

void test_sleeping_plants_do_not_charge() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& puff = w.plant_factory.create(plant_type::puffshroom, 0, 0);
    CHECK(puff.is_sleeping);
    puff.countdown.generate = 10;

    for (int i = 0; i < 5; ++i) {
        w.plant_system.update();
    }
    CHECK(puff.countdown.generate == 10);

    auto& coffee = w.plant_factory.create(plant_type::coffee_bean, 0, 0);
    damage(w.scene).activate_plant(coffee);
    for (int i = 0; i < 100; ++i) {
        w.plant_system.update();
    }
    CHECK(!puff.is_sleeping);
}

void test_sunshroom_single_update_and_seashroom_type() {
    world w(scene_type::night);
    w.scene.stop_spawn = true;

    auto& sunshroom = w.plant_factory.create(plant_type::sunshroom, 0, 0);
    sunshroom.is_sleeping = false;
    sunshroom.countdown.generate = 10;
    sunshroom.countdown.status = 100;

    w.plant_system.update();
    CHECK(sunshroom.countdown.generate == 9);

    auto& seashroom = w.plant_factory.create(plant_type::seashroom, 2, 0);
    CHECK(seashroom.type == plant_type::seashroom);
}

void test_normal_shooter_flags_are_ground_only() {
    world w(scene_type::day);

    auto& pea = w.plant_factory.create(plant_type::pea_shooter, 0, 0);
    CHECK(pea.get_attack_flags() == static_cast<unsigned int>(attack_flags::ground));

    auto& cabbage = w.plant_factory.create(plant_type::cabbagepult, 1, 0);
    CHECK((cabbage.get_attack_flags() & attack_flags::lurking_snorkel) != 0);

    auto& puff = w.plant_factory.create(plant_type::puffshroom, 2, 0);
    CHECK(puff.get_attack_flags() ==
          (attack_flags::ground | attack_flags::dog));

    auto& gloom = w.plant_factory.create(plant_type::gloomshroom, 3, 0);
    CHECK(gloom.get_attack_flags() ==
          (attack_flags::ground | attack_flags::dog));

    auto& scaredy = w.plant_factory.create(plant_type::scaredyshroom, 4, 0);
    CHECK(scaredy.get_attack_flags() ==
          static_cast<unsigned int>(attack_flags::ground));

    auto& cattail = w.plant_factory.create(plant_type::cattail, 0, 1);
    // 1051 Plant::GetDamageRangeFlags@0x45EB88 returns 0x0B for cattail.
    CHECK(cattail.get_attack_flags() == 0x0B);
}

void test_grid_plant_target_matches_pool_scan() {
    auto compare = [](world& w) {
        auto& z = make_zombie(w, zombie_type::zombie, 0, 0);
        zombie_base targeting(w.scene);
        // Wide boxes deliberately overlap adjacent columns to exercise pool priority.
        z.attack_box.width = 180;
        for (unsigned row = 0; row < w.scene.rows; ++row) {
            z.row = row;
            for (auto status : {zombie_status::walking, zombie_status::digger_dig,
                                zombie_status::ladder_walking}) {
                z.status = status;
                for (unsigned flags = 0; flags < 4; ++flags) {
                    z.is_hypno = (flags & 1) != 0;
                    z.is_in_water = (flags & 2) != 0;
                    for (auto type : {zombie_attack_type::smash_or_eat, zombie_attack_type::crush,
                                      zombie_attack_type::jump, zombie_attack_type::place_ladder}) {
                        for (int x = -100; x <= 900; x += 11) {
                            z.x = static_cast<float>(x);
                            z.int_x = x;
                            rect zr;
                            z.get_attack_box(zr);
                            plant* expected = nullptr;
                            // Original implementation is the oracle, independent of plant_map enumeration.
                            for (auto& p : w.scene.plants) {
                                if (p.row == z.row && targeting.can_attack_plant(z, p, type)) {
                                    rect pr;
                                    p.get_hit_box(pr);
                                    if (zr.get_overlap_len(pr) >= 20) {
                                        expected = &p;
                                        break;
                                    }
                                }
                            }
                            CHECK(targeting.find_target(z, type) == expected);
                            plant* crush_expected = nullptr;
                            for (auto& p : w.scene.plants) {
                                if (p.row != z.row || p.type == plant_type::spikeweed ||
                                    p.type == plant_type::spikerock) {
                                    continue;
                                }
                                rect pr;
                                p.get_hit_box(pr);
                                if (zr.get_overlap_len(pr) >= 20 &&
                                    targeting.can_attack_plant(z, p, zombie_attack_type::crush)) {
                                    crush_expected = &p;
                                    break;
                                }
                            }
                            CHECK(targeting.find_target(z, zombie_attack_type::crush, true) == crush_expected);
                        }
                    }
                }
            }
        }
        w.zombie_factory.destroy(z);
        w.scene.zombies.shrink_to_fit();
    };

    for (auto scene_type : {scene_type::day, scene_type::pool, scene_type::roof}) {
        auto w = std::make_unique<world>(scene_type);
        for (unsigned row = 0; row < w->scene.rows; ++row) {
            // Allocation order differs from grid and layer order.
            w->plant_factory.create(plant_type::cob_cannon, row, 5);
            w->plant_factory.create(plant_type::flower_pot, row, 3);
            w->plant_factory.create(plant_type::fumeshroom, row, 3);
            w->plant_factory.create(plant_type::pumpkin, row, 3);
            w->plant_factory.create(plant_type::coffee_bean, row, 3);
            w->plant_factory.create(plant_type::tallnut, row, 8);
            w->plant_factory.create(row % 2 == 0 ? plant_type::spikeweed : plant_type::spikerock, row, 1);
            w->plant_factory.create(plant_type::squash, row, 0);
            w->plant_factory.create(plant_type::potato_mine, row, 2);
            w->plant_factory.create(plant_type::snow_pea, row, 4).x -= 23;
            w->plant_factory.create(plant_type::imitater, row, 7, plant_type::pumpkin);
        }
        compare(*w);
        // Reuse the earliest slot with a newer generation; priority is slot, not full ID.
        w->plant_factory.destroy(*w->scene.plant_map[0][5].content);
        w->scene.plants.shrink_to_fit();
        w->plant_factory.create(plant_type::cob_cannon, 0, 5);
        w->scene.plant_map[0][3].pumpkin->is_smashed = true;
        w->scene.plant_map[0][3].content->edible = plant_edible_status::invisible_and_not_edible;
        compare(*w);
        auto copied = std::make_unique<world>(*w);
        compare(*copied);
        w->scene = copied->scene;
        compare(*w);

        // The raw factory permits overlapping layers; hidden older objects must remain targets.
        auto& hidden = w->plant_factory.create(plant_type::pea_shooter, 0, 4);
        auto& cover = w->plant_factory.create(plant_type::cherry_bomb, 0, 4);
        CHECK(w->scene.plant_map_incomplete[0]);
        compare(*w);
        w->plant_factory.destroy(cover);
        compare(*w);
        copied->scene = w->scene;
        compare(*copied);
        w->scene.unregister_plant_from_map(hidden);
        compare(*w);
        w->reset();
        CHECK(!w->scene.plant_map_incomplete[0]);
        compare(*w);
    }
}

void test_potato_mine_uses_native_explosion_center() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& potato = w.plant_factory.create(plant_type::potato_mine, 2, 3);
    auto& left_edge = make_zombie(w, zombie_type::zombie, 2, potato.x - 118);
    auto& old_center_only = make_zombie(w, zombie_type::zombie, 2, potato.x + 64);

    left_edge.int_y = potato.y;
    old_center_only.int_y = potato.y;

    damage(w.scene).activate_plant(potato);

    CHECK(left_edge.hp == 0);
    CHECK(old_center_only.hp == 270);
    CHECK(old_center_only.status == zombie_status::walking);
}

void test_hypno_snorkel_walking_does_not_enter_pool() {
    world w(scene_type::pool);
    w.scene.stop_spawn = true;

    auto& snorkel = w.zombie_factory.create(zombie_type::snorkel, 2);
    snorkel.status = zombie_status::snorkel_walking;
    snorkel.is_hypno = true;
    snorkel.is_in_water = false;
    snorkel.dy = 37.0f;
    snorkel.reanim.progress = 0.5f;
    snorkel.reanim.n_repeated = 1;

    zombie_snorkel(w.scene).update(snorkel);

    CHECK(snorkel.status == zombie_status::snorkel_walking);
    CHECK(!snorkel.is_in_water);
    CHECK(snorkel.dy == 37.0f);
}

void test_squash_nearest_and_locked_target() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& squash = w.plant_factory.create(plant_type::squash, 0, 3);
    auto& far = make_zombie(w, zombie_type::zombie, 0, squash.x + 100);
    auto& near = make_zombie(w, zombie_type::zombie, 0, squash.x + 80);

    w.plant_system.update();
    CHECK(squash.status == plant_status::squash_look);
    CHECK(squash.target == static_cast<int>(w.scene.zombies.get_id(near)));

    far.x = static_cast<float>(squash.x + 60);
    far.int_x = squash.x + 60;

    for (int i = 0; i < 125 && squash.status != plant_status::squash_stop_in_the_air; ++i) {
        w.plant_system.update();
    }

    CHECK(squash.status == plant_status::squash_stop_in_the_air);
    CHECK(squash.cannon.x == static_cast<int>(zombie_base(w.scene).predict_after(near, 30)) - squash.attack_box.width / 2);
}

void test_jack_radius_arguments() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& jack = make_zombie(w, zombie_type::jack_in_the_box, 2, 300);
    jack.status = zombie_status::jackbox_pop;
    jack.countdown.action = 0;
    jack.hit_box.offset_x = 0;
    jack.hit_box.offset_y = 0;
    jack.is_hypno = true;

    auto& same_row_near = make_zombie(w, zombie_type::zombie, 2, 350);
    auto& same_row_far = make_zombie(w, zombie_type::zombie, 2, 470);
    auto& adjacent_row = make_zombie(w, zombie_type::zombie, 3, 350);
    auto& far_row = make_zombie(w, zombie_type::zombie, 4, 350);

    zombie_jack_in_the_box(w.scene).update(jack);

    CHECK(same_row_near.is_dead);
    CHECK(!same_row_far.is_dead);
    CHECK(adjacent_row.is_dead);
    CHECK(!far_row.is_dead);
}

void test_dancer_eating_early_return() {
    world w(scene_type::day);
    auto& dancer = make_zombie(w, zombie_type::dancing, 2, 300);
    dancer.is_eating = true;
    dancer.catapult_or_jackson.summon_countdown = 1;
    dancer.status = zombie_status::dancing_walking;

    zombie_dancing(w.scene).update(dancer);

    CHECK(dancer.catapult_or_jackson.summon_countdown == 1);
    CHECK(dancer.status == zombie_status::dancing_walking);
}

void test_reanim_wrap_uses_previous_progress() {
    object::reanim anim{};
    anim.prev_fps = 0.2f;
    anim.prev_progress = 0.8f;
    anim.progress = 0.1f;

    CHECK(anim.is_in_progress(0.9));
    CHECK(anim.is_in_progress(0.05));
    CHECK(!anim.is_in_progress(0.5));
}

void test_catapult_stops_after_last_basketball() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& plant = w.plant_factory.create(plant_type::sunflower, 0, 0);
    auto& catapult = make_zombie(w, zombie_type::catapult, 0, 500);
    catapult.status = zombie_status::catapult_idle;
    catapult.countdown.action = 0;
    catapult.catapult_or_jackson.n_basketballs = 0;

    zombie_catapult(w.scene).update(catapult);

    CHECK(catapult.status == zombie_status::walking);
    CHECK(!plant.is_smashed);

    catapult.status = zombie_status::catapult_shoot;
    catapult.catapult_or_jackson.n_basketballs = 2;
    catapult.reanim.n_repeated = 1;
    zombie_catapult(w.scene).update(catapult);
    CHECK(catapult.status == zombie_status::catapult_idle);
    CHECK(catapult.reanim.type == reanim_type::loop);

    catapult.countdown.slow = 100;
    system::reanim animation(w.scene);
    animation.set(catapult, zombie_reanim_name::anim_idle, reanim_type::loop, 12);
    animation.set(catapult, zombie_reanim_name::anim_idle, reanim_type::loop, 0);
    CHECK(catapult.reanim.fps == 6);
    CHECK(catapult.reanim.prev_fps == 12);
    catapult.countdown.slow = 0;
    animation.update_fps(catapult);
    CHECK(catapult.reanim.fps == 12);
}

void test_vehicle_crushes_all_plants_in_square() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& base = w.plant_factory.create(plant_type::flower_pot, 0, 3);
    auto& content = w.plant_factory.create(plant_type::sunflower, 0, 3);
    auto& pumpkin = w.plant_factory.create(plant_type::pumpkin, 0, 3);
    auto& cherry = w.plant_factory.create(plant_type::cherry_bomb, 0, 3);
    auto& z = make_zombie(w, zombie_type::zomboni, 0, content.x + 20);
    z.dx = 0;

    w.zombie.update();

    CHECK(base.is_smashed);
    CHECK(content.is_smashed);
    CHECK(pumpkin.is_smashed);
    CHECK(cherry.is_dead);
}

void test_thrown_imp_inherits_slow() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& garg = make_zombie(w, zombie_type::gargantuar, 0, 500);
    garg.status = zombie_status::gargantuar_throw;
    garg.countdown.slow = 321;
    garg.has_item_or_walk_left = true;
    garg.reanim.prev_progress = 0.7f;
    garg.reanim.progress = 0.8f;

    zombie_gargantuar(w.scene).update(garg);

    bool found = false;
    for (auto& z : w.scene.zombies) {
        if (&z != &garg && z.type == zombie_type::imp) {
            found = true;
            CHECK(z.countdown.slow == 321);
        }
    }
    CHECK(found);
}

void test_fireball_splash_width_and_torchwood_lookahead() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;

    auto& main = make_zombie(w, zombie_type::zombie, 0, 80);
    auto& secondary = make_zombie(w, zombie_type::zombie, 0, 150);
    auto& fire = projectile_factory(w.scene).create(projectile_type::fire_pea, 0, 100, 0);
    fire.flags = static_cast<unsigned int>(attack_flags::ground);
    w.projectile.update();

    CHECK(main.hp < static_cast<int>(main.max_hp));
    CHECK(secondary.hp < static_cast<int>(secondary.max_hp));

    world torch_world(scene_type::day);
    torch_world.scene.stop_spawn = true;
    auto& torch = torch_world.plant_factory.create(plant_type::torchwood, 0, 2);
    auto& zombie = make_zombie(torch_world, zombie_type::zombie, 0, torch.x + 100);
    auto& pea = projectile_factory(torch_world.scene).create(projectile_type::pea, 0, torch.x - 80, 0);
    pea.flags = static_cast<unsigned int>(attack_flags::ground);

    torch_world.projectile.update();

    CHECK(!pea.is_disappeared);
    CHECK(zombie.hp == static_cast<int>(zombie.max_hp));

    auto lifted_torch_world = std::make_unique<world>(scene_type::day);
    lifted_torch_world->scene.stop_spawn = true;
    auto& lifted_torch = lifted_torch_world->plant_factory.create(plant_type::torchwood, 0, 2);
    lifted_torch.edible = plant_edible_status::invisible_and_not_edible;
    auto& lifted_zombie = make_zombie(*lifted_torch_world, zombie_type::zombie, 0, lifted_torch.x - 116);
    auto& lifted_pea = projectile_factory(lifted_torch_world->scene).create(
        projectile_type::snow_pea,
        0,
        lifted_torch.x - 80,
        0);
    lifted_pea.flags = static_cast<unsigned int>(attack_flags::ground);

    lifted_torch_world->projectile.update();

    CHECK(lifted_pea.is_disappeared);
    CHECK(lifted_zombie.hp < static_cast<int>(lifted_zombie.max_hp));

    world switch_world(scene_type::day);
    switch_world.scene.stop_spawn = true;
    auto& switch_torch = switch_world.plant_factory.create(plant_type::torchwood, 0, 2);
    auto& switch_target = make_zombie(
        switch_world,
        zombie_type::zombie,
        0,
        switch_torch.x - 123);
    auto& switch_pea = projectile_factory(switch_world.scene).create(
        projectile_type::snow_pea,
        0,
        switch_torch.x - 80,
        0);
    switch_pea.motion_type = projectile_motion_type::left_straight;
    switch_pea.flags = static_cast<unsigned int>(attack_flags::ground);
    switch_world.projectile.update();
    CHECK(switch_pea.is_disappeared);
    CHECK(switch_target.hp < static_cast<int>(switch_target.max_hp));
}

void test_dolphin_leaving_pool_switches_without_dolphin_phase() {
    world w(scene_type::pool);
    w.scene.stop_spawn = true;
    auto& dolphin = w.zombie_factory.create(zombie_type::dolphin_rider, 2);
    dolphin.status = zombie_status::dolphin_walk_in_pool;
    dolphin.x = 10;
    dolphin.int_x = 10;
    dolphin.action = zombie_action::none;

    zombie_dolphin_rider(w.scene).update(dolphin);

    CHECK(dolphin.status == zombie_status::dolphin_walk_without_dolphin);
    CHECK(dolphin.action == zombie_action::leaving_pool);
    CHECK(dolphin.dy == -40);
    CHECK(dolphin.dx >= 0.88999999f);
    CHECK(dolphin.dx <= 0.91000003f);
}

void test_kernelpult_butter_state_clears_after_launch() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;
    auto& kernel = w.plant_factory.create(plant_type::kernelpult, 0, 0);
    kernel.status = plant_status::kernelpult_launch_butter;
    kernel.countdown.launch = 2;
    make_zombie(w, zombie_type::zombie, 0, 400);

    w.plant_system.update();

    CHECK(kernel.status == plant_status::idle);
    CHECK(w.scene.projectiles.size() == 1);
    CHECK((*w.scene.projectiles.begin()).type == projectile_type::butter);
}

void test_imitater_waits_on_status_countdown() {
    world w(scene_type::day);
    auto& imitater = w.plant_factory.create(plant_type::imitater, 0, 0, plant_type::pea_shooter);

    CHECK(imitater.countdown.status == 200);
    CHECK(imitater.countdown.effect == 0);

    w.plant_system.update();

    CHECK(imitater.status != plant_status::imitater_explode);
    CHECK(imitater.countdown.status == 199);
}

void test_gloom_upgrade_preserves_pending_wake_countdown() {
    world w(scene_type::day);
    auto& fume = w.plant_factory.create(plant_type::fumeshroom, 0, 0);
    fume.set_sleep(true);
    fume.countdown.awake = 37;
    w.scene.cards[0].type = plant_type::gloomshroom;
    w.scene.cards[0].cold_down = 0;

    auto* gloom = w.plant_factory.plant(0, 0, 0);

    CHECK(gloom != nullptr);
    CHECK(gloom->type == plant_type::gloomshroom);
    CHECK(gloom->is_sleeping);
    CHECK(gloom->countdown.awake == 37);
}

void test_magnetshroom_skips_non_running_jackbox() {
    world w(scene_type::night);
    w.scene.stop_spawn = true;
    auto& magnet = w.plant_factory.create(plant_type::magnetshroom, 1, 1);
    auto& jack = make_zombie(w, zombie_type::jack_in_the_box, 1, magnet.x + 80);
    jack.status = zombie_status::jackbox_pop;
    jack.action = zombie_action::none;

    w.plant_system.update();

    CHECK(magnet.status != plant_status::magnetshroom_working);
    CHECK(magnet.countdown.status == 0);
}

void test_starfruit_uses_predicted_center_for_angle() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;
    auto& star = w.plant_factory.create(plant_type::starfruit, 0, 0);
    auto& z = make_zombie(w, zombie_type::zombie, 1, 255);
    z.dx = 0;
    z.int_x = static_cast<int>(z.x);
    z.y = zombie_init_y(w.scene.type, z, z.row);
    z.int_y = static_cast<int>(z.y);

    plant_starfruit(w.scene).set_launch_countdown(star);

    CHECK(star.countdown.launch == 40);
}

void test_roof_slope_fireball_impact_splashes_without_target() {
    world w(scene_type::roof);
    w.scene.stop_spawn = true;

    auto& secondary = make_zombie(w, zombie_type::zombie, 0, 160);
    auto& fire = projectile_factory(w.scene).create(projectile_type::fire_pea, 0, 100, 0);
    fire.flags = static_cast<unsigned int>(attack_flags::ground);
    fire.shadow_y = fire.y + 20;

    w.projectile.update();

    CHECK(fire.is_disappeared);
    CHECK(secondary.hp < static_cast<int>(secondary.max_hp));
}

void test_card_cooldown_boundaries() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;
    CHECK(w.select_plants({plant_type::pea_shooter, plant_type::wallnut, plant_type::cherry_bomb}));

    w.update(std::make_tuple(static_cast<int>(plant_type::pea_shooter), 0, 0));
    for (int i = 1; i < 750; ++i) {
        w.update();
    }
    CHECK(w.scene.cards[0].cold_down == 1);
    w.update();
    CHECK(w.scene.cards[0].cold_down == 0);

    w.update(std::make_tuple(static_cast<int>(plant_type::wallnut), 0, 1));
    for (int i = 1; i < 3000; ++i) {
        w.update();
    }
    CHECK(w.scene.cards[1].cold_down == 1);
    w.update();
    CHECK(w.scene.cards[1].cold_down == 0);

    w.update(std::make_tuple(static_cast<int>(plant_type::cherry_bomb), 0, 2));
    for (int i = 1; i < 5000; ++i) {
        w.update();
    }
    CHECK(w.scene.cards[2].cold_down == 1);
    w.update();
    CHECK(w.scene.cards[2].cold_down == 0);
}

void test_death_animation_retention_and_ash_removal() {
    world w(scene_type::day);

    auto& normal = make_zombie(w, zombie_type::zombie, 0, 300);
    damage(w.scene).take(normal, normal.hp, 0);
    CHECK(normal.is_dead);

    auto& ash = make_zombie(w, zombie_type::zombie, 0, 300);
    damage(w.scene).take(
        ash,
        1800,
        zombie_damage_flags::disable_ballon_pop | zombie_damage_flags::not_reduce);
    CHECK(ash.is_dead);
}

void test_dancer_remains_keep_group_slowdown_and_summon_delay() {
    world w(scene_type::day);
    w.scene.rng.lock(7);
    auto& leader = make_zombie(w, zombie_type::dancing, 2, 400);
    zombie_dancing dance(w.scene);
    leader.status = zombie_status::dancing_point;
    leader.reanim.n_repeated = 1;
    dance.update(leader);
    leader.status = zombie_status::dancing_walking;
    leader.catapult_or_jackson.summon_countdown = 0;
    w.scene.zombie_dancing_clock = 0;
    auto* follower = w.scene.zombies.try_to_get(static_cast<std::uint32_t>(leader.partners[2]));
    CHECK(follower != nullptr);
    follower->countdown.slow = 100;
    damage(w.scene).take(*follower, follower->hp, 0);
    CHECK(follower->has_death_status() && !follower->is_dead);
    dance.update(leader);
    CHECK(leader.catapult_or_jackson.summon_countdown == 0);
    CHECK(is_slowed(w.scene, leader));
    w.zombie_factory.destroy(*follower);
    w.scene.zombies.shrink_to_fit();
    dance.update(leader);
    CHECK(leader.catapult_or_jackson.summon_countdown == 100);
    CHECK(!is_slowed(w.scene, leader));
}

void test_no_head_core_semantics() {
    world w(scene_type::day);
    w.scene.stop_spawn = true;
    w.scene.rng.seed(1);

    auto& z = make_zombie(w, zombie_type::buckethead, 0, 250);
    z.accessory_1.type = zombie_accessories_type_1::none;
    z.is_eating = true;
    z.countdown.freeze = 123;
    z.countdown.butter = 456;

    damage(w.scene).take(z, z.hp - 70, static_cast<unsigned int>(zombie_damage_flags::ignore_accessory_2));
    CHECK(!z.is_not_dying);
    CHECK(damage(w.scene).can_be_attacked(z, static_cast<unsigned int>(attack_flags::ground)));
    CHECK(z.is_eating);
    CHECK(z.countdown.freeze == 123);
    CHECK(z.countdown.butter == 0);

    auto& plant = w.plant_factory.create(plant_type::pea_shooter, 0, 3);
    z.int_x = 250;
    z.x = 250;
    const int hp_before = plant.hp;
    w.zombie.update();
    CHECK(z.is_eating);
    CHECK(plant.hp == hp_before);

    for (int i = 0; i < 2000 && !z.is_dead; ++i) {
        w.zombie.update();
    }
    CHECK(z.is_dead);

    auto& garg = make_zombie(w, zombie_type::gargantuar, 0, 300);
    damage(w.scene).take(garg, garg.hp - static_cast<int>(garg.max_hp / 4), 0);
    CHECK(garg.is_not_dying);
    damage(w.scene).take(garg, garg.hp, 0);
    CHECK(garg.is_dead);

    auto pool = std::make_unique<world>(scene_type::pool);
    auto& dolphin = make_zombie(*pool, zombie_type::dolphin_rider, 2, 300);
    damage(pool->scene).take(
        dolphin,
        dolphin.hp - static_cast<int>(dolphin.max_hp / 3) + 1,
        static_cast<unsigned int>(zombie_damage_flags::ignore_accessory_2));
    CHECK(!dolphin.is_not_dying);

    auto balloon_world = std::make_unique<world>(scene_type::day);
    auto& balloon = make_zombie(*balloon_world, zombie_type::balloon, 0, 300);
    balloon.status = zombie_status::balloon_walking;
    balloon.has_balloon = false;
    balloon.dy = 0;
    damage(balloon_world->scene).take(
        balloon,
        balloon.hp - static_cast<int>(balloon.max_hp / 3) + 1,
        static_cast<unsigned int>(zombie_damage_flags::ignore_accessory_2));
    CHECK(!balloon.is_not_dying);
}

void test_snorkel_swim_head_drop_dies_immediately() {
    world w(scene_type::pool);
    auto& snorkel = make_zombie(w, zombie_type::snorkel, 2, 300);
    snorkel.status = zombie_status::snorkel_swim;
    snorkel.is_in_water = true;

    damage(w.scene).take(
        snorkel,
        static_cast<unsigned int>(snorkel.hp - 70),
        static_cast<unsigned int>(zombie_damage_flags::ignore_accessory_2));

    CHECK(!snorkel.is_not_dying);
    CHECK(snorkel.is_dead);
}

void test_destroyed_shield_preserves_max_hp() {
    world w(scene_type::day);
    auto& door = make_zombie(w, zombie_type::screendoor, 0, 300);
    const auto max_hp = door.accessory_2.max_hp;

    damage(w.scene).take(door, door.accessory_2.hp, 0);

    CHECK(door.accessory_2.type == zombie_accessories_type_2::none);
    CHECK(door.accessory_2.hp == 0);
    CHECK(door.accessory_2.max_hp == max_hp);
}

void test_night_grave_spawn_fills_to_max() {
    world w(scene_type::night);
    create_night_graves(w, 11);
    arm_night_w20_grave_spawn(w);

    w.spawn.update();

    CHECK(count_graves(w) == 12);
}

void test_night_grave_spawn_stops_at_max() {
    world w(scene_type::night);
    create_night_graves(w, 12);
    arm_night_w20_grave_spawn(w);

    w.spawn.update();

    CHECK(count_graves(w) == 12);
}

void test_w20_clear_field_starts_endgame_countdown() {
    world w(scene_type::pool);
    arm_w20_countdown(w, 10);
    w.scene.ice_path.countdown[0] = 1;
    w.scene.ice_path.countdown[1] = 700;

    w.spawn.update();

    CHECK(w.scene.spawn.countdown.endgame == 500);
    CHECK(w.scene.spawn.countdown.next_wave == 10);
    check_ice_timers_are_500(w);
}

void test_w20_enemy_blocks_clear_field_endgame_countdown() {
    world w(scene_type::pool);
    arm_w20_countdown(w, 10);
    make_zombie(w, zombie_type::zombie, 0, 300);

    w.spawn.update();

    CHECK(w.scene.spawn.countdown.endgame == 0);
    CHECK(w.scene.spawn.countdown.next_wave == 9);
}

void test_w20_clear_field_ignores_non_enemy_zombies() {
    world w(scene_type::pool);
    arm_w20_countdown(w, 10);

    auto& no_head = make_zombie(w, zombie_type::zombie, 0, 300);
    no_head.is_not_dying = false;
    auto& dead = make_zombie(w, zombie_type::zombie, 1, 300);
    dead.is_dead = true;
    auto& dying = make_zombie(w, zombie_type::zombie, 2, 300);
    dying.status = zombie_status::dying;
    auto& hypno = make_zombie(w, zombie_type::zombie, 3, 300);
    hypno.is_hypno = true;

    w.spawn.update();

    CHECK(w.scene.spawn.countdown.endgame == 500);
    CHECK(w.scene.spawn.countdown.next_wave == 10);
}

void test_w20_timeout_starts_shared_endgame_countdown() {
    world w(scene_type::pool);
    arm_w20_countdown(w, 1);
    make_zombie(w, zombie_type::zombie, 0, 300);
    w.scene.ice_path.countdown[0] = 1;
    w.scene.ice_path.countdown[1] = 700;

    w.spawn.update();

    CHECK(w.scene.spawn.countdown.endgame == 500);
    CHECK(w.scene.spawn.countdown.next_wave == 0);
    check_ice_timers_are_500(w);
}

void test_scene_copy_check_build_and_json_regressions() {
    auto cob_world = std::make_unique<world>(scene_type::day);
    auto& cob = cob_world->plant_factory.create(plant_type::cob_cannon, 0, 2);
    auto copied_scene = std::make_unique<scene>(cob_world->scene);
    CHECK(copied_scene->plant_map[0][2].content != nullptr);
    CHECK(copied_scene->plant_map[0][3].content != nullptr);
    CHECK(copied_scene->plant_map[0][2].content == copied_scene->plant_map[0][3].content);
    CHECK(copied_scene->plant_map[0][2].content->type == cob.type);

    auto build_world = std::make_unique<world>(scene_type::day);
    build_world->plant_factory.create(plant_type::flower_pot, 0, 0);
    CHECK(!build_world->check_build({{plant_type::coffee_bean, 0, 0}}));
    build_world->plant_factory.create(plant_type::coffee_bean, 0, 1);
    CHECK(build_world->check_build({{plant_type::coffee_bean, 0, 1}}));
    CHECK(!build_world->check_build({{
        plant_type::coffee_bean,
        build_world->scene.rows,
        0}}));
    CHECK(!build_world->check_build({{plant_type::coffee_bean, 0, 9}}));
    CHECK(!build_world->check_build({{
        plant_type::coffee_bean,
        std::numeric_limits<unsigned int>::max(),
        std::numeric_limits<unsigned int>::max()}}));

    auto json_world = std::make_unique<world>(scene_type::day);
    auto& json_cob = json_world->plant_factory.create(plant_type::cob_cannon, 0, 2);
    json_cob.x = 111;
    json_cob.y = 222;
    json_cob.cannon.x = 333;
    json_cob.cannon.y = 444;
    json_world->scene.spawn.countdown.hugewave_fade = 123;
    json_world->scene.spawn.countdown.endgame = 321;
    rapidjson::StringBuffer buffer;
    rapidjson::Writer<rapidjson::StringBuffer> writer(buffer);
    json_world->scene.to_json(writer);
    const std::string json(buffer.GetString());
    CHECK(json.find("\"hugewave_fade\":123") != std::string::npos);
    CHECK(json.find("\"endgame\":321") != std::string::npos);
    CHECK(json.find("\"cannon\":{\"x\":333,\"y\":444}") != std::string::npos);
}

void test_sun_and_plant_initialization_regressions() {
    world production_world(scene_type::day);
    production_world.scene.sun.sun = 0;
    production_world.scene.sun.natural_sun_countdown = 100;
    auto& sunflower = production_world.plant_factory.create(plant_type::sunflower, 0, 0);
    sunflower.countdown.generate = 1;
    production_world.plant_system.update();
    CHECK(production_world.scene.sun.sun == 25);
    CHECK(production_world.scene.sun.natural_sun_countdown == 100);

    bool reached_700_endpoint = false;
    for (std::uint32_t seed = 0; seed < 10000 && !reached_700_endpoint; ++seed) {
        world candidate(scene_type::day);
        candidate.scene.rng.seed(seed);
        candidate.scene.sun.natural_sun_generated = 0;
        candidate.scene.sun.natural_sun_countdown = 0;
        candidate.sun.update();
        CHECK(candidate.scene.sun.natural_sun_countdown >= 424);
        CHECK(candidate.scene.sun.natural_sun_countdown <= 699);
        reached_700_endpoint = candidate.scene.sun.natural_sun_countdown == 699;
    }
    CHECK(reached_700_endpoint);

    world grave_world(scene_type::day);
    grave_world.scene.rng.seed(12345);
    scene expected_scene(grave_world.scene);
    const auto expected_fps = system::rng(expected_scene).randfloat(10, 15);
    auto& grave_buster = grave_world.plant_factory.create(plant_type::grave_buster, 0, 0);
    CHECK(grave_buster.reanim.fps == expected_fps);
    CHECK(grave_buster.reanim.fps >= 10.0f);
    CHECK(grave_buster.reanim.fps <= 15.0f);
    CHECK(grave_world.scene.rng == expected_scene.rng);
}

void test_newspaper_and_ladder_chew_phase() {
    auto triggers_hypno = [](zombie_type type, float previous, float current) {
        world w(scene_type::night);
        auto& mushroom = w.plant_factory.create(plant_type::hypnoshroom, 0, 3);
        auto& zombie = w.zombie_factory.create(type, 0);
        zombie.row = 0;
        zombie.x = static_cast<float>(mushroom.x + 20);
        zombie.y = static_cast<float>(mushroom.y);
        zombie.int_x = static_cast<int>(zombie.x);
        zombie.int_y = static_cast<int>(zombie.y);
        damage(w.scene).set_is_eating(zombie);
        zombie.reanim.prev_progress = previous;
        zombie.reanim.progress = current;
        w.zombie.update();
        return zombie.is_hypno;
    };

    CHECK(triggers_hypno(zombie_type::zombie, 0.10f, 0.15f));
    CHECK(!triggers_hypno(zombie_type::newspaper, 0.10f, 0.15f));
    CHECK(!triggers_hypno(zombie_type::ladder, 0.10f, 0.15f));
    CHECK(triggers_hypno(zombie_type::newspaper, 0.40f, 0.43f));
    CHECK(triggers_hypno(zombie_type::ladder, 0.40f, 0.43f));
}

void test_deferred_copy_and_public_input_safety() {
    world original(scene_type::day);
    CHECK(original.scene.sun.natural_sun_countdown == 0);
    CHECK(original.scene.spawn.wave == 0);
    for (const auto flag : original.scene.spawn.spawn_flags) {
        CHECK(!flag);
    }

    original.scene.rng.seed(1234);
    original.scene.sun.sun = 4321;
    original.scene.sun.natural_sun_generated = 7;
    original.scene.sun.natural_sun_countdown = 222;
    original.scene.spawn.wave = 11;
    original.scene.spawn.countdown.next_wave = 333;
    auto& plant = original.plant_factory.create(plant_type::pea_shooter, 0, 0);
    const auto plant_id = original.scene.plants.get_id(plant);
    const auto expected_rng = original.scene.rng;

    world copied(original);
    CHECK(copied.scene.rng == expected_rng);
    CHECK(copied.scene.sun.sun == 4321);
    CHECK(copied.scene.sun.natural_sun_generated == 7);
    CHECK(copied.scene.sun.natural_sun_countdown == 222);
    CHECK(copied.scene.spawn.wave == 11);
    CHECK(copied.scene.spawn.countdown.next_wave == 333);
    auto* copied_plant = copied.scene.plants.try_to_get(plant_id);
    CHECK(copied_plant != nullptr);
    CHECK(copied_plant != &plant);
    CHECK(copied.scene.plant_map[0][0].content == copied_plant);

    world squash_world(scene_type::day);
    auto& squash = squash_world.plant_factory.create(plant_type::squash, 0, 3);
    squash.status = plant_status::squash_stop_in_the_air;
    squash_world.scene.unregister_plant_from_map(squash);
    world squash_copy(squash_world);
    CHECK(squash_copy.scene.plant_map[0][3].content == nullptr);

    CHECK(!original.check_build({{plant_type::pea_shooter, 99, 0}}));
    CHECK(!original.check_build({{plant_type::pea_shooter, 0, 99}}));
    CHECK(!original.plant_factory.can_plant(0, 1, plant_type::none));
    expect_invalid_argument([&]() {
        (void)original.plant_factory.get_cost(plant_type::none);
    });
    expect_invalid_argument([&]() {
        original.plant_factory.create(
            plant_type::imitater, 0, 1, plant_type::cob_cannon);
    });

    world first(scene_type::day);
    world second(scene_type::day);
    std::vector<world*> worlds = {&first};
    world::action_vector all_actions;
    world::action_vector actions;
    world::batch_action_masks masks = {{7}};
    world::check_list checks;
    std::vector<int> check_result = {8};
    std::vector<int> done = {9};
    expect_invalid_argument([&]() {
        world::update_all(
            worlds, all_actions, actions, masks, checks, check_result, done, 1);
    });
    CHECK(check_result == std::vector<int>({8}));
    CHECK(done == std::vector<int>({9}));

    actions.push_back({-3, 0, 0});
    expect_invalid_argument([&]() {
        world::update_all(
            worlds, all_actions, actions, masks, checks, check_result, done, 0);
    });
    std::vector<world*> null_worlds = {nullptr};
    expect_invalid_argument([&]() {
        world::update_all(
            null_worlds, all_actions, actions, masks, checks, check_result, done, 1);
    });
    std::vector<world*> duplicate_worlds = {&first, &first};
    world::action_vector duplicate_actions = {{-3, 0, 0}, {-3, 0, 0}};
    expect_invalid_argument([&]() {
        world::update_all(
            duplicate_worlds,
            all_actions,
            duplicate_actions,
            masks,
            checks,
            check_result,
            done,
            1);
    });

    worlds.clear();
    actions.clear();
    world::update_all(
        worlds, all_actions, actions, masks, checks, check_result, done, 1);
    CHECK(masks.empty());
    CHECK(check_result.empty());
    CHECK(done.empty());
}

void test_observation_layout_and_capacity_safety() {
    world first(scene_type::day);
    world second(scene_type::day);
    first.plant_factory.create(plant_type::pea_shooter, 0, 0);
    first.plant_factory.create(plant_type::sunflower, 0, 1);
    first.scene.ice_path.x[0] = 400;
    first.scene.ice_path.countdown[0] = 1500;

    learning::observation_factory observation(scene_type::day, 0, 2, 0, 0);
    world::action_masks no_masks;
    std::vector<float> values;
    observation.create(first, no_masks, values);
    CHECK(values[0] != values[learning::observation_factory::plant_size]);
    const std::size_t cards_offset =
        2 * learning::observation_factory::plant_size;
    for (std::size_t slot = 0; slot < 10; ++slot) {
        CHECK(values[cards_offset + 3 * slot + 2] == 0.0f);
    }
    const auto ice_offset = cards_offset + 30;
    CHECK(values[ice_offset] == 0.5f);
    CHECK(values[ice_offset + 1] == 0.5f);

    std::vector<world*> empty_worlds;
    world::batch_action_masks empty_masks;
    values = {1.0f};
    observation.create(empty_worlds, empty_masks, values);
    CHECK(values.empty());

    std::vector<world*> two_worlds = {&first, &second};
    world::batch_action_masks bad_masks = {{1}, {1, 2}};
    values = {3.0f};
    expect_invalid_argument([&]() {
        observation.create(two_worlds, bad_masks, values);
    });
    CHECK(values == std::vector<float>({3.0f}));

    expect_invalid_argument([&]() {
        learning::observation_factory too_large(
            scene_type::day,
            std::numeric_limits<unsigned int>::max(),
            0,
            0,
            0);
        (void)too_large;
    });
}

void test_off_ground_plant_map_and_animation_transitions() {
    world smashed_world(scene_type::day);
    auto& smashed = smashed_world.plant_factory.create(
        plant_type::repeater, 0, 0);
    damage(smashed_world.scene).set_smashed(smashed);
    CHECK(smashed.is_smashed);
    CHECK(smashed_world.scene.plant_map[0][0].content == nullptr);
    CHECK(smashed_world.plant_factory.can_plant_at(
              0, 0, plant_type::pea_shooter) == planting_reason::ok);

    world cob_world(scene_type::day);
    cob_world.plant_factory.create(plant_type::kernelpult, 0, 0);
    auto& lifted_right = cob_world.plant_factory.create(
        plant_type::kernelpult, 0, 1);
    lifted_right.edible = plant_edible_status::invisible_and_not_edible;
    cob_world.scene.unregister_plant_from_map(lifted_right);
    CHECK(cob_world.plant_factory.can_plant_at(
              0, 0, plant_type::cob_cannon) == planting_reason::needs_upgrade);

    world chomper_world(scene_type::day);
    auto& chomper = chomper_world.plant_factory.create(
        plant_type::chomper, 0, 3);
    auto& target = make_zombie(
        chomper_world, zombie_type::zombie, 0, chomper.x + 60);
    target.y = static_cast<float>(chomper.y);
    target.int_y = chomper.y;
    chomper.reanim.progress = 0.73f;
    chomper.reanim.prev_progress = 0.60f;
    plant_chomper(chomper_world.scene).update(chomper);
    CHECK(chomper.status == plant_status::chomper_bite_begin);
    CHECK(chomper.reanim.progress == 0.0f);
    CHECK(chomper.reanim.prev_progress == -1.0f);

    world squash_world(scene_type::day);
    auto& squash = squash_world.plant_factory.create(
        plant_type::squash, 0, 3);
    squash.status = plant_status::squash_jump_up;
    squash.countdown.status = 0;
    squash.cannon.x = 321;
    plant_squash(squash_world.scene).update(squash);
    CHECK(squash.status == plant_status::squash_stop_in_the_air);
    CHECK(squash.cannon.x == 321);
    CHECK(squash_world.scene.plant_map[0][3].content == nullptr);
}

void test_special_zombie_damage_and_reanimation_transitions() {
    world bungee_world(scene_type::day);
    auto& target = bungee_world.plant_factory.create(
        plant_type::pea_shooter, 0, 0);
    auto& bungee = bungee_world.zombie_factory.create(zombie_type::bungee, 0);
    bungee.status = zombie_status::bungee_grab;
    bungee.bungee_target = static_cast<int>(
        bungee_world.scene.plants.get_id(target));
    target.edible = plant_edible_status::invisible_and_edible;
    rect hit_box;
    bungee.get_hit_box(hit_box);
    damage(bungee_world.scene).take_instant_kill(
        0,
        hit_box.x + hit_box.width / 2,
        hit_box.y + hit_box.height / 2,
        100,
        0,
        true,
        static_cast<unsigned char>(attack_flags::ground));
    CHECK(bungee.is_dead);
    CHECK(bungee.bungee_target == -1);
    CHECK(target.edible == plant_edible_status::visible_and_edible);

    world pogo_world(scene_type::day);
    auto& pogo = pogo_world.zombie_factory.create(zombie_type::pogo, 0);
    damage(pogo_world.scene).take(
        pogo,
        static_cast<unsigned int>(pogo.hp - 165),
        static_cast<unsigned int>(zombie_damage_flags::ignore_accessory_2));
    CHECK(!pogo.is_not_dying);
    CHECK(!pogo.has_item_or_walk_left);
    CHECK(pogo.status == zombie_status::walking);

    const zombie_status dolphin_statuses[] = {
        zombie_status::dolphin_walk_with_dolphin,
        zombie_status::dolphin_jump_in_pool,
        zombie_status::dolphin_ride,
        zombie_status::dolphin_jump,
        zombie_status::dolphin_walk_in_pool,
        zombie_status::dolphin_walk_without_dolphin,
    };
    for (const auto status : dolphin_statuses) {
        world dolphin_world(scene_type::pool);
        auto& dolphin = dolphin_world.zombie_factory.create(
            zombie_type::dolphin_rider, 0);
        dolphin.row = 2;
        dolphin.status = status;
        damage(dolphin_world.scene).take(
            dolphin,
            static_cast<unsigned int>(dolphin.hp + 1),
            static_cast<unsigned int>(zombie_damage_flags::ignore_accessory_2));
        CHECK(dolphin.is_dead);
    }

    world yeti_world(scene_type::day);
    auto& yeti = yeti_world.zombie_factory.create(zombie_type::yeti, 0);
    yeti.status = zombie_status::walking;
    system::reanim(yeti_world.scene).update_dx(yeti, false);
    CHECK(yeti.dx == 0.40000001f);
    yeti.status = zombie_status::yeti_escape;
    system::reanim(yeti_world.scene).update_dx(yeti, false);
    CHECK(yeti.dx == 0.80000001f);
}

void test_endgame_gates_spawn_and_natural_sun() {
    world clear_world(scene_type::pool);
    arm_w20_countdown(clear_world, 10);
    clear_world.scene.spawn.countdown.lurking_squad = 1;
    clear_world.scene.sun.sun = 0;
    clear_world.scene.sun.natural_sun_countdown = 100;
    CHECK(!clear_world.update());
    CHECK(clear_world.scene.spawn.countdown.endgame == 499);
    CHECK(clear_world.scene.spawn.countdown.lurking_squad == 1);
    CHECK(clear_world.scene.sun.natural_sun_countdown == 100);

    world timeout_world(scene_type::pool);
    arm_w20_countdown(timeout_world, 1);
    make_zombie(timeout_world, zombie_type::zombie, 0, 300);
    timeout_world.scene.sun.sun = 0;
    timeout_world.scene.sun.natural_sun_countdown = 100;
    CHECK(!timeout_world.update());
    CHECK(timeout_world.scene.spawn.countdown.endgame == 499);
    CHECK(timeout_world.scene.sun.natural_sun_countdown == 99);
    timeout_world.update();
    CHECK(timeout_world.scene.sun.natural_sun_countdown == 99);
}

void test_real_object_pool_reuse_and_stale_ids() {
    world w(scene_type::day);

    auto& old_plant = w.plant_factory.create(plant_type::pea_shooter, 0, 0);
    const auto plant_id = w.scene.plants.get_id(old_plant);
    CHECK(w.scene.plants.get_id_unchecked(old_plant) == plant_id);
    w.plant_factory.destroy(old_plant);
    CHECK(w.scene.plants.get_id_unchecked(old_plant) == plant_id);
    w.scene.plants.shrink_to_fit();
    CHECK(w.scene.plants.try_to_get(plant_id) == nullptr);
    auto& new_plant = w.plant_factory.create(plant_type::pea_shooter, 0, 0);
    CHECK(w.scene.plants.get_id(new_plant) != plant_id);
    CHECK(w.scene.plants.get_id_unchecked(new_plant) == w.scene.plants.get_id(new_plant));

    auto& old_projectile = projectile_factory(w.scene).create(
        projectile_type::pea, 0, 100, 0);
    old_projectile.time_since_created = 123;
    const auto projectile_id = w.scene.projectiles.get_id(old_projectile);
    CHECK(w.scene.projectiles.get_id_unchecked(old_projectile) == projectile_id);
    projectile_factory(w.scene).destroy(old_projectile);
    CHECK(w.scene.projectiles.get_id_unchecked(old_projectile) == projectile_id);
    w.scene.projectiles.shrink_to_fit();
    CHECK(w.scene.projectiles.try_to_get(projectile_id) == nullptr);
    auto& new_projectile = projectile_factory(w.scene).create(
        projectile_type::pea, 0, 100, 0);
    CHECK(w.scene.projectiles.get_id(new_projectile) != projectile_id);
    CHECK(w.scene.projectiles.get_id_unchecked(new_projectile) == w.scene.projectiles.get_id(new_projectile));
    CHECK(new_projectile.time_since_created == 0);

    auto& old_item = w.griditem_factory.create(griditem_type::grave, 0, 0);
    const auto item_id = w.scene.griditems.get_id(old_item);
    CHECK(w.scene.griditems.get_id_unchecked(old_item) == item_id);
    w.griditem_factory.destroy(old_item);
    CHECK(w.scene.griditems.get_id_unchecked(old_item) == item_id);
    w.scene.griditems.shrink_to_fit();
    CHECK(w.scene.griditems.try_to_get(item_id) == nullptr);
    auto& new_item = w.griditem_factory.create(griditem_type::grave, 0, 0);
    CHECK(w.scene.griditems.get_id(new_item) != item_id);
    CHECK(w.scene.griditems.get_id_unchecked(new_item) == w.scene.griditems.get_id(new_item));
    CHECK(!new_item.is_disappeared);

    auto& old_ladder = w.zombie_factory.create(zombie_type::ladder, 0);
    old_ladder.accessory_2.hp = 500;
    const auto zombie_id = w.scene.zombies.get_id(old_ladder);
    CHECK(w.scene.zombies.get_id_unchecked(old_ladder) == zombie_id);
    w.zombie_factory.destroy(old_ladder);
    CHECK(w.scene.zombies.get_id_unchecked(old_ladder) == zombie_id);
    w.scene.zombies.shrink_to_fit();
    CHECK(w.scene.zombies.try_to_get(zombie_id) == nullptr);
    auto& normal = w.zombie_factory.create(zombie_type::zombie, 0);
    CHECK(w.scene.zombies.get_id(normal) != zombie_id);
    CHECK(w.scene.zombies.get_id_unchecked(normal) == w.scene.zombies.get_id(normal));
    CHECK(normal.accessory_2.type == zombie_accessories_type_2::none);
    CHECK(normal.accessory_2.hp == 0);
    CHECK(normal.accessory_2.max_hp == 0);
}

void test_bungee_lift_and_pool_dancer_boundaries() {
    world lifted_world(scene_type::pool);
    auto& target = lifted_world.plant_factory.create(
        plant_type::pea_shooter, 0, 0);
    const auto target_id = lifted_world.scene.plants.get_id(target);
    auto& bungee = lifted_world.zombie_factory.create(zombie_type::bungee, 0);
    bungee.row = 0;
    bungee.bungee_col = 0;
    bungee.status = zombie_status::bungee_grab;
    bungee.bungee_target = static_cast<int>(target_id);
    bungee.reanim.n_repeated = 1;
    zombie_bungee(lifted_world.scene).update(bungee);
    CHECK(bungee.status == zombie_status::bungee_raise);
    CHECK(target.edible == plant_edible_status::invisible_and_not_edible);
    CHECK(lifted_world.scene.plant_map[0][0].content == nullptr);
    CHECK(lifted_world.plant_factory.can_plant_at(
              0, 0, plant_type::pea_shooter) == planting_reason::ok);

    world copied(lifted_world);
    auto* copied_target = copied.scene.plants.try_to_get(target_id);
    CHECK(copied_target != nullptr);
    CHECK(copied_target->edible ==
          plant_edible_status::invisible_and_not_edible);
    CHECK(copied.scene.plant_map[0][0].content == nullptr);

    world dancer_world(scene_type::pool);
    dancer_world.scene.zombie_dancing_clock = 0;
    auto& dancer = make_zombie(
        dancer_world, zombie_type::dancing, 0, 500);
    dancer.status = zombie_status::dancing_walking;
    dancer.catapult_or_jackson.summon_countdown = 0;
    zombie_dancing(dancer_world.scene).update(dancer);
    CHECK(dancer.catapult_or_jackson.summon_countdown == 100);
}

void test_original_rng_contract_and_pool_capacity() {
    const auto spawn_vector = [](unsigned int seed) {
        world sample(scene_type::day);
        sample.scene.rng.seed(seed);
        sample.scene.level_rng.seed(seed);
        sample.spawn.reset();
        std::vector<unsigned int> flags;
        for (unsigned int index = 0; index < sample.scene.spawn.spawn_flags.size(); ++index) {
            if (sample.scene.spawn.spawn_flags[index]) {
                flags.push_back(index);
            }
        }
        return flags;
    };
    bool saw_giga = false;
    for (const auto seed : {0u, 0x5eed1051u}) {
        const auto flags = spawn_vector(seed);
        CHECK(flags.size() == 12);
        CHECK(std::find(flags.begin(), flags.end(), 0u) != flags.end());
        saw_giga |= std::find(flags.begin(), flags.end(), 32u) != flags.end();
    }
    CHECK(saw_giga);
    scene seeded(scene_type::day);
    seeded.rng.seed(0x5eed1051);
    system::rng random(seeded);
    const unsigned int expected[] = {
        253997743u, 1118501867u, 1017248747u, 1169085152u, 223323071u,
    };
    for (const auto value : expected) {
        CHECK(random.randint(0x80000000u) == value);
    }

    seeded.rng.seed(0);
    CHECK(system::rng(seeded).randint(0x80000000u) == 2146374468u);

    seeded.rng.lock(1118501867u);
    const auto native_float = system::rng(seeded).randfloat(10.0f, 15.0f);
    std::uint32_t native_float_bits = 0;
    std::memcpy(&native_float_bits, &native_float, sizeof(native_float_bits));
    CHECK(native_float_bits == 0x4149aadeu);

    seeded.rng.seed(7);
    seeded.level_rng.seed(11);
    const auto battle_before_level_draws = seeded.rng;
    system::rng(seeded.level_rng).randint(10);
    system::rng(seeded.level_rng).randint(10);
    CHECK(seeded.rng == battle_before_level_draws);
    system::rng(seeded).randint(10);
    CHECK(!(seeded.rng == battle_before_level_draws));
    seeded.level_rng.reseed_engine(112);
    random_stream fresh_level(112);
    CHECK(seeded.level_rng.seed_value() == 112);
    CHECK(seeded.level_rng.next() == fresh_level.next());

    world next_round(scene_type::day);
    next_round.scene.rng.seed(7);
    next_round.scene.level_rng.seed(11);
    next_round.scene.natural_sun_drop_disabled = true;
    next_round.scene.stop_spawn = true;
    next_round.scene.spawn.countdown.endgame = 1;
    next_round.plant_factory.create(plant_type::pea_shooter, 0, 0);
    CHECK(next_round.update());
    CHECK(next_round.scene.level_rng.seed_value() == 112);
    CHECK(next_round.scene.level_rng.source_seed_value() == 112);
    CHECK(next_round.scene.plants.size() == 1);

    world zero_seed_round(scene_type::day);
    zero_seed_round.scene.level_rng.seed(0);
    zero_seed_round.scene.stop_spawn = true;
    zero_seed_round.scene.spawn.countdown.endgame = 1;
    CHECK(zero_seed_round.update());
    CHECK(zero_seed_round.scene.level_rng.seed_value() == 101);
    CHECK(zero_seed_round.scene.level_rng.source_seed_value() == 101);

    world locked_next_round(scene_type::day);
    locked_next_round.scene.level_rng.seed(11);
    locked_next_round.scene.level_rng.lock(7);
    locked_next_round.scene.stop_spawn = true;
    locked_next_round.scene.spawn.countdown.endgame = 1;
    CHECK(locked_next_round.update());
    CHECK(locked_next_round.scene.level_rng.seed_value() == 112);
    CHECK(locked_next_round.scene.level_rng.source_seed_value() == 112);
    CHECK(locked_next_round.scene.level_rng.locked());
    CHECK(locked_next_round.scene.level_rng.fixed_value() == 7);
    CHECK(locked_next_round.scene.level_rng.next() == 7);

    std::array<unsigned int, 3> weights = {1, 2, 3};
    const auto picked = system::rng(seeded).random_weighted_sample(weights);
    CHECK(picked < weights.size());

    std::array<float, 3> smooth_weights = {0.1f, 0.2f, 0.7f};
    seeded.rng.lock(0);
    CHECK(system::rng(seeded).random_weighted_sample(smooth_weights) == 0);
    seeded.rng.lock(0x7fffffffu);
    CHECK(system::rng(seeded).random_weighted_sample(smooth_weights) == 2);
    std::array<float, 4> native_boundary_weights = {0.0025f, 0.0025f, 0.0025f, 0.0025f};
    seeded.rng.lock(0x20000000u);
    CHECK(system::rng(seeded).random_weighted_sample(native_boundary_weights) == 0);
    CHECK(decltype(seeded.plants)::capacity() == 1024);

    auto full = std::make_unique<world>(scene_type::day);
    const auto zombie_limit = decltype(full->scene.zombies)::capacity() - 1;
    for (unsigned int i = 0; i < zombie_limit; ++i) {
        CHECK(full->zombie_factory.add_zombie_in_row(
                  zombie_type::zombie, 0, 0) != nullptr);
    }
    CHECK(full->scene.zombies.active_count() == zombie_limit);
    set_single_zombie_wave(*full, 0, zombie_type::zombie);
    full->scene.spawn.wave = 0;
    full->scene.spawn.countdown.next_wave = 1;
    full->scene.spawn.countdown.next_wave_initial = 1;
    full->spawn.update();
    CHECK(full->scene.zombies.active_count() == zombie_limit);
}

void test_projectile_target_pruning_preserves_collision_rules() {
    for (float height : {45.0f, 46.0f}) {
        world w(scene_type::pool);
        auto& snorkel = make_zombie(w, zombie_type::snorkel, 2, 300);
        snorkel.status = zombie_status::snorkel_swim;
        snorkel.is_in_water = true;
        auto& proj = projectile_factory(w.scene).create(projectile_type::cactus, 2, 330, 300);
        proj.flags = static_cast<unsigned int>(attack_flags::lurking_snorkel);
        proj.shadow_y = 350;
        proj.dy1 = height;
        const auto hp = snorkel.hp;
        w.projectile.update();
        CHECK(snorkel.hp == hp - (height > 45.0f ? 20 : 0));
    }
    for (auto type : {projectile_type::pea, projectile_type::star, projectile_type::cactus}) {
        for (bool hypno : {false, true}) {
            for (int overlap : {-1, 0, 1}) {
                world w(scene_type::day);
                auto& z = make_zombie(w, zombie_type::zombie, 0, 300);
                z.is_hypno = hypno;
                auto& proj = projectile_factory(w.scene).create(type, 0, 300, 100);
                proj.flags = static_cast<unsigned int>(attack_flags::ground) |
                    (hypno ? static_cast<unsigned int>(attack_flags::hypno_zombies) : 0u);
                proj.shadow_y = 150;
                rect zr, pr;
                z.get_hit_box(zr);
                proj.get_attack_box(pr);
                proj.int_x += zr.x - pr.x - pr.width + overlap;
                proj.x = static_cast<float>(proj.int_x);
                const auto hp = z.hp;
                w.projectile.update();
                CHECK(z.hp == hp - (overlap >= 0 ? 20 : 0));
                CHECK(proj.is_disappeared == (overlap >= 0));
            }
        }
    }

    for (bool ineligible_first : {false, true}) {
        world w(scene_type::day);
        auto& first = make_zombie(w, zombie_type::zombie, 0, ineligible_first ? 290 : 310);
        first.is_hypno = ineligible_first;
        auto& target = make_zombie(w, zombie_type::zombie, 0, 300);
        auto& tied = make_zombie(w, zombie_type::zombie, 0, 300);
        auto& proj = projectile_factory(w.scene).create(projectile_type::pea, 0, 340, 100);
        proj.flags = static_cast<unsigned int>(attack_flags::ground);
        proj.shadow_y = 150;
        const auto hp = target.hp;
        w.projectile.update();
        CHECK(first.hp == static_cast<int>(first.max_hp));
        CHECK(target.hp == hp - 20);
        CHECK(tied.hp == static_cast<int>(tied.max_hp));
    }

    for (bool backward : {false, true}) {
        for (unsigned age : {23u, 24u}) {
            world w(scene_type::day);
            auto& digger = make_zombie(w, zombie_type::digger, 0, 300);
            digger.status = zombie_status::digger_walk_right;
            digger.accessory_1.type = zombie_accessories_type_1::none;
            digger.accessory_1.hp = 0;
            rect zr;
            digger.get_hit_box(zr);
            auto& star = projectile_factory(w.scene).create(projectile_type::star, 0, zr.x, 100);
            star.flags = static_cast<unsigned int>(attack_flags::ground);
            star.shadow_y = 150;
            star.motion_type = projectile_motion_type::starfruit;
            star.dx = backward ? -3.33f : 3.33f;
            star.time_since_created = age;
            const auto hp = digger.hp;
            w.projectile.update();
            CHECK(digger.hp == hp - (backward || age == 24 ? 20 : 0));
        }
    }
}

void test_head_shooters_do_not_reset_body_reanimation_after_launch() {
    world w(scene_type::day);
    for (const auto type : {
             plant_type::repeater,
             plant_type::gatling_pea,
             plant_type::split_pea,
         })
    {
        auto& plant = w.plant_factory.create(
            type, 0, static_cast<unsigned int>(w.scene.plants.size()));
        plant.countdown.generate = 1000;
        plant.countdown.launch = 1;
        plant.reanim.fps = 12.0f;
        plant.reanim.begin_frame = 4;
        plant.reanim.n_frames = 25;
        plant.reanim.n_repeated = 1;
        plant.reanim.progress = 0.5f;
        plant.reanim.prev_progress = 0.49f;
        plant.reanim.type = reanim_type::loop;
    }

    w.update();

    for (const auto& plant : w.scene.plants) {
        CHECK(plant.reanim.n_repeated == 1);
        CHECK(plant.reanim.progress > 0.5f);
    }
}

void test_wave_spawn_private_rng_scope() {
    using spawn_state = std::tuple<unsigned int, float, float, float>;
    const auto run = [](std::uint32_t base_seed) {
        world sample(scene_type::night, 11, 22, 0);
        sample.scene.rng.lock(7);
        sample.scene.wave_spawn_random_enabled = true;
        sample.scene.wave_spawn_random_seed = base_seed;
        for (auto& entry : sample.scene.spawn.spawn_list[0]) {
            entry = zombie_type::zombie;
        }
        sample.scene.spawn.wave = 0;
        sample.scene.spawn.countdown.next_wave = 1;
        sample.scene.spawn.countdown.next_wave_initial = 1;
        const auto global_rng = sample.scene.rng;

        sample.spawn.update();

        CHECK(sample.scene.rng == global_rng);
        CHECK(sample.scene.battle_rng_override == nullptr);
        CHECK(!sample.scene.battle_rng_override_active);
        CHECK(sample.scene.zombies.active_count() == 50);
        std::vector<spawn_state> states;
        for (const auto& zombie : sample.scene.zombies) {
            states.emplace_back(zombie.row, zombie.x, zombie.dx, zombie.reanim.fps);
        }
        return states;
    };

    const auto first = run(0x5eed1051u);
    const auto repeated = run(0x5eed1051u);
    const auto other = run(0x5eed1052u);
    CHECK(first == repeated);
    CHECK(first != other);

    bool varied_row = false;
    bool varied_x = false;
    bool varied_speed = false;
    for (const auto& state : first) {
        varied_row |= std::get<0>(state) != std::get<0>(first.front());
        varied_x |= std::get<1>(state) != std::get<1>(first.front());
        varied_speed |= std::get<2>(state) != std::get<2>(first.front());
    }
    CHECK(varied_row);
    CHECK(!varied_x);
    CHECK(!varied_speed);
}

void test_pool_lurking_uses_native_column_major_grid_order() {
    world sample(scene_type::pool, 11, 22, 0);
    sample.scene.rng.lock(7);
    sample.scene.spawn.countdown.lurking_squad = 1;
    sample.scene.spawn.countdown.next_wave = 100;

    sample.spawn.update();

    CHECK(sample.scene.zombies.active_count() == 3);
    const unsigned int expected_rows[] = {2, 3, 2};
    const float expected_x[] = {415.0f, 415.0f, 495.0f};
    unsigned int index = 0;
    for (const auto& zombie : sample.scene.zombies) {
        CHECK(index < 3);
        CHECK(zombie.type == zombie_type::zombie);
        CHECK(zombie.status == zombie_status::rising_from_ground);
        CHECK(zombie.row == expected_rows[index]);
        CHECK(zombie.x == expected_x[index]);
        CHECK(zombie.reanim.progress == 0.0f);
        CHECK(zombie.reanim.prev_progress == -1.0f);
        ++index;
    }
    CHECK(index == 3);
}

void test_update_all_rethrows_worker_exception() {
    world w(scene_type::day);
    w.plant_factory.create(plant_type::pea_shooter, 0, 0);
    auto& catapult = w.zombie_factory.create(zombie_type::catapult, 0);
    catapult.status = zombie_status::catapult_shoot;
    catapult.countdown.action = 100;
    catapult.reanim.prev_progress = 0.54f;
    catapult.reanim.progress = 0.55f;

    projectile_factory projectiles(w.scene);
    for (std::uint32_t i = 0; i < decltype(w.scene.projectiles)::capacity(); ++i) {
        projectiles.create(projectile_type::pea, 0, 100, 80);
    }
    CHECK(w.scene.projectiles.active_count() ==
          decltype(w.scene.projectiles)::capacity());

    std::vector<world*> worlds = {&w};
    world::action_vector all_actions;
    world::action_vector actions = {{-3, 0, 0}};
    world::batch_action_masks masks;
    world::check_list checks;
    std::vector<int> check_result;
    std::vector<int> done;
    expect_bad_alloc([&]() {
        world::update_all(
            worlds,
            all_actions,
            actions,
            masks,
            checks,
            check_result,
            done,
            1);
    });
}

void test_rising_zombie_reanimation_keeps_advancing() {
    world w(scene_type::night);
    auto& zombie = make_zombie(w, zombie_type::zombie, 0, 300);
    zombie.status = zombie_status::rising_from_ground;
    zombie.countdown.action = 2;
    zombie.reanim.fps = 12.0f;
    zombie.reanim.n_frames = 100;
    zombie.reanim.progress = 0.25f;
    const float progress = zombie.reanim.progress;

    w.zombie.update();

    CHECK(zombie.status == zombie_status::rising_from_ground);
    CHECK(zombie.reanim.progress > progress);
}

void test_coffee_bean_bypasses_ice_for_existing_mushroom() {
    world w(scene_type::pool);
    auto& mushroom = w.plant_factory.create(plant_type::puffshroom, 0, 0);
    mushroom.is_sleeping = true;
    mushroom.countdown.awake = 0;
    w.scene.ice_path.countdown[0] = 100;
    w.scene.ice_path.x[0] = 0;

    CHECK(w.plant_factory.can_plant_at(
              1, 0, plant_type::pea_shooter) == planting_reason::not_here);
    CHECK(w.plant_factory.can_plant_at(
              0, 0, plant_type::coffee_bean) == planting_reason::ok);

    CHECK(w.griditem_factory.add_crater(0, 0) != nullptr);
    CHECK(w.griditem_factory.add_grave_stone(0, 0) != nullptr);
    CHECK(w.plant_factory.can_plant_at(
              0, 0, plant_type::coffee_bean) == planting_reason::ok);
}

void test_cattail_rejects_lily_pad_being_grabbed() {
    world w(scene_type::pool);
    auto& lily = w.plant_factory.create(plant_type::lily_pad, 2, 0);
    lily.edible = plant_edible_status::invisible_and_edible;

    CHECK(w.plant_factory.can_plant_at(
              0, 2, plant_type::cattail) != planting_reason::ok);
    lily.edible = plant_edible_status::visible_and_edible;
    CHECK(w.plant_factory.can_plant_at(
              0, 2, plant_type::cattail) == planting_reason::ok);
}

void test_dolphin_ride_uses_integer_left_bank_boundary() {
    world w(scene_type::pool);
    auto& dolphin = w.zombie_factory.create(zombie_type::dolphin_rider, 0);
    dolphin.row = 2;
    dolphin.status = zombie_status::dolphin_ride;
    dolphin.x = 10.5f;
    dolphin.int_x = 10;
    dolphin.dy = 0.0f;
    dolphin.action = zombie_action::none;

    zombie_dolphin_rider(w.scene).update(dolphin);

    CHECK(dolphin.status == zombie_status::dolphin_walk_with_dolphin);
    CHECK(dolphin.action == zombie_action::leaving_pool);
    CHECK(dolphin.dy == -40.0f);
}

void test_board_edge_uses_integer_x_and_inclusive_no_head_limit() {
    world premature_world(scene_type::day);
    premature_world.scene.zombies_die_at_house = true;
    auto& premature = make_zombie(
        premature_world, zombie_type::zombie, 0, -99);
    premature.x = -100.2f;
    premature.is_eating = true;

    premature_world.zombie.update();

    CHECK(!premature.is_dead);

    world no_head_world(scene_type::day);
    auto& no_head = make_zombie(
        no_head_world, zombie_type::zombie, 0, -30);
    no_head.is_not_dying = false;
    no_head.is_eating = true;

    no_head_world.zombie.update();

    CHECK(no_head.is_dead);
}

void test_starfruit_bottom_row_clamps_before_edge_collision() {
    world w(scene_type::night);
    auto& target = make_zombie(w, zombie_type::zombie, 4, 438);
    target.countdown.freeze = 100;
    const int hp = target.hp;

    auto& star = projectile_factory(w.scene).create(
        projectile_type::star, 4, 305, 505);
    star.motion_type = projectile_motion_type::starfruit;
    star.dx = 2.8838645f;
    star.dy2 = 1.665f;
    star.flags = static_cast<unsigned int>(attack_flags::ground);

    for (int age = 0; age < 45; ++age) {
        w.projectile.update();
    }

    rect projectile_box;
    rect zombie_box;
    star.get_attack_box(projectile_box);
    target.get_hit_box(zombie_box);
    CHECK(star.y < 580.0f);
    CHECK(projectile_box.get_overlap_len(zombie_box) == 0.0f);

    w.projectile.update();

    CHECK(star.y > 580.0f);
    CHECK(target.hp == hp - 20);
    CHECK(star.row == 4);
}

void test_cattail_homing_uses_integer_centers() {
    world w(scene_type::pool);
    auto& target = make_zombie(w, zombie_type::zombie, 0, 300);
    auto& needle = projectile_factory(w.scene).create(
        projectile_type::cactus, 0, 100, 37);
    needle.motion_type = projectile_motion_type::cattail;
    needle.target = static_cast<int>(w.scene.zombies.get_id(target));
    needle.flags = static_cast<unsigned int>(attack_flags::ground);
    needle.dx = 2.0f;
    needle.dy2 = 0.0f;

    w.projectile.update();

    CHECK(needle.dy2 == 0.0f);
}

void test_cattail_homing_matches_native_rounding() {
    world w(scene_type::pool);
    auto& target = make_zombie(w, zombie_type::zombie, 0, 798);
    target.y = 365.0f;
    target.int_y = 365;
    target.hit_box = {0, 0, 42, 115};
    auto& needle = projectile_factory(w.scene).create(
        projectile_type::cactus, 1, 390, 247);
    needle.y = 247.01219177246094f;
    needle.motion_type = projectile_motion_type::cattail;
    needle.target = static_cast<int>(w.scene.zombies.get_id(target));
    needle.flags = static_cast<unsigned int>(attack_flags::ground);
    needle.dx = 1.999993085861206f;
    needle.dy2 = 0.005238102283328772f;
    needle.time_since_created = 5;
    w.projectile.update();
    // Actual 1051 Full Witness, locked 7, frame 706 -> 707.
    CHECK(needle.dx == 1.9999864101409912f);
    CHECK(needle.dy2 == 0.00734360096976161f);
    CHECK(needle.y == 247.01953125f);
}

void test_homing_needles_survive_immunity_but_not_target_removal() {
    world w(scene_type::day);
    auto& target = make_zombie(w, zombie_type::pole_vaulting, 0, 400);
    target.status = zombie_status::pole_valuting_jumping;
    auto& needle = projectile_factory(w.scene).create(projectile_type::cactus, 0, 100, 37);
    needle.motion_type = projectile_motion_type::cattail;
    needle.flags = static_cast<unsigned int>(attack_flags::ground);
    needle.target = static_cast<int>(w.scene.zombies.get_id(target));
    needle.dx = 2;
    w.projectile.update();
    CHECK(!needle.is_disappeared);
    w.zombie_factory.destroy(target);
    w.projectile.update();
    CHECK(needle.is_disappeared);
}

void test_pult_vertical_velocity_rounds_before_subtraction() {
    world w(scene_type::pool);
    auto& melon = w.plant_factory.create(plant_type::winter_melon, 1, 0);
    melon.y = 165;
    auto& target = make_zombie(w, zombie_type::zombie, 1, 800);
    target.int_y = 57;
    target.hit_box.y = 0;
    w.plant_system.launch(melon, &target, 1, false);
    w.projectile.update();
    // 1051 Full Witness frame 2971: vertical range -62, first flight update.
    CHECK(w.scene.projectiles.get(0)->ddy == -7.401666641235352f);
}

void test_peashooter_head_shooting_hold_does_not_reset_body_reanim() {
    world w(scene_type::day);
    auto& pea = w.plant_factory.create(plant_type::pea_shooter, 0, 0);
    make_zombie(w, zombie_type::zombie, 0, 500);
    pea.countdown.generate = 1;
    pea.reanim.progress = 0.999f;

    w.plant_system.update();
    CHECK(pea.countdown.launch == 35);
    CHECK(pea.shooting_reanim_countdown == 71);
    CHECK(pea.reanim.n_repeated == 1);

    for (int frame = 0; frame < 34; ++frame) {
        w.plant_system.update();
    }
    CHECK(pea.countdown.launch == 1);
    CHECK(w.scene.projectiles.active_count() == 1);

    const float body_progress = pea.reanim.progress;
    for (int frame = 0; frame < 37; ++frame) {
        w.plant_system.update();
    }
    CHECK(pea.countdown.launch == 1);
    CHECK(pea.shooting_reanim_countdown == 0);
    CHECK(pea.reanim.progress > body_progress);

    w.plant_system.update();
    CHECK(pea.countdown.launch == 0);

    pea.countdown.generate = 1;
    w.plant_system.update();
    CHECK(pea.countdown.launch == 35);
    CHECK(pea.shooting_reanim_countdown == 0);
    for (int frame = 0; frame < 34; ++frame) {
        w.plant_system.update();
    }
    CHECK(pea.countdown.launch == 1);
    w.plant_system.update();
    CHECK(pea.countdown.launch == 0);

    world late(scene_type::day);
    auto& late_pea = late.plant_factory.create(
        plant_type::pea_shooter, 0, 0);
    late_pea.countdown.generate = 1000;
    for (int frame = 0; frame < 200; ++frame) {
        late.plant_system.update();
    }
    CHECK(late_pea.reanim.n_repeated > 0);

    make_zombie(late, zombie_type::zombie, 0, 500);
    late_pea.countdown.generate = 1;
    late.plant_system.update();
    CHECK(late_pea.countdown.launch == 35);
    CHECK(late_pea.shooting_reanim_countdown == 0);
    for (int frame = 0; frame < 34; ++frame) {
        late.plant_system.update();
    }
    CHECK(late_pea.countdown.launch == 1);
    late.plant_system.update();
    CHECK(late_pea.countdown.launch == 0);

    world snow(scene_type::day);
    auto& snow_pea = snow.plant_factory.create(
        plant_type::snow_pea, 0, 0);
    snow_pea.countdown.generate = 1000;
    for (int frame = 0; frame < 200; ++frame) {
        snow.plant_system.update();
    }
    CHECK(snow_pea.reanim.n_repeated > 0);

    make_zombie(snow, zombie_type::zombie, 0, 500);
    snow_pea.countdown.generate = 1;
    snow.plant_system.update();
    CHECK(snow_pea.countdown.launch == 35);
    CHECK(snow_pea.shooting_reanim_countdown == 0);
    for (int frame = 0; frame < 34; ++frame) {
        snow.plant_system.update();
    }
    CHECK(snow_pea.countdown.launch == 1);
    const auto body_loops = snow_pea.reanim.n_repeated;
    snow.plant_system.update();
    CHECK(snow_pea.countdown.launch == 0);
    CHECK(snow_pea.reanim.n_repeated >= body_loops);
}

void test_threepeater_shooting_state_returns_to_zero() {
    world w(scene_type::day);
    auto& threepeater = w.plant_factory.create(
        plant_type::threepeater, 2, 0);
    threepeater.countdown.generate = 1000;
    threepeater.countdown.launch = 2;

    w.plant_system.update();
    CHECK(w.scene.projectiles.active_count() == 3);
    CHECK(threepeater.threepeater_time_since_first_shot == 1);

    for (int frame = 0; frame < 30; ++frame) {
        w.plant_system.update();
    }
    CHECK(threepeater.countdown.launch == 1);
    CHECK(threepeater.threepeater_time_since_first_shot == 31);

    w.plant_system.update();
    CHECK(threepeater.countdown.launch == 0);
    CHECK(threepeater.threepeater_time_since_first_shot == 0);
}

void test_native_float_zombie_prediction() {
    world w(scene_type::day);
    auto& squash = w.plant_factory.create(plant_type::squash, 0, 3);
    auto& target = make_zombie(w, zombie_type::zombie, 0, 360);
    target.dx = 0.5f;
    target.countdown.slow = 100;
    squash.status = plant_status::squash_jump_up;
    squash.countdown.status = 0;
    squash.target = static_cast<int>(w.scene.zombies.get_id(target));

    plant_squash(w.scene).update(squash);

    CHECK(squash.cannon.x == 371);

    auto& football = make_zombie(w, zombie_type::football, 0, 500);
    rect hit_box;
    football.get_hit_box(hit_box);
    CHECK(zombie_base(w.scene).predict_after(football, 0) ==
          hit_box.x + hit_box.width / 2);
}

void test_start_eating_uses_zombie_type_for_snorkel() {
    world pool(scene_type::pool);
    auto& snorkel = make_zombie(pool, zombie_type::snorkel, 2, 300);
    snorkel.status = zombie_status::snorkel_swim;
    snorkel.reanim.progress = 0.5f;

    damage(pool.scene).set_is_eating(snorkel);

    CHECK(snorkel.is_eating);
    CHECK(snorkel.reanim.progress == 0.5f);

    world day(scene_type::day);
    auto& pole = make_zombie(day, zombie_type::pole_vaulting, 0, 300);
    pole.status = zombie_status::pole_valuting_running;
    pole.reanim.progress = 0.5f;

    damage(day.scene).set_is_eating(pole);

    CHECK(pole.is_eating);
    CHECK(pole.reanim.progress == 0.0f);
}

void test_flag_head_drop_item_state() {
    world w(scene_type::day);
    auto& flag = make_zombie(w, zombie_type::flag, 0, 300);
    CHECK(flag.has_item_or_walk_left);

    damage(w.scene).take(
        flag,
        static_cast<unsigned int>(flag.hp - 70),
        static_cast<unsigned int>(zombie_damage_flags::ignore_accessory_2));

    CHECK(!flag.has_item_or_walk_left);

    auto& retained = make_zombie(w, zombie_type::flag, 0, 300);
    damage(w.scene).take(
        retained,
        static_cast<unsigned int>(retained.hp - 70),
        static_cast<unsigned int>(zombie_damage_flags::ignore_accessory_2 |
            zombie_damage_flags::disable_ballon_pop));
    CHECK(retained.has_item_or_walk_left);
}

void test_grave_buster_bypasses_grid_gates() {
    world w(scene_type::day);
    CHECK(w.griditem_factory.add_grave_stone(0, 0) != nullptr);
    CHECK(w.griditem_factory.add_crater(0, 0) != nullptr);
    w.scene.ice_path.countdown[0] = 100;
    w.scene.ice_path.x[0] = 0;

    CHECK(w.plant_factory.can_plant_at(
              0, 0, plant_type::grave_buster) == planting_reason::ok);
}

void test_coffee_bean_crumble_dispatches_update() {
    world w(scene_type::day);
    auto& coffee = w.plant_factory.create(plant_type::coffee_bean, 0, 0);
    coffee.status = plant_status::work;
    coffee.reanim.n_repeated = 1;

    w.plant_system.update();

    CHECK(coffee.is_dead);
    CHECK(w.scene.plant_map[0][0].coffee_bean == nullptr);
}

void test_scene_copy_assignment_rebuilds_plant_map() {
    world source(scene_type::day);
    auto& source_plant = source.plant_factory.create(
        plant_type::pea_shooter, 0, 0);
    const auto plant_id = source.scene.plants.get_id(source_plant);
    world destination(scene_type::day);

    destination.scene = source.scene;

    auto* assigned_plant = destination.scene.plants.try_to_get(plant_id);
    CHECK(assigned_plant != nullptr);
    CHECK(assigned_plant != &source_plant);
    CHECK(destination.scene.plant_map[0][0].content == assigned_plant);
}

void test_basketball_velocity_uses_float_origin_and_integer_target() {
    world w(scene_type::day);
    auto& catapult = make_zombie(w, zombie_type::catapult, 0, 500);
    catapult.x = 500.5f;
    auto& target = w.plant_factory.create(plant_type::sunflower, 0, 1);

    projectile_factory(w.scene).create(
        projectile_type::basketball, catapult, &target);

    auto* targeted = w.scene.projectiles.get(0);
    CHECK(targeted != nullptr);
    CHECK(targeted->x == 613.0f);
    CHECK(targeted->dx == -(613.5f - target.x - 20.0f) / 120.0f);

    projectile_factory(w.scene).create(
        projectile_type::basketball, catapult, nullptr);

    auto* untargeted = w.scene.projectiles.get(1);
    CHECK(untargeted != nullptr);
    CHECK(untargeted->dx == -(613.5f - 200.0f - 20.0f) / 120.0f);
}

void test_starfruit_target_uses_integer_predicted_left_edge() {
    world w(scene_type::day);
    auto& starfruit = w.plant_factory.create(plant_type::starfruit, 0, 0);
    starfruit.countdown.launch = 0;
    auto& ladder = make_zombie(w, zombie_type::ladder, 1, 23);
    ladder.x = 23.0f;
    ladder.int_x = 23;
    ladder.y = 150.0f;
    ladder.int_y = 150;
    ladder.dx = 0.8f;

    plant_starfruit(w.scene).set_launch_countdown(starfruit);

    CHECK(starfruit.countdown.launch == 0);
}

void test_zombie_clip_height_is_subtracted_before_truncation() {
    zombie z{};
    z.status = zombie_status::rising_from_ground;
    z.dy = 0.5f;
    z.countdown.action = 40;
    z.hit_box.height = 115;
    z.attack_box.height = 115;

    rect hit_box;
    rect attack_box;
    z.get_hit_box(hit_box);
    z.get_attack_box(attack_box);

    CHECK(hit_box.height == 75);
    CHECK(attack_box.height == 75);
}

void test_factory_destroy_rejects_foreign_objects() {
    world owner(scene_type::day);
    world receiver(scene_type::day);

    auto& foreign_plant = owner.plant_factory.create(
        plant_type::pea_shooter, 0, 0);
    auto& receiver_ladder = receiver.griditem_factory.create(
        griditem_type::ladder, 0, 0);
    expect_invalid_argument([&]() {
        receiver.plant_factory.destroy(foreign_plant);
    });
    CHECK(!foreign_plant.is_dead);
    CHECK(!receiver_ladder.is_disappeared);

    auto& receiver_plant = receiver.plant_factory.create(
        plant_type::pea_shooter, 0, 0);
    receiver_plant.edible = plant_edible_status::invisible_and_not_edible;
    auto& foreign_zombie = owner.zombie_factory.create(zombie_type::bungee, 0);
    foreign_zombie.status = zombie_status::bungee_grab;
    foreign_zombie.bungee_target = static_cast<int>(
        receiver.scene.plants.get_id(receiver_plant));
    expect_invalid_argument([&]() {
        receiver.zombie_factory.destroy(foreign_zombie);
    });
    CHECK(!foreign_zombie.is_dead);
    CHECK(!receiver_plant.is_dead);
}

void test_cattail_integer_distance_tie_keeps_first_target() {
    world w(scene_type::pool);
    auto& cattail = w.plant_factory.create(plant_type::cattail, 2, 3);
    auto& first = make_zombie(w, zombie_type::zombie, 2, 268);
    first.y = 220.0f;
    first.int_y = 220;
    auto& second = make_zombie(w, zombie_type::zombie, 2, 264);
    second.y = 220.0f;
    second.int_y = 220;

    rect first_box;
    rect second_box;
    first.get_hit_box(first_box);
    second.get_hit_box(second_box);
    CHECK(first_box.x + first_box.width / 2 == 325);
    CHECK(second_box.x + second_box.width / 2 == 321);
    CHECK(first_box.y + first_box.height / 2 == 277);
    CHECK(second_box.y + second_box.height / 2 == 277);

    CHECK(plant_base(w.scene).find_target(
              cattail, cattail.row, false) == &first);
}

void test_reanim_progress_matches_native_pc24_stages() {
    world w(scene_type::day);

    object::reanim witness_animation{};
    witness_animation.fps = float_from_bits(0x41233C40u);
    witness_animation.n_frames = 47;
    witness_animation.type = reanim_type::repeat;
    system::reanim(w.scene).update_progress(witness_animation);
    CHECK(float_bits(witness_animation.progress) == 0x3B0E4203u);

    object::reanim animation{};
    animation.fps = 6.0f;
    animation.n_frames = 10;
    animation.type = reanim_type::repeat;

    for (int frame = 0; frame < 4; ++frame) {
        system::reanim(w.scene).update_progress(animation);
    }

    CHECK(float_bits(animation.progress) == 0x3CC49BA6u);
}

void test_zomboni_speed_curve_uses_integer_x() {
    world w(scene_type::day);
    auto& zomboni = make_zombie(w, zombie_type::zomboni, 0, 699);
    zomboni.x = 699.75f;

    zombie_zomboni(w.scene).update(zomboni);

    CHECK(float_bits(zomboni.dx) == 0x3E7F7CEEu);

    auto& pc24_boundary = make_zombie(w, zombie_type::zomboni, 1, 634);
    pc24_boundary.x = 634.75f;

    zombie_zomboni(w.scene).update(pc24_boundary);

    CHECK(float_bits(pc24_boundary.dx) == 0x3E5E3540u);
}

void test_zomboni_ice_edge_truncates_after_offset() {
    world w(scene_type::day);
    auto& zomboni = make_zombie(w, zombie_type::zomboni, 0, -10);
    zomboni.x = -10.25f;

    zombie_zomboni(w.scene).update(zomboni);

    CHECK(w.scene.ice_path.x[0] == 107);
    CHECK(!w.plant_factory.can_plant(0, 0, plant_type::pea_shooter));
}

void test_gargantuar_throw_uses_native_float_stages() {
    world w(scene_type::day);
    auto& gargantuar = make_zombie(w, zombie_type::gargantuar, 0, 432);
    gargantuar.x = 432.33331298828125f;
    gargantuar.status = zombie_status::gargantuar_throw;
    gargantuar.has_item_or_walk_left = true;
    gargantuar.reanim.prev_progress = 0.73f;
    gargantuar.reanim.progress = 0.75f;

    zombie_gargantuar(w.scene).update(gargantuar);

    zombie* imp = nullptr;
    for (auto& candidate : w.scene.zombies) {
        if (&candidate != &gargantuar && candidate.type == zombie_type::imp) {
            imp = &candidate;
            break;
        }
    }
    CHECK(imp != nullptr);
    CHECK(float_bits(imp->d2y) == 0x3F1A4FA3u);

    for (int frame = 0; frame < 72; ++frame) {
        zombie_imp(w.scene).update(*imp);
    }
    CHECK(float_bits(imp->dy) == 0x36500000u);
    CHECK(imp->status == zombie_status::imp_flying);

    zombie_imp(w.scene).update(*imp);
    CHECK(imp->dy == 0.0f);
    CHECK(imp->status == zombie_status::imp_landing);
}

void test_public_factory_invalid_inputs_are_side_effect_free() {
    const auto rejects_invalid_argument = [](const auto& action) {
        try {
            action();
        } catch (const std::invalid_argument&) {
            return true;
        }
        return false;
    };
    const auto pool_is_pristine = [](const auto& pool) {
        return pool.active_count() == 0 &&
            pool.max_used_count() == 0 &&
            pool.next_id_key() == 1001;
    };

    world plant_world(scene_type::day);
    const auto plant_rng = plant_world.scene.rng;
    const bool plant_rejected = rejects_invalid_argument([&]() {
        plant_world.plant_factory.create(
            plant_type::pea_shooter,
            static_cast<int>(plant_world.scene.rows),
            0);
    });
    const bool cob_rejected = rejects_invalid_argument([&]() {
        plant_world.plant_factory.create(plant_type::cob_cannon, 0, 8);
    });

    world zombie_world(scene_type::day);
    const auto zombie_rng = zombie_world.scene.rng;
    const bool zombie_create_rejected = rejects_invalid_argument([&]() {
        zombie_world.zombie_factory.create(zombie_type::none, 0);
    });
    zombie* const invalid_zombie =
        zombie_world.zombie_factory.try_create(zombie_type::none, 0);

    world lurking_world(scene_type::pool);
    const auto lurking_rng = lurking_world.scene.rng;
    const bool invalid_lurking_type = lurking_world.zombie_factory.create_lurking(
        zombie_type::none, 2, 5);
    const bool invalid_lurking_position =
        lurking_world.zombie_factory.create_lurking(
            zombie_type::zombie, lurking_world.scene.rows, 9);

    world griditem_world(scene_type::day);
    const auto griditem_rng = griditem_world.scene.rng;
    const bool griditem_type_rejected = rejects_invalid_argument([&]() {
        griditem_world.griditem_factory.create(
            static_cast<griditem_type>(0), 0, 0);
    });
    const bool griditem_position_rejected = rejects_invalid_argument([&]() {
        griditem_world.griditem_factory.create(
            griditem_type::grave, griditem_world.scene.rows, 9);
    });

    world projectile_world(scene_type::day);
    const auto projectile_rng = projectile_world.scene.rng;
    const bool projectile_type_rejected = rejects_invalid_argument([&]() {
        projectile_factory(projectile_world.scene).create(
            static_cast<projectile_type>(0x7f), 0, 100, 80);
    });
    const bool projectile_position_rejected = rejects_invalid_argument([&]() {
        projectile_factory(projectile_world.scene).create(
            projectile_type::pea,
            projectile_world.scene.rows,
            100,
            80);
    });

    CHECK(plant_rejected);
    CHECK(cob_rejected);
    CHECK(zombie_create_rejected);
    CHECK(invalid_zombie == nullptr);
    CHECK(!invalid_lurking_type);
    CHECK(!invalid_lurking_position);
    CHECK(griditem_type_rejected);
    CHECK(griditem_position_rejected);
    CHECK(projectile_type_rejected);
    CHECK(projectile_position_rejected);

    CHECK(pool_is_pristine(plant_world.scene.plants));
    CHECK(plant_world.scene.plant_map[5][0].content == nullptr);
    CHECK(plant_world.scene.rng == plant_rng);
    CHECK(pool_is_pristine(zombie_world.scene.zombies));
    CHECK(zombie_world.scene.rng == zombie_rng);
    CHECK(pool_is_pristine(lurking_world.scene.zombies));
    CHECK(lurking_world.scene.rng == lurking_rng);
    CHECK(pool_is_pristine(griditem_world.scene.griditems));
    CHECK(griditem_world.scene.rng == griditem_rng);
    CHECK(pool_is_pristine(projectile_world.scene.projectiles));
    CHECK(projectile_world.scene.rng == projectile_rng);
}

void test_random_zombie_create_rejects_scene_incompatible_types() {
    struct test_case {
        scene_type scene;
        zombie_type zombie;
    };
    const test_case cases[] = {
        {scene_type::day, zombie_type::ducky_tube},
        {scene_type::day, zombie_type::snorkel},
        {scene_type::roof, zombie_type::dancing},
    };

    for (const auto& test : cases) {
        world nullable_world(test.scene);
        const auto nullable_rng = nullable_world.scene.rng;
        decltype(nullable_world.scene.spawn.row_random) nullable_rows;
        std::memcpy(nullable_rows,
            nullable_world.scene.spawn.row_random,
            sizeof(nullable_rows));

        CHECK(nullable_world.zombie_factory.try_create(test.zombie, 0) == nullptr);
        CHECK(nullable_world.scene.zombies.active_count() == 0);
        CHECK(nullable_world.scene.zombies.max_used_count() == 0);
        CHECK(nullable_world.scene.zombies.next_id_key() == 1001);
        CHECK(nullable_world.scene.rng == nullable_rng);
        CHECK(std::memcmp(nullable_world.scene.spawn.row_random,
            nullable_rows,
            sizeof(nullable_rows)) == 0);

        world reference_world(test.scene);
        const auto reference_rng = reference_world.scene.rng;
        decltype(reference_world.scene.spawn.row_random) reference_rows;
        std::memcpy(reference_rows,
            reference_world.scene.spawn.row_random,
            sizeof(reference_rows));

        expect_invalid_argument([&]() {
            reference_world.zombie_factory.create(test.zombie, 0);
        });
        CHECK(reference_world.scene.zombies.active_count() == 0);
        CHECK(reference_world.scene.zombies.max_used_count() == 0);
        CHECK(reference_world.scene.zombies.next_id_key() == 1001);
        CHECK(reference_world.scene.rng == reference_rng);
        CHECK(std::memcmp(reference_world.scene.spawn.row_random,
            reference_rows,
            sizeof(reference_rows)) == 0);
    }
}

void test_roof_lurking_no_spawn_row_is_side_effect_free() {
    world rejected(scene_type::roof);
    const auto rng = rejected.scene.rng;
    decltype(rejected.scene.spawn.row_random) rows;
    std::memcpy(rows,
        rejected.scene.spawn.row_random,
        sizeof(rows));

    CHECK(!rejected.zombie_factory.create_lurking(
        zombie_type::dancing, 2, 5));
    CHECK(rejected.scene.zombies.active_count() == 0);
    CHECK(rejected.scene.zombies.max_used_count() == 0);
    CHECK(rejected.scene.zombies.next_id_key() == 1001);
    CHECK(rejected.scene.rng == rng);
    CHECK(std::memcmp(rejected.scene.spawn.row_random,
        rows,
        sizeof(rows)) == 0);

    world supported(scene_type::roof);
    CHECK(supported.zombie_factory.create_lurking(
        zombie_type::digger, 2, 5));
    CHECK(supported.scene.zombies.active_count() == 2);
}

void test_reanim_timed_event_argument_uses_float() {
    object::reanim animation{};
    animation.fps = 1.0f;
    animation.n_frames = 1;
    animation.prev_progress = 0.3f;
    animation.progress = 0.31f;

    CHECK(animation.is_in_progress(0.30000001));
}

void test_dolphin_jump_clip_includes_float_endpoint() {
    zombie dolphin{};
    dolphin.type = zombie_type::dolphin_rider;
    dolphin.status = zombie_status::dolphin_jump;
    dolphin.reanim.progress = 0.76f;
    dolphin.hit_box.height = 115;

    CHECK(dolphin.get_height_bias() == -13.0f);

    rect hit_box;
    dolphin.get_hit_box(hit_box);
    CHECK(hit_box.height == 128);
}

void test_dolphin_jump_timed_events_are_else_if() {
    world w(scene_type::pool);
    auto& dolphin = make_zombie(w, zombie_type::dolphin_rider, 2, 300);
    dolphin.status = zombie_status::dolphin_jump;
    dolphin.dx = 0.5f;
    dolphin.reanim.prev_progress = 0.2f;
    dolphin.reanim.progress = 0.6f;

    zombie_dolphin_rider(w.scene).update(dolphin);

    CHECK(dolphin.dx == 0.5f);
}

void test_dolphin_jump_height_uses_native_curve_stages() {
    world w(scene_type::pool);
    auto& dolphin = make_zombie(w, zombie_type::dolphin_rider, 2, 300);
    dolphin.status = zombie_status::dolphin_jump;
    dolphin.countdown.action = 115;
    dolphin.reanim.prev_progress = 0.0f;
    dolphin.reanim.progress = 0.0f;
    dolphin.reanim.n_repeated = 0;

    zombie_dolphin_rider(w.scene).update(dolphin);

    CHECK(float_bits(dolphin.dy) == 0x3ED55556u);
}

void test_pole_jump_uses_float_threshold() {
    world w(scene_type::day);
    auto& tallnut = w.plant_factory.create(plant_type::tallnut, 0, 3);
    auto& pole = make_zombie(w, zombie_type::pole_vaulting, 0, tallnut.x + 20);
    pole.status = zombie_status::pole_valuting_jumping;
    pole.action = zombie_action::none;
    pole.reanim.progress = 0.6f;
    pole.reanim.n_repeated = 0;
    const float x = pole.x;

    zombie_pole_vaulting(w.scene).update(pole);

    CHECK(pole.status == zombie_status::pole_valuting_jumping);
    CHECK(pole.action == zombie_action::none);
    CHECK(pole.x == x);
}

void test_snorkel_jump_age_uses_extended_product() {
    world w(scene_type::pool);
    auto& snorkel = make_zombie(w, zombie_type::snorkel, 2, 300);
    snorkel.status = zombie_status::snorkel_jump_in_the_pool;
    snorkel.reanim.progress = 0.06f;
    snorkel.reanim.n_repeated = 0;

    zombie_snorkel(w.scene).update(snorkel);

    CHECK(float_bits(snorkel.dy) == 0x3F199999u);

    const std::uint32_t progress_bits = 0x3EA3D70Au;
    std::memcpy(&snorkel.reanim.progress, &progress_bits, sizeof(progress_bits));
    zombie_snorkel(w.scene).update(snorkel);

    CHECK(float_bits(snorkel.dy) == 0x404CCCCCu);
}

void test_jack_countdown_uses_extended_division() {
    world w(scene_type::day);
    w.scene.rng.seed(15193);
    zombie jack{};

    zombie_jack_in_the_box(w.scene).init(jack, 0);

    CHECK(float_bits(jack.dx) == 0x3F28FEDCu);
    CHECK(jack.countdown.action == 1416);
}

void test_add_sun_saturates_without_unsigned_wrap() {
    world w(scene_type::day);
    w.scene.sun.sun = 500;
    w.sun.add_sun(std::numeric_limits<unsigned int>::max());
    CHECK(w.scene.sun.sun == 9990);

    w.scene.sun.sun = 9990;
    w.sun.add_sun(std::numeric_limits<unsigned int>::max());
    CHECK(w.scene.sun.sun == 9990);

    w.scene.sun.sun = 500;
    w.sun.add_sun(25);
    w.sun.add_sun(0);
    CHECK(w.scene.sun.sun == 525);
}

void test_basketball_create_rejects_foreign_sources() {
    world receiver(scene_type::day);
    world foreign(scene_type::day);
    auto& local_zombie = make_zombie(receiver, zombie_type::catapult, 0, 600);
    auto& local_plant = receiver.plant_factory.create(plant_type::pea_shooter, 0, 2);
    auto& foreign_zombie = make_zombie(foreign, zombie_type::catapult, 0, 600);
    auto& foreign_plant = foreign.plant_factory.create(plant_type::pea_shooter, 0, 2);
    const auto receiver_rng = receiver.scene.rng;
    const int local_zombie_hp = local_zombie.hp;
    const int local_hp = local_plant.hp;

    projectile_factory factory(receiver.scene);
    expect_invalid_argument([&]() {
        factory.create(projectile_type::basketball, foreign_zombie, &local_plant);
    });
    expect_invalid_argument([&]() {
        factory.create(projectile_type::basketball, local_zombie, &foreign_plant);
    });

    CHECK(receiver.scene.projectiles.active_count() == 0);
    CHECK(receiver.scene.projectiles.max_used_count() == 0);
    CHECK(receiver.scene.projectiles.next_id_key() == 1001);
    CHECK(local_zombie.hp == local_zombie_hp);
    CHECK(local_plant.hp == local_hp);
    CHECK(receiver.scene.rng == receiver_rng);
}

void test_reanim_frame_position_rounds_once() {
    object::reanim animation{};
    const std::uint32_t progress_bits = 0x3EAAAAA9u;
    std::memcpy(&animation.progress, &progress_bits, sizeof(progress_bits));
    animation.begin_frame = 2;
    animation.n_frames = 4;

    reanim_frame_status status;
    animation.get_frame_status(status);

    CHECK(status.frame == 2);
    CHECK(status.next_frame == 3);

    for (int begin : {0, 2, 44, 32767, 16777216}) {
        for (int count : {1, 2, 4, 47, 100, 4097}) {
            animation.begin_frame = begin;
            animation.n_frames = count;
            for (int step = 0; step <= 4096; ++step) {
                animation.progress = static_cast<float>(step) / 4096;
                animation.get_frame_status(status);
                const float current = static_cast<float>(
                    static_cast<double>(animation.progress) * (count - 1) + begin);
                const double floored = std::floor(current);
                const int end = begin + count - 1;
                const auto before = static_cast<unsigned int>(floored < end ? floored : end);
                const auto after = static_cast<unsigned int>(floored < end ? floored + 1 : end);
                CHECK(status.frame == before);
                CHECK(status.next_frame == after);
                CHECK(float_bits(status.frame_progress) == float_bits(static_cast<float>(current - floored)));
            }
        }
    }
}

void test_ground_velocity_uses_native_pc24_stages() {
    world w(scene_type::day);
    auto& zombie = make_zombie(w, zombie_type::zombie, 0, 300);
    zombie.reanim.begin_frame = 44;
    zombie.reanim.n_frames = 47;
    zombie.reanim.progress = 0.0f;
    zombie.reanim.fps = 12.0f;

    CHECK(float_bits(zombie.get_dx_from_ground()) == 0x3E2C0835u);

    zombie.dx = 0.28f;
    system::reanim(w.scene).update_fps(zombie);
    CHECK(float_bits(zombie.reanim.fps) == 0x4146B8A7u);

    auto& ladder = make_zombie(w, zombie_type::ladder, 0, 300);
    ladder.reanim.begin_frame = 25;
    ladder.reanim.n_frames = 47;
    const std::uint32_t progress_bits = 0x3E0A7B02u;
    const std::uint32_t fps_bits = 0x41D9318Au;
    std::memcpy(&ladder.reanim.progress, &progress_bits, sizeof(progress_bits));
    std::memcpy(&ladder.reanim.fps, &fps_bits, sizeof(fps_bits));

    CHECK(float_bits(ladder.get_dx_from_ground()) == 0x3F429AFFu);
}

void test_balloon_and_pogo_initial_y_matches_native_type_order() {
    world balloon_world(scene_type::day);
    auto* balloon_ptr = balloon_world.zombie_factory.add_zombie_in_row(zombie_type::balloon, 0, 0);
    CHECK(balloon_ptr != nullptr);
    auto& balloon = *balloon_ptr;
    CHECK(balloon.y == 50.0f);
    CHECK(balloon.int_y == 50);
    for (int frame = 1; frame <= 30; ++frame) {
        balloon_world.zombie.update();
        CHECK(balloon.y == 50.0f - frame);
        CHECK(balloon.int_y == 50 - frame);
    }

    world pogo_world(scene_type::day);
    auto* pogo_ptr = pogo_world.zombie_factory.add_zombie_in_row(zombie_type::pogo, 0, 0);
    CHECK(pogo_ptr != nullptr);
    auto& pogo = *pogo_ptr;
    CHECK(pogo.y == 50.0f);
    CHECK(pogo.int_y == 50);
    for (int frame = 1; frame <= 16; ++frame) {
        pogo_world.zombie.update();
        CHECK(pogo.y == 50.0f - frame);
        CHECK(pogo.int_y == 50 - frame);
    }
}

#undef CHECK

}

int main() {
    test_rising_zombie_reanimation_keeps_advancing();
    test_coffee_bean_bypasses_ice_for_existing_mushroom();
    test_cattail_rejects_lily_pad_being_grabbed();
    test_dolphin_ride_uses_integer_left_bank_boundary();
    test_board_edge_uses_integer_x_and_inclusive_no_head_limit();
    test_starfruit_bottom_row_clamps_before_edge_collision();
    test_cattail_homing_uses_integer_centers();
    test_cattail_homing_matches_native_rounding();
    test_homing_needles_survive_immunity_but_not_target_removal();
    test_pult_vertical_velocity_rounds_before_subtraction();
    test_peashooter_head_shooting_hold_does_not_reset_body_reanim();
    test_threepeater_shooting_state_returns_to_zero();
    test_native_float_zombie_prediction();
    test_start_eating_uses_zombie_type_for_snorkel();
    test_flag_head_drop_item_state();
    test_grave_buster_bypasses_grid_gates();
    test_coffee_bean_crumble_dispatches_update();
    test_scene_copy_assignment_rebuilds_plant_map();
    test_basketball_velocity_uses_float_origin_and_integer_target();
    test_starfruit_target_uses_integer_predicted_left_edge();
    test_zombie_clip_height_is_subtracted_before_truncation();
    test_factory_destroy_rejects_foreign_objects();
    test_cattail_integer_distance_tie_keeps_first_target();
    test_reanim_progress_matches_native_pc24_stages();
    test_zomboni_speed_curve_uses_integer_x();
    test_zomboni_ice_edge_truncates_after_offset();
    test_gargantuar_throw_uses_native_float_stages();
    test_public_factory_invalid_inputs_are_side_effect_free();
    test_random_zombie_create_rejects_scene_incompatible_types();
    test_roof_lurking_no_spawn_row_is_side_effect_free();
    test_reanim_timed_event_argument_uses_float();
    test_dolphin_jump_clip_includes_float_endpoint();
    test_dolphin_jump_timed_events_are_else_if();
    test_dolphin_jump_height_uses_native_curve_stages();
    test_pole_jump_uses_float_threshold();
    test_snorkel_jump_age_uses_extended_product();
    test_jack_countdown_uses_extended_division();
    test_add_sun_saturates_without_unsigned_wrap();
    test_basketball_create_rejects_foreign_sources();
    test_magnetshroom_float_distance_tie_keeps_first_target();
    test_reanim_frame_position_rounds_once();
    test_ground_velocity_uses_native_pc24_stages();
    test_balloon_and_pogo_initial_y_matches_native_type_order();
    test_upgrade_plant_placement_regressions();
    test_natural_sun_zero_countdown_waits_one_frame();
    test_cob_cannon_rejects_double_square_pumpkins();
    test_magnetshroom_ladder_range_and_row_weight();
    test_straight_projectile_edge_overlap_hits();
    test_basketball_hits_low_plants();
    test_basketball_skips_not_on_ground_top_layer();
    test_fire_pea_fire_resistant_target_no_splash();
    test_iceshroom_water_freeze_fixed_300();
    test_opening_jackbox_immune_to_freeze_and_butter();
    test_jalapeno_shortens_ice_path();
    test_bungee_grab_uses_content_before_pumpkin_and_base();
    test_bungee_top_query_skips_not_on_ground_plants();
    test_bungee_skips_unstealable_target_cells();
    test_catapult_ignores_too_close_and_right_side_plants();
    test_bungee_target_order_is_column_major();
    test_catapult_target_skips_not_on_ground_top_layer();
    test_roof_gargantuar_imp_throw_uses_negative_min_d2y();
    test_backup_dancer_spawning_dy_uses_float_ratio();
    test_split_pea_second_shot_uses_stored_attack_flags();
    test_potato_mine_trigger_rect_special_cases_pole_only();
    test_hypno_zombie_attacks_no_head_enemy();
    test_hypno_target_refreshes_between_updates();
    test_hypno_conversion_is_visible_later_in_same_update();
    test_grid_plant_target_matches_pool_scan();
    test_pool_spawn_never_selects_ducky_tube();
    test_hugewave_fade_zero_frame_spawns_wave();
    test_spawn_giga_candidates_and_weight();
    test_spawn_current_type_counts_reset_each_wave();
    test_spawn_giga_cap_recomputed_within_wave();
    test_splash_damage_caps();
    test_chill_on_helm_only();
    test_sleeping_plants_do_not_charge();
    test_sunshroom_single_update_and_seashroom_type();
    test_normal_shooter_flags_are_ground_only();
    test_potato_mine_uses_native_explosion_center();
    test_hypno_snorkel_walking_does_not_enter_pool();
    test_squash_nearest_and_locked_target();
    test_jack_radius_arguments();
    test_dancer_eating_early_return();
    test_reanim_wrap_uses_previous_progress();
    test_catapult_stops_after_last_basketball();
    test_vehicle_crushes_all_plants_in_square();
    test_thrown_imp_inherits_slow();
    test_fireball_splash_width_and_torchwood_lookahead();
    test_projectile_target_pruning_preserves_collision_rules();
    test_dolphin_leaving_pool_switches_without_dolphin_phase();
    test_kernelpult_butter_state_clears_after_launch();
    test_imitater_waits_on_status_countdown();
    test_gloom_upgrade_preserves_pending_wake_countdown();
    test_magnetshroom_skips_non_running_jackbox();
    test_starfruit_uses_predicted_center_for_angle();
    test_roof_slope_fireball_impact_splashes_without_target();
    test_card_cooldown_boundaries();
    test_death_animation_retention_and_ash_removal();
    test_dancer_remains_keep_group_slowdown_and_summon_delay();
    test_no_head_core_semantics();
    test_snorkel_swim_head_drop_dies_immediately();
    test_destroyed_shield_preserves_max_hp();
    test_night_grave_spawn_fills_to_max();
    test_night_grave_spawn_stops_at_max();
    test_w20_clear_field_starts_endgame_countdown();
    test_w20_enemy_blocks_clear_field_endgame_countdown();
    test_w20_clear_field_ignores_non_enemy_zombies();
    test_w20_timeout_starts_shared_endgame_countdown();
    test_scene_copy_check_build_and_json_regressions();
    test_sun_and_plant_initialization_regressions();
    test_newspaper_and_ladder_chew_phase();
    test_deferred_copy_and_public_input_safety();
    test_update_all_rethrows_worker_exception();
    test_observation_layout_and_capacity_safety();
    test_off_ground_plant_map_and_animation_transitions();
    test_head_shooters_do_not_reset_body_reanimation_after_launch();
    test_special_zombie_damage_and_reanimation_transitions();
    test_endgame_gates_spawn_and_natural_sun();
    test_real_object_pool_reuse_and_stale_ids();
    test_bungee_lift_and_pool_dancer_boundaries();
    test_original_rng_contract_and_pool_capacity();
    test_wave_spawn_private_rng_scope();
    test_pool_lurking_uses_native_column_major_grid_order();
    return 0;
}
