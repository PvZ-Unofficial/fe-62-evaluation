//! Current plant repair and configuration.

pub use crate::logic::plant_fixer::{PlantFixer, PlantFixerBackend, PlantFixerError, PlantFixerGridError};

use crate::logic::IntoGrid;
use crate::runtime::RuntimeResult;
use rsvz_backend_api::backend::PlantReadBackend;
use rsvz_current::CurrentBackend;
use rsvz_model::model::PlantKind;
use rsvz_schedule::tick::with_scheduler;
use rsvz_schedule::{TickControl, TickOptions, TickTaskState};

fn with_fixer<T>(f: impl FnOnce(&mut PlantFixer) -> T) -> T {
    with_plant_fixer(|resource| f(&mut resource.core))
}

fn register_tick()
where
    CurrentBackend: PlantFixerBackend + 'static,
{
    with_plant_fixer(|resource| {
        with_scheduler(|scheduler| {
            if resource
                .task
                .is_some_and(|handle| scheduler.state(handle) != TickTaskState::Stopped)
            {
                return;
            }
            resource.task = Some(scheduler.spawn(
                TickOptions::playing_frame().idle_neutral().name("plant_fixer"),
                |_meta| {
                    tick();
                    Ok(TickControl::Continue)
                },
            ));
        });
    });
}

pub fn start_plant_fixer<I, G>(kind: PlantKind, grids: I) -> RuntimeResult<()>
where
    CurrentBackend: PlantFixerBackend + 'static,
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    with_fixer(|fixer| fixer.start(kind, grids))?;
    register_tick();
    Ok(())
}

pub fn start_with<I, G>(kind: PlantKind, grids: I, fix_threshold: f32, use_imitator: bool) -> RuntimeResult<()>
where
    CurrentBackend: PlantFixerBackend + 'static,
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    with_fixer(|fixer| fixer.start_with(kind, grids, fix_threshold, use_imitator))?;
    register_tick();
    Ok(())
}

pub fn set_plant_fixer_hp(hp: i32) {
    with_fixer(|fixer| fixer.set_hp(hp));
}

pub fn set_check_cards(enabled: bool) {
    with_fixer(|fixer| fixer.set_check_cards(enabled));
}

pub fn set_not_interrupt(enabled: bool) {
    with_fixer(|fixer| fixer.set_not_interrupt(enabled));
}

pub fn set_skip_covered_bottom(enabled: bool) {
    with_fixer(|fixer| fixer.set_skip_covered_bottom(enabled));
}

pub fn set_sun_threshold(sun: u32) {
    with_fixer(|fixer| fixer.set_sun_threshold(sun));
}

pub fn set_run_interval(interval: u32) -> RuntimeResult<()> {
    with_fixer(|fixer| fixer.set_run_interval(interval)).map_err(Into::into)
}

pub fn set_list<I, G>(grids: I) -> RuntimeResult<()>
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    with_fixer(|fixer| fixer.set_list(grids)).map_err(Into::into)
}

pub fn set_hp_threshold(hp: u32) {
    with_fixer(|fixer| fixer.set_hp_threshold(hp));
}

pub fn auto_set_list()
where
    CurrentBackend: PlantReadBackend,
{
    with_fixer(|fixer| fixer.auto_set_list())
}

pub fn erase_from_list<I, G>(grids: I) -> RuntimeResult<()>
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    with_fixer(|fixer| fixer.erase_from_list(grids)).map_err(Into::into)
}

pub fn move_to_list_top<I, G>(grids: I) -> RuntimeResult<()>
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    with_fixer(|fixer| fixer.move_to_list_top(grids)).map_err(Into::into)
}

pub fn move_to_list_bottom<I, G>(grids: I) -> RuntimeResult<()>
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    with_fixer(|fixer| fixer.move_to_list_bottom(grids)).map_err(Into::into)
}

pub fn set_use_coffee(enabled: bool) {
    with_fixer(|fixer| fixer.set_use_coffee(enabled));
}

pub fn tick()
where
    CurrentBackend: PlantFixerBackend,
{
    if let Err(error) = with_fixer(|fixer| fixer.try_tick()) {
        crate::diagnostics::report_operation_error(PlantFixerError::Rejected(error));
    }
}

pub fn reset() {
    with_plant_fixer(|resource| {
        if let Some(handle) = resource.task.take() {
            with_scheduler(|scheduler| {
                scheduler.stop(handle);
            });
        }
        resource.reset_for_script();
    });
}

pub use crate::resources::PlantFixerResource;
use std::cell::RefCell;

std::thread_local! {
    static PLANT_FIXER: RefCell<PlantFixerResource> = RefCell::new(PlantFixerResource::default());
}

pub fn with_plant_fixer<R>(f: impl FnOnce(&mut PlantFixerResource) -> R) -> R {
    crate::resources::ensure_script_reset(&PLANT_FIXER);
    PLANT_FIXER.with_borrow_mut(f)
}
