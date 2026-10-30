use rsvz_model::model::{CardSelection, CheckedCardSelection, GridItemKind, PlantKind, ZombieKind};

use crate::{PeBackendError, Result};

// PE and model kinds share the native numeric codes. Contract tests below pin
// the named correspondences, sentinel rejection and unsupported kind gaps.
pub const fn plant_from_pe(kind: pe_rs::PlantType) -> Result<PlantKind> {
    match PlantKind::try_from_code(kind.to_raw()) {
        Ok(kind) => Ok(kind),
        Err(_) => Err(PeBackendError::InvalidKind {
            kind: "plant",
            raw: kind.to_raw(),
        }),
    }
}

pub const fn plant_to_pe(kind: PlantKind) -> Result<pe_rs::PlantType> {
    let unsupported = match kind {
        PlantKind::ExplodeONut => "ExplodeONut",
        PlantKind::GiantWallNut => "GiantWallNut",
        PlantKind::Seedling => "Seedling",
        PlantKind::ReverseRepeater => "ReverseRepeater",
        _ => return Ok(pe_rs::PlantType::from_raw(kind.code()).expect("PE and model plant codes must agree")),
    };
    Err(PeBackendError::unsupported_kind("plant", unsupported))
}

pub const fn zombie_from_pe(kind: pe_rs::ZombieType) -> Result<ZombieKind> {
    match ZombieKind::try_from_code(kind.to_raw()) {
        Ok(kind) => Ok(kind),
        Err(_) => Err(PeBackendError::InvalidKind {
            kind: "zombie",
            raw: kind.to_raw(),
        }),
    }
}

pub fn zombie_to_pe(kind: ZombieKind) -> Result<pe_rs::ZombieType> {
    let unsupported = match kind {
        ZombieKind::Bobsled => "Bobsled",
        ZombieKind::Boss => "Boss",
        ZombieKind::PeaHead => "PeaHead",
        ZombieKind::WallNutHead => "WallNutHead",
        ZombieKind::JalapenoHead => "JalapenoHead",
        ZombieKind::GatlingHead => "GatlingHead",
        ZombieKind::SquashHead => "SquashHead",
        ZombieKind::TallNutHead => "TallNutHead",
        _ => {
            return Ok(pe_rs::ZombieType::from_raw(kind.code()).expect("PE zombie codes must agree"));
        }
    };
    Err(PeBackendError::unsupported_kind("zombie", unsupported))
}

pub fn card_to_pe(selection: CheckedCardSelection) -> Result<(pe_rs::PlantType, pe_rs::PlantType)> {
    let packet = plant_to_pe(selection.packet_kind())?;
    let target = match selection.imitator_target() {
        Some(kind) => plant_to_pe(kind)?,
        None => pe_rs::PlantType::None,
    };
    Ok((packet, target))
}

pub fn card_from_pe(plant_type: pe_rs::PlantType, imitater_type: pe_rs::PlantType) -> Result<CheckedCardSelection> {
    let packet = plant_from_pe(plant_type)?;
    let target = if matches!(imitater_type, pe_rs::PlantType::None) {
        None
    } else {
        Some(plant_from_pe(imitater_type)?)
    };
    CardSelection::from_packet_parts(packet, target)
        .map_err(|_error| PeBackendError::InvalidCardSelection("invalid packet/imitator relation"))
}

