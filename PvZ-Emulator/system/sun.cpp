#include <algorithm>
#include "sun.h"

namespace pvz_emulator::system {

using namespace pvz_emulator::object;

unsigned int sun::gen_nature_sun_countdown() {
    int c = data.natural_sun_generated * 10 + 425;
    return std::min(c, 950) + rng.randint(275);
}

bool sun::can_take_sun_money(int amount) const {
    return amount >= 0 &&
        (scene.ignore_sun_cost || static_cast<unsigned int>(amount) <= data.sun);
}

bool sun::take_sun_money(int amount) {
    if (!can_take_sun_money(amount)) {
        return false;
    }

    if (!scene.ignore_sun_cost) {
        data.sun -= static_cast<unsigned int>(amount);
    }
    return true;
}

void sun::reset() {
    data.natural_sun_countdown = 425 + rng.randint(276);
}

void sun::update() {
    if (scene.spawn.countdown.endgame > 0) {
        return;
    }

    if (scene.natural_sun_drop_disabled) {
        return;
    }

    if (scene.type != scene_type::pool &&
        scene.type != scene_type::day &&
        scene.type != scene_type::roof)
    {
        return;
    }

    if (data.natural_sun_countdown == 0) {
        if (data.natural_sun_generated == 0) {
            reset();
        } else {
            data.natural_sun_countdown = gen_nature_sun_countdown();
        }
    }

    if (--data.natural_sun_countdown == 0) {
        add_sun(25);
        data.natural_sun_generated++;
        data.natural_sun_countdown = gen_nature_sun_countdown();
    }
}

}
