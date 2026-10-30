#include <cassert>
#include <stdexcept>
#include "system/util.h"
#include "projectile_factory.h"

namespace pvz_emulator::system {

using namespace pvz_emulator::object;

projectile&
projectile_factory::alloc_and_init(projectile_type type, int row, int x, int y) {
    auto& p = scene.projectiles.alloc();

    p.type = type;
    p.motion_type = projectile_motion_type::straight;

    p.row = row;

    p.x = static_cast<float>(x);
    p.y = static_cast<float>(y);
    p.shadow_y = static_cast<float>(
        get_y_by_row_and_col(scene.type, row, std::max(0, get_col_by_x(x)))) + 67;

    if ((scene.type == scene_type::roof ||
        scene.type == scene_type::moon_night) &&
        x < 480)
    {
        p.shadow_y -= 12;
    }

    p.flags = 0;

    p.dx = p.dy1 = p.dy2 = p.ddy = p.dddy = 0;

    p.cannon_x = 0;
    p.cannon_row = 0;

    p.attack_box.width = p.attack_box.height = 40;
    p.time_since_created = 0;

    p.is_disappeared = false;
    p.is_visible = true;

    if (p.type == projectile_type::cob_cannon) {
        p.attack_box.width = 162;
        p.attack_box.height = 78;
    } else if (p.type == projectile_type::star) {
        p.shadow_y += 15;
    }
    
    p.int_x = static_cast<int>(p.x);
    p.int_y = static_cast<int>(p.y);

    p.last_torchwood_col = -1;

    p.target = -1;

    return p;
}

void projectile_factory::create(projectile_type type, zombie& z, plant* p) {
    if (type != projectile_type::basketball ||
        !scene.zombies.is_active(&z) ||
        (p != nullptr && !scene.plants.is_active(p)))
    {
        throw std::invalid_argument("invalid basketball source or target");
    }

    const float origin_x = z.x + 113.0f;
    const float origin_y = z.y - 44.0f;
    const int target_x = p == nullptr ?
        static_cast<int>(z.x - 300.0f) : p->x;
    const int target_y = p == nullptr ? 0 : p->y;

    auto& o = alloc_and_init(
        type,
        z.row,
        static_cast<int>(origin_x),
        static_cast<int>(origin_y));

    float range_x = origin_x - target_x - 20.0f;
    const float range_y = target_y - origin_y;
    range_x = std::max(40.0f, range_x);

    o.motion_type = projectile_motion_type::parabola;
    o.dx = -range_x / 120.0f;
    o.dy2 = 0;
    o.ddy = range_y / 120.0f - 7.0f;
    o.dddy = 0.115f;
}

projectile& projectile_factory::create(
    projectile_type type,
    unsigned int row,
    int x,
    int y)
{
    const auto raw_type = static_cast<int>(type);
    if (raw_type < static_cast<int>(projectile_type::pea) ||
        raw_type > static_cast<int>(projectile_type::butter) ||
        row >= scene.rows)
    {
        throw std::invalid_argument("invalid projectile type or row");
    }
    return alloc_and_init(type, row, x, y);
}

}
