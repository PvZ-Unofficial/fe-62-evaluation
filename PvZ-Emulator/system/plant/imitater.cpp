#include "plant.h"
#include "system/plant/plant_factory.h"
#include <stdexcept>

namespace pvz_emulator::system {

using namespace pvz_emulator::object;

plant& plant_imitater::morph(plant& p) {
    if (p.is_dead || p.type != plant_type::imitater) {
        throw std::invalid_argument("morph requires a live imitater");
    }
    if (scene.plants.active_count() >= scene.plants.capacity()) {
        throw std::invalid_argument("no plant slot for imitater successor");
    }
    plant_factory factory(scene);
    factory.destroy(p);
    auto& successor = factory.create(p.imitater_target, p.row, p.col, plant_type::imitater);
    p.imitater_morph_successor_id = scene.plants.get_id(successor);
    return successor;
}

void plant_imitater::update(plant& p) {
    if (p.status == plant_status::imitater_explode) {
        if (p.reanim.n_repeated > 0) {
            morph(p);
        }
    } else if (p.countdown.status == 0) {
        p.status = plant_status::imitater_explode;
        p.set_reanim(plant_reanim_name::anim_explode, reanim_type::once, 26);
    }
}

void plant_imitater::init(
    plant & p,
    plant_type target,
    unsigned int row,
    unsigned int col)
{
    plant_base::init(p, plant_type::imitater, row, col, target);
    p.reanim.fps = rng.randfloat(25, 30);
    p.countdown.status = 200;
}

}
