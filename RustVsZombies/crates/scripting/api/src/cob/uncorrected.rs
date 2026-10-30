//! Explicitly uncorrected cannon commands. Input forms match the ordinary cannon commands.
use super::{CobBackend, CobBatchError, CobBatchReport, CobError, CobManager, IntoCobFireDrops, IntoCobTarget};
use rsvz_backend_api::{BoardReadinessBackend, ClockBackend, GameUiBackend, SceneBackend};
use rsvz_game::cob::__private::FireInput;

crate::callable::callable_api! {
    /// 不做屋顶时间修正；其余选炮、炮序、预约检查与对应普通接口一致。
    /// 支持默认/显式管理器、单个/批量落点。恢复版本仍等待冷却；try 版本返回错误，普通版本报告失败。
    pub fire_uncorrected: FireUncorrected;
    where { rsvz_current::CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static, }
    impl<C> where { (i32, C): IntoCobTarget, }
    call(row: i32, col: C) -> Option<i32> { rsvz_game::cob::fire_uncorrected(row, col) }
    impl<T> where { T: FireInput, }
    call(targets: T) -> T::Output<Option<i32>, Vec<Option<i32>>> {
        targets.dispatch(
            |target| rsvz_game::cob::__private::fire_target(None, target, false, true),
            |targets| rsvz_game::cob::fire_uncorrected_many(targets),
        )
    }
    impl<C> where { (i32, C): IntoCobTarget, }
    call(manager: &CobManager, row: i32, col: C) -> Option<i32> { rsvz_game::cob::fire_uncorrected_with_manager(manager, row, col) }
    impl<T> where { T: FireInput, }
    call(manager: &CobManager, targets: T) -> T::Output<Option<i32>, Vec<Option<i32>>> {
        targets.dispatch(
            |target| rsvz_game::cob::__private::fire_target(Some(manager), target, false, true),
            |targets| rsvz_game::cob::fire_uncorrected_many_with_manager(manager, targets),
        )
    }
}

crate::callable::callable_api! {
    /// 不做屋顶时间修正；其余选炮、炮序、预约检查与对应普通接口一致。
    /// 支持默认/显式管理器、单个/批量落点。恢复版本仍等待冷却；try 版本返回错误，普通版本报告失败。
    pub try_fire_uncorrected: TryFireUncorrected;
    where { rsvz_current::CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static, }
    impl<C> where { (i32, C): IntoCobTarget, }
    call(row: i32, col: C) -> Result<i32, rsvz_game::cob::CobError> { rsvz_game::cob::try_fire_uncorrected(row, col) }
    impl<T> where { T: FireInput, }
    call(targets: T) -> T::Output<Result<i32, CobError>, Result<CobBatchReport, CobBatchError>> {
        targets.dispatch(
            |target| rsvz_game::cob::__private::try_fire_target(None, target, false, true),
            |targets| rsvz_game::cob::try_fire_uncorrected_many(targets),
        )
    }
    impl<C> where { (i32, C): IntoCobTarget, }
    call(manager: &CobManager, row: i32, col: C) -> Result<i32, rsvz_game::cob::CobError> { rsvz_game::cob::try_fire_uncorrected_with_manager(manager, row, col) }
    impl<T> where { T: FireInput, }
    call(manager: &CobManager, targets: T) -> T::Output<Result<i32, CobError>, Result<CobBatchReport, CobBatchError>> {
        targets.dispatch(
            |target| rsvz_game::cob::__private::try_fire_target(Some(manager), target, false, true),
            |targets| rsvz_game::cob::try_fire_uncorrected_many_with_manager(manager, targets),
        )
    }
}

