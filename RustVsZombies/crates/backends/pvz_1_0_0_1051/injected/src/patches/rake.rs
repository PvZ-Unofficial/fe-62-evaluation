//! PT's three temporary native rake sites. Coordinate variants are fixed data;
//! failed restoration retains ownership for the physical host cleanup retry.
use super::spec::{PatchSet, PatchSpec};
use crate::error::{Pvz1051Error, Result};
use std::cell::RefCell;

static ROW_BYTES: [[u8; 7]; 6] = {
    let mut rows = [[0xba, 0, 0, 0, 0, 0x90, 0x90]; 6];
    let mut i = 0;
    while i < 6 {
        rows[i][1] = i as u8;
        i += 1;
    }
    rows
};
static COL_BYTES: [[u8; 8]; 9] = {
    let mut cols = [[0xc7, 0x44, 0x24, 0x10, 0, 0, 0, 0]; 9];
    let mut i = 0;
    while i < 9 {
        cols[i][4] = i as u8;
        i += 1;
    }
    cols
};
static ROW_SPECS: [PatchSpec; 6] = {
    let mut specs = [PatchSpec {
        name: "rake row",
        addr: 0x40bb25,
        off: &[0x83, 0x80, 0x20, 0x02, 0, 0, 0xff],
        on: &ROW_BYTES[0],
    }; 6];
    let mut i = 0;
    while i < 6 {
        specs[i].on = &ROW_BYTES[i];
        i += 1;
    }
    specs
};
static COL_SPECS: [PatchSpec; 9] = {
    let mut specs = [PatchSpec {
        name: "rake column",
        addr: 0x40ba8e,
        off: &[0xc7, 0x44, 0x24, 0x10, 7, 0, 0, 0],
        on: &COL_BYTES[0],
    }; 9];
    let mut i = 0;
    while i < 9 {
        specs[i].on = &COL_BYTES[i];
        i += 1;
    }
    specs
};
static STOCK: PatchSpec = PatchSpec {
    name: "rake stock admission",
    addr: 0x40b9e2,
    off: &[0x0f, 0x84],
    on: &[0x0f, 0x80],
};
thread_local! {static OWNED:RefCell<Option<PatchSet>>=const{RefCell::new(None)};}

pub(crate) fn with_position<T>(row: usize, col: usize, action: impl FnOnce() -> Result<T>) -> Result<T> {
    let row = ROW_SPECS.get(row).ok_or(Pvz1051Error::InvalidGrid)?;
    let col = COL_SPECS.get(col).ok_or(Pvz1051Error::InvalidGrid)?;
    OWNED.with_borrow_mut(|owned| {
        if owned.is_some() {
            return Err(Pvz1051Error::OperationRejected("rake patch restoration is pending"));
        }
        *owned = Some(PatchSet::default());
        let patches = owned.as_mut().expect("installed ownership before writes");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            [row, col, &STOCK]
                .into_iter()
                .try_for_each(|spec| patches.enable_one(*spec))?;
            action()
        }));
        let restored = patches.restore();
        if restored.is_ok() {
            *owned = None;
        }
        match result {
            Ok(result) => super::memory::finish_restoration(result, restored, "rake position override"),
            Err(panic) => {
                let _ = super::memory::finish_restoration(Ok(()), restored, "rake unwind restoration");
                std::panic::resume_unwind(panic)
            }
        }
    })
}
pub(crate) fn restore() -> Result<()> {
    OWNED.with_borrow_mut(|owned| {
        if let Some(patches) = owned.as_mut() {
            patches.restore()?;
        }
        *owned = None;
        Ok(())
    })
}
