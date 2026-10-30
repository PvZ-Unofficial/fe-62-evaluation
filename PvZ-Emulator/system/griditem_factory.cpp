#include <cassert>
#include <stdexcept>
#include "system/plant/plant_factory.h"
#include "griditem_factory.h"

namespace pvz_emulator::system {

griditem& griditem_factory::create(
    griditem_type type,
    unsigned int row,
    unsigned int col)
{
    if ((type != griditem_type::grave &&
         type != griditem_type::crater &&
         type != griditem_type::ladder) ||
        row >= scene.rows || col >= 9)
    {
        throw std::invalid_argument("invalid griditem type or position");
    }

    auto& item = scene.griditems.alloc();

    item.type = type;
    item.col = col;
    item.row = row;
    item.is_disappeared = false;

    switch (type) {
    case pvz_emulator::object::griditem_type::grave:
        for (auto& p : scene.plants) {
            if (p.row == item.row && p.col == item.col) {
                plant_factory(scene).destroy(p);
            }
        }

        item.countdown = -static_cast<int>(rng.randint(50));
        break;

    case pvz_emulator::object::griditem_type::crater:
        item.countdown = 18000;
        break;

    case pvz_emulator::object::griditem_type::ladder:
        item.countdown = 0;
        break;

    default:
        assert(false);
        break;
    }

    return item;
}

griditem* griditem_factory::add_griditem(
    griditem_type type,
    int grid_x,
    int grid_y)
{
    if (grid_x < 0 || grid_x >= 9 || !scene.row_can_have_zombies(grid_y) ||
        scene.griditems.active_count() >= scene.griditems.capacity())
    {
        return nullptr;
    }
    return &create_strict(
        type,
        static_cast<unsigned int>(grid_y),
        static_cast<unsigned int>(grid_x));
}

griditem& griditem_factory::create_strict(
    griditem_type type,
    unsigned int row,
    unsigned int col)
{
    auto& item = scene.griditems.alloc();
    item.type = type;
    item.col = col;
    item.row = row;
    item.is_disappeared = false;
    item.countdown = type == griditem_type::grave ?
        -static_cast<int>(rng.randint(50)) : 0;
    return item;
}

griditem* griditem_factory::add_ladder(int grid_x, int grid_y) {
    return add_griditem(griditem_type::ladder, grid_x, grid_y);
}

griditem* griditem_factory::add_crater(int grid_x, int grid_y) {
    return add_griditem(griditem_type::crater, grid_x, grid_y);
}

griditem* griditem_factory::add_grave_stone(int grid_x, int grid_y) {
    return add_griditem(griditem_type::grave, grid_x, grid_y);
}

}
