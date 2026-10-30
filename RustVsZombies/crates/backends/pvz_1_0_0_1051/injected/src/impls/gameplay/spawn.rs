use std::ptr::{self, NonNull};

use rsvz_backend_api::backend::{GameUiBackend, SpawnScheduleBackend};
use rsvz_model::model::{GameUi, SPAWN_SLOTS_PER_WAVE, SpawnWaveSlot, ZombieKind};

use crate::error::{Pvz1051Error, Result};
use crate::raw::layout as ptrs;
use crate::runtime::Pvz1051Backend;

impl SpawnScheduleBackend for Pvz1051Backend {
    fn spawn_wave_count(&self) -> Result<usize> {
        ensure_spawn_list_game_ui(self)?;
        board_wave_count(self.board()?)
    }

    fn set_spawn_type_allowed(&self, kind: ZombieKind, allowed: bool) -> Result<()> {
        ensure_spawn_list_game_ui(self)?;
        let board = self.board()?;
        // SAFETY: the array is embedded in this live Board; ZombieKind is 0..=32.
        unsafe { *ptrs::Board::zombie_type_list(board.as_ptr()).add(kind.code() as usize) = u8::from(allowed) };
        Ok(())
    }

    fn set_spawn_slot(&self, wave: usize, slot: SpawnWaveSlot, kind: Option<ZombieKind>) -> Result<()> {
        ensure_spawn_list_game_ui(self)?;
        let board = self.board()?;
        let wave_count = board_wave_count(board)?;
        if wave >= wave_count {
            return Err(Pvz1051Error::AbiPreconditionFailed(
                "spawn list wave is outside current board range",
            ));
        }
        // SAFETY: the list is embedded in Board, wave is checked, and slot is bounded by its type.
        unsafe { write_spawn_slot(ptrs::Board::zombie_list(board.as_ptr()), wave, slot, kind) };
        Ok(())
    }

    fn pick_spawn_list(&self) -> Result<()> {
        ensure_spawn_list_game_ui(self)?;
        let board = self.board()?;
        // SAFETY: the UI and board checks above establish the live board
        // required by the verified Board::PickZombieWaves wrapper.
        if let Some(stage) = crate::impls::reset::pending_world_stage() {
            // SAFETY: `board` is live and its Challenge pointer is checked before the temporary
            // stage write. The original chooser stage is restored before returning.
            let challenge = unsafe { ptrs::Board::challenge(board.as_ptr()) };
            let challenge = NonNull::new(challenge).ok_or(Pvz1051Error::NullChallenge)?;
            with_survival_stage(challenge, stage, || unsafe {
                crate::raw::abi::board_pick_zombie_waves();
            });
        } else {
            // SAFETY: the UI and board checks above establish the live board required by the
            // verified Board::PickZombieWaves wrapper.
            unsafe { crate::raw::abi::board_pick_zombie_waves() };
        }
        Ok(())
    }

    fn refresh_spawn_preview(&self) -> Result<()> {
        ensure_spawn_list_game_ui(self)?;
        let board = self.board()?;
        refresh_zombie_preview_if_level_intro(self, board)
    }
}

impl rsvz_backend_api::SpawnScheduleReadBackend for Pvz1051Backend {
    fn spawn_type_allowed(&self, kind: ZombieKind) -> Result<bool> {
        ensure_spawn_list_game_ui(self)?;
        let board = self.board()?;
        // SAFETY: bounded native enum and current embedded allowed-type array.
        Ok(unsafe { *ptrs::Board::zombie_type_list(board.as_ptr()).add(kind.code() as usize) != 0 })
    }
    fn spawn_slot(&self, wave: usize, slot: SpawnWaveSlot) -> Result<Option<ZombieKind>> {
        ensure_spawn_list_game_ui(self)?;
        let board = self.board()?;
        if wave >= board_wave_count(board)? {
            return Err(Pvz1051Error::OperationRejected("spawn wave is outside current range"));
        }
        // SAFETY: validated native wave and bounded slot, embedded 100x50 table.
        let raw = unsafe { *ptrs::Board::zombie_list(board.as_ptr()).add(wave * SPAWN_SLOTS_PER_WAVE + slot.index()) };
        if raw == u32::MAX {
            Ok(None)
        } else {
            crate::raw::kind::zombie_kind_from_raw(raw as i32).map(Some)
        }
    }
    fn spawn_huge_wave(&self, wave: usize) -> Result<bool> {
        ensure_spawn_list_game_ui(self)?;
        let board = self.board()?;
        if wave >= board_wave_count(board)? {
            return Err(Pvz1051Error::OperationRejected("spawn wave is outside current range"));
        }
        // SAFETY: verified EDX/stack/AL ABI and nonzero bounded native wave count.
        Ok(unsafe { crate::raw::abi::board_is_flag_wave(board.as_ptr(), wave as i32) != 0 })
    }
}

