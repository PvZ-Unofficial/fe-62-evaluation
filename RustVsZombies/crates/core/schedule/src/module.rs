//! Execution source for the two supported native modules.
//!
//! Storage remains owned by each independently linked library. This module selects execution identity;
//! it does not own callbacks, resources, a backend, or a physical update loop.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModuleSlot {
    #[default]
    Script,
    Modifier,
}

thread_local! {
    static CURRENT: Cell<ModuleSlot> = const { Cell::new(ModuleSlot::Script) };
}

pub fn current_module() -> ModuleSlot {
    CURRENT.get()
}

/// Selects the source for one synchronous dispatch, including nested registrations.
/// Unwinding restores the caller's source. Native host entry points must establish
/// this scope before invoking module code; standalone scripts retain the default.
pub fn with_module<R>(module: ModuleSlot, body: impl FnOnce() -> R) -> R {
    struct Restore(ModuleSlot);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.set(self.0);
        }
    }
    let _restore = Restore(CURRENT.replace(module));
    body()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn nested_and_unwinding_scopes_restore_the_caller() {
        with_module(ModuleSlot::Modifier, || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_module(ModuleSlot::Script, || panic!("module callback failed"));
            }));
            assert!(result.is_err());
            assert_eq!(current_module(), ModuleSlot::Modifier);
        });
        assert_eq!(current_module(), ModuleSlot::Script);
    }
}
