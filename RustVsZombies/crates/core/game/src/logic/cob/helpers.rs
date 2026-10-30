use crate::backend::{CobFireBackend, GridGeometryBackend, PlantReadBackend};
use crate::model::{CobTarget, PixelPos};
use crate::model::{PlantId, PlantKind};
use rsvz_current::CurrentBackend;

use crate::plant::find_plant_at_kind;

/// 查找锚点为 `grid` 的存活玉米加农炮。
///
/// 元组使用 1-based 坐标，Grid 使用 0-based 坐标；无效元组或没有匹配炮返回 None。
pub fn find_cob_at<G: crate::logic::IntoGrid>(grid: G) -> Option<PlantId>
where
    CurrentBackend: PlantReadBackend,
{
    find_plant_at_kind(grid, PlantKind::CobCannon)
}

fn cob_recover_time_handle<'a>(
    backend: &'a CurrentBackend, plant: <CurrentBackend as PlantReadBackend>::PlantHandle<'a>,
) -> i32
where
    CurrentBackend: PlantReadBackend,
{
    match backend.plant_state(plant) {
        35 => 125 + backend.plant_state_countdown(plant),
        36 => crate::live_value::read_or_abort(backend.plant_reanim_circulation(plant), "plant_reanim_circulation")
            .map_or(-1, |progress| (125.0 * (1.0 - progress) + 0.5) as i32 + 1),
        37 => 0,
        38 => crate::live_value::read_or_abort(backend.plant_reanim_circulation(plant), "plant_reanim_circulation")
            .map_or(-1, |progress| 3125 + (350.0 * (1.0 - progress) + 0.5) as i32),
        _ => -1,
    }
}

/// 根据 P01/P04/P07 借用事实计算炮恢复时间。
pub fn cob_recover_time(cob: PlantId) -> i32
where
    CurrentBackend: PlantReadBackend,
{
    crate::access::with_backend(|backend| {
        let Some(plant) = crate::live_value::read_or_abort(backend.plant(cob), "plant") else {
            return -1;
        };
        if crate::live_value::read_or_abort(backend.plant_kind(plant), "plant_kind") != PlantKind::CobCannon {
            return -1;
        }
        cob_recover_time_handle(backend, plant)
    })
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum CobFireAttempt {
    Fired,
    NotReady(i32),
    Unavailable,
    Rejected(crate::runtime::RuntimeError),
}

pub(super) fn fire_cob_attempt(
    cob: PlantId, target: PixelPos,
) -> Result<CobFireAttempt, rsvz_current::CurrentBackendError>
where
    rsvz_current::CurrentBackend: CobFireBackend,
{
    crate::access::with_backend(|backend| {
        let Some(plant) = crate::live_value::read_or_abort(backend.plant(cob), "plant") else {
            return Ok(CobFireAttempt::Unavailable);
        };
        if crate::live_value::read_or_abort(backend.plant_kind(plant), "plant_kind") != PlantKind::CobCannon {
            return Ok(CobFireAttempt::Unavailable);
        }
        match cob_recover_time_handle(backend, plant) {
            ..=-1 => Ok(CobFireAttempt::Unavailable),
            0 => {
                match backend.fire_cob(plant, target) {
                    Ok(()) => Ok(CobFireAttempt::Fired),
                    Err(error) => {
                        // A user-held cannon is an ordinary refusal. True access/ABI faults
                        // still abort here; managers must receive a normal outcome otherwise.
                        let rejection = crate::access::rejected_error(error);
                        Ok(CobFireAttempt::Rejected(crate::runtime::RuntimeError::from_source(
                            rejection,
                        )))
                    }
                }
            }
            remaining @ 1.. => Ok(CobFireAttempt::NotReady(remaining)),
        }
    })
}

/// 把 `0-based` 行与脚本浮点落点列转换为 backend 发炮原语使用的、已夹紧
/// PvZ 像素落点。
pub fn cob_target_to_pixel(target: CobTarget) -> Result<PixelPos, rsvz_current::CurrentBackendError>
where
    rsvz_current::CurrentBackend: GridGeometryBackend,
{
    crate::access::with_backend(|backend| {
        // AvZ permits a right-edge pseudo-column 10, but PvZ's safe grid geometry
        // only has columns 1..=9. Both columns share the same vertical baseline.
        let target_col = ((target.drop_col + 0.5) as i32).clamp(1, 9);
        let x = ((target.drop_col * 80.0 + 1.0e-3) as i32).clamp(0, 799);
        let y =
            (crate::live_value::read_or_abort(backend.grid_to_pixel_y(target_col - 1, target.row), "grid_to_pixel_y")
                + 40)
                .clamp(0, 599);
        Ok(PixelPos { x, y })
    })
}

/// 当 `cob` 存活、确为玉米加农炮且已经恢复时向 `target` 发炮。
///
/// 这是立即执行的像素原语：不选择管理器、不检查延迟预约、不推进炮序，
/// 不记录最近发炮，也不做屋顶时间修正。普通脚本应使用管理器发炮入口。
/// 植物不存在、类型不对、未恢复或动作被拒绝时返回 false；必要读取/访问
/// 故障可能中止当前回调，不能承诺全部由返回的 Err 表达。
pub fn fire_cob(cob: PlantId, target: PixelPos) -> Result<bool, rsvz_current::CurrentBackendError>
where
    rsvz_current::CurrentBackend: CobFireBackend,
{
    fire_cob_attempt(cob, target).map(|attempt| attempt == CobFireAttempt::Fired)
}
