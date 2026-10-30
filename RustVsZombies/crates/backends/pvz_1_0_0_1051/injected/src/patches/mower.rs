//! PT ready-mower initialization. Ownership survives a failed patch restore.
use super::spec::{PatchSet, PatchSpec};
use crate::error::{Pvz1051Error, Result};
use std::cell::RefCell;

const SITES: [PatchSpec; 3] = [
    PatchSpec {
        name: "ready mower row admission",
        addr: 0x40bc98,
        off: &[0x75, 0x09],
        on: &[0xeb, 0x60],
    },
    PatchSpec {
        name: "ready mower visibility",
        addr: 0x40bd17,
        off: &[0],
        on: &[1],
    },
    PatchSpec {
        name: "ready mower initial x",
        addr: 0x458002,
        off: &[0xf8, 0x9b],
        on: &[0xfc, 0x99],
    },
];
thread_local! { static OWNED:RefCell<Option<PatchSet>>=const{RefCell::new(None)}; }

pub(crate) fn initialize(action: impl FnOnce() -> Result<()>) -> Result<()> {
    OWNED.with_borrow_mut(|owned| {
        if owned.is_some() {
            return Err(Pvz1051Error::OperationRejected("mower patch restoration is pending"));
        }
        *owned = Some(PatchSet::default());
        let patches = owned.as_mut().expect("ownership installed before native writes");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            SITES.into_iter().try_for_each(|site| patches.enable_one(site))?;
            action()
        }));
        let restored = patches.restore();
        if restored.is_ok() {
            *owned = None;
        }
        match result {
            Ok(result) => super::memory::finish_restoration(result, restored, "ready mower initialization"),
            Err(panic) => {
                let _ = super::memory::finish_restoration(Ok(()), restored, "mower unwind restoration");
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
