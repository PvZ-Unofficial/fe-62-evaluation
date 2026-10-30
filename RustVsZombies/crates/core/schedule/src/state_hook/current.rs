//! Current state_hook instance, independent of game lifecycle policy.

use super::{StateHookCommandOutcome, StateHookHandle, StateHookRegistry};
use rsvz_backend_api::error::RuntimeError;
use std::cell::RefCell;

std::thread_local! {
    static STATE_HOOKS: RefCell<StateHookRegistry<RuntimeError>> = RefCell::new(StateHookRegistry::new());
}

pub fn with_state_hooks<R>(f: impl FnOnce(&mut StateHookRegistry<RuntimeError>) -> R) -> R {
    STATE_HOOKS.with_borrow_mut(f)
}

#[doc(hidden)]
pub fn with_state_hooks_ref<R>(f: impl FnOnce(&StateHookRegistry<RuntimeError>) -> R) -> R {
    STATE_HOOKS.with_borrow(f)
}

/// Removes a hook, including from owners dropped during thread teardown.
pub fn remove_state_hook(handle: StateHookHandle) -> StateHookCommandOutcome {
    // Task-owned plans can outlive the registry during TLS destruction (e.g.
    // closing 1051 with a completed CardRestore). A destroyed registry has no
    // remaining hook to remove. Live borrow/handle checks still apply normally.
    STATE_HOOKS
        .try_with(|registry| registry.borrow_mut().remove(handle))
        .unwrap_or(StateHookCommandOutcome::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{ModuleSlot, with_module};
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::mpsc,
    };

    struct RemoveOnDrop {
        handle: StateHookHandle,
        result: mpsc::Sender<(bool, Option<StateHookCommandOutcome>)>,
    }

    impl Drop for RemoveOnDrop {
        fn drop(&mut self) {
            let destroyed = STATE_HOOKS.try_with(|_| ()).is_err();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                with_module(ModuleSlot::Modifier, || remove_state_hook(self.handle))
            }))
            .ok();
            let _ = self.result.send((destroyed, outcome));
        }
    }

    thread_local! {
        static OWNER: RefCell<Option<RemoveOnDrop>> = const { RefCell::new(None) };
    }

    #[test]
    fn hook_removal_survives_either_tls_destruction_order() {
        for registry_drops_first in [false, true] {
            let (sender, receiver) = mpsc::channel();
            std::thread::spawn(move || {
                if registry_drops_first {
                    OWNER.with(|_| ());
                }
                let handle = with_module(ModuleSlot::Modifier, || {
                    with_state_hooks(|registry| registry.register_cleanup(0, || Ok(())))
                });
                OWNER.with_borrow_mut(|owner| *owner = Some(RemoveOnDrop { handle, result: sender }));
            })
            .join()
            .unwrap();
            assert_eq!(
                receiver.recv().unwrap(),
                (
                    registry_drops_first,
                    Some(if registry_drops_first {
                        StateHookCommandOutcome::NotFound
                    } else {
                        StateHookCommandOutcome::Applied
                    })
                )
            );
        }
    }
}
