use std::ptr::NonNull;

use rsvz_backend_api::backend::{
    GameUiBackend, PlantCreateBackend, PlantHealthWriteBackend, PlantIdleAnimationBackend, PlantPlacementBackend,
    PlantPoolBackend, PlantReadBackend, PlantRemoveBackend, PlantSleepBackend, PlantStateCountdownWriteBackend,
    PlantStateWriteBackend, pool_has_free_slot,
};
use rsvz_model::model::{
    CheckedCardSelection, GameUi, Grid, ObjectEditOutcome, Plantability, PositiveFiniteF32, PositiveHp,
};

use crate::error::{Pvz1051Error, Result};
use crate::impls::handles::PvzPlantHandle;
use crate::raw::layout as ptrs;
use crate::runtime::Pvz1051Backend;

fn created_plant(plant: *mut ptrs::Plant, symbol: &'static str) -> Result<NonNull<ptrs::Plant>> {
    NonNull::new(plant).ok_or(Pvz1051Error::AbiPreconditionFailed(symbol))
}

fn plant_pool(board: NonNull<ptrs::Board>) -> NonNull<ptrs::DataArray<ptrs::Plant>> {
    // SAFETY: `board` is current and the plant DataArray is embedded at Board+0xac.
    unsafe { NonNull::new_unchecked(ptrs::Board::plants(board.as_ptr())) }
}

impl Pvz1051Backend {
    fn plant_create_board(&self, grid: Grid) -> Result<NonNull<ptrs::Board>> {
        match self.game_ui()? {
            GameUi::LevelIntro | GameUi::Playing => {}
            _ => {
                return Err(Pvz1051Error::UnsupportedToolMode(
                    "plant creation requires level-intro or playing UI",
                ));
            }
        }
        let board = self.board()?;
        self.validate_grid(grid)?;
        let plants = plant_pool(board);
        // SAFETY: `plants` points to the current embedded DataArray; the scalar header fields are
        // the source-level DataArray allocation bounds.
        let has_slot = unsafe {
            pool_has_free_slot(
                ptrs::DataArray::active_count(plants.as_ptr()),
                ptrs::DataArray::max_size(plants.as_ptr()),
            )
        };
        if !has_slot {
            return Err(Pvz1051Error::AbiPreconditionFailed("plant pool is full"));
        }
        Ok(board)
    }
}

impl PlantPoolBackend for Pvz1051Backend {
    fn plant_pool_next_slot(&self) -> Result<u32> {
        let board = self.board()?;
        // SAFETY: allocator header of the live borrowed plant pool.
        Ok(unsafe { ptrs::DataArray::free_list_head(ptrs::Board::plants(board.as_ptr())) })
    }

    fn plant_pool_active_count(&self) -> Result<u32> {
        Ok({
            let board = self.board()?;
            // SAFETY: the plant DataArray is embedded in this borrowed Board.
            unsafe { ptrs::DataArray::active_count(ptrs::Board::plants(board.as_ptr())) }
        })
    }

    fn plant_pool_capacity(&self) -> Result<u32> {
        Ok({
            let board = self.board()?;
            // SAFETY: the plant DataArray is embedded in this borrowed Board.
            unsafe { ptrs::DataArray::max_size(ptrs::Board::plants(board.as_ptr())) }
        })
    }
}

impl PlantPlacementBackend for Pvz1051Backend {
    fn is_column_challenge(&self) -> Result<bool> {
        // SAFETY: backend token owns a live App; COLUMN is native mode 0x1b.
        Ok(unsafe { ptrs::LawnApp::game_mode(self.app().as_ptr()) } == 27)
    }
    fn is_wallnut_bowling(&self) -> Result<bool> {
        // SAFETY: live App; ECX/AL ABI verified at 0x453840.
        Ok(unsafe { crate::raw::abi::lawn_app_is_wallnut_bowling() } != 0)
    }
    fn can_plant_at(&self, selection: CheckedCardSelection, grid: Grid) -> Result<Plantability> {
        match self.game_ui()? {
            GameUi::LevelIntro | GameUi::Playing => {}
            _ => {
                return Err(Pvz1051Error::UnsupportedToolMode(
                    "plant placement query requires level-intro or playing UI",
                ));
            }
        }
        let _board = self.board()?;
        self.validate_grid(grid)?;
        let planting_type = selection.effective_kind();
        // SAFETY: the Board is current, grid bounds and the effective plant enum were checked.
        // Board::CanPlantAt is placement-only; it does not perform a sun-money check.
        let reason = unsafe { crate::raw::abi::board_can_plant_at(planting_type.code(), grid.row, grid.col) };
        Ok(Plantability::from_game_rule_code(reason))
    }
}

