//! Coin::ScoreCoin compare/write immediates, verified at 0x430A1D and 0x430A72.
use crate::error::{Pvz1051Error, Result};
use std::cell::Cell;

#[derive(Clone, Copy, Default)]
struct Ownership {
    original: Option<u32>,
    installed: Option<u32>,
}
thread_local! {
    static SUN: Cell<Ownership> = const { Cell::new(Ownership { original: None, installed: None }) };
    static MONEY: Cell<Ownership> = const { Cell::new(Ownership { original: None, installed: None }) };
}
const SUN_POINTS: [usize; 2] = [0x430a1f, 0x430a2b];
const MONEY_POINTS: [usize; 2] = [0x430a74, 0x430a7d];

fn read(points: [usize; 2]) -> Result<u32> {
    // SAFETY: these are the two verified i32 immediates in the supported image.
    let values = points.map(|point| unsafe { std::ptr::read_unaligned(point as *const i32) });
    if values[0] < 0 || values[0] != values[1] {
        return Err(Pvz1051Error::InvariantViolated(
            "pickup limit instructions disagree or are negative",
        ));
    }
    Ok(values[0] as u32)
}

fn set(points: [usize; 2], state: &'static std::thread::LocalKey<Cell<Ownership>>, value: u32) -> Result<()> {
    if value > i32::MAX as u32 {
        return Err(Pvz1051Error::AbiPreconditionFailed("pickup limit exceeds i32"));
    }
    let previous = read(points)?;
    let mut owned = state.get();
    if let Some(installed) = owned.installed
        && previous != installed
        && Some(previous) != owned.original
    {
        return Err(Pvz1051Error::InvariantViolated("pickup limit changed while owned"));
    }
    let original = owned.original.unwrap_or(previous);
    if previous != value {
        super::memory::write_bytes(points[0], &value.to_le_bytes())?;
        if let Err(error) = super::memory::write_bytes(points[1], &value.to_le_bytes()) {
            if super::memory::write_bytes(points[0], &previous.to_le_bytes()).is_err() {
                crate::runtime::hook::fail_and_request_unload();
            }
            return Err(error);
        }
    }
    owned = if value == original {
        Ownership::default()
    } else {
        Ownership {
            original: Some(original),
            installed: Some(value),
        }
    };
    state.set(owned);
    Ok(())
}

pub(crate) fn sunlight_limit() -> Result<u32> {
    read(SUN_POINTS)
}
pub(crate) fn set_sunlight_limit(value: u32) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.limit_set_sunlight_limit)(value));
    }
    set(SUN_POINTS, &SUN, value)
}
pub(crate) fn money_limit() -> Result<u32> {
    read(MONEY_POINTS)
}
pub(crate) fn set_money_limit(value: u32) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.limit_set_money_limit)(value));
    }
    set(MONEY_POINTS, &MONEY, value)
}

pub(crate) fn restore() -> Result<()> {
    let sun = SUN.get().original.map_or(Ok(()), set_sunlight_limit);
    let money = MONEY.get().original.map_or(Ok(()), set_money_limit);
    sun.and(money)
}

#[cfg(test)]
mod tests {
    use super::*;
    thread_local! { static OWNER: Cell<Ownership> = const { Cell::new(Ownership { original: None, installed: None }) }; }
    #[test]
    fn paired_limits_restore_and_refuse_mismatched_or_foreign_values() {
        let mut values = [9990_u32; 2];
        let points = [values.as_mut_ptr() as usize, unsafe { values.as_mut_ptr().add(1) }
            as usize];
        set(points, &OWNER, 20000).unwrap();
        assert_eq!(values, [20000, 20000]);
        assert!(set(points, &OWNER, u32::MAX).is_err());
        assert_eq!(values, [20000, 20000]);
        values[1] = 30000;
        assert!(set(points, &OWNER, 50000).is_err());
        values[0] = 30000;
        assert!(set(points, &OWNER, 50000).is_err());
        values.fill(20000);
        set(points, &OWNER, 9990).unwrap();
        assert_eq!(values, [9990, 9990]);
        assert!(OWNER.get().original.is_none());
    }
}
