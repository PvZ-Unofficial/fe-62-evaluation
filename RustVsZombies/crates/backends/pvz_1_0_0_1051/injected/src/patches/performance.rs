use rsvz_model::FastForwardPerformance;
use std::sync::LazyLock;

use crate::error::Result;

use super::spec::{PatchSet, PatchSpec};

pub(crate) fn performance_patches(performance: FastForwardPerformance) -> Result<PatchSet> {
    let improve = match performance {
        FastForwardPerformance::Off => &[][..],
        FastForwardPerformance::Basic | FastForwardPerformance::Aggressive => IMPROVE_PERFORMANCE_PATCHES,
    };
    let accelerate = match performance {
        FastForwardPerformance::Aggressive => ACCELERATE_GAME_PATCHES,
        FastForwardPerformance::Off | FastForwardPerformance::Basic => &[][..],
    };
    let mut patches = PatchSet::enable(improve.iter().chain(accelerate))?;
    if performance == FastForwardPerformance::Aggressive {
        let operation = (|| {
            let on = EATING_JUMP
                .as_ref()
                .map_err(|error| crate::error::Pvz1051Error::Patch(error.to_string()))?;
            patches.enable_one(PatchSpec {
                name: "AccelerateGame::EatingCadence",
                addr: 0x52f4ca,
                off: &[0x8b, 0x47, 0x24, 0x83, 0xf8, 0x14],
                on,
            })
        })();
        if operation.is_err() {
            return super::memory::finish_restoration(operation, patches.restore(), "eating cadence").map(|()| patches);
        }
    }
    Ok(patches)
}

// Immutable installed bytes, initialized only when enabling the patch, never on a game tick.
static EATING_JUMP: LazyLock<Result<[u8; 6]>> = LazyLock::new(|| {
    let mut bytes = [0x90; 6];
    bytes[..5].copy_from_slice(&super::memory::encode_rel32(
        0xe9,
        0x52f4ca,
        eating_cadence as *const () as usize,
        "eating cadence",
    )?);
    Ok(bytes)
});

// 1051 52F4C0 receives Zombie in EAX; its prologue saves EDI and copies EAX to EDI.
// 52F640..52F65B tests signed age(+60h) % (chilled(+ACh)>0 ? 8 : 4).
// Divisibility by these powers of two is identical for negative ages too. No registers
// are clobbered before replaying MOV/CMP; both exits reuse the original stack frame.
#[unsafe(naked)]
unsafe extern "C" fn eating_cadence() {
    core::arch::naked_asm!(
        "test dword ptr [edi + 0x60], 3",
        "jnz 3f",
        "cmp dword ptr [edi + 0xac], 0",
        "jle 2f",
        "test dword ptr [edi + 0x60], 7",
        "jnz 3f",
        "2:",
        "mov eax, [edi + 0x24]",
        "cmp eax, 0x14",
        "push 0x52f4d0",
        "ret",
        "3:",
        "push 0x52f6bc",
        "ret",
    );
}

const IMPROVE_PERFORMANCE_PATCHES: &[PatchSpec] = super::spec::checked_specs(&[PatchSpec {
    name: "ImprovePerformance",
    addr: 0x6a66f4,
    off: &[0x01],
    on: &[0x00],
}]);

