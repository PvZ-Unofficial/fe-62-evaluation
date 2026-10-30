#pragma once
#include <array>
#include <cstddef>
#include <cstdint>
#include <cassert>

#include "rapidjson/writer.h"
#include "rapidjson/stringbuffer.h"

#include "object/obj_list.h"
#include "object/griditem.h"
#include "object/zombie.h"
#include "object/plant.h"
#include "object/projectile.h"
#include "system/event.h"
#include "system/random_stream.h"

namespace pvz_emulator::object {

struct grid_plant_status {
    object::plant* pumpkin;
    object::plant* base;
    object::plant* content;
    object::plant* coffee_bean;

    grid_plant_status():
        pumpkin(nullptr),
        base(nullptr),
        content(nullptr),
        coffee_bean(nullptr) {}
};

enum class scene_type {
    day = 0x0,
    night = 0x1,
    pool = 0x2,
    fog = 0x3,
    roof = 0x4,
    moon_night = 0x5,
    mushroom_garden = 0x6,
};

enum class maid_cheat_state : std::uint8_t {
    stop,
    call_partner,
    dancing,
    move,
};

enum class kernel_pult_projectile_rule : std::uint8_t {
    normal,
    always_butter,
    always_kernel,
};

enum class plant_damage_rule : std::uint8_t {
    normal,
    invincible,
    weak,
};

class scene {
public:
    scene_type type;

    system::random_stream rng;
    system::random_stream level_rng;
    system::random_stream* battle_rng_override;
    bool battle_rng_override_active;

    unsigned int zombie_dancing_clock;
    std::uint32_t main_counter;

    unsigned int rows;

    obj_list<object::zombie, 1024> zombies;
    obj_list<object::plant, 1024> plants;
    obj_list<object::griditem, 128> griditems;
    obj_list<object::projectile, 1024> projectiles;

    system::event_sink events;

    std::array<std::array<grid_plant_status, 9>, 6> plant_map;
    // Low-level overlapping allocations can leave live plants outside this index.
    // Such rows retain pool-based targeting until the map is rebuilt or reset.
    std::array<bool, 6> plant_map_incomplete{};

    struct spawn_data {
        std::array<std::array<object::zombie_type, 50>, 20> spawn_list;

        unsigned int total_flags;
        unsigned int wave;

        struct {
            unsigned int initial;
            int threshold;
        } hp;

        struct {
            unsigned int next_wave;
            unsigned int next_wave_initial;
            unsigned int lurking_squad;
            unsigned int hugewave_fade;
            unsigned int endgame;
            unsigned int pool;
        } countdown;

        struct row_random_tuple {
            float b;
            float c;
            float d;
        } row_random[6];

        std::array<bool, 33> spawn_flags;
        bool is_hugewave_shown;

        spawn_data() {
            memset(this, 0, sizeof(*this));

            total_flags = 1000;
            countdown.next_wave = 600;
            countdown.next_wave_initial = 600;
        }
    } spawn;

    struct sun_data {
        unsigned int sun;
        unsigned int natural_sun_generated;
        unsigned int natural_sun_countdown;
        unsigned int limit;

        sun_data() :
            sun(9990),
            natural_sun_generated(0),
            natural_sun_countdown(0),
            limit(9990) {}
    } sun;

    struct ice_path_data {
        std::array<unsigned int, 6> countdown;
        std::array<int, 6> x;

        ice_path_data() {
            memset(&countdown, 0, sizeof(countdown));
            for (auto& p : x) {
                p = 800;
            }
        }
    } ice_path;

    struct card_data {
        plant_type type;
        plant_type imitater_type;
        unsigned int cold_down;

        card_data() :
            type(plant_type::none),
            imitater_type(plant_type::none),
            cold_down(0) {}
    };
    
    std::array<card_data, 10> cards;

    bool is_game_over;

    bool is_zombie_dance;
    zombie_dance_cheat common_zombie_dance_cheat;
    maid_cheat_state maid_cheat;
    bool is_future_enabled;

