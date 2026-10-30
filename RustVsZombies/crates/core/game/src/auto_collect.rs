//! Automatic item collection for the selected backend.

pub use crate::resources::ItemCollectorResource;
use std::cell::RefCell;

pub use crate::logic::item_collector::{ItemCollector, ItemCollectorConfigError, ItemCollectorError};
pub use rsvz_model::AutoCollectMode;

use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::backend::{AutoCollectBackend, ClockBackend, ItemClickCollectBackend, ItemReadBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::model::ItemKind;
use rsvz_schedule::tick::with_scheduler;
use rsvz_schedule::{TickControl, TickLane, TickOptions, TickPriority, TickTaskState};

std::thread_local! {
    static COLLECTION_MODE: RefCell<crate::control::OwnedRule<AutoCollectMode>> =
        const { RefCell::new(crate::control::OwnedRule::new()) };
}

/// Declares this execution module's native collection mode.
/// Click collection remains in its configured collector task.
pub fn apply_mode(mode: AutoCollectMode) -> RuntimeResult<()>
where
    CurrentBackend: AutoCollectBackend,
{
    if CurrentBackend::AUTO_COLLECT_IS_NOOP {
        return Ok(());
    }
    crate::control::declare(&COLLECTION_MODE, mode, native_mode, |mode| {
        crate::access::with_backend(|backend| backend.set_normal_auto_collect_enabled(mode == AutoCollectMode::Normal))
            .map_err(crate::access::operation_error)
    })
}

/// Releases this source's collection declaration without declaring Off.
pub fn release_mode() -> RuntimeResult<()>
where
    CurrentBackend: AutoCollectBackend,
{
    if CurrentBackend::AUTO_COLLECT_IS_NOOP {
        return Ok(());
    }
    crate::control::release(&COLLECTION_MODE, native_mode, |mode| {
        crate::access::with_backend(|backend| backend.set_normal_auto_collect_enabled(mode == AutoCollectMode::Normal))
            .map_err(crate::access::operation_error)
    })
}

fn native_mode() -> RuntimeResult<AutoCollectMode>
where
    CurrentBackend: AutoCollectBackend,
{
    crate::access::with_backend(|backend| backend.normal_auto_collect_enabled())
        .map(|enabled| {
            if enabled {
                AutoCollectMode::Normal
            } else {
                AutoCollectMode::Off
            }
        })
        .map_err(crate::access::operation_error)
}

pub fn effective_mode() -> RuntimeResult<AutoCollectMode>
where
    CurrentBackend: AutoCollectBackend,
{
    match COLLECTION_MODE.with_borrow(|rule| rule.effective_declaration()) {
        Some(mode) => Ok(mode),
        None => native_mode(),
    }
}

fn with_collector<R>(f: impl FnOnce(&mut ItemCollector) -> R) -> R {
    with_item_collector(|resource| f(&mut resource.core))
}

pub fn set_mode(mode: AutoCollectMode) {
    with_collector(|collector| collector.set_mode(mode));
}

#[must_use]
pub fn mode() -> AutoCollectMode {
    with_collector(|collector| collector.mode())
}

pub fn normal() {
    set_mode(AutoCollectMode::Normal)
}

pub fn click() {
    set_mode(AutoCollectMode::Click)
}

pub fn off() {
    set_mode(AutoCollectMode::Off)
}

pub fn set_interval(interval: u32) -> RuntimeResult<()> {
    with_collector(|collector| collector.set_interval(interval)).map_err(runtime_error)
}

pub fn set_type_list<I>(types: I) -> RuntimeResult<()>
where
    I: IntoIterator<Item = i32>,
{
    with_collector(|collector| collector.set_type_list(types)).map_err(runtime_error)
}

pub fn set_types<I>(types: I)
where
    I: IntoIterator<Item = ItemKind>,
{
    with_collector(|collector| collector.set_types(types));
}

pub fn set_play_sound(play_sound: bool) {
    with_collector(|collector| collector.set_play_sound(play_sound));
}

pub fn set_allow_cursor_side_effects(allow: bool) {
    with_collector(|collector| collector.set_allow_cursor_side_effects(allow));
}

pub fn tick() -> RuntimeResult<()>
where
    CurrentBackend: AutoCollectBackend,
{
    let click_enabled = with_collector(|collector| collector.tick_native()).map_err(runtime_error)?;
    if click_enabled {
        return Err(RuntimeError::new(
            "click auto collect requires ItemReadBackend + ItemClickCollectBackend; call tick_click instead",
        ));
    }
    Ok(())
}

pub fn tick_click() -> RuntimeResult<()>
where
    CurrentBackend: AutoCollectBackend + ClockBackend + ItemReadBackend + ItemClickCollectBackend,
{
    with_collector(|collector| collector.tick()).map_err(runtime_error)
}

pub fn register_tick()
where
    CurrentBackend: AutoCollectBackend + 'static,
{
    if CurrentBackend::AUTO_COLLECT_IS_NOOP {
        return;
    }
    with_item_collector(|resource| {
        with_scheduler(|scheduler| {
            if resource
                .native_task
                .is_some_and(|handle| scheduler.state(handle) != TickTaskState::Stopped)
            {
                return;
            }
            resource.native_task = Some(
                scheduler.spawn(
                    TickOptions::playing_frame()
                        .lane(TickLane::After)
                        .priority(TickPriority::NORMAL)
                        .idle_neutral()
                        .name("auto_collect"),
                    |_meta| {
                        with_collector(|collector| collector.tick_native()).map_err(runtime_error)?;
                        Ok(TickControl::Continue)
                    },
                ),
            );
        });
    });
}

pub fn register_click_tick()
where
    CurrentBackend: ClockBackend + ItemReadBackend + ItemClickCollectBackend + 'static,
{
    if !CurrentBackend::ITEMS_CAN_EXIST {
        return;
    }
    with_item_collector(|resource| {
        with_scheduler(|scheduler| {
            if resource
                .click_task
                .is_some_and(|handle| scheduler.state(handle) != TickTaskState::Stopped)
            {
                return;
            }
            resource.click_task = Some(
                scheduler.spawn(
                    TickOptions::playing_frame()
                        .lane(TickLane::After)
                        .priority(TickPriority::NORMAL)
                        .idle_neutral()
                        .name("auto_collect_click"),
                    |_meta| {
                        with_collector(|collector| collector.tick_click()).map_err(runtime_error)?;
                        Ok(TickControl::Continue)
                    },
                ),
            );
        });
    });
}

pub fn reset() {
    with_item_collector(ItemCollectorResource::reset_for_script);
}

fn runtime_error(error: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::new(error.to_string())
}

std::thread_local! {
    static ITEM_COLLECTOR: RefCell<ItemCollectorResource> = RefCell::new(ItemCollectorResource::default());
}

pub fn with_item_collector<R>(f: impl FnOnce(&mut ItemCollectorResource) -> R) -> R {
    crate::resources::ensure_script_reset(&ITEM_COLLECTOR);
    ITEM_COLLECTOR.with_borrow_mut(f)
}
