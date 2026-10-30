#include "scene.h"
#include "system/util.h"
#include "system/reanim.h"

namespace pvz_emulator::object {

namespace {

void rebuild_plant_map(scene& value) {
    memset(&value.plant_map, 0, sizeof(value.plant_map));
    value.plant_map_incomplete.fill(false);

    for (auto& p : value.plants) {
        if (p.edible == plant_edible_status::invisible_and_not_edible ||
            p.is_squash_attacking() ||
            p.is_smashed)
        {
            continue;
        }

        const auto map_type = p.type == plant_type::imitater ?
            p.imitater_target : p.type;
        auto assign = [&](plant*& slot) {
            if (slot && slot != &p) {
                value.plant_map_incomplete[p.row] = true;
            }
            slot = &p;
        };
        if (map_type == plant_type::flower_pot ||
            map_type == plant_type::lily_pad)
        {
            assign(value.plant_map[p.row][p.col].base);
        } else if (map_type == plant_type::coffee_bean) {
            assign(value.plant_map[p.row][p.col].coffee_bean);
        } else if (map_type == plant_type::pumpkin) {
            assign(value.plant_map[p.row][p.col].pumpkin);
        } else if (map_type == plant_type::cob_cannon) {
            assign(value.plant_map[p.row][p.col].content);
            if (p.col + 1 < value.plant_map[p.row].size()) {
                assign(value.plant_map[p.row][p.col + 1].content);
            }
        } else {
            assign(value.plant_map[p.row][p.col].content);
        }
    }
}

}

void scene::set_dance_mode(bool enabled) {
    is_zombie_dance = enabled;
    system::reanim animation(*this);
    for (auto& z : zombies) {
        if ((z.type == zombie_type::zombie || z.type == zombie_type::conehead ||
             z.type == zombie_type::buckethead) && !z.is_dead && !z.has_death_status() &&
            !system::is_not_movable(*this, z)) {
            animation.update_status(z);
        }
    }
}

void scene::unregister_plant_from_map(plant& p) {
    if (p.row >= plant_map.size() || p.col >= plant_map[p.row].size()) {
        return;
    }

    auto& status = plant_map[p.row][p.col];
    if (!system::plant_not_on_ground(p)) {
        plant_map_incomplete[p.row] = true;
    }

    if (status.pumpkin == &p) {
        status.pumpkin = nullptr;
    }

    if (status.coffee_bean == &p) {
        status.coffee_bean = nullptr;
    }

    if (status.base == &p) {
        status.base = nullptr;
    }

    if (status.content == &p) {
        status.content = nullptr;
    }

    if (p.type == plant_type::cob_cannon &&
        p.col + 1 < plant_map[p.row].size() &&
        plant_map[p.row][p.col + 1].content == &p)
    {
        plant_map[p.row][p.col + 1].content = nullptr;
    }
}

scene::scene(const scene& s) :
    type(s.type),
    rng(s.rng),
    level_rng(s.level_rng),
    battle_rng_override(nullptr),
    battle_rng_override_active(false),
    zombie_dancing_clock(s.zombie_dancing_clock),
    main_counter(s.main_counter),
    rows(s.rows),
    zombies(s.zombies),
    plants(s.plants),
    griditems(s.griditems),
    projectiles(s.projectiles),
    events(),
    spawn(s.spawn),
    sun(s.sun),
    ice_path(s.ice_path),
    cards(s.cards),
    is_game_over(s.is_game_over),
    is_zombie_dance(s.is_zombie_dance),
    common_zombie_dance_cheat(s.common_zombie_dance_cheat),
    maid_cheat(s.maid_cheat),
    is_future_enabled(s.is_future_enabled),
    wave_spawn_random_enabled(s.wave_spawn_random_enabled),
    wave_spawn_random_seed(s.wave_spawn_random_seed),
    stop_spawn(s.stop_spawn),
    seed_recharge_ignored(s.seed_recharge_ignored),
    ignore_sun_cost(s.ignore_sun_cost),
    instant_special_effects(s.instant_special_effects),
    easy_planting_cheat(s.easy_planting_cheat),
    ignore_planting_restrictions(s.ignore_planting_restrictions),
    mushrooms_awake(s.mushrooms_awake),
    disable_cob_delay(s.disable_cob_delay),
    cob_fixed_delay(s.cob_fixed_delay),
    cob_recharge_shortened(s.cob_recharge_shortened),
    cob_drift_fixed(s.cob_drift_fixed),
    natural_sun_drop_disabled(s.natural_sun_drop_disabled),
    jack_explosions_disabled(s.jack_explosions_disabled),
    special_events_disabled(s.special_events_disabled),
    zombies_die_at_house(s.zombies_die_at_house),
    enable_split_pea_bug(s.enable_split_pea_bug),
    kernel_pult_projectiles(s.kernel_pult_projectiles),
    plant_damage(s.plant_damage)
{
    rebuild_plant_map(*this);
}

scene& scene::operator=(const scene& s) {
    if (this == &s) {
        return *this;
    }

    type = s.type;
    rng = s.rng;
    level_rng = s.level_rng;
    battle_rng_override = nullptr;
    battle_rng_override_active = false;
    zombie_dancing_clock = s.zombie_dancing_clock;
    main_counter = s.main_counter;
    rows = s.rows;
    zombies = s.zombies;
    plants = s.plants;
    griditems = s.griditems;
    projectiles = s.projectiles;
    events.clear();
    spawn = s.spawn;
    sun = s.sun;
    ice_path = s.ice_path;
    cards = s.cards;
    is_game_over = s.is_game_over;
    is_zombie_dance = s.is_zombie_dance;
    common_zombie_dance_cheat = s.common_zombie_dance_cheat;
    maid_cheat = s.maid_cheat;
    is_future_enabled = s.is_future_enabled;
    wave_spawn_random_enabled = s.wave_spawn_random_enabled;
    wave_spawn_random_seed = s.wave_spawn_random_seed;
    stop_spawn = s.stop_spawn;
    seed_recharge_ignored = s.seed_recharge_ignored;
    ignore_sun_cost = s.ignore_sun_cost;
    instant_special_effects = s.instant_special_effects;
    easy_planting_cheat = s.easy_planting_cheat;
    ignore_planting_restrictions = s.ignore_planting_restrictions;
    mushrooms_awake = s.mushrooms_awake;
    disable_cob_delay = s.disable_cob_delay;
    cob_fixed_delay = s.cob_fixed_delay;
    cob_recharge_shortened = s.cob_recharge_shortened;
    cob_drift_fixed = s.cob_drift_fixed;
    natural_sun_drop_disabled = s.natural_sun_drop_disabled;
    jack_explosions_disabled = s.jack_explosions_disabled;
    special_events_disabled = s.special_events_disabled;
    zombies_die_at_house = s.zombies_die_at_house;
    enable_split_pea_bug = s.enable_split_pea_bug;
    kernel_pult_projectiles = s.kernel_pult_projectiles;
    plant_damage = s.plant_damage;

    rebuild_plant_map(*this);
    return *this;
}

void scene::to_json(rapidjson::Writer<rapidjson::StringBuffer>& writer) {
    writer.StartObject();

    writer.Key("type");

    switch (type) {
    case scene_type::day:
        writer.String("day");
        break;

    case scene_type::night:
        writer.String("night");
        break;

    case scene_type::pool:
        writer.String("pool");
        break;

    case scene_type::fog:
        writer.String("fog");
        break;

    case scene_type::roof:
        writer.String("roof");
        break;

    case scene_type::moon_night:
        writer.String("moon_night");
        break;

    case scene_type::mushroom_garden:
        writer.String("mushroom_garden");
        break;

    default:
        break;
    }

    writer.Key("rows");
    writer.Uint(rows);

    writer.Key("zombies");
    writer.StartArray();
    for (auto& z : zombies) {
        z.to_json(*this, writer);
    }
    writer.EndArray();

    writer.Key("plants");
    writer.StartArray();
    for (auto& p : plants) {
        p.to_json(*this, writer);
    }
    writer.EndArray();

    writer.Key("griditems");
    writer.StartArray();
    for (auto& item : griditems) {
        item.to_json(*this, writer);
    }
    writer.EndArray();

    writer.Key("projectiles");
    writer.StartArray();
    for (auto& proj : projectiles) {
        proj.to_json(*this, writer);
    }
    writer.EndArray();

    writer.Key("spawn");
    writer.StartObject();

    writer.Key("spawn_list");
    writer.StartArray();
    for (auto& wave : spawn.spawn_list) {
        writer.StartArray();
        for (auto& type : wave) {
            writer.String(zombie::type_to_string(type));
        }
        writer.EndArray();
    }
    writer.EndArray();

    writer.Key("total_flags");
    writer.Uint(spawn.total_flags);

    writer.Key("wave");
    writer.Uint(spawn.wave);

    writer.Key("hp");
    writer.StartObject();

    writer.Key("initial");
    writer.Uint(spawn.hp.initial);

    writer.Key("threshold");
    writer.Int(spawn.hp.threshold);
    writer.EndObject();

    writer.Key("countdown");
    writer.StartObject();

    writer.Key("next_wave");
    writer.Uint(spawn.countdown.next_wave);

    writer.Key("next_wave_initial");
    writer.Uint(spawn.countdown.next_wave_initial);

    writer.Key("lurking_squad");
    writer.Uint(spawn.countdown.lurking_squad);

    writer.Key("hugewave_fade");
    writer.Uint(spawn.countdown.hugewave_fade);

    writer.Key("endgame");
    writer.Uint(spawn.countdown.endgame);

    writer.Key("pool");
    writer.Uint(spawn.countdown.pool);
    writer.EndObject();

    static const zombie_type ALL_ZOMBIES[] = {
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
        zombie_type::ducky_tube,
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

    writer.Key("spawn_flags");
    writer.StartObject();
    for (auto& type : ALL_ZOMBIES) {
        writer.Key(zombie::type_to_string(type));
        writer.Bool(spawn.spawn_flags[static_cast<int>(type)]);
    }
    writer.EndObject();

    writer.Key("is_hugewave_shown");
    writer.Bool(spawn.is_hugewave_shown);
    writer.EndObject();

    writer.Key("sun");
    writer.StartObject();

    writer.Key("sun");
    writer.Uint(sun.sun);

    writer.Key("natural_sun_generated");
    writer.Uint(sun.natural_sun_generated);

    writer.Key("natural_sun_countdown");
    writer.Uint(sun.natural_sun_countdown);
    writer.EndObject();

    writer.Key("ice_path");
    writer.StartObject();

    writer.Key("countdown");
    writer.StartArray();
    for (auto& t : ice_path.countdown) {
        writer.Uint(t);
    }
    writer.EndArray();

    writer.Key("x");
    writer.StartArray();
    for (auto& x : ice_path.x) {
        writer.Uint(x);
    }
    writer.EndArray();
    writer.EndObject();

    writer.Key("cards");
    writer.StartArray();
    for (auto& card : cards) {
        writer.StartObject();
        writer.Key("type");
        writer.String(plant::type_to_string(card.type));
        
        writer.Key("imitater_type");
        writer.String(plant::type_to_string(card.imitater_type));

        writer.Key("cold_down");
        writer.Uint(card.cold_down);
        writer.EndObject();
    }
    writer.EndArray();

    writer.Key("is_zombie_dance");
    writer.Bool(is_zombie_dance);

    writer.Key("maid_cheat");
    switch (maid_cheat) {
    case maid_cheat_state::stop:
        writer.String("stop");
        break;

    case maid_cheat_state::call_partner:
        writer.String("call_partner");
        break;

    case maid_cheat_state::dancing:
        writer.String("dancing");
        break;

    case maid_cheat_state::move:
        writer.String("move");
        break;
    }

    writer.Key("is_future_enabled");
    writer.Bool(is_future_enabled);

    writer.Key("is_game_over");
    writer.Bool(is_game_over);

    writer.Key("stop_spawn");
    writer.Bool(stop_spawn);

    writer.Key("ignore_sun_cost");
    writer.Bool(ignore_sun_cost);

    writer.Key("easy_planting_cheat");
    writer.Bool(easy_planting_cheat);

    writer.Key("ignore_planting_restrictions");
    writer.Bool(ignore_planting_restrictions);

    writer.Key("mushrooms_awake");
    writer.Bool(mushrooms_awake);

    writer.Key("natural_sun_drop_disabled");
    writer.Bool(natural_sun_drop_disabled);

    writer.Key("zombies_die_at_house");
    writer.Bool(zombies_die_at_house);

    writer.EndObject();
}

int scene::grid_to_pixel_x(int grid_x, int grid_y) const {
    assert(grid_x >= 0 && grid_x < 9 && row_can_have_zombies(grid_y));
    return system::get_x_by_col(grid_x);
}

int scene::grid_to_pixel_y(int grid_x, int grid_y) const {
    assert(grid_x >= 0 && grid_x < 9 && row_can_have_zombies(grid_y));
    return system::get_y_by_row_and_col(type, grid_y, grid_x);
}

float scene::get_pos_y_based_on_row(float pos_x, int row) const {
    assert(row_can_have_zombies(row));
    return system::get_y_by_row_and_x(type, static_cast<unsigned int>(row), pos_x);
}

void scene::reset() {
    rng.seed(std::random_device{}());
    level_rng.seed(std::random_device{}());

    zombie_dancing_clock = rng.next() % 10000;
    reset_state();
}

void scene::reset(scene_type type, std::uint32_t battle_seed,
    std::uint32_t level_seed, std::uint32_t dancer_clock)
{
    set_type(type);
    rng.seed(battle_seed);
    level_rng.seed(level_seed);
    zombie_dancing_clock = dancer_clock;
    reset_state();
}

void scene::reset_state() {
    main_counter = 0;
    battle_rng_override = nullptr;
    battle_rng_override_active = false;
    is_zombie_dance = false;
    common_zombie_dance_cheat = zombie_dance_cheat::none;
    maid_cheat = maid_cheat_state::stop;
    is_future_enabled = false;
    is_game_over = false;
    wave_spawn_random_enabled = false;
    wave_spawn_random_seed = 0;
    stop_spawn = false;
    seed_recharge_ignored = false;
    ignore_sun_cost = false;
    instant_special_effects = false;
    easy_planting_cheat = false;
    ignore_planting_restrictions = false;
    mushrooms_awake = false;
    disable_cob_delay = false;
    cob_fixed_delay = false;
    cob_recharge_shortened = false;
    cob_drift_fixed = false;
    natural_sun_drop_disabled = false;
    jack_explosions_disabled = false;
    special_events_disabled = false;
    zombies_die_at_house = false;
    enable_split_pea_bug = true;
    kernel_pult_projectiles = kernel_pult_projectile_rule::normal;
    plant_damage = plant_damage_rule::normal;

    zombies.clear();
    plants.clear();
    griditems.clear();
    projectiles.clear();

    memset(&spawn, 0, sizeof(spawn));
    spawn.total_flags = 1000;
    spawn.countdown.next_wave = 600;
    spawn.countdown.next_wave_initial = 600;

    memset(&plant_map, 0, sizeof(plant_map));
    plant_map_incomplete.fill(false);

    sun.sun = 9990;
    sun.limit = 9990;
    sun.natural_sun_generated = 0;
    sun.natural_sun_countdown = 0;

    memset(&ice_path.countdown, 0, sizeof(ice_path.countdown));
    for (auto& p : ice_path.x) {
        p = 800;
    }

    for (auto& card : cards) {
        card.type = plant_type::none;
        card.imitater_type = plant_type::none;
        card.cold_down = 0;
    }

}

}
