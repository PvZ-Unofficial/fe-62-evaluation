#pragma once
#include <algorithm>
#include "object/scene.h"
#include "rng.h"

namespace pvz_emulator::system {

class sun {
    object::scene& scene;
    object::scene::sun_data& data;
    system::rng rng;

    unsigned int gen_nature_sun_countdown();
public:
    void add_sun(unsigned int sun) {
        if (data.sun >= data.limit || sun >= data.limit - data.sun) {
            data.sun = data.limit;
        } else {
            scene.sun.sun += sun;
        }
    }

    sun(object::scene &s): scene(s), data(s.sun), rng(s) {}

    bool can_take_sun_money(int amount) const;
    bool take_sun_money(int amount);
    void reset();
    void update();
};

}
