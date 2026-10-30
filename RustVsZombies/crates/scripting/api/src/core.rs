//! `rsvz::core` reexports operations and types from their owning core modules.
//!
//! Current operations are directly usable by scripts; input adaptation and DSL live in the script API.
//!
//! `card`/`shovel` 的独立 `row, col` 参数以及炮目标元组仍使用脚本
//! `1-based` 坐标；显式 [`model::Grid`] / [`model::CobTarget`] 遵守各自的
//! core 字段约定。更完整的模型、backend trait 与自然功能模块分别位于
//! [`model`]、[`backend`] 和各功能模块。炮的非重载入口位于 [`cob`]，
//! 与脚本同名操作保持相同错误报告和初始化语义。

pub use rsvz_game::logic::cards::{
    card, card_any, card_cd, card_slot, card_slot_any, cards, cards_any, cards_any_iter,
};
pub mod cob;
pub use rsvz_game::logic::shovel::{shovel, shovel_many, shovel_ops, shovel_target};
pub use rsvz_game::logic::zombies::set_spawn_wave;
pub use rsvz_game::modifier::remove_plant_by_id;

pub(crate) use rsvz_game::logic::cards::{card_any_prepared, card_slot_any_prepared, validate_card_selection};

pub mod backend {
    pub use rsvz_backend_api::backend::*;
    pub use rsvz_game::cob::CobBackend;
    pub use rsvz_game::logic::ContactGeometryBackend;
}

// Reexport named feature modules, not the implementation's aggregate logic tree.
pub use rsvz_game::logic::{
    active_time, blover, card_timing, cards, cleanup, contact, fast_forward, grid, ice_filler, imitator_ice,
    item_collector, plant_fixer, shovel, smart_fodder, smart_remove, tactics, waves, zombie_geometry, zombie_motion,
    zombies,
};

pub mod model {
    pub use rsvz_game::lineup::{
        self, Lineup, LineupApplyOptions, LineupBase, LineupCell, LineupParseError, LineupPlant, LineupReloadPolicy,
        PendingLineup,
    };
    pub use rsvz_game::logic::zombie_motion::{VanillaZombieMotionRules, ZombieMotionRules};
    pub use rsvz_model::{model, model::*};
}

pub mod measure {
    pub use rsvz_game::measure::*;
}

pub mod modifier {
    pub use rsvz_game::modifier::*;
}

pub mod runtime {
    pub use crate::runtime::*;
}

pub mod setup {
    pub use rsvz_game::setup::*;
}

pub mod tick {
    pub use rsvz_schedule::tick::*;
}

pub mod timeline {
    pub use rsvz_schedule::timeline::*;
}

pub use backend::Backend;
pub use model::{CardSelection, Grid, KeyCode, PixelPos, PlantKind, Position, ZombieKind};

pub use rsvz_game::timing;

pub use rsvz_game::board;
pub use rsvz_game::chooser;
pub use rsvz_game::navigation;
pub use rsvz_game::window_control;
pub use rsvz_game::{game_pause, game_speed};

pub use rsvz_game::caption;
pub use rsvz_game::dance;
pub use rsvz_game::level_finish;
pub use rsvz_game::lineup;
pub use rsvz_game::placement;
pub use rsvz_game::plant_slots;
pub use rsvz_game::seed_edit;
pub use rsvz_game::seed_visibility;
pub use rsvz_game::single_step;
pub use rsvz_game::spawn_distribution;
pub use rsvz_game::spawn_rows;
pub use rsvz_game::spawn_update;
pub use rsvz_game::warnings;

pub use rsvz_game::mowers;

pub use rsvz_game::assist_control;
pub use rsvz_game::replay;
pub use rsvz_game::saved_game;
pub use rsvz_game::scene;
pub use rsvz_game::{six_row, smart_card, user_card};

pub use rsvz_game::board_click;
pub use rsvz_game::screen_buttons;