crate::callable::callable_api! {
    /// 不做屋顶时间修正；其余选炮、炮序、预约检查与对应普通接口一致。
    /// 支持默认/显式管理器、单个/批量落点。恢复版本仍等待冷却；try 版本返回错误，普通版本报告失败。
    pub recover_fire_uncorrected: RecoverFireUncorrected;
    where { rsvz_current::CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static, }
    impl<C> where { (i32, C): IntoCobTarget, }
    call(row: i32, col: C) -> Option<i32> { rsvz_game::cob::recover_fire_uncorrected(row, col) }
    impl<T> where { T: FireInput, }
    call(targets: T) -> T::Output<Option<i32>, Vec<Option<i32>>> {
        targets.dispatch(
            |target| rsvz_game::cob::__private::fire_target(None, target, true, true),
            |targets| rsvz_game::cob::recover_fire_uncorrected_many(targets),
        )
    }
    impl<C> where { (i32, C): IntoCobTarget, }
    call(manager: &CobManager, row: i32, col: C) -> Option<i32> { rsvz_game::cob::recover_fire_uncorrected_with_manager(manager, row, col) }
    impl<T> where { T: FireInput, }
    call(manager: &CobManager, targets: T) -> T::Output<Option<i32>, Vec<Option<i32>>> {
        targets.dispatch(
            |target| rsvz_game::cob::__private::fire_target(Some(manager), target, true, true),
            |targets| rsvz_game::cob::recover_fire_uncorrected_many_with_manager(manager, targets),
        )
    }
}

crate::callable::callable_api! {
    /// 不做屋顶时间修正；其余选炮、炮序、预约检查与对应普通接口一致。
    /// 支持默认/显式管理器、单个/批量落点。恢复版本仍等待冷却；try 版本返回错误，普通版本报告失败。
    pub try_recover_fire_uncorrected: TryRecoverFireUncorrected;
    where { rsvz_current::CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static, }
    impl<C> where { (i32, C): IntoCobTarget, }
    call(row: i32, col: C) -> Result<i32, rsvz_game::cob::CobError> { rsvz_game::cob::try_recover_fire_uncorrected(row, col) }
    impl<T> where { T: FireInput, }
    call(targets: T) -> T::Output<Result<i32, CobError>, Result<CobBatchReport, CobBatchError>> {
        targets.dispatch(
            |target| rsvz_game::cob::__private::try_fire_target(None, target, true, true),
            |targets| rsvz_game::cob::try_recover_fire_uncorrected_many(targets),
        )
    }
    impl<C> where { (i32, C): IntoCobTarget, }
    call(manager: &CobManager, row: i32, col: C) -> Result<i32, rsvz_game::cob::CobError> { rsvz_game::cob::try_recover_fire_uncorrected_with_manager(manager, row, col) }
    impl<T> where { T: FireInput, }
    call(manager: &CobManager, targets: T) -> T::Output<Result<i32, CobError>, Result<CobBatchReport, CobBatchError>> {
        targets.dispatch(
            |target| rsvz_game::cob::__private::try_fire_target(Some(manager), target, true, true),
            |targets| rsvz_game::cob::try_recover_fire_uncorrected_many_with_manager(manager, targets),
        )
    }
}

crate::callable::callable_api! {
    /// 按指定炮位立即发射，不做屋顶修正；仍检查冷却和共享预约，不推进列表炮序。
    /// 接受四个坐标参数，或一项/一批炮位与落点对；try 版本返回错误，普通版本报告失败。
    pub raw_fire_uncorrected: RawFireUncorrected;
    where { rsvz_current::CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static, }
    impl<D> where { D: IntoCobFireDrops, }
    call(drops: D) -> () { rsvz_game::cob::raw_fire_uncorrected(drops) }
    impl<C> where { (i32, i32, i32, C): IntoCobFireDrops, }
    call(cob_row: i32, cob_col: i32, row: i32, col: C) -> () {
        rsvz_game::cob::raw_fire_uncorrected_coordinates(cob_row, cob_col, row, col)
    }
}

crate::callable::callable_api! {
    /// 按指定炮位立即发射，不做屋顶修正；仍检查冷却和共享预约，不推进列表炮序。
    /// 接受四个坐标参数，或一项/一批炮位与落点对；try 版本返回错误，普通版本报告失败。
    pub try_raw_fire_uncorrected: TryRawFireUncorrected;
    where { rsvz_current::CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static, }
    impl<D> where { D: IntoCobFireDrops, }
    call(drops: D) -> Result<(), rsvz_game::cob::CobBatchError> { rsvz_game::cob::try_raw_fire_uncorrected(drops) }
    impl<C> where { (i32, i32, i32, C): IntoCobFireDrops, }
    call(cob_row: i32, cob_col: i32, row: i32, col: C) -> Result<(), rsvz_game::cob::CobBatchError> {
        rsvz_game::cob::try_raw_fire_uncorrected_coordinates(cob_row, cob_col, row, col)
    }
}