    bool wave_spawn_random_enabled;
    std::uint32_t wave_spawn_random_seed;
    bool stop_spawn;
    bool seed_recharge_ignored;
    bool ignore_sun_cost;
    bool instant_special_effects;
    bool easy_planting_cheat;
    bool ignore_planting_restrictions;
    bool mushrooms_awake;
    bool disable_cob_delay;
    bool cob_fixed_delay;
    bool cob_recharge_shortened;
    bool cob_drift_fixed;
    bool natural_sun_drop_disabled;
    bool jack_explosions_disabled;
    bool special_events_disabled;
    bool zombies_die_at_house;
    bool enable_split_pea_bug;
    kernel_pult_projectile_rule kernel_pult_projectiles;
    plant_damage_rule plant_damage;

    scene(scene_type t) : scene(t, std::random_device{}(), std::random_device{}(), 0) {
        zombie_dancing_clock = rng.next() % 10000;
    }

    scene(scene_type t, std::uint32_t battle_seed, std::uint32_t level_seed,
        std::uint32_t dancer_clock) : type(t),
        rng(battle_seed),
        level_rng(level_seed),
        battle_rng_override(nullptr),
        battle_rng_override_active(false),
        zombie_dancing_clock(dancer_clock),
        main_counter(0),
        rows(get_max_row()),
        is_game_over(false),
        is_zombie_dance(false),
        common_zombie_dance_cheat(zombie_dance_cheat::none),
        maid_cheat(maid_cheat_state::stop),
        is_future_enabled(false),
        wave_spawn_random_enabled(false),
        wave_spawn_random_seed(0),
        stop_spawn(false),
        seed_recharge_ignored(false),
        ignore_sun_cost(false),
        instant_special_effects(false),
        easy_planting_cheat(false),
        ignore_planting_restrictions(false),
        mushrooms_awake(false),
        disable_cob_delay(false),
        cob_fixed_delay(false),
        cob_recharge_shortened(false),
        cob_drift_fixed(false),
        natural_sun_drop_disabled(false),
        jack_explosions_disabled(false),
        special_events_disabled(false),
        zombies_die_at_house(false),
        enable_split_pea_bug(true),
        kernel_pult_projectiles(kernel_pult_projectile_rule::normal),
        plant_damage(plant_damage_rule::normal) {}

    scene(const scene& s);
    scene& operator=(const scene& s);

    system::random_stream& active_battle_rng() {
        return battle_rng_override == nullptr || !battle_rng_override_active ? rng : *battle_rng_override;
    }

    void unregister_plant_from_map(plant& p);

    // Board::SetDanceMode: one immediate restart, not a persistent per-update cheat.
    void set_dance_mode(bool enabled);

    bool is_water_grid(int row, int col) {
        return is_pool_square(col, row);
    }

    bool is_pool_square(int grid_x, int grid_y) const {
        return (type == scene_type::fog || type == scene_type::pool) &&
            (grid_y == 2 || grid_y == 3) && grid_x >= 0 && grid_x <= 8;
    }

    bool row_can_have_zombies(int row) const {
        return row >= 0 && static_cast<unsigned int>(row) < rows;
    }

    int grid_to_pixel_x(int grid_x, int grid_y) const;
    int grid_to_pixel_y(int grid_x, int grid_y) const;
    float get_pos_y_based_on_row(float pos_x, int row) const;

    unsigned int get_max_row() {
        return type == scene_type::pool || type == scene_type::fog ?  6 : 5;
    }

    void set_type(scene_type type) {
        this->type = type;
        rows = get_max_row();
    }

    void to_json(rapidjson::Writer<rapidjson::StringBuffer>& writer);

    void reset();
    void reset(scene_type type, std::uint32_t battle_seed,
        std::uint32_t level_seed, std::uint32_t dancer_clock);

    void reset(scene_type type) {
        set_type(type);
        reset();
    }

private:
    void reset_state();
};

}

