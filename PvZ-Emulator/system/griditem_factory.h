#pragma once
#include "object/scene.h"
#include "object/griditem.h"
#include "system/rng.h"

namespace pvz_emulator::system {

using namespace pvz_emulator::object;

class griditem_factory {
private:
    object::scene& scene;
    system::rng rng;
    griditem& create_strict(griditem_type type, unsigned int row, unsigned int col);
    griditem* add_griditem(griditem_type type, int grid_x, int grid_y);

public:
    griditem_factory(object::scene& s) : scene(s), rng(s) {}

    griditem& create(griditem_type type, unsigned int row, unsigned int col);

    griditem* add_ladder(int grid_x, int grid_y);
    griditem* add_crater(int grid_x, int grid_y);
    griditem* add_grave_stone(int grid_x, int grid_y);

    void destroy(object::griditem& item) {
        item.is_disappeared = true;
    }
};

}