impl rsvz_backend_api::CurrentNaturalSpawnBackend for Pvz1051Backend {
    fn board_spawn_seed(&self) -> Result<u32> {
        let board = self.board()?;
        // SAFETY: current board, verified mBoardRandSeed scalar.
        Ok(unsafe { ptrs::Board::board_rand_seed(board.as_ptr()) as u32 })
    }
    fn generate_current_natural_spawn(&self, seed: u32) -> Result<()> {
        if let Some(host) = crate::module_link::services() {
            return crate::module_link::checked((host.natural_spawn)(seed));
        }
        use crate::patches::leases::{self, BoolPatchId};
        ensure_spawn_list_game_ui(self)?;
        let board = self.board()?;
        let count = board_wave_count(board)?;
        let app = self.app();
        // GetLevelRandSeed dereferences both the profile and Challenge in its
        // mode-specific branches; reject missing dependencies before mutation.
        if unsafe {
            ptrs::LawnApp::player_info(app.as_ptr()).is_null() || ptrs::Board::challenge(board.as_ptr()).is_null()
        } {
            return Err(Pvz1051Error::OperationRejected(
                "native level-seed dependencies are unavailable",
            ));
        }
        if BoolPatchId::SpawnCountPreserved.enabled()? {
            return Err(Pvz1051Error::OperationRejected(
                "native picker count override is already active",
            ));
        }
        // GetLevelRandSeed is wrapping addition of board seed and native
        // profile/stage/mode context. Compute the requested value without writes.
        let level_seed = unsafe { crate::raw::abi::board_level_random_seed(board.as_ptr()) }
            .wrapping_sub(unsafe { ptrs::Board::board_rand_seed(board.as_ptr()) as u32 })
            .wrapping_add(seed);
        crate::patches::random::with_picker_seed(level_seed, || {
            leases::set_bool_patch(BoolPatchId::SpawnCountPreserved, true)?;
            // SAFETY: checked 1..=100 waves; native picker sees this same count
            // and cannot change it through the temporarily bypassed setup block.
            unsafe {
                ptrs::Board::set_board_rand_seed(board.as_ptr(), seed as i32);
                for slot in 0..count * SPAWN_SLOTS_PER_WAVE {
                    ptr::write(ptrs::Board::zombie_list(board.as_ptr()).add(slot), u32::MAX);
                }
                crate::raw::abi::board_pick_zombie_waves();
            }
            crate::patches::memory::finish_restoration(
                Ok(()),
                leases::set_bool_patch(BoolPatchId::SpawnCountPreserved, false),
                "native spawn-count preservation",
            )
        })
    }
}

fn with_survival_stage<T>(challenge: NonNull<ptrs::Challenge>, stage: i32, action: impl FnOnce() -> T) -> T {
    // SAFETY: `challenge` is board-owned and non-null for the duration of the synchronous native
    // picker call. Both accesses target the objdump-verified mSurvivalStage field.
    let original = unsafe { ptrs::Challenge::endless_rounds(challenge.as_ptr()) };
    unsafe { *ptrs::Challenge::endless_rounds_mut(challenge.as_ptr()) = stage };
    let output = action();
    // SAFETY: the same checked Challenge remains live because the action cannot advance or replace
    // the world; restore the chooser-visible stage before returning to the host.
    unsafe { *ptrs::Challenge::endless_rounds_mut(challenge.as_ptr()) = original };
    output
}

fn ensure_spawn_list_game_ui(backend: &Pvz1051Backend) -> Result<()> {
    match backend.game_ui()? {
        GameUi::LevelIntro | GameUi::Playing => Ok(()),
        _ => Err(Pvz1051Error::AbiPreconditionFailed(
            "spawn list commit requires level-intro or playing UI",
        )),
    }
}