pub fn grid_item_from_pe(kind: pe_rs::GridItemType) -> Result<GridItemKind> {
    Ok(match kind {
        pe_rs::GridItemType::Grave => GridItemKind::Grave,
        pe_rs::GridItemType::Crater => GridItemKind::Crater,
        pe_rs::GridItemType::Ladder => GridItemKind::Ladder,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pe_plant_codes_match_named_kinds_and_keep_unsupported_gaps_typed() {
        let pairs = [
            (PlantKind::Peashooter, pe_rs::PlantType::PeaShooter),
            (PlantKind::Sunflower, pe_rs::PlantType::Sunflower),
            (PlantKind::CherryBomb, pe_rs::PlantType::CherryBomb),
            (PlantKind::WallNut, pe_rs::PlantType::WallNut),
            (PlantKind::PotatoMine, pe_rs::PlantType::PotatoMine),
            (PlantKind::SnowPea, pe_rs::PlantType::SnowPea),
            (PlantKind::Chomper, pe_rs::PlantType::Chomper),
            (PlantKind::Repeater, pe_rs::PlantType::Repeater),
            (PlantKind::PuffShroom, pe_rs::PlantType::PuffShroom),
            (PlantKind::SunShroom, pe_rs::PlantType::SunShroom),
            (PlantKind::FumeShroom, pe_rs::PlantType::FumeShroom),
            (PlantKind::GraveBuster, pe_rs::PlantType::GraveBuster),
            (PlantKind::HypnoShroom, pe_rs::PlantType::HypnoShroom),
            (PlantKind::ScaredyShroom, pe_rs::PlantType::ScaredyShroom),
            (PlantKind::IceShroom, pe_rs::PlantType::IceShroom),
            (PlantKind::DoomShroom, pe_rs::PlantType::DoomShroom),
            (PlantKind::LilyPad, pe_rs::PlantType::LilyPad),
            (PlantKind::Squash, pe_rs::PlantType::Squash),
            (PlantKind::Threepeater, pe_rs::PlantType::Threepeater),
            (PlantKind::TangleKelp, pe_rs::PlantType::TangleKelp),
            (PlantKind::Jalapeno, pe_rs::PlantType::Jalapeno),
            (PlantKind::Spikeweed, pe_rs::PlantType::Spikeweed),
            (PlantKind::Torchwood, pe_rs::PlantType::Torchwood),
            (PlantKind::TallNut, pe_rs::PlantType::TallNut),
            (PlantKind::SeaShroom, pe_rs::PlantType::SeaShroom),
            (PlantKind::Plantern, pe_rs::PlantType::Plantern),
            (PlantKind::Cactus, pe_rs::PlantType::Cactus),
            (PlantKind::Blover, pe_rs::PlantType::Blover),
            (PlantKind::SplitPea, pe_rs::PlantType::SplitPea),
            (PlantKind::Starfruit, pe_rs::PlantType::Starfruit),
            (PlantKind::Pumpkin, pe_rs::PlantType::Pumpkin),
            (PlantKind::MagnetShroom, pe_rs::PlantType::MagnetShroom),
            (PlantKind::CabbagePult, pe_rs::PlantType::CabbagePult),
            (PlantKind::FlowerPot, pe_rs::PlantType::FlowerPot),
            (PlantKind::KernelPult, pe_rs::PlantType::KernelPult),
            (PlantKind::CoffeeBean, pe_rs::PlantType::CoffeeBean),
            (PlantKind::Garlic, pe_rs::PlantType::Garlic),
            (PlantKind::UmbrellaLeaf, pe_rs::PlantType::UmbrellaLeaf),
            (PlantKind::Marigold, pe_rs::PlantType::Marigold),
            (PlantKind::MelonPult, pe_rs::PlantType::MelonPult),
            (PlantKind::GatlingPea, pe_rs::PlantType::GatlingPea),
            (PlantKind::TwinSunflower, pe_rs::PlantType::TwinSunflower),
            (PlantKind::GloomShroom, pe_rs::PlantType::GloomShroom),
            (PlantKind::Cattail, pe_rs::PlantType::Cattail),
            (PlantKind::WinterMelon, pe_rs::PlantType::WinterMelon),
            (PlantKind::GoldMagnet, pe_rs::PlantType::GoldMagnet),
            (PlantKind::Spikerock, pe_rs::PlantType::Spikerock),
            (PlantKind::CobCannon, pe_rs::PlantType::CobCannon),
            (PlantKind::Imitator, pe_rs::PlantType::Imitater),
        ];
        assert_eq!(pairs.len() + 4, PlantKind::COUNT);
        for (kind, pe) in pairs {
            assert_eq!(kind.code(), pe.to_raw());
            assert!(matches!(plant_to_pe(kind), Ok(actual) if actual == pe));
            assert!(matches!(plant_from_pe(pe), Ok(actual) if actual == kind));
        }
        for kind in PlantKind::ALL {
            let unsupported_name = match kind {
                PlantKind::ExplodeONut => Some("ExplodeONut"),
                PlantKind::GiantWallNut => Some("GiantWallNut"),
                PlantKind::Seedling => Some("Seedling"),
                PlantKind::ReverseRepeater => Some("ReverseRepeater"),
                _ => None,
            };
            for selection in [CardSelection::Plant(kind), CardSelection::Imitator(kind)] {
                let Ok(selection) = selection.checked() else { continue };
                match card_to_pe(selection) {
                    Ok((packet, target)) => {
                        assert!(unsupported_name.is_none(), "{kind:?} unexpectedly supported");
                        assert_eq!(card_from_pe(packet, target).unwrap(), selection);
                    }
                    Err(PeBackendError::UnsupportedKind { kind: "plant", name }) => {
                        assert_eq!(Some(name), unsupported_name);
                    }
                    other => panic!("unexpected conversion for {kind:?}: {other:?}"),
                }
            }
        }
        assert!(matches!(
            plant_from_pe(pe_rs::PlantType::None),
            Err(PeBackendError::InvalidKind { kind: "plant", raw: -1 })
        ));
    }

    #[test]
    fn pe_zombie_codes_match_named_kinds_and_keep_unsupported_gaps_typed() {
        let pairs = [
            (ZombieKind::Normal, pe_rs::ZombieType::Normal),
            (ZombieKind::Flag, pe_rs::ZombieType::Flag),
            (ZombieKind::Conehead, pe_rs::ZombieType::Conehead),
            (ZombieKind::PoleVaulting, pe_rs::ZombieType::PoleVaulting),
            (ZombieKind::Buckethead, pe_rs::ZombieType::Buckethead),
            (ZombieKind::Newspaper, pe_rs::ZombieType::Newspaper),
            (ZombieKind::ScreenDoor, pe_rs::ZombieType::ScreenDoor),
            (ZombieKind::Football, pe_rs::ZombieType::Football),
            (ZombieKind::Dancing, pe_rs::ZombieType::Dancing),
            (ZombieKind::BackupDancer, pe_rs::ZombieType::BackupDancer),
            (ZombieKind::DuckyTube, pe_rs::ZombieType::DuckyTube),
            (ZombieKind::Snorkel, pe_rs::ZombieType::Snorkel),
            (ZombieKind::Zomboni, pe_rs::ZombieType::Zomboni),
            (ZombieKind::DolphinRider, pe_rs::ZombieType::DolphinRider),
            (ZombieKind::JackInTheBox, pe_rs::ZombieType::JackInTheBox),
            (ZombieKind::Balloon, pe_rs::ZombieType::Balloon),
            (ZombieKind::Digger, pe_rs::ZombieType::Digger),
            (ZombieKind::Pogo, pe_rs::ZombieType::Pogo),
            (ZombieKind::Yeti, pe_rs::ZombieType::Yeti),
            (ZombieKind::Bungee, pe_rs::ZombieType::Bungee),
            (ZombieKind::Ladder, pe_rs::ZombieType::Ladder),
            (ZombieKind::Catapult, pe_rs::ZombieType::Catapult),
            (ZombieKind::Gargantuar, pe_rs::ZombieType::Gargantuar),
            (ZombieKind::Imp, pe_rs::ZombieType::Imp),
            (ZombieKind::GigaGargantuar, pe_rs::ZombieType::GigaGargantuar),
        ];
        assert_eq!(pairs.len() + 8, ZombieKind::COUNT);
        for (kind, pe) in pairs {
            assert_eq!(kind.code(), pe.to_raw());
            assert!(matches!(zombie_to_pe(kind), Ok(actual) if actual == pe));
            assert!(matches!(zombie_from_pe(pe), Ok(actual) if actual == kind));
        }
        assert!(matches!(
            zombie_from_pe(pe_rs::ZombieType::None),
            Err(PeBackendError::InvalidKind {
                kind: "zombie",
                raw: -1
            })
        ));
        for kind in ZombieKind::ALL {
            let unsupported = matches!(
                kind,
                ZombieKind::Bobsled
                    | ZombieKind::Boss
                    | ZombieKind::PeaHead
                    | ZombieKind::WallNutHead
                    | ZombieKind::JalapenoHead
                    | ZombieKind::GatlingHead
                    | ZombieKind::SquashHead
                    | ZombieKind::TallNutHead
            );
            match zombie_to_pe(kind) {
                Ok(pe) => {
                    assert!(!unsupported, "{kind:?} unexpectedly became supported");
                    assert!(matches!(zombie_from_pe(pe), Ok(actual) if actual == kind));
                }
                Err(PeBackendError::UnsupportedKind { kind: "zombie", .. }) => {
                    assert!(unsupported, "{kind:?} unexpectedly became unsupported");
                }
                Err(error) => panic!("unexpected conversion error for {kind:?}: {error}"),
            }
        }
    }

    #[test]
    fn pe_grid_item_subset_does_not_invent_missing_native_kinds() {
        assert!(matches!(
            grid_item_from_pe(pe_rs::GridItemType::Grave),
            Ok(GridItemKind::Grave)
        ));
        assert!(matches!(
            grid_item_from_pe(pe_rs::GridItemType::Crater),
            Ok(GridItemKind::Crater)
        ));
        assert!(matches!(
            grid_item_from_pe(pe_rs::GridItemType::Ladder),
            Ok(GridItemKind::Ladder)
        ));
    }
}
