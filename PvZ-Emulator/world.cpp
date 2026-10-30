#include <set>
#include <map>
#include <array>
#include <exception>
#include <stdexcept>

#include "rapidjson/writer.h"
#include "rapidjson/stringbuffer.h"

#include "world.h"

using namespace pvz_emulator::object;

namespace pvz_emulator {

namespace {

bool is_grid_in_bounds(const scene& scene, unsigned int row, unsigned int col) {
    return row < scene.rows && col < 9;
}

}

void world::clean_obj_lists() {
    scene.plants.shrink_to_fit();
    scene.zombies.shrink_to_fit();
    scene.projectiles.shrink_to_fit();
    scene.griditems.shrink_to_fit();
}

bool world::update() {
    if (scene.is_game_over) {
        return true;
    }

    scene.zombie_dancing_clock += 1;

    clean_obj_lists();

    griditem.update();

    plant_system.update();

    const bool game_over = zombie.update();
    scene.is_game_over = game_over;

    projectile.update();

    if (game_over) {
        return true;
    }

    for (auto& card : scene.cards) {
        if (scene.seed_recharge_ignored) {
            card.cold_down = 0;
        } else if (card.cold_down > 0) {
            --card.cold_down;
        }
    }

    ++scene.main_counter;

    spawn.try_start_endgame_for_clear();

    sun.update();

    if (!scene.stop_spawn) {
        spawn.update();
    }

    ice_path.update();

    if (scene.spawn.countdown.pool > 0) {
        --scene.spawn.countdown.pool;
    }

    if (endgame.update()) {
        const bool level_rng_locked = scene.level_rng.locked();
        const auto locked_value = scene.level_rng.fixed_value();
        scene.level_rng.reseed_engine(scene.level_rng.source_seed_value() + 101);
        if (level_rng_locked) {
            scene.level_rng.lock(locked_value);
        }
        spawn.reset();
        scene.spawn.total_flags++;
        return true;
    } else {
        return false;
    }
}

bool world::update(const std::tuple<int, int, int> &action) {
    int op = std::get<0>(action);
    int row = std::get<1>(action);
    int col = std::get<2>(action);

    if (row >= 0 && row < scene.rows && col >= 0 && col < 9) {
        if (op == -2) {
            if (scene.plant_map[row][col].pumpkin) {
                plant_factory.destroy(*scene.plant_map[row][col].pumpkin);
            }
        } else if (op == -1) {
            if (scene.plant_map[row][col].coffee_bean) {
                plant_factory.destroy(*scene.plant_map[row][col].coffee_bean);
            } else if (scene.plant_map[row][col].content) {
                plant_factory.destroy(*scene.plant_map[row][col].content);
            } else if (scene.plant_map[row][col].base) {
                plant_factory.destroy(*scene.plant_map[row][col].base);
            }
        } else {
            for (int i = 0; i < 10; i++) {
                if (static_cast<int>(scene.cards[i].type) == op) {
                    plant(i, row, col);
                    break;
                }
            }
        }
    }

    return update();
}

void world::get_available_actions(
    const action_vector& actions,
    std::vector<int>& action_masks) const
{
    std::array<bool, static_cast<int>(plant_type::imitater) + 1> card_flags = {false};
    std::array<int, static_cast<int>(plant_type::imitater) + 1> card_index = {0};
    for (int i = 0; i < 10; i++) {
        const auto& card = scene.cards[i];

        if (plant_factory.can_pick_up(card)) {
            card_flags[static_cast<int>(card.type)] = true;
            card_index[static_cast<int>(card.type)] = i;
        }
    }

    action_masks.resize(actions.size() + 1, 0);
    std::fill(action_masks.begin(), action_masks.end(), 0);
    action_masks.back() = 1;

    std::set<std::pair<unsigned int, unsigned int>> imitater_type_actions;
    std::map<int, std::pair<unsigned int, unsigned int>> imitater_actions;

    for (int i = 0; i < actions.size(); i++) {
        const auto& [op, row, col] = actions[i];

        if (row < 0 || row >= scene.rows || col < 0 || col >= 9) {
            continue;
        }

        if (op == -2) {
            if (scene.plant_map[row][col].pumpkin &&
                !scene.plant_map[row][col].pumpkin->is_dead &&
                !scene.plant_map[row][col].pumpkin->is_smashed)
            {
                action_masks[i] = 1;
            }
        } else if (op == -1) {
            if (scene.plant_map[row][col].coffee_bean &&
                !scene.plant_map[row][col].coffee_bean->is_dead &&
                !scene.plant_map[row][col].coffee_bean->is_smashed)
            {
                action_masks[i] = 1;
            } else if (scene.plant_map[row][col].content &&
               !scene.plant_map[row][col].content->is_dead &&
               !scene.plant_map[row][col].content->is_smashed)
            {
                action_masks[i] = 1;
            } else if (scene.plant_map[row][col].base &&
               !scene.plant_map[row][col].base->is_dead &&
               !scene.plant_map[row][col].base->is_smashed)
            {
                action_masks[i] = 1;
            }
        } else if (op >= 0) {
            if (op <= static_cast<int>(plant_type::imitater) &&
                card_flags[op] &&
                plant_factory.can_pick_up(scene.cards[card_index[op]]) &&
                plant_factory.can_plant(
                    row,
                    col,
                    scene.cards[card_index[op]].type,
                    scene.cards[card_index[op]].imitater_type))
            {
                action_masks[i] = 1;
            }
        }
    }
}

void world::update_all(
    std::vector<world *>& w,
    const action_vector & all_actions,
    action_vector& actions,
    batch_action_masks& action_masks,
    const check_list &build_check_list,
    std::vector<int>& check_result,
    std::vector<int>& done,
    unsigned int frames)
{
    if (frames == 0) {
        throw std::invalid_argument("frames must be greater than zero");
    }
    if (actions.size() != w.size()) {
        throw std::invalid_argument("actions size must match worlds size");
    }
    std::set<world*> unique_worlds;
    for (auto world : w) {
        if (world == nullptr) {
            throw std::invalid_argument("worlds must not contain null pointers");
        }
        if (!unique_worlds.insert(world).second) {
            throw std::invalid_argument("worlds must not contain duplicate pointers");
        }
    }

    std::atomic<decltype(w.size())> i = 0;
    done.assign(w.size(), 0);
    check_result.assign(w.size(), 0);
    action_masks.assign(w.size(), {});

    if (w.empty()) {
        return;
    }

    std::vector<std::thread> threads;
    std::vector<std::exception_ptr> errors(w.size());
    const auto worker_count = std::min<std::size_t>(
        w.size(),
        std::max(1u, std::thread::hardware_concurrency()));
    threads.reserve(worker_count);
    try {
        for (std::size_t j = 0; j < worker_count; j++) {
            threads.emplace_back([&]() {
                for (auto k = i.fetch_add(1); k < w.size(); k = i.fetch_add(1)) {
                    try {
                        done[k] = w[k]->update(actions[k]);
                        for (unsigned int l = 0; l < frames - 1 && !done[k]; l++) {
                            done[k] = w[k]->update();
                        }
                        check_result[k] = w[k]->check_build(build_check_list);
                        w[k]->get_available_actions(all_actions, action_masks[k]);
                    } catch (...) {
                        errors[k] = std::current_exception();
                    }
                }
            });
        }
    } catch (...) {
        for (auto& t : threads) {
            t.join();
        }
        throw;
    }

    for (auto& t : threads) {
        t.join();
    }

    for (const auto& error : errors) {
        if (error) {
            std::rethrow_exception(error);
        }
    }
}

void world::to_json(std::string& s) {
    rapidjson::StringBuffer sb;
    rapidjson::Writer<rapidjson::StringBuffer> writer(sb);

    scene.to_json(writer);

    s = sb.GetString();
}

using namespace pvz_emulator::object;

bool world::select_plants(
    const std::vector<plant_type>& cards,
    plant_type imitater_type)
{
    if (cards.size() > scene.cards.size()) {
        return false;
    }

    std::array<bool, static_cast<int>(plant_type::imitater) + 1> seen = {false};

    for (std::size_t i = 0; i < cards.size(); i++) {
        const auto type = cards[i];
        const auto raw_type = static_cast<int>(type);
        if (raw_type < 0 || raw_type > static_cast<int>(plant_type::imitater)) {
            return false;
        }

        if (type == plant_type::imitater &&
            !is_valid_imitater_target(imitater_type))
        {
            return false;
        }

        if (seen[raw_type]) {
            return false;
        }
        seen[raw_type] = true;
    }

    unsigned int prev_imitater_cd = 0;
    plant_type prev_imitater_type = plant_type::none;

    std::array<unsigned int, static_cast<int>(plant_type::imitater) + 1> cooldown = {0};
    std::array<bool, static_cast<int>(plant_type::imitater) + 1> has_cooldown = {false};

    for (auto &card : scene.cards) {
        if (card.type != plant_type::none) {
            if (card.type == plant_type::imitater) {
                prev_imitater_type = card.imitater_type;
                prev_imitater_cd = card.cold_down;
            } else {
                const auto raw_type = static_cast<int>(card.type);
                if (raw_type >= 0 && raw_type < static_cast<int>(plant_type::imitater)) {
                    cooldown[raw_type] = card.cold_down;
                    has_cooldown[raw_type] = true;
                }
            }
        }
    }

    for (std::size_t i = 0; i < scene.cards.size(); i++) {
        if (i < cards.size()) {
            if (cards[i] == plant_type::imitater) {
                scene.cards[i].type = plant_type::imitater;
                scene.cards[i].imitater_type = imitater_type;

                if (imitater_type == prev_imitater_type) {
                    scene.cards[i].cold_down = prev_imitater_cd;
                } else if (prev_imitater_cd > 0 && is_valid_imitater_target(prev_imitater_type)) {
                    const auto old_total = object::plant::CD_TABLE[static_cast<int>(prev_imitater_type)] + 1;
                    const auto new_total = object::plant::CD_TABLE[static_cast<int>(imitater_type)] + 1;
                    const auto elapsed = prev_imitater_cd < old_total ? old_total - prev_imitater_cd : 0;
                    scene.cards[i].cold_down = elapsed < new_total ? new_total - elapsed : 0;
                } else {
                    scene.cards[i].cold_down = 0;
                }
            } else {
                scene.cards[i].type = cards[i];
                scene.cards[i].imitater_type = plant_type::none;
                const auto raw_type = static_cast<int>(cards[i]);
                scene.cards[i].cold_down = has_cooldown[raw_type] ? cooldown[raw_type] : 0;
            }
        } else {
            scene.cards[i].type =
                scene.cards[i].imitater_type =
                plant_type::none;

            scene.cards[i].cold_down = 0;
        }
    }

    return true;
}

bool world::plant(object::plant_type type, unsigned int row, unsigned int col) {
    for (int i = 0; i < 10; i++) {
        if (scene.cards[i].type == type) {
            return plant_factory.plant(i, row, col) != nullptr;
        }
    }

    return false;
}

bool world::check_build(const check_list &plants) {
    for (auto& t : plants) {
        auto& [type, row, col] = t;

        if (!is_grid_in_bounds(scene, row, col)) {
            return false;
        }

        if (type == plant_type::pumpkin) {
            if (scene.plant_map[row][col].pumpkin == nullptr) {
                return false;
            }
        } else if (type == plant_type::lily_pad || type == plant_type::flower_pot) {
            if (scene.plant_map[row][col].base == nullptr ||
                scene.plant_map[row][col].base->type != type)
            {
                return false;
            }
        } else if (type == plant_type::coffee_bean) {
            if (scene.plant_map[row][col].coffee_bean == nullptr ||
                scene.plant_map[row][col].coffee_bean->type != type)
            {
                return false;
            }
        } else if (scene.plant_map[row][col].content == nullptr ||
            scene.plant_map[row][col].content->type != type)
        {
            return false;
        }
    }

    return true;
}

}
