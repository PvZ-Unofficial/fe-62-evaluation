//! Feature-owned requests within one independently linked execution module.
use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_schedule::state_hook::{StateHookHandle, with_state_hooks};
use std::{cell::RefCell, thread::LocalKey};

pub(crate) struct OwnedRule<T: Copy> {
    baseline: Option<T>,
    persistent: bool,
    declaration: Option<T>,
    fallback: Option<T>,
    cleanup: Option<StateHookHandle>,
}
impl<T: Copy> OwnedRule<T> {
    pub const fn new() -> Self {
        Self {
            baseline: None,
            persistent: false,
            declaration: None,
            fallback: None,
            cleanup: None,
        }
    }
    pub(crate) fn map_values(&mut self, mut map: impl FnMut(T) -> T) {
        self.baseline = self.baseline.map(&mut map);
        self.declaration = self.declaration.map(&mut map);
        self.fallback = self.fallback.map(&mut map);
    }
    pub(crate) fn replace_baseline(&mut self, value: T) {
        if self.baseline.is_some() {
            self.baseline = Some(value);
        }
    }
    pub(crate) fn declaration(&self) -> Option<T> {
        self.declaration
    }
    pub(crate) fn effective_declaration(&self) -> Option<T> {
        self.declaration.or(self.fallback)
    }
    fn change(
        &mut self, value: T, read: impl FnOnce() -> RuntimeResult<T>, apply: impl FnOnce(T) -> RuntimeResult<()>,
    ) -> RuntimeResult<()> {
        let baseline = match self.baseline {
            Some(value) => value,
            None => read()?,
        };
        // Repeated declarations can have native side effects even when values match.
        apply(value)?;
        self.baseline = Some(baseline);
        self.declaration = Some(value);
        Ok(())
    }
    fn set_fallback(
        &mut self, value: T, read: impl FnOnce() -> RuntimeResult<T>, apply: impl FnOnce(T) -> RuntimeResult<()>,
    ) -> RuntimeResult<()> {
        let baseline = match self.baseline {
            Some(value) => value,
            None => read()?,
        };
        if self.declaration.is_none() {
            apply(value)?;
        }
        self.baseline = Some(baseline);
        self.fallback = Some(value);
        Ok(())
    }
    fn release(
        &mut self, read: impl FnOnce() -> RuntimeResult<T>, apply: impl FnOnce(T) -> RuntimeResult<()>,
    ) -> RuntimeResult<()> {
        if self.declaration.is_none() && self.fallback.is_none() {
            return Ok(());
        }
        let baseline = match self.baseline {
            Some(value) => value,
            None => read()?,
        };
        apply(baseline)?;
        self.baseline = None;
        self.declaration = None;
        self.fallback = None;
        Ok(())
    }
    fn forget(&mut self) {
        self.baseline = None;
        self.declaration = None;
        self.fallback = None;
    }
}
pub(crate) fn release<T: Copy + 'static>(
    key: &'static LocalKey<RefCell<OwnedRule<T>>>, read: fn() -> RuntimeResult<T>, apply: fn(T) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    key.with_borrow_mut(|rule| rule.release(read, apply))
}
/// Ordinary native settings survive automatic teardown; explicit release restores.
pub(crate) fn declare_persistent<T: Copy + 'static>(
    key: &'static LocalKey<RefCell<OwnedRule<T>>>, value: T, read: fn() -> RuntimeResult<T>,
    apply: fn(T) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    key.with_borrow_mut(|rule| rule.persistent = true);
    declare(key, value, read, apply)
}
pub(crate) fn declare<T: Copy + 'static>(
    key: &'static LocalKey<RefCell<OwnedRule<T>>>, value: T, read: fn() -> RuntimeResult<T>,
    apply: fn(T) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    ensure_cleanup(key, read, apply)?;
    key.with_borrow_mut(|rule| rule.change(value, read, apply))
}
pub(crate) fn declare_fallback<T: Copy + 'static>(
    key: &'static LocalKey<RefCell<OwnedRule<T>>>, value: T, read: fn() -> RuntimeResult<T>,
    apply: fn(T) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    ensure_cleanup(key, read, apply)?;
    key.with_borrow_mut(|rule| rule.set_fallback(value, read, apply))
}
fn ensure_cleanup<T: Copy + 'static>(
    key: &'static LocalKey<RefCell<OwnedRule<T>>>, read: fn() -> RuntimeResult<T>, apply: fn(T) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    if matches!(
        crate::lifecycle::with_lifecycle(|lifecycle| lifecycle.session()),
        crate::lifecycle::SessionState::Finalizing | crate::lifecycle::SessionState::Done
    ) {
        return Err(RuntimeError::new("cannot acquire a rule while the module is stopping"));
    }
    let installed = key
        .with_borrow(|rule| rule.cleanup)
        .is_some_and(|handle| with_state_hooks(|hooks| hooks.is_registered(handle)));
    if !installed {
        let handle = crate::state_hook::register_native_cleanup(i32::MAX, move || {
            key.with_borrow_mut(|rule| {
                if rule.persistent {
                    rule.forget();
                    Ok(())
                } else {
                    rule.release(read, apply)
                }
            })
        });
        key.with_borrow_mut(|rule| rule.cleanup = Some(handle));
    }
    Ok(())
}

