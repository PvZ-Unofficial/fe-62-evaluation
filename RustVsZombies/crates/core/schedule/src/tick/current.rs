//! Current tick instance, independent of game lifecycle policy.

use super::TickScheduler;
use std::cell::RefCell;

std::thread_local! {
    // Const initialization avoids large array temporaries on native game-thread stacks.
    static SCHEDULER: RefCell<TickScheduler> = const { RefCell::new(TickScheduler::new()) };
}

pub fn with_scheduler<R>(f: impl FnOnce(&mut TickScheduler) -> R) -> R {
    SCHEDULER.with_borrow_mut(f)
}

#[doc(hidden)]
pub fn with_scheduler_ref<R>(f: impl FnOnce(&TickScheduler) -> R) -> R {
    SCHEDULER.with_borrow(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tick::{TickControl, TickOptions, TickTaskState};

    #[test]
    fn first_scheduler_access_fits_a_small_native_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                with_scheduler(|scheduler| {
                    assert!(scheduler.is_idle());
                    let handle = scheduler.spawn(TickOptions::any_dispatch(), |_| Ok(TickControl::Continue));
                    assert_eq!(scheduler.state(handle), TickTaskState::Running);
                    scheduler.clear_all();
                    assert_eq!(scheduler.state(handle), TickTaskState::Stopped);
                    assert!(scheduler.is_idle());
                });
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
