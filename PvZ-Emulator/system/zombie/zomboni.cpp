#include <algorithm>
#include "zombie.h"

namespace pvz_emulator::system {

using namespace pvz_emulator::object;

void zombie_zomboni::update(object::zombie& z) {
    if (z.x > 400) {
        const int curve_age = static_cast<int>(z.x);
        float dx = static_cast<float>(
            static_cast<double>(curve_age - 700) / -400.0);

        if (dx > 0) {
            if (dx < 1) {
                const float scaled = static_cast<float>(
                    static_cast<double>(dx) * -0.19999999925494194);
                dx = static_cast<float>(
                    static_cast<double>(scaled) + 0.25);
            } else {
                dx = 0.050000001;
            }
        }
        else {
            dx = 0.25;
        }

        z.dx = dx;
    }

    const int ice_x = static_cast<int>(
        static_cast<double>(z.x) + 118.0);
    int px;
    if (scene.type == scene_type::roof ||
        scene.type == scene_type::moon_night)
    {
        px = std::max(500, ice_x);
    } else {
        px = std::max(25, ice_x);
    }

    if (px < scene.ice_path.x[z.row]) {
        scene.ice_path.x[z.row] = px;
    }

    if (px < 800) {
        scene.ice_path.countdown[z.row] = 3000;
    }
}

void zombie_zomboni::init(zombie &z, unsigned int row) {
    z._ground = nullptr;

    zombie_base::init(z, zombie_type::zomboni, row);

    z.hp = 1350;
    z.x = static_cast<float>(rng.randint(10) + 800);

    z.garlic_tick.b = 2;
    z.garlic_tick.a = 8;

    reanim.set(z, zombie_reanim_name::anim_drive, reanim_type::repeat, 12);

    z.hit_box.x = 0;
    z.hit_box.y = -13;
    z.hit_box.width = 153;
    z.hit_box.height = 140;

    z.attack_box.x = 10;
    z.attack_box.y = -13;
    z.attack_box.width = 133;
    z.attack_box.height = 140;

    set_common_fields(z);
}

}
