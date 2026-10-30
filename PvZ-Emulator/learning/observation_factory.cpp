#include <thread>
#include <algorithm>
#include <limits>
#include <stdexcept>
#include "observation_factory.h"

namespace pvz_emulator::learning {

using namespace pvz_emulator::object;

const unsigned int observation_factory::zombie_size = 14;
const unsigned int observation_factory::plant_size = 9;
const unsigned int observation_factory::projectiles_size = 4;
const unsigned int observation_factory::griditems_size = 3;
const unsigned int observation_factory::meta_size = 8;
const unsigned int observation_factory::spawn_flags_size = 33;

observation_factory::observation_factory(
        scene_type type,
        unsigned int num_zombies,
        unsigned int num_plants,
        unsigned int num_projectiles,
        unsigned int num_griditems) :
    num_zombies(num_zombies),
    num_plants(num_plants),
    num_projectiles(num_projectiles),
    num_griditems(num_griditems),
    num_ice_paths(type == scene_type::night ? 0 :
        type == scene_type::pool || type == scene_type::fog ? 6 : 5),
    single_size(0)
{
    if (num_zombies > decltype(scene::zombies)::capacity() ||
        num_plants > decltype(scene::plants)::capacity() ||
        num_projectiles > decltype(scene::projectiles)::capacity() ||
        num_griditems > decltype(scene::griditems)::capacity())
    {
        throw std::invalid_argument("observation object limit exceeds pool capacity");
    }

    const std::size_t calculated_size =
        static_cast<std::size_t>(num_zombies) * zombie_size +
        static_cast<std::size_t>(num_plants) * plant_size +
        static_cast<std::size_t>(num_projectiles) * projectiles_size +
        static_cast<std::size_t>(num_griditems) * griditems_size +
        30 + // Cards
        2 * num_ice_paths +
        spawn_flags_size +
        meta_size;
    if (calculated_size > std::numeric_limits<unsigned int>::max()) {
        throw std::length_error("observation row is too large");
    }
    single_size = static_cast<unsigned int>(calculated_size);
}

void observation_factory::create(
    std::vector<world *> &worlds,
    const world::batch_action_masks& action_masks,
    std::vector<float> &ob)
{
    if (action_masks.size() != worlds.size()) {
        throw std::invalid_argument("action mask batch size must match worlds size");
    }
    if (worlds.empty()) {
        ob.clear();
        return;
    }

    const auto mask_size = action_masks.front().size();
    for (std::size_t i = 0; i < worlds.size(); i++) {
        if (worlds[i] == nullptr) {
            throw std::invalid_argument("worlds must not contain null pointers");
        }
        if (action_masks[i].size() != mask_size) {
            throw std::invalid_argument("all action mask rows must have equal size");
        }
    }

    if (mask_size > std::numeric_limits<std::size_t>::max() - single_size) {
        throw std::length_error("observation row is too large");
    }
    const std::size_t row_size = single_size + mask_size;
    if (row_size != 0 && worlds.size() > ob.max_size() / row_size) {
        throw std::length_error("observation batch is too large");
    }
    ob.assign(worlds.size() * row_size, 0);

    std::atomic<decltype(worlds.size())> i = 0;
    std::vector<std::thread> threads;

    const auto worker_count = std::min<std::size_t>(
        worlds.size(),
        std::max(1u, std::thread::hardware_concurrency()));
    for (std::size_t j = 0; j < worker_count; j++) {
        threads.emplace_back([&]() {
            for (auto k = i.fetch_add(1); k < worlds.size(); k = i.fetch_add(1)) {
                create(*worlds[k], action_masks[k], &ob[row_size * k]);
            }
        });
    }

    for (auto& t: threads) {
        t.join();
    }
}

void observation_factory::create(
    world &world,
    const world::action_masks &action_masks,
    std::vector<float> &ob)
{
    if (action_masks.size() > ob.max_size() - single_size) {
        throw std::length_error("observation row is too large");
    }
    ob.assign(static_cast<std::size_t>(single_size) + action_masks.size(), 0);

    create(world, action_masks, ob.data());
}

void observation_factory::create(
    world &w,
    const world::action_masks& masks,
    float *base)
{
    auto& scene = w.scene;

    base = fill_ob_vector<zombie>(
        scene.zombies,
        base,
        zombie_size,
        num_zombies,
        [&](float *a, const zombie& z) {
            a[0] = static_cast<float>(z.type) / 32.0;
            a[1] = static_cast<float>(z.status) / 91.0;
            a[2] = static_cast<float>(z.action) / 9.0;
            a[3] = z.x / 820.0;
            a[4] = z.y / 600.0;
            a[5] = static_cast<float>(z.row) / scene.rows;
            a[6] = static_cast<float>(z.bungee_col) / 9.0;
            a[7] = static_cast<float>(z.hp) / z.max_hp;
            a[8] = static_cast<float>(z.accessory_1.hp) / std::max(1u, z.accessory_1.max_hp);
            a[9] = static_cast<float>(z.accessory_2.hp) / std::max(1u, z.accessory_2.max_hp);
            a[10] = static_cast<float>(z.countdown.freeze) / 600.0;
            a[11] = static_cast<float>(z.countdown.butter) / 400.0;
            a[12] = static_cast<float>(z.countdown.slow) / 2000.0;
            a[13] = static_cast<float>(z.countdown.action) / 2300.0;
        });

    base = fill_ob_vector<plant>(
        scene.plants,
        base,
        plant_size,
        num_plants,
        [&](float *a, const plant& p) {
            a[0] = static_cast<float>(p.type) / 48.0;
            a[1] = static_cast<float>(p.status) / 48.0;
            a[2] = static_cast<float>(p.hp) / p.max_hp;
            a[3] = static_cast<float>(p.row) / scene.rows;
            a[4] = static_cast<float>(p.col) / 9;
            a[5] = static_cast<float>(p.countdown.status) / 12000.;
            a[6] = static_cast<float>(p.countdown.effect) / 200.0;
            a[7] = static_cast<float>(p.countdown.awake) / 100.0;
            a[8] = static_cast<float>(p.countdown.launch) / 206.0;
        });

    base = fill_ob_vector<projectile>(
        scene.projectiles,
        base,
        projectiles_size,
        num_projectiles,
        [&](float *a, const projectile& proj) {
            a[0] = static_cast<float>(proj.type) / 12.0;
            a[1] = static_cast<float>(proj.row) / scene.rows;
            a[2] = proj.x / 800.0;
            a[3] = proj.y / 600.0;
        });

    base = fill_ob_vector<griditem>(
        scene.griditems,
        base,
        griditems_size,
        num_griditems,
        [&](float *a, const griditem& item) {
            a[0] = static_cast<float>(item.type) / 3.0;
            a[1] = static_cast<float>(item.row) / scene.rows;
            a[2] = static_cast<float>(item.col) / 9.0;
        });

    for (unsigned int i = 0; i < 10; i++) {
        auto& card = scene.cards[i];
        int target_type = static_cast<int>(card.type == plant_type::imitater ?
            card.imitater_type :
            card.type);

        base[3 * i + 0] = static_cast<float>(card.type) / 48.0;
        base[3 * i + 1] = static_cast<float>(card.imitater_type) / 48.0;
        base[3 * i + 2] = target_type >= 0 &&
            target_type < static_cast<int>(plant::CD_TABLE.size()) ?
            static_cast<float>(card.cold_down) / plant::CD_TABLE[target_type] :
            0.0f;
    }

    base += 30;

    for (unsigned i = 0; i < num_ice_paths; i++) {
        base[2 * i + 0] = scene.ice_path.x[i] / 800.0f;
        base[2 * i + 1] = scene.ice_path.countdown[i] / 3000.0f;
    }

    base += num_ice_paths * 2;

    std::copy(
        scene.spawn.spawn_flags.begin(),
        scene.spawn.spawn_flags.end(),
        base);

    base += scene.spawn.spawn_flags.size();

    base[0] = static_cast<float>(scene.spawn.wave) / 20;
    base[1] = static_cast<float>(scene.spawn.countdown.next_wave) / scene.spawn.countdown.next_wave_initial;
    base[2] = static_cast<float>(scene.spawn.countdown.lurking_squad) / 200.0;
    base[3] = static_cast<float>(scene.spawn.countdown.hugewave_fade) / 750.0;
    base[4] = static_cast<float>(scene.spawn.countdown.endgame) / 500.0;
    base[5] = static_cast<float>(scene.spawn.countdown.pool) / 300.0;
    base[6] = static_cast<float>(scene.sun.sun) / 9990.0;
    base[7] = static_cast<float>(scene.spawn.total_flags);

    base += meta_size;
    std::copy(masks.cbegin(), masks.cend(), base);
}

}
