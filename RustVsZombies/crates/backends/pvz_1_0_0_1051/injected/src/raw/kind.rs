use rsvz_model::model::{GameUi, ItemKind, PlantKind, SceneKind, ZombieKind, ZombiePhase};

use crate::error::{Pvz1051Error, Result};

pub(crate) fn game_ui_from_raw(raw: i32) -> Result<GameUi> {
    GameUi::try_from_code(raw).map_err(|_| Pvz1051Error::UnknownRawKind {
        category: "game_ui",
        raw,
    })
}

pub(crate) fn scene_from_raw(raw: i32) -> Result<SceneKind> {
    SceneKind::try_from_code(raw).map_err(|_| Pvz1051Error::UnknownRawKind { category: "scene", raw })
}

pub(crate) fn plant_kind_from_raw(raw: i32) -> Result<PlantKind> {
    PlantKind::try_from_code(raw).map_err(|_| Pvz1051Error::UnknownRawKind { category: "plant", raw })
}

pub(crate) fn zombie_kind_from_raw(raw: i32) -> Result<ZombieKind> {
    ZombieKind::try_from_code(raw).map_err(|_| Pvz1051Error::UnknownRawKind {
        category: "zombie",
        raw,
    })
}

pub(crate) fn zombie_phase_from_raw(raw: i32) -> Result<ZombiePhase> {
    ZombiePhase::try_from_code(raw).map_err(|_error| Pvz1051Error::UnknownRawKind {
        category: "zombie_phase",
        raw,
    })
}

pub(crate) fn item_kind_from_raw(raw: i32) -> Result<ItemKind> {
    ItemKind::try_from(raw).map_err(|raw| Pvz1051Error::UnknownRawKind { category: "item", raw })
}

pub(crate) fn grid_item_kind_from_raw(raw: i32) -> Option<rsvz_model::model::GridItemKind> {
    rsvz_model::model::GridItemKind::try_from_code(raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_kind_conversions_are_explicit() {
        assert_eq!(plant_kind_from_raw(47).expect("cob"), PlantKind::CobCannon);
        assert_eq!(zombie_kind_from_raw(17).expect("digger"), ZombieKind::Digger);
        assert_eq!(
            zombie_phase_from_raw(91).expect("yeti running"),
            ZombiePhase::YetiRunning
        );
        assert!(plant_kind_from_raw(999).is_err());
        assert!(zombie_kind_from_raw(999).is_err());
        assert!(zombie_phase_from_raw(999).is_err());
        assert_eq!(item_kind_from_raw(4).expect("sun"), ItemKind::Sun);
        assert!(item_kind_from_raw(28).is_err());
        assert!(item_kind_from_raw(999).is_err());
    }

    #[test]
    fn native_zombie_phase_table_is_complete() {
        for raw in 0..=95 {
            assert!(zombie_phase_from_raw(raw).is_ok(), "missing native phase {raw}");
        }
        assert!(zombie_phase_from_raw(-1).is_err());
        assert!(zombie_phase_from_raw(96).is_err());
    }
}
