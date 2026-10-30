use crate::error::Result;

use super::spec::{PatchSet, PatchSpec};

pub(crate) fn advanced_pause_patch() -> Result<PatchSet> {
    PatchSet::enable(ADVANCED_PAUSE_PATCHES)
}

const ADVANCED_PAUSE_PATCHES: &[PatchSpec] = super::spec::checked_specs(&[PatchSpec {
    name: "AdvancedPause::BoardUpdateSkip",
    addr: 0x41600e,
    off: &[0x8b, 0xfd],
    on: &[0xeb, 0x2a],
}]);
