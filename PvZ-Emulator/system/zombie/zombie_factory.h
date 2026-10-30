#pragma once
#include <optional>

#include "object/scene.h"
#include "object/zombie.h"
#include "system/rng.h"
#include "system/reanim.h"
#include "system/zombie/zombie.h"

namespace pvz_emulator {
class world;
}

namespace pvz_emulator::system {

class zombie_factory {
private:
    friend class ::pvz_emulator::world;

    object::scene& scene;
    object::scene::spawn_data& data;
    system::reanim reanim;
    system::rng rng;

    zombie_subsystems subsystems;
    
    bool can_spawn_at_row(object::zombie_type type, unsigned int row);
    unsigned int get_spawn_row(object::zombie_type type);
    object::zombie& create_in_row_impl(
        object::zombie_type type,
        unsigned int row,
        std::optional<unsigned int> spawn_wave);
    object::zombie* try_create_in_row(
        object::zombie_type type,
        unsigned int row,
        std::optional<unsigned int> spawn_wave = std::nullopt);
    object::zombie* try_create_in_grid(
        object::zombie_type type,
        unsigned int row,
        unsigned int col,
        std::optional<unsigned int> spawn_wave = std::nullopt);
	bool has_spawn_row(object::zombie_type type);

    bool create_pool_or_night_lurking(object::zombie_type type, unsigned int row, unsigned int col);
    bool create_roof_lurking(object::zombie_type type, unsigned int row, unsigned int col);

public:
    zombie_factory(object::scene& s) :
        scene(s),
        data(s.spawn),
        reanim(s),
        rng(s),
        subsystems(s) {}

    object::zombie& create(
        object::zombie_type type,
        std::optional<unsigned int> spawn_wave = std::nullopt);
    object::zombie* try_create(
        object::zombie_type type,
        std::optional<unsigned int> spawn_wave = std::nullopt);
    object::zombie* add_zombie_in_row(
        object::zombie_type type,
        int row,
        int from_wave);
    object::zombie* place_zombie(
        object::zombie_type type,
        int grid_x,
        int grid_y);
    bool create_lurking(object::zombie_type type, unsigned int row, unsigned int col);

    void destroy(object::zombie& z);
};

}
