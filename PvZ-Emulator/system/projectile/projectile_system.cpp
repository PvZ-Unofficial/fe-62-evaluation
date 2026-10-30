#include <vector>
#include <algorithm>
#include "system/util.h"
#include "projectile_system.h"
#include "system/event.h"

namespace pvz_emulator::system {

using namespace pvz_emulator::object;

namespace {

bool is_fire_resistant(const zombie& z) {
    return z.type == zombie_type::catapult ||
        z.type == zombie_type::zomboni ||
        z.accessory_2.type == zombie_accessories_type_2::screen_door ||
        z.accessory_2.type == zombie_accessories_type_2::ladder;
}

}

bool projectile_system::is_in_torchwood(object::projectile& proj) {
    if (proj.motion_type != projectile_motion_type::straight ||
        proj.type != projectile_type::pea &&
        proj.type != projectile_type::snow_pea)
    {
        return false;
    }

    rect proj_rect;
    proj.get_attack_box(proj_rect);
    proj_rect.x += 40;

    for (auto& p : scene.plants) {
        if (p.row == proj.row &&
            p.type == plant_type::torchwood &&
            !p.is_smashed &&
            !p.is_dead &&
            p.edible != plant_edible_status::invisible_and_not_edible &&
            proj.last_torchwood_col != p.col)
        {
            rect pr;
            p.get_attack_box(pr);

            if (proj_rect.get_overlap_len(pr) > 10) {
                return true;
            }
        }
    }

    return false;
}

zombie* projectile_system::find_zombie_target(projectile& proj) {
    if (is_in_torchwood(proj)) {
        return nullptr;
    }

    rect proj_rect;
    proj.get_attack_box(proj_rect);

    int min_x;
    zombie* target = nullptr;

    for (auto& z : scene.zombies) {
        // Equal-x candidates cannot replace the first target in pool order.
        if (z.row != proj.row || (target && z.int_x >= min_x)) {
            continue;
        }
        rect zr;
        z.get_hit_box(zr);
        if (proj_rect.get_overlap_len(zr) < 0) {
            continue;
        }

        if (damage.can_be_attacked(z, proj.flags) &&
            (z.status != zombie_status::snorkel_swim || proj.dy1 > 45) &&
            (proj.type != projectile_type::star ||
                proj.time_since_created >= 25 ||
                proj.dx < 0 ||
                z.type != zombie_type::digger))
        {
            target = &z;
            min_x = z.int_x;
        }
    }

    return target;
}

plant* projectile_system::find_plant_target(projectile& proj) {
    rect proj_rect;
    proj.get_attack_box(proj_rect);

    plant* target = nullptr;

    for (auto& p : scene.plants) {
        if (p.row == proj.row)
        {
            rect pr;
            p.get_hit_box(pr);

            if (proj_rect.get_overlap_len(pr) > 8) {
                target = &p;
                break;
            }
        }
    }

    if (target == nullptr) {
        return nullptr;
    }

    return get_top_plant_at(scene, target->row, target->col);
}

bool projectile_system::is_covered_by_suppter(
    const projectile& proj,
    const zombie& z)
{
    rect proj_rect;
    proj.get_attack_box(proj_rect);
    if (proj.type == projectile_type::fire_pea) {
        proj_rect.width = 100;
    }

    rect zr;
    z.get_hit_box(zr);

    if (proj.type == projectile_type::fire_pea && is_fire_resistant(z))
    {
        return false;
    }

    if (proj.type == projectile_type::fire_pea && proj.row != z.row ||
        abs(static_cast<int>(proj.row) - static_cast<int>(z.row)) > 1 ||
        !damage.can_be_attacked(z, proj.flags))
    {
        return false;
    }

    return zr.get_overlap_len(proj_rect) >= 0;
}

void projectile_system::suppter_attack(projectile& proj, zombie* main_target) {
    int n = 0;
    std::vector<zombie*> targets;

    for (auto& z : scene.zombies) {
        if (is_covered_by_suppter(proj, z)) {
            targets.emplace_back(&z);

            if (&z != main_target) {
                n++;
            }
        }
    }

    int o = projectile::DAMAGE[static_cast<int>(proj.type)];
    int dmg = o / 3;
    int m = proj.type == projectile_type::fire_pea ?  o : 7 * o;

    if (n * dmg > m) {
        int limited = proj.type == projectile_type::fire_pea ?
            m / n :
            o * m / (n * dmg * 3);
        dmg = limited >= 1 ? limited : 1;
    }

    for (auto z : targets) {
        auto flags = proj.get_flags_with_zombie(*z);

        if (z == main_target) {
            damage.take(*z, o, flags);
        } else {
            damage.take(*z, dmg, flags);
        }
    }
}

void projectile_system::attack_target_zombie(projectile& proj, zombie* z) {
    if ((proj.type == projectile_type::fire_pea && z != nullptr && is_fire_resistant(*z)) ||
        (proj.type != projectile_type::melon &&
            proj.type != projectile_type::wintermelon &&
            proj.type != projectile_type::fire_pea))
    {
        if (z) {
            damage.take(
                *z,
                projectile::DAMAGE[static_cast<int>(proj.type)],
                proj.get_flags_with_zombie(*z));
        }
    } else {
        if (proj.type == projectile_type::fire_pea && z != nullptr) {
            debuff.remove_by_fire_pea(*z);
        }

        suppter_attack(proj, z);
    }

    if (proj.type == projectile_type::butter) {
        if (z != nullptr) {
            debuff.set_butter(*z);
        }
    }

    projectile_factory.destroy(proj);
}

void projectile_system::parabola_do_attack(projectile& proj, zombie* z) {
    if (proj.type == projectile_type::cob_cannon) {
        damage.take_instant_kill(
            proj.row,
            static_cast<int>(proj.x + 80),
            static_cast<int>(proj.y + 40),
            115,
            1,
            true,
            proj.flags);
        
        attack_target_zombie(proj, nullptr);
    } else {
        attack_target_zombie(proj, z);
    }
}

void projectile_system::do_parabola_motion(projectile& proj) {
    if (proj.type == projectile_type::cob_cannon && proj.dy1 < -700) {
        proj.ddy = 8;
        proj.row = proj.cannon_row;
        proj.x = proj.cannon_x;

        proj.y = static_cast<float>(get_y_by_row_and_col(
            scene.type,
            proj.cannon_row,
            std::max(0, get_col_by_x(static_cast<int>(proj.cannon_x)))));

        proj.shadow_y = proj.y + 67;
    }

    proj.ddy += proj.dddy;
    proj.x += proj.dx;
    proj.y += proj.dy2;
    proj.dy1 += proj.ddy;

    bool ddy_below_0 = proj.ddy < 0;
    if (ddy_below_0 &&
        (proj.type == projectile_type::basketball ||
        proj.type == projectile_type::cob_cannon))
    {
        return;
    }

    if (proj.time_since_created > 20) {
        if (ddy_below_0) {
            return;
        }

        float top_dy1;
        switch (proj.type) {
        case projectile_type::butter:
            top_dy1 = -32;
            break;

        case projectile_type::basketball:
            top_dy1 = 60;
            break;

        case projectile_type::melon:
        case projectile_type::wintermelon:
            top_dy1 = -35;
            break;

        case projectile_type::cabbage:
        case projectile_type::kernel:
            top_dy1 = -30;
            break;

        case projectile_type::cob_cannon:
            top_dy1 = -60;
            break;

        default:
            top_dy1 = 0;
            break;
        }

        if (scene.is_water_grid(proj.row, 0)) {
            top_dy1 += 40;
        }

        if (top_dy1 >= proj.dy1) {
            return;
        }
    }

    plant* plant_target = nullptr;
    zombie* zombie_target = nullptr;
    if (proj.type == projectile_type::basketball) {
        plant_target = find_plant_target(proj);
    } else {
        zombie_target = find_zombie_target(proj);
    }

    if (zombie_target || (proj.type == projectile_type::cob_cannon && scene.disable_cob_delay)) {
        parabola_do_attack(proj, zombie_target);
    } else if (plant_target == nullptr) {
        const float landing_threshold = proj.type == projectile_type::cob_cannon ?
            (scene.cob_fixed_delay ? -600.0f : -40.0f) : 80.0f;
        if (proj.dy1 <= landing_threshold) {
            return;
        }

        parabola_do_attack(proj, zombie_target);
    } else {
        for (auto& p : scene.plants) {
            if (p.type == plant_type::umbrella_leaf &&
                !p.is_smashed &&
                p.edible != plant_edible_status::invisible_and_not_edible &&
                !p.is_dead &&
                abs(static_cast<int>(p.col) -
                    static_cast<int>(plant_target->col)) <= 1 &&
                abs(static_cast<int>(p.row) -
                    static_cast<int>(plant_target->row)) <= 1)
            {
                if (p.status == plant_status::umbrella_leaf_shrink) {
                    projectile_factory.destroy(proj);
                } else if (p.status != plant_status::umbrella_leaf_block) {
                    damage.activate_plant(p);
                }

                return;
            }
        }

        const int damage_amount = projectile::DAMAGE[static_cast<int>(proj.type)];
        if (!scene.events.interested(event_interest::basketball)) {
            damage.take_plant(*plant_target, damage_amount);
        } else {
            const auto attempt = begin_effect(
                scene,
                plant_effect_source::basketball,
                scene.projectiles.get_id(proj),
                *plant_target,
                plant_effect::hp_damage,
                damage_amount);
            if (!attempt.suppressed) {
                damage.take_plant(*plant_target, damage_amount);
            }
            finish_effect(scene, *plant_target, attempt);
        }
        projectile_factory.destroy(proj);
    }
}

void projectile_system::others_do_attack(projectile& proj) {
    if (proj.motion_type == projectile_motion_type::puff &&
        proj.time_since_created >= 75 ||
        proj.x > 800 ||
        proj.attack_box.width + proj.x < 0)
    {
        projectile_factory.destroy(proj);
        return;
    }

    if (proj.motion_type == projectile_motion_type::cattail) {
        auto z = scene.zombies.try_to_get(static_cast<std::uint32_t>(proj.target));

        if (z != nullptr && damage.can_be_attacked(*z, proj.flags)) {
            rect proj_rect;
            proj.get_attack_box(proj_rect);

            rect zr;
            z->get_hit_box(zr);

            if (zr.get_overlap_len(proj_rect) >= 0 &&
                zr.y < proj.y &&
                zr.height + zr.y > proj.y)
            {
                attack_target_zombie(proj, z);
            }
        }

        return;
    }

    if (proj.type == projectile_type::star && (proj.y > 600 || proj.y < 40)) {
        projectile_factory.destroy(proj);
        return;
    }

    if (static_cast<int>(proj.type) > 0 && proj.type != projectile_type::star ||
        proj.shadow_y - proj.y <= 90)
    {
        if (auto target = find_zombie_target(proj)) {
            attack_target_zombie(proj, target);
        }
    }
}

void projectile_system::roof_set_disappear(projectile& proj) {
    if (proj.is_disappeared) {
        return;
    }

    auto diff = proj.shadow_y - proj.y;

    if ((proj.type == projectile_type::pea ||
        proj.type == projectile_type::snow_pea ||
        proj.type == projectile_type::fire_pea ||
        proj.type == projectile_type::cactus ||
        proj.type == projectile_type::cob_cannon) &&
        diff < 28)
    {
        attack_target_zombie(proj, nullptr);
    } else if (proj.type == projectile_type::puff && diff < 0) {
        attack_target_zombie(proj, nullptr);
    } else if (proj.type == projectile_type::star && diff < 23) {
        attack_target_zombie(proj, nullptr);
    }
}

static void get_vector_cos_and_sin(float x, float y, float& c, float& s) {
    // 1051 Normalize@0x46EE80 stores the square root as binary32 before dividing.
    const float xx = static_cast<float>(static_cast<double>(x) * x);
    const float yy = static_cast<float>(static_cast<double>(y) * y);
    const float length_squared = static_cast<float>(static_cast<double>(xx) + yy);
    const float d = static_cast<float>(std::sqrt(static_cast<double>(length_squared)));

    if (d == 0) {
        c = x;
        s = y;
    }
    else {
        c = x / d;
        s = y / d;
    }
}

void projectile_system::do_other_motion(projectile& proj) {
    switch (proj.motion_type) {
    case projectile_motion_type::left_straight:
        proj.x -= 3.329999923706055f;
        break;

    case projectile_motion_type::cattail: {
        auto z = scene.zombies.try_to_get(static_cast<std::uint32_t>(proj.target));

        // Homing needles never acquire another target. Keep live targets during
        // temporary invulnerability, but discard permanently targetless needles.
        if (z == nullptr || z->is_dead || z->has_death_status()) {
            projectile_factory.destroy(proj);
            return;
        }

        if (z != nullptr && damage.can_be_attacked(*z, proj.flags)) {
            rect zr;
            z->get_hit_box(zr);

            float a2[2];

            get_vector_cos_and_sin(
                static_cast<float>(
                    static_cast<float>(zr.x + zr.width / 2) -
                    static_cast<float>(proj.x + proj.attack_box.width / 2)),
                static_cast<float>(
                    static_cast<float>(zr.y + zr.height / 2) -
                    static_cast<float>(proj.y + proj.attack_box.height / 2)),
                a2[0],
                a2[1]);

            float res[2];
            // 0x46D9B1..0x46D9F4: age * double(0.001f), then direction * scale,
            // then add velocity, with a binary32 result at each stage.
            const float steering = static_cast<float>(
                static_cast<double>(proj.time_since_created) * 0.0010000000474974513);
            const float steer_x = static_cast<float>(static_cast<double>(a2[0]) * steering);
            const float steer_y = static_cast<float>(static_cast<double>(a2[1]) * steering);
            get_vector_cos_and_sin(
                static_cast<float>(static_cast<double>(proj.dx) + steer_x),
                static_cast<float>(static_cast<double>(proj.dy2) + steer_y),
                res[0],
                res[1]);

            proj.dx = res[0] * 2.0;
            proj.dy2 = res[1] * 2.0;
        }
        proj.y += proj.dy2;
        proj.x += proj.dx;
        proj.shadow_y += proj.dy2;

        proj.row = std::max(
            0,
            get_row_by_x_and_y(
                scene.type,
                static_cast<int>(std::max(40.0f, proj.x)),
                static_cast<int>(proj.y)));

        break;
    }

    case projectile_motion_type::starfruit:
        proj.y += proj.dy2;
        proj.x += proj.dx;
        proj.shadow_y += proj.dy2;

        if (proj.dy2 != 0) {
            proj.row = get_star_row_by_x_and_y(
                static_cast<int>(proj.x),
                static_cast<int>(proj.y));
        }

        break;

    default:
        proj.x += 3.329999923706055f;

        if (proj.motion_type == projectile_motion_type::switch_way) {
            proj.y += proj.dy2;
            proj.dy2 *= 0.9700000286102295;
            proj.shadow_y += proj.dy2;
        }

        break;
    }

    others_do_attack(proj);
    roof_set_disappear(proj);
}

void projectile_system::update() {
    for (auto& proj : scene.projectiles) {
        proj.time_since_created++;

        if (scene.is_game_over) {
            continue;
        }


        auto row = proj.row;
        float y_before = get_y_by_row_and_x(scene.type, proj.row, proj.x);

        if (proj.motion_type == projectile_motion_type::parabola) {
            do_parabola_motion(proj);
        } else {
            do_other_motion(proj);
        }

        float y_after = get_y_by_row_and_x(scene.type, row, proj.x);

        float diff = y_after - y_before;

        if (!(scene.cob_drift_fixed && proj.type == projectile_type::cob_cannon)) {
            if (proj.motion_type == projectile_motion_type::parabola) {
                proj.y += diff;
                proj.dy1 -= diff;
            }
            proj.shadow_y += diff;
        }

        proj.int_x = static_cast<int>(proj.x);
        proj.int_y = static_cast<int>(proj.dy1 + proj.y);
    }
}

}
