use crate::logic::cards::{CardPlantingBackend, PlantSeedOutcome, find_seed_slot, try_plant_seed};
use crate::model::{CardSelection, Grid, PlantKind};

use super::find_plant_at_kind;

/// 在 core `0-based` 炮位推进一次补炮状态机。
///
/// 返回 `true` 表示本次成功种下玉米加农炮；缺卡、冷却或落点暂不可用时
/// 返回 `false`。这不是“该格已经有炮”的查询，也不自行注册后续重试；
/// 普通自动补炮使用 `plant_cob`。原生动作拒绝报告后返回 `false`；
/// 必要读取或访问故障结束当前回调。
pub fn try_plant_cob(grid: Grid) -> bool
where
    rsvz_current::CurrentBackend: CardPlantingBackend,
{
    let Some(right_col) = grid.col.checked_add(1) else {
        return false;
    };
    let cob = CardSelection::Plant(PlantKind::CobCannon);
    let kernel = CardSelection::Plant(PlantKind::KernelPult);

    let Some(cob_slot) = find_seed_slot(cob) else {
        return false;
    };
    let Some(kernel_slot) = find_seed_slot(kernel) else {
        return false;
    };

    for kernel_grid in [
        grid,
        Grid {
            row: grid.row,
            col: right_col,
        },
    ] {
        if find_plant_at_kind(kernel_grid, PlantKind::KernelPult).is_some() {
            continue;
        }
        match try_plant_seed(kernel_slot, kernel_grid) {
            Ok(PlantSeedOutcome::Planted(_)) => {}
            Ok(_) => return false,
            Err(error) => {
                crate::diagnostics::report_operation_error(error);
                return false;
            }
        }
    }

    match try_plant_seed(cob_slot, grid) {
        Ok(outcome) => matches!(outcome, PlantSeedOutcome::Planted(_)),
        Err(error) => {
            crate::diagnostics::report_operation_error(error);
            false
        }
    }
}