impl PlantCreateBackend for Pvz1051Backend {
    fn morph_imitator<'a>(&'a self, placeholder: PvzPlantHandle<'a>) -> Result<PvzPlantHandle<'a>> {
        if !self.plant_is_alive(placeholder) || self.plant_raw_kind(placeholder)? != rsvz_model::PlantKind::Imitator {
            return Err(Pvz1051Error::AbiPreconditionFailed("morph requires a live imitater"));
        }
        let board = self.plant_create_board(Grid {
            row: self.plant_row(placeholder),
            col: self.plant_col(placeholder),
        })?;
        // SAFETY: the checked current Board owns its plant pool. As in PT/PTK,
        // capture the next allocation slot before morph; Die only marks the old
        // slot, and this native call creates exactly one successor without a tick.
        let successor = unsafe { ptrs::DataArray::next_allocation(plant_pool(board).as_ptr()) }
            .ok_or(Pvz1051Error::AbiPreconditionFailed("plant pool is full"))?;
        if unsafe { ptrs::Board::challenge(board.as_ptr()) }.is_null() {
            return Err(Pvz1051Error::NullChallenge);
        }
        // SAFETY: validated live imitater, free successor slot and Challenge;
        // the ESI/no-stack ABI is verified against the original EXE.
        unsafe { crate::raw::abi::plant_imitater_morph(placeholder.ptr.as_ptr()) };
        if !unsafe { ptrs::Plant::is_alive(successor.as_ptr()) } {
            return Err(Pvz1051Error::AbiPreconditionFailed(
                "imitater morph did not create successor",
            ));
        }
        Ok(PvzPlantHandle::new(successor))
    }
    fn new_plant<'a>(&'a self, selection: CheckedCardSelection, grid: Grid) -> Result<PvzPlantHandle<'a>> {
        let _board = self.plant_create_board(grid)?;
        let plant_type = selection.packet_kind();
        let imitater_type = selection.imitator_target().map_or(-1, rsvz_model::PlantKind::code);
        // SAFETY: pool capacity, UI, grid, and native seed/imitater arguments were checked. The
        // wrapper maps Grid { col,row } to NewPlant(grid_x,grid_y,...) and returns EAX Plant*.
        let plant = unsafe { crate::raw::abi::board_new_plant(grid.row, grid.col, plant_type.code(), imitater_type) };
        Ok(PvzPlantHandle::new(created_plant(
            plant,
            "Board::NewPlant returned null",
        )?))
    }

    fn add_plant<'a>(&'a self, selection: CheckedCardSelection, grid: Grid) -> Result<PvzPlantHandle<'a>> {
        self.add_native_plant(
            grid,
            selection.packet_kind().code(),
            selection.imitator_target().map_or(-1, rsvz_model::PlantKind::code),
        )
    }
}

impl rsvz_backend_api::NativePlantPlacementBackend for Pvz1051Backend {
    fn add_placement_plant<'a>(
        &'a self, selection: rsvz_model::PlantPlacementSelection, grid: Grid,
    ) -> Result<PvzPlantHandle<'a>> {
        self.add_native_plant(grid, selection.packet(), selection.imitater())
    }
}
impl Pvz1051Backend {
    fn add_native_plant(&self, grid: Grid, packet: i32, imitater: i32) -> Result<PvzPlantHandle<'_>> {
        let board = self.plant_create_board(grid)?;
        // AddPlant calls Challenge::PlantAdded unconditionally after NewPlant.
        // SAFETY: `board` is current and its nullable Challenge owner is checked before the call.
        if unsafe { ptrs::Board::challenge(board.as_ptr()) }.is_null() {
            return Err(Pvz1051Error::NullChallenge);
        }
        // SAFETY: the same preconditions as NewPlant hold. AddPlant additionally performs native
        // planting effects and Challenge::PlantAdded, and returns its Plant* in EAX.
        let plant = unsafe { crate::raw::abi::board_add_plant(grid.row, grid.col, packet, imitater) };
        Ok(PvzPlantHandle::new(created_plant(
            plant,
            "Board::AddPlant returned null",
        )?))
    }
}

