#include <cassert>
#include <stdexcept>

#include "plant_factory.h"
#include "system/sun.h"

namespace pvz_emulator::system {

using namespace pvz_emulator::object;

bool plant_factory::can_plant_advanced_plant(
    grid_plant_status& status,
    plant_type advanced) const
{
    if (advanced != plant_type::cattail &&
        (status.content == nullptr ||
        status.content->edible == plant_edible_status::invisible_and_edible))
    {
        return false;
    }

    switch (advanced) {
    case plant_type::gatling_pea:
        return status.content->type == plant_type::repeater;

    case plant_type::twin_sunflower:
        return status.content->type == plant_type::sunflower;

    case plant_type::gloomshroom:
        return status.content->type == plant_type::fumeshroom;

    case plant_type::winter_melon:
        return status.content->type == plant_type::melonpult;

    case plant_type::gold_magnet:
        return status.content->type == plant_type::magnetshroom;

    case plant_type::spikerock:
        return status.content->type == plant_type::spikeweed;

    case plant_type::cattail: {
        return status.base != nullptr &&
            status.base->type == plant_type::lily_pad &&
            status.base->edible != plant_edible_status::invisible_and_edible &&
            status.content == nullptr;
    }

    default:
        return false;
    }
}

bool plant_factory::can_plant_cob_cannon_at(
    unsigned int row,
    unsigned int col) const
{
    if (row >= scene.rows || col >= 8) {
        return false;
    }

    const auto side_is_valid = [&](unsigned int side_col) {
        const auto& status = scene.plant_map[row][side_col];
        if (status.pumpkin != nullptr) {
            return false;
        }
        if (status.content != nullptr) {
            return status.content->type == plant_type::kernelpult &&
                status.content->edible !=
                    plant_edible_status::invisible_and_edible;
        }
        return scene.easy_planting_cheat &&
            can_plant_at(
                static_cast<int>(side_col),
                static_cast<int>(row),
                plant_type::kernelpult) == planting_reason::ok;
    };

    const auto& left = scene.plant_map[row][col];
    const auto& right = scene.plant_map[row][col + 1];
    return side_is_valid(col) && side_is_valid(col + 1) &&
        (left.base == nullptr) == (right.base == nullptr);
}

bool plant_factory::planting_requirements_met(plant_type type) const {
    plant_type required = plant_type::none;
    switch (type) {
    case plant_type::gatling_pea:
        required = plant_type::repeater;
        break;
    case plant_type::twin_sunflower:
        required = plant_type::sunflower;
        break;
    case plant_type::gloomshroom:
        required = plant_type::fumeshroom;
        break;
    case plant_type::winter_melon:
        required = plant_type::melonpult;
        break;
    case plant_type::gold_magnet:
        required = plant_type::magnetshroom;
        break;
    case plant_type::spikerock:
        required = plant_type::spikeweed;
        break;
    case plant_type::cattail:
        for (const auto& row : scene.plant_map) {
            for (const auto& status : row) {
                if (status.base != nullptr &&
                    status.base->type == plant_type::lily_pad &&
                    status.content == nullptr)
                {
                    return true;
                }
            }
        }
        return false;
    case plant_type::cob_cannon:
        for (unsigned int row = 0; row < scene.rows; ++row) {
            for (unsigned int col = 0; col < 8; ++col) {
                if (can_plant_cob_cannon_at(row, col)) {
                    return true;
                }
            }
        }
        return false;
    default:
        return true;
    }

    for (const auto& plant : scene.plants) {
        if (plant.type == required) {
            return true;
        }
    }
    return false;
}

bool plant_factory::is_pos_valid(unsigned row, unsigned col) const {
    if (scene.type == scene_type::pool || scene.type == scene_type::fog) {
        if (row > 5) {
            return false;
        }
    } else if (row > 4) {
        return false;
    }

    if (col > 8) {
        return false;
    }

    return true;
}

bool plant_factory::is_not_covered_by_ice_path(unsigned row, unsigned col) const {
    static const int ICE_PATH_TABLE[] = {
        108, 188, 268, 348, 428, 508, 588, 668, 751
    };

    return scene.ice_path.countdown[row] <= 0 ||
        scene.ice_path.x[row] >= ICE_PATH_TABLE[col];
}

void plant_factory::is_covered_by_griditem(
    unsigned row,
    unsigned col,
    bool& has_grave,
    bool& has_crater) const
{
    has_grave = false;
    has_crater = false;

    for (auto& item : scene.griditems) {
        if (item.col == col && item.row == row) {
            if (item.type == griditem_type::grave) {
                has_grave = true;
            } else if (item.type == griditem_type::crater) {
                has_crater = true;
            }

            if (has_grave && has_crater) {
                return;
            }
        }
    }
}

unsigned int plant_factory::get_cost(object::plant_type type) const {
    const auto raw_type = static_cast<int>(type);
    if (raw_type < static_cast<int>(plant_type::pea_shooter) ||
        raw_type > static_cast<int>(plant_type::cob_cannon))
    {
        throw std::invalid_argument("plant type has no seed cost");
    }

    if (type >= plant_type::gatling_pea) {
        int n = 0;

        for (auto& p : scene.plants) {
            if (p.type == type) {
                n++;
            }
        }

        return plant::COST_TABLE[static_cast<int>(type)] + 50 * n;
    } else {
        return plant::COST_TABLE[static_cast<int>(type)];
    }
}

planting_reason plant_factory::can_plant_at(
    int grid_x,
    int grid_y,
    plant_type planting_type) const
{
    if (!is_selectable_plant_type(planting_type) ||
        planting_type == plant_type::imitater ||
        grid_x < 0 || grid_y < 0 ||
        !is_pos_valid(static_cast<unsigned int>(grid_y), static_cast<unsigned int>(grid_x)))
    {
        return planting_reason::not_here;
    }

    const auto row = static_cast<unsigned int>(grid_y);
    const auto col = static_cast<unsigned int>(grid_x);
    auto& status = scene.plant_map[row][col];
    if (planting_type == plant_type::coffee_bean) {
        if (scene.ignore_planting_restrictions) {
            return planting_reason::ok;
        }
        if (status.coffee_bean != nullptr || status.content == nullptr ||
            !status.content->is_sleeping || status.content->countdown.awake != 0 ||
            status.content->edible == plant_edible_status::invisible_and_edible)
        {
            return planting_reason::needs_sleeping;
        }
        return planting_reason::ok;
    }

    bool has_grave, has_crater;
    is_covered_by_griditem(row, col, has_grave, has_crater);
    if (planting_type == plant_type::grave_buster) {
        if (scene.ignore_planting_restrictions) {
            return planting_reason::ok;
        }
        if (status.content != nullptr) {
            return planting_reason::not_here;
        }
        return has_grave ? planting_reason::ok : planting_reason::only_on_graves;
    }

    if (!is_not_covered_by_ice_path(row, col)) {
        return planting_reason::not_here;
    }

    if (has_crater) {
        return planting_reason::not_on_crater;
    }

    if (planting_type == plant_type::cob_cannon && col + 1 >= 9) {
        return planting_reason::needs_upgrade;
    }

    if (scene.ignore_planting_restrictions) {
        return planting_reason::ok;
    }

    if (has_grave) {
        return planting_reason::not_on_grave;
    }

    const bool is_roof_scene = scene.type == scene_type::roof ||
        scene.type == scene_type::moon_night;
    const bool is_water_row = scene.is_pool_square(grid_x, grid_y);
    const bool has_pot = status.base &&
        status.base->type == plant_type::flower_pot &&
        status.base->edible != plant_edible_status::invisible_and_edible;
    const bool has_lily = status.base &&
        status.base->type == plant_type::lily_pad &&
        status.base->edible != plant_edible_status::invisible_and_edible;

    switch (planting_type) {
    case plant_type::lily_pad:
    case plant_type::tangle_kelp:
    case plant_type::seashroom:
        if (!is_water_row) {
            return planting_reason::only_in_pool;
        }
        return status.base == nullptr && status.content == nullptr ?
            planting_reason::ok : planting_reason::not_here;

    case plant_type::spikeweed:
        if (is_water_row || is_roof_scene || status.base != nullptr) {
            return planting_reason::needs_ground;
        }
        return status.content == nullptr ? planting_reason::ok : planting_reason::not_here;

    case plant_type::spikerock:
        if (is_water_row || is_roof_scene || status.base != nullptr) {
            return planting_reason::needs_ground;
        }
        if (can_plant_advanced_plant(status, planting_type) ||
            (scene.easy_planting_cheat && status.content == nullptr))
        {
            return planting_reason::ok;
        }
        return planting_reason::needs_upgrade;

    case plant_type::cob_cannon:
        return can_plant_cob_cannon_at(row, col) ?
            planting_reason::ok : planting_reason::needs_upgrade;

    case plant_type::cattail:
        if (!is_water_row) {
            return planting_reason::not_here;
        }
        if (can_plant_advanced_plant(status, planting_type) ||
            (scene.easy_planting_cheat && status.content == nullptr))
        {
            return planting_reason::ok;
        }
        return planting_reason::needs_upgrade;

    case plant_type::flower_pot:
        if (is_water_row) {
            return planting_reason::not_on_water;
        }
        return status.pumpkin == nullptr && status.content == nullptr &&
            status.base == nullptr ? planting_reason::ok : planting_reason::not_here;

    case plant_type::pumpkin:
        if (is_roof_scene && !has_pot) {
            return planting_reason::needs_pot;
        }
        if (is_water_row && !has_lily &&
            !(status.content && status.content->type == plant_type::cattail))
        {
            return planting_reason::not_on_water;
        }
        if (status.content && status.content->type == plant_type::cob_cannon) {
            return planting_reason::not_here;
        }
        if (status.pumpkin == nullptr ||
            (status.pumpkin->type == plant_type::pumpkin &&
                status.pumpkin->hp < 2 * 4000 / 3 &&
                status.pumpkin->edible != plant_edible_status::invisible_and_edible))
        {
            return planting_reason::ok;
        }
        return planting_reason::not_here;

    default:
        break;
    }

    if (is_roof_scene && !has_pot) {
        return planting_reason::needs_pot;
    }
    if (is_water_row && !has_lily) {
        if (!(planting_type == plant_type::cattail && scene.easy_planting_cheat)) {
            return planting_reason::not_on_water;
        }
    }

    switch (planting_type) {
    case plant_type::gatling_pea:
    case plant_type::twin_sunflower:
    case plant_type::gloomshroom:
    case plant_type::winter_melon:
    case plant_type::gold_magnet:
        if (can_plant_advanced_plant(status, planting_type)) {
            return planting_reason::ok;
        }
        if (scene.easy_planting_cheat && status.content == nullptr)
        {
            return planting_reason::ok;
        }
        return planting_reason::needs_upgrade;

    case plant_type::wallnut:
        if (status.content == nullptr ||
            (status.content->type == plant_type::wallnut &&
                status.content->hp < 2 * 4000 / 3 &&
                status.content->edible != plant_edible_status::invisible_and_edible))
        {
            return planting_reason::ok;
        }
        return planting_reason::not_here;

    case plant_type::tallnut:
        if (status.content == nullptr ||
            (status.content->type == plant_type::tallnut &&
                status.content->hp < 2 * 8000 / 3 &&
                status.content->edible != plant_edible_status::invisible_and_edible))
        {
            return planting_reason::ok;
        }
        return planting_reason::not_here;

    default:
        if (status.content != nullptr) {
            return planting_reason::not_here;
        }
        break;
    }

    if (planting_type == plant_type::potato_mine && is_water_row) {
        return planting_reason::only_on_ground;
    }
    return planting_reason::ok;
}

bool plant_factory::can_plant(
    unsigned int row,
    unsigned int col,
    plant_type type,
    plant_type imitater_type) const
{
    if (!is_selectable_plant_type(type) ||
        (type == plant_type::imitater && !is_valid_imitater_target(imitater_type)))
    {
        return false;
    }

    const auto planting_type = type == plant_type::imitater ? imitater_type : type;
    if (!scene.easy_planting_cheat &&
        !system::sun(scene).can_take_sun_money(
            get_current_plant_cost(type, imitater_type)))
    {
        return false;
    }

    return can_plant_at(
        static_cast<int>(col), static_cast<int>(row), planting_type) ==
        planting_reason::ok;
}

plant& plant_factory::create(
    plant_type type,
    int row,
    int col,
    plant_type imitater_target)
{
    const auto effective_type = type == plant_type::imitater ?
        imitater_target : type;
    if (!is_selectable_plant_type(type) ||
        (type == plant_type::imitater && !is_valid_imitater_target(imitater_target)) ||
        row < 0 || col < 0 ||
        !is_pos_valid(static_cast<unsigned int>(row), static_cast<unsigned int>(col)) ||
        (effective_type == plant_type::cob_cannon && col + 1 >= 9))
    {
        throw std::invalid_argument("invalid plant type, target, or position");
    }

    auto& p = scene.plants.alloc();

    switch (type) {
    case plant_type::pea_shooter:
        subsystems.pea_shooter.init(p, row, col);
        break;
    case plant_type::sunflower:
        subsystems.sunflower.init(p, row, col);
        break;
    case plant_type::cherry_bomb:
        subsystems.cherry_bomb.init(p, row, col);
        break;
    case plant_type::wallnut:
        subsystems.wallnut.init(p, row, col);
        break;
    case plant_type::potato_mine:
        subsystems.potato_mine.init(p, row, col);
        break;
    case plant_type::snow_pea:
        subsystems.snow_pea.init(p, row, col);
        break;
    case plant_type::chomper:
        subsystems.chomper.init(p, row, col);
        break;
    case plant_type::repeater:
        subsystems.repeater.init(p, row, col);
        break;
    case plant_type::puffshroom:
        subsystems.puffshroom.init(p, row, col);
        break;
    case plant_type::sunshroom:
        subsystems.sunshroom.init(p, row, col);
        break;
    case plant_type::fumeshroom:
        subsystems.fumeshroom.init(p, row, col);
        break;
    case plant_type::grave_buster:
        subsystems.grave_buster.init(p, row, col);
        break;
    case plant_type::hypnoshroom:
        subsystems.hypnoshroom.init(p, row, col);
        break;
    case plant_type::scaredyshroom:
        subsystems.scaredyshroom.init(p, row, col);
        break;
    case plant_type::iceshroom:
        subsystems.iceshroom.init(p, row, col);
        break;
    case plant_type::doomshroom:
        subsystems.doomshroom.init(p, row, col);
        break;
    case plant_type::lily_pad:
        subsystems.lily_pad.init(p, row, col);
        break;
    case plant_type::squash:
        subsystems.squash.init(p, row, col);
        break;
    case plant_type::threepeater:
        subsystems.threepeater.init(p, row, col);
        break;
    case plant_type::tangle_kelp:
        subsystems.tangle_kelp.init(p, row, col);
        break;
    case plant_type::jalapeno:
        subsystems.jalapeno.init(p, row, col);
        break;
    case plant_type::spikeweed:
        subsystems.spikeweed.init(p, row, col);
        break;
    case plant_type::torchwood:
        subsystems.torchwood.init(p, row, col);
        break;
    case plant_type::tallnut:
        subsystems.tallnut.init(p, row, col);
        break;
    case plant_type::seashroom:
        subsystems.seashroom.init(p, row, col);
        break;
    case plant_type::plantern:
        subsystems.plantern.init(p, row, col);
        break;
    case plant_type::cactus:
        subsystems.cactus.init(p, row, col);
        break;
    case plant_type::blover:
        subsystems.blover.init(p, row, col);
        break;
    case plant_type::split_pea:
        subsystems.split_pea.init(p, row, col);
        break;
    case plant_type::starfruit:
        subsystems.starfruit.init(p, row, col);
        break;
    case plant_type::pumpkin:
        subsystems.pumpkin.init(p, row, col);
        break;
    case plant_type::magnetshroom:
        subsystems.magnetshroom.init(p, row, col);
        break;
    case plant_type::cabbagepult:
        subsystems.cabbagepult.init(p, row, col);
        break;
    case plant_type::flower_pot:
        subsystems.flower_pot.init(p, row, col);
        break;
    case plant_type::kernelpult:
        subsystems.kernelpult.init(p, row, col);
        break;
    case plant_type::coffee_bean:
        subsystems.coffee_bean.init(p, row, col);
        break;
    case plant_type::garlic:
        subsystems.garlic.init(p, row, col);
        break;
    case plant_type::umbrella_leaf:
        subsystems.umbrella_leaf.init(p, row, col);
        break;
    case plant_type::marigold:
        subsystems.marigold.init(p, row, col);
        break;
    case plant_type::melonpult:
        subsystems.melonpult.init(p, row, col);
        break;
    case plant_type::gatling_pea:
        subsystems.gatling_pea.init(p, row, col);
        break;
    case plant_type::twin_sunflower:
        subsystems.twin_sunflower.init(p, row, col);
        break;
    case plant_type::gloomshroom:
        subsystems.gloomshroom.init(p, row, col);
        break;
    case plant_type::cattail:
        subsystems.cattail.init(p, row, col);
        break;
    case plant_type::winter_melon:
        subsystems.winter_melon.init(p, row, col);
        break;
    case plant_type::gold_magnet:
        subsystems.gold_magnet.init(p, row, col);
        break;
    case plant_type::spikerock:
        subsystems.spikerock.init(p, row, col);
        break;
    case plant_type::cob_cannon:
        subsystems.cob_cannon.init(p, row, col);
        break;
    case plant_type::imitater:
        subsystems.imitater.init(p, imitater_target, row, col);
        break;
    default:
        assert(false);
        break;
    }

    if (type != plant_type::imitater) {
        p.imitater_target = imitater_target;
    }

    const auto map_type = p.type == plant_type::imitater ?
        p.imitater_target : p.type;
    auto assign = [&](object::plant*& slot) {
        if (slot && slot != &p && !slot->is_dead) {
            this->scene.plant_map_incomplete[p.row] = true;
        }
        slot = &p;
    };
    if (map_type == plant_type::pumpkin) {
        assign(scene.plant_map[p.row][p.col].pumpkin);
    } else if (map_type == plant_type::coffee_bean) {
        assign(scene.plant_map[p.row][p.col].coffee_bean);
    } else if (map_type == plant_type::flower_pot || map_type == plant_type::lily_pad) {
        assign(scene.plant_map[p.row][p.col].base);
    } else if (map_type == plant_type::cob_cannon) {
        assign(scene.plant_map[p.row][p.col].content);
        assign(scene.plant_map[p.row][p.col + 1].content);
    } else {
        assign(scene.plant_map[p.row][p.col].content);
    }

    return p;
}

plant* plant_factory::new_plant(
    int grid_x,
    int grid_y,
    plant_type seed_type,
    plant_type imitater_type)
{
    const bool valid_imitater = seed_type == plant_type::imitater ?
        is_valid_imitater_target(imitater_type) :
        (imitater_type == plant_type::none || imitater_type == plant_type::imitater);
    const auto effective_type = seed_type == plant_type::imitater ?
        imitater_type : seed_type;
    if (!is_selectable_plant_type(seed_type) || !valid_imitater ||
        grid_x < 0 || grid_y < 0 ||
        !is_pos_valid(static_cast<unsigned int>(grid_y), static_cast<unsigned int>(grid_x)) ||
        (effective_type == plant_type::cob_cannon && grid_x + 1 >= 9) ||
        scene.plants.active_count() >= scene.plants.capacity())
    {
        return nullptr;
    }

    return &create(seed_type, grid_y, grid_x, imitater_type);
}

plant* plant_factory::add_plant(
    int grid_x,
    int grid_y,
    plant_type seed_type,
    plant_type imitater_type)
{
    return new_plant(grid_x, grid_y, seed_type, imitater_type);
}

plant* plant_factory::plant(
    unsigned int i,
    unsigned int row,
    unsigned int col)
{
    if (i >= scene.cards.size() ||
        !can_pick_up(scene.cards[i]) ||
        scene.plants.active_count() >= scene.plants.capacity())
    {
        return nullptr;
    }

    auto type = scene.cards[i].type;
    auto imitater_type = scene.cards[i].imitater_type;
    auto target_type = type == plant_type::imitater ? imitater_type : type;

    if (!can_plant(row, col, type, imitater_type)) {
        return nullptr;
    }

    if (!scene.easy_planting_cheat &&
        !system::sun(scene).take_sun_money(
            get_current_plant_cost(target_type, plant_type::none)))
    {
        return nullptr;
    }

    auto& status = scene.plant_map[row][col];

    bool is_gloom_awake = false;
    unsigned int gloom_awake_countdown = 0;

    switch (target_type) {
    case plant_type::wallnut:
    case plant_type::tallnut:
        if (status.content && status.content->type == target_type) {
            destroy(*status.content);
        }
        break;

    case plant_type::pumpkin:
        if (target_type == plant_type::pumpkin && status.pumpkin) {
            destroy(*status.pumpkin);
        }
        break;

    case plant_type::gatling_pea:
    case plant_type::twin_sunflower:
    case plant_type::gloomshroom:
    case plant_type::winter_melon:
    case plant_type::gold_magnet:
    case plant_type::spikerock:
        if (status.content) {
            if (target_type == plant_type::gloomshroom) {
                gloom_awake_countdown = status.content->countdown.awake;
                is_gloom_awake = !status.content->is_sleeping;
            }

            destroy(*status.content);
        }
        break;

    case plant_type::cattail:
        if (status.base) {
            destroy(*status.base);
        }
        break;
        
    case plant_type::cob_cannon:
        if (status.content) {
            destroy(*status.content);
        }

        if (col + 1 < 9) {
            auto& s = scene.plant_map[row][col + 1];
            if (s.content) {
                destroy(*s.content);
            }
        }

        break;

    default:
        break;
    }

    auto p = add_plant(
        static_cast<int>(col), static_cast<int>(row), type, imitater_type);
    assert(p != nullptr);

    if (p->type == plant_type::gloomshroom) {
        if (is_gloom_awake) {
            p->set_sleep(false);
        }

        p->countdown.awake = gloom_awake_countdown;
    }

    was_planted(scene.cards[i]);

    return p;
}

void plant_factory::destroy(object::plant& p) {
    if (!scene.plants.is_active(&p)) {
        throw std::invalid_argument("plant does not belong to this scene");
    }

    p.is_dead = true;

    if (p.type == plant_type::tangle_kelp && p.target != -1) {
        if (auto z = scene.zombies.try_to_get(static_cast<std::uint32_t>(p.target))) {
            zombie_factory.destroy(*z);
        }
    }

    if (p.type != plant_type::coffee_bean) {
        for (auto& item : scene.griditems) {
            if (item.row == p.row &&
                item.col == p.col &&
                item.type == griditem_type::ladder)
            {
                griditem_factory.destroy(item);
            }
        }
    }

    auto& status = scene.plant_map[p.row][p.col];
    if (status.coffee_bean == nullptr &&
        status.content == nullptr &&
        status.pumpkin == nullptr &&
        status.base != nullptr &&
        status.base->type == plant_type::flower_pot &&
        !status.base->is_squash_attacking() &&
        status.base->edible != plant_edible_status::invisible_and_not_edible &&
        !status.base->is_dead &&
        !status.base->is_smashed)
    {
        status.base->reanim.fps = rng.randfloat(10, 15);
    }

    scene.unregister_plant_from_map(p);
}

int plant_factory::get_current_plant_cost(
    plant_type seed_type,
    plant_type imitater_type) const
{
    if (!is_selectable_plant_type(seed_type) ||
        (seed_type == plant_type::imitater &&
            !is_valid_imitater_target(imitater_type)))
    {
        throw std::invalid_argument("plant type has no seed cost");
    }

    const auto effective_type = seed_type == plant_type::imitater ?
        imitater_type : seed_type;
    int cost = static_cast<int>(plant::COST_TABLE[static_cast<int>(effective_type)]);

    if (seed_type >= plant_type::gatling_pea &&
        seed_type <= plant_type::cob_cannon)
    {
        for (const auto& p : scene.plants) {
            if (p.type == seed_type) {
                cost += 50;
            }
        }
    }

    return cost;
}

bool plant_factory::can_pick_up(const scene::card_data& card) const {
    if (!is_selectable_plant_type(card.type) ||
        (card.type == plant_type::imitater &&
            !is_valid_imitater_target(card.imitater_type)))
    {
        return false;
    }

    if (scene.easy_planting_cheat) {
        return true;
    }

    if (!scene.seed_recharge_ignored && card.cold_down > 0) {
        return false;
    }

    if (!system::sun(scene).can_take_sun_money(
            get_current_plant_cost(card.type, card.imitater_type)))
    {
        return false;
    }

    const auto planting_type = card.type == plant_type::imitater ?
        card.imitater_type : card.type;
    return planting_requirements_met(planting_type);
}

void plant_factory::was_planted(scene::card_data& card) const {
    if (!is_selectable_plant_type(card.type)) {
        return;
    }

    if (scene.seed_recharge_ignored) {
        card.cold_down = 0;
        return;
    }

    const auto planting_type = card.type == plant_type::imitater ?
        card.imitater_type : card.type;
    if (!is_selectable_plant_type(planting_type) ||
        planting_type == plant_type::imitater)
    {
        return;
    }
    card.cold_down = plant::CD_TABLE[static_cast<int>(planting_type)] + 1;
}

}
