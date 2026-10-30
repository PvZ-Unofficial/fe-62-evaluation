//! Current ice storage and coffee operations.

use crate::logic::IntoGrid;
use crate::logic::cards::CardPlantingBackend;
pub use crate::logic::ice_filler::{IceFiller, IceFillerError, IceFillerPriority};
use crate::runtime::RuntimeResult;
use rsvz_backend_api::backend::SceneBackend;
use rsvz_current::CurrentBackend;
use rsvz_model::model::Grid;
use rsvz_schedule::tick::with_scheduler;
use rsvz_schedule::{TickControl, TickOptions, TickTaskState};

fn with_filler<T>(f: impl FnOnce(&mut IceFiller) -> T) -> T {
    with_ice_filler(|resource| f(&mut resource.core))
}

#[doc(hidden)]
pub fn register_tick_with_scheduler(scheduler: &mut rsvz_schedule::tick::TickScheduler)
where
    CurrentBackend: CardPlantingBackend + 'static,
{
    with_ice_filler(|resource| {
        if resource
            .task
            .is_some_and(|handle| scheduler.state(handle) != TickTaskState::Stopped)
        {
            return;
        }
        resource.task = Some(scheduler.spawn(
            TickOptions::playing_frame().idle_neutral().name("ice_filler"),
            |_meta| {
                tick();
                Ok(TickControl::Continue)
            },
        ));
    });
}

pub fn start<I, G>(grids: I) -> RuntimeResult<()>
where
    CurrentBackend: CardPlantingBackend + SceneBackend + 'static,
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    with_filler(|filler| filler.start(grids))?;
    with_scheduler(register_tick_with_scheduler);
    Ok(())
}

pub fn coffee() -> RuntimeResult<Option<Grid>>
where
    CurrentBackend: CardPlantingBackend,
{
    with_filler(|filler| filler.coffee()).map_err(Into::into)
}

pub fn tick()
where
    CurrentBackend: CardPlantingBackend,
{
    if let Err(error) = with_filler(|filler| filler.try_tick()) {
        crate::diagnostics::report_operation_error(error);
    }
}

pub fn reset() {
    with_ice_filler(|resource| {
        if let Some(handle) = resource.task.take() {
            with_scheduler(|scheduler| {
                scheduler.stop(handle);
            });
        }
        resource.reset_for_script();
    });
}

#[doc(hidden)]
pub fn start_prepared(grids: Vec<Grid>) -> RuntimeResult<()>
where
    CurrentBackend: CardPlantingBackend + SceneBackend,
{
    with_filler(|filler| filler.start_prepared(grids)).map_err(Into::into)
}

pub use crate::resources::IceFillerResource;
use std::cell::RefCell;

std::thread_local! {
    static ICE_FILLER: RefCell<IceFillerResource> = RefCell::new(IceFillerResource::default());
}

pub fn with_ice_filler<R>(f: impl FnOnce(&mut IceFillerResource) -> R) -> R {
    crate::resources::ensure_script_reset(&ICE_FILLER);
    ICE_FILLER.with_borrow_mut(f)
}