const ACCELERATE_GAME_PATCHES: &[PatchSpec] = super::spec::checked_specs(&[
    PatchSpec {
        // After the native prologue and mDead check, reuse DeleteNonCrossFading and
        // the forced-dead epilogue below. Skip fields, particles and UpdateSpawning.
        name: "AccelerateGame::ParticleEmitterEarlyDelete",
        addr: 0x517563,
        off: &[0x83, 0x47, 0x28, 0x01, 0x8b, 0x47, 0x28],
        on: &[0xe9, 0x0a, 0x01, 0x00, 0x00, 0x90, 0x90],
    },
    PatchSpec {
        // EBP points to this row's ice timer; EDI is its decremented value, already
        // stored by 414134. Preserve expiry (ice min X = 800), then use the native
        // next-row loop at 414246. No particle lookup/creation or x87 work remains.
        name: "AccelerateGame::IceSparkle",
        addr: 0x414137,
        off: &[
            0x8b, 0x83, 0x8c, 0, 0, 0, 0x8b, 0x80, 0x20, 0x08, 0, 0, 0x8b, 0x10, 0x75, 0x04, 0x33, 0xf6, 0xeb, 0x20,
        ],
        on: &[
            0x85, 0xff, 0x0f, 0x85, 0x07, 0x01, 0, 0, 0xc7, 0x45, 0xe8, 0x20, 0x03, 0, 0, 0xe9, 0xfb, 0, 0, 0,
        ],
    },
    PatchSpec {
        // 473AE0 PropogateColorToAttachments takes this in ECX and returns with RET.
        // Keep animation clocks, event tests, attachment transforms and updates.
        name: "AccelerateGame::AttachmentColor",
        addr: 0x473ae0,
        off: &[0x83],
        on: &[0xc3],
    },
    PatchSpec {
        name: "AccelerateGame::BoardUpdateToolTip",
        addr: 0x40ef00,
        off: &[0x6a, 0xff, 0x64],
        on: &[0xc2, 0x04, 0x00],
    },
    PatchSpec {
        // Skips cursor-preview state refresh, not only drawing, during aggressive fast-forward.
        name: "AccelerateGame::CursorPreviewUpdate",
        addr: 0x438da0,
        off: &[0x83, 0xec, 0x08],
        on: &[0xc3, 0x90, 0x90],
    },
    PatchSpec {
        name: "AccelerateGame::LawnAppEnforceCursor",
        addr: 0x455930,
        off: &[0x80],
        on: &[0xc3],
    },
    PatchSpec {
        name: "AccelerateGame::MusicUpdateMusicBurst",
        addr: 0x45b260,
        off: &[0x8b],
        on: &[0xc3],
    },
    PatchSpec {
        name: "AccelerateGame::LawnAppPlaySample",
        addr: 0x4560c0,
        off: &[0x80, 0xb9, 0xc5],
        on: &[0xc2, 0x04, 0x00],
    },
    PatchSpec {
        name: "AccelerateGame::CobCannonReadyTrackFlash",
        addr: 0x461123,
        off: &[0x8b, 0x9f],
        on: &[0xeb, 0x6d],
    },
    PatchSpec {
        name: "AccelerateGame::PlantUpdateReanimColor",
        addr: 0x4635c0,
        off: &[0x55, 0x8b, 0xec],
        on: &[0xc2, 0x04, 0x00],
    },
    PatchSpec {
        name: "AccelerateGame::PlantDrawHeightOffset",
        addr: 0x465040,
        off: &[0x55, 0x8b, 0xec],
        on: &[0xd9, 0xee, 0xc3],
    },
    PatchSpec {
        // 1051 Reanimation::Update: skip attacher__ track parsing, not attachment updates.
        name: "AccelerateGame::ReanimationUpdateAttacherTrack",
        addr: 0x471dcf,
        off: &[0x8b, 0x4e, 0x0c],
        on: &[0xeb, 0x24, 0x90],
    },
    PatchSpec {
        // Reisen lists 0x515020 twice. PvZ 1.0.0.1051 EN decomp/objdump identify it as
        // `TodFoley::PlayFoleyPitch(FoleyType, float)`; the duplicate is idempotent, so rsvz
        // keeps one immediate `ret 4` entry.
        name: "AccelerateGame::TodFoleyPlayFoleyPitch",
        addr: 0x515020,
        off: &[0x55, 0x8b, 0xec],
        on: &[0xc2, 0x04, 0x00],
    },
    PatchSpec {
        // 1051 TodParticleEmitter::Update, after UpdateSpawning: ignore the lifetime condition.
        name: "AccelerateGame::ParticleEmitterForceDeleteNonCrossFading",
        addr: 0x517670,
        off: &[0x74, 0x17],
        on: &[0x90, 0x90],
    },
    PatchSpec {
        // Finish the emitter even when cross-fading particles remain.
        name: "AccelerateGame::ParticleEmitterForceDead",
        addr: 0x51767b,
        off: &[0x75, 0x0c],
        on: &[0x90, 0x90],
    },
]);