fn refresh_zombie_preview_if_level_intro(backend: &Pvz1051Backend, board: NonNull<ptrs::Board>) -> Result<()> {
    if backend.game_ui()? != GameUi::LevelIntro {
        return Ok(());
    }

    // SAFETY: the caller already validated that the current UI owns a live board. This mirrors
    // AvZ's `AUpdateZombiesPreview`: Board::RemoveCutsceneZombies clears current level-intro preview
    // zombies before the cutscene flag is reset below.
    unsafe { crate::raw::abi::board_remove_cutscene_zombies() };

    // SAFETY: `board` is the non-null current board used for the spawn-list write. The cutscene
    // pointer can be absent during level-intro construction and is checked before writing.
    let cut_scene = unsafe { ptrs::Board::cut_scene(board.as_ptr()) };
    let Some(cut_scene) = NonNull::new(cut_scene) else {
        return Ok(());
    };

    // SAFETY: `cut_scene` is non-null and board-owned. Offset 0x35 is PvZ 1051's
    // CutScene::mPlacedZombies flag; setting it false asks the game to rebuild the chooser
    // zombie preview from the newly written zombie type list.
    unsafe { ptrs::CutScene::set_zombie_preview_created(cut_scene.as_ptr(), false) };
    Ok(())
}

fn board_wave_count(board: NonNull<ptrs::Board>) -> Result<usize> {
    // SAFETY: `board` is non-null and points to the active Board.
    let raw_count = unsafe { ptrs::Board::num_waves(board.as_ptr()) };
    checked_wave_count(raw_count)
}

fn checked_wave_count(raw_count: i32) -> Result<usize> {
    let Ok(wave_count) = usize::try_from(raw_count) else {
        return Err(Pvz1051Error::AbiPreconditionFailed("board wave count is negative"));
    };
    if !(1..=ptrs::Board::SPAWN_WAVE_CAPACITY).contains(&wave_count) {
        return Err(Pvz1051Error::AbiPreconditionFailed(
            "board wave count is outside supported 1051 spawn-list range",
        ));
    }
    Ok(wave_count)
}

/// Caller supplies a live native wave list and a wave below its checked count (at most 100).
unsafe fn write_spawn_slot(list: *mut u32, wave: usize, slot: SpawnWaveSlot, kind: Option<ZombieKind>) {
    let raw = kind.map_or(u32::MAX, |kind| kind.code() as u32);
    // SAFETY: the caller checked wave, and SpawnWaveSlot bounds the column.
    unsafe { ptr::write_unaligned(list.add(wave * SPAWN_SLOTS_PER_WAVE + slot.index()), raw) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_wave_capacity_is_not_the_endless_round_length() {
        for waves in [5, 10, 20, 30, 40, 100] {
            assert_eq!(checked_wave_count(waves).unwrap(), waves as usize);
        }
        for waves in [-1, 0, 101, i32::MAX] {
            assert!(checked_wave_count(waves).is_err());
        }
        let mut list = [0_u32; 100 * SPAWN_SLOTS_PER_WAVE + 1];
        // SAFETY: wave 99 and slot 49 are the last cell of this live 100-wave array.
        unsafe {
            write_spawn_slot(
                list.as_mut_ptr(),
                99,
                SpawnWaveSlot::new(49).unwrap(),
                Some(ZombieKind::GigaGargantuar),
            )
        };
        assert_eq!(list[4999], ZombieKind::GigaGargantuar.code() as u32);
        assert_eq!(list[5000], 0);
    }

    #[test]
    fn empty_spawn_slot_uses_native_invalid_sentinel() {
        let mut list = [0_u32; SPAWN_SLOTS_PER_WAVE];
        let list = NonNull::new(list.as_mut_ptr()).expect("array pointer");

        // SAFETY: the local array covers wave 0 and the checked slot.
        unsafe { write_spawn_slot(list.as_ptr(), 0, SpawnWaveSlot::new(3).unwrap(), None) };

        // SAFETY: `list` points to the live local array and index 3 is in bounds.
        assert_eq!(unsafe { list.as_ptr().add(3).read() }, u32::MAX);
    }

    #[test]
    fn spawn_picker_temporarily_uses_the_pending_survival_stage() {
        let mut storage = [0_u8; 0x70];
        let challenge = NonNull::new(storage.as_mut_ptr().cast::<ptrs::Challenge>()).expect("challenge storage");
        // SAFETY: `storage` covers the verified mSurvivalStage offset used by the layout accessor.
        unsafe { *ptrs::Challenge::endless_rounds_mut(challenge.as_ptr()) = 0 };

        let observed = with_survival_stage(challenge, 63, || unsafe {
            ptrs::Challenge::endless_rounds(challenge.as_ptr())
        });

        assert_eq!(observed, 63);
        // SAFETY: `challenge` still points into the live local storage.
        assert_eq!(unsafe { ptrs::Challenge::endless_rounds(challenge.as_ptr()) }, 0);
    }
}