macro_rules! rule_ledgers {
    ($($key:ident: $ty:ty,)*) => {
        std::thread_local! {
            $(static $key: std::cell::RefCell<$crate::control::OwnedRule<$ty>> = const { std::cell::RefCell::new($crate::control::OwnedRule::new()) };)*
        }
    };
}
pub(crate) use rule_ledgers;

macro_rules! declare_native_rule {
    ($ty:ty, $value:expr, $read:ident, $write:ident, $ledger:ident) => {{ $crate::control::request_native_rule!($ty, Some($value), $read, $write, $ledger) }};
}
pub(crate) use declare_native_rule;

macro_rules! request_native_rule {
    ($ty:ty, $value:expr, $read:ident, $write:ident, $ledger:ident) => {{
        let read = || $crate::access::with_backend(|backend| backend.$read()).map_err($crate::access::operation_error);
        let write = |value| {
            $crate::access::with_backend(|backend| backend.$write(value)).map_err($crate::access::operation_error)
        };
        match $value {
            Some(value) => $crate::control::declare_persistent(&$ledger, value, read, write),
            None => $crate::control::release(&$ledger, read, write),
        }
    }};
}
pub(crate) use request_native_rule;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn declarations_keep_initial_baseline_and_repeat_native_actions() {
        let mut rule = OwnedRule::new();
        let native = Cell::new(7);
        let calls = Cell::new(0);
        for value in [10, 20, 20] {
            rule.change(
                value,
                || Ok(native.get()),
                |value| {
                    native.set(value);
                    calls.set(calls.get() + 1);
                    Ok(())
                },
            )
            .unwrap();
        }
        assert_eq!(calls.get(), 3);
        rule.release(
            || unreachable!(),
            |value| {
                native.set(value);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(native.get(), 7);
        assert!(rule.effective_declaration().is_none());
    }

    #[test]
    fn fallback_does_not_overwrite_a_declaration() {
        let mut rule = OwnedRule::new();
        rule.set_fallback(
            8,
            || Ok(7),
            |value| {
                assert_eq!(value, 8);
                Ok(())
            },
        )
        .unwrap();
        rule.change(10, || unreachable!(), |_| Ok(())).unwrap();
        rule.set_fallback(9, || unreachable!(), |_| panic!("declaration takes precedence"))
            .unwrap();
        assert_eq!(rule.effective_declaration(), Some(10));
        rule.release(
            || unreachable!(),
            |value| {
                assert_eq!(value, 7);
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn failed_write_or_release_preserves_the_previous_state() {
        let mut rule = OwnedRule::new();
        rule.change(true, || Ok(false), |_| Ok(())).unwrap();
        assert!(
            rule.change(false, || unreachable!(), |_| Err(RuntimeError::new("write failed")))
                .is_err()
        );
        assert!(
            rule.release(|| unreachable!(), |_| Err(RuntimeError::new("restore failed")))
                .is_err()
        );
        assert_eq!(rule.declaration(), Some(true));
        assert_eq!(rule.baseline, Some(false));
        rule.release(
            || unreachable!(),
            |value| {
                assert!(!value);
                Ok(())
            },
        )
        .unwrap();
        assert!(rule.baseline.is_none());
    }

    #[test]
    fn persistent_cleanup_forgets_the_request_without_a_native_write() {
        let mut rule = OwnedRule::new();
        rule.persistent = true;
        rule.change(10, || Ok(7), |_| Ok(())).unwrap();
        rule.forget();
        assert!(rule.baseline.is_none());
        rule.release(|| panic!("no native read"), |_| panic!("no native write"))
            .unwrap();
    }
}