impl PlantRemoveBackend for Pvz1051Backend {
    fn shovel_plant<'a>(&'a self, handle: Self::PlantHandle<'a>) -> Result<()> {
        if !self.plant_is_alive(handle) {
            return Ok(());
        }
        let board = self.board()?;
        // SAFETY: native shovel success effects at 0x4111CE..0x4111DF; no mouse calls.
        unsafe {
            crate::raw::abi::lawn_app_play_foley(8);
            ptrs::Board::increment_plants_shoveled(board.as_ptr());
        }
        self.remove_plant(handle)?;
        // SAFETY: current Board; the tutorial field is read at 0x41123F.
        if matches!(unsafe { ptrs::Board::tutorial_state(board.as_ptr()) }, 15..=17) {
            let mut any_peashooter = false;
            for plant in self.plants()? {
                if self.plant_raw_kind(plant)? == rsvz_model::PlantKind::Peashooter {
                    any_peashooter = true;
                    break;
                }
            }
            // SAFETY: native tutorial action with checked Board. DIG also removes the pickup
            // arrow; semantic shovelling does not require physically picking up the shovel.
            unsafe {
                if ptrs::Board::tutorial_state(board.as_ptr()) == 15 {
                    crate::raw::abi::board_set_tutorial_state(board.as_ptr(), 16);
                }
                crate::raw::abi::board_set_tutorial_state(board.as_ptr(), if any_peashooter { 17 } else { 18 });
            }
        }
        Ok(())
    }
    fn remove_plant<'a>(&'a self, handle: Self::PlantHandle<'a>) -> Result<()> {
        // SAFETY: handle was constructed from a live current-frame plant DataArray slot.
        unsafe { crate::raw::abi::plant_die(handle.ptr.as_ptr()) };
        Ok(())
    }
}

impl rsvz_backend_api::IZombiePlantStyleBackend for Pvz1051Backend {
    fn setup_izombie_plant<'a>(&'a self, plant: PvzPlantHandle<'a>) -> Result<()> {
        let board = self.board()?;
        // SAFETY: current Board's required Challenge and borrow-bound live plant.
        let challenge =
            NonNull::new(unsafe { ptrs::Board::challenge(board.as_ptr()) }).ok_or(Pvz1051Error::NullChallenge)?;
        unsafe { crate::raw::abi::challenge_setup_izombie_plant(challenge.as_ptr(), plant.ptr.as_ptr()) };
        Ok(())
    }
}

impl PlantSleepBackend for Pvz1051Backend {
    fn set_plant_sleeping<'a>(&'a self, handle: Self::PlantHandle<'a>, asleep: bool) -> Result<()> {
        // SAFETY: handle is current and Plant::SetSleeping takes the native bool value on stack.
        unsafe { crate::raw::abi::plant_set_sleeping(handle.ptr.as_ptr(), i32::from(asleep)) };
        Ok(())
    }

    fn set_plant_wake_up_counter<'a>(&'a self, handle: Self::PlantHandle<'a>, counter: i32) -> Result<()> {
        // SAFETY: handle is current; this writes only Plant::mWakeUpCounter at +0x130.
        unsafe { ptrs::Plant::set_wake_up_counter(handle.ptr.as_ptr(), counter) };
        Ok(())
    }
}

impl PlantIdleAnimationBackend for Pvz1051Backend {
    fn play_plant_idle_animation<'a>(&'a self, handle: Self::PlantHandle<'a>, fps: PositiveFiniteF32) -> Result<()> {
        // SAFETY: handle is current, fps is positive and finite, and Plant::PlayIdleAnim's EDI/stack ABI was
        // objdump-verified.
        unsafe { crate::raw::abi::plant_play_idle_anim(handle.ptr.as_ptr(), fps.get()) };
        Ok(())
    }
}

impl PlantStateCountdownWriteBackend for Pvz1051Backend {
    fn set_plant_state_countdown<'a>(&'a self, handle: Self::PlantHandle<'a>, countdown: i32) -> Result<()> {
        // SAFETY: handle is current; this writes only Plant::mStateCountdown at +0x54.
        unsafe { ptrs::Plant::set_state_countdown(handle.ptr.as_ptr(), countdown) };
        Ok(())
    }
}

impl PlantStateWriteBackend for Pvz1051Backend {
    fn set_plant_state<'a>(&'a self, handle: Self::PlantHandle<'a>, state: i32) -> Result<()> {
        // SAFETY: handle is current; this writes only Plant::mState at +0x3c.
        unsafe { ptrs::Plant::set_state(handle.ptr.as_ptr(), state) };
        Ok(())
    }
}

impl PlantHealthWriteBackend for Pvz1051Backend {
    fn set_plant_hp<'a>(&'a self, handle: Self::PlantHandle<'a>, hp: PositiveHp) -> Result<ObjectEditOutcome> {
        // SAFETY: handle was constructed from a live current-frame plant DataArray slot; this writes
        // only Plant::mPlantHealth.
        unsafe { ptrs::Plant::set_health(handle.ptr.as_ptr(), hp.get()) };
        Ok(ObjectEditOutcome::Applied)
    }
}
