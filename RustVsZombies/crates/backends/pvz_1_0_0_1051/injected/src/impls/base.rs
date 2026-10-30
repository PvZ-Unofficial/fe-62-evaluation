use std::ptr::NonNull;

use rsvz_backend_api::backend::{
    Backend, BattleStatusBackend, BoardReadinessBackend, CardSelectionReadBackend, ChooserCooldownReadBackend,
    ClockBackend, CurrentWaveBackend, CursorQueryBackend, GameUiBackend, GridGeometryBackend, GridItemReadBackend,
    GridTerrainBackend, ImitatorMorphBackend, PlantReadBackend, SceneBackend, SeedBankReadBackend,
    SeedCooldownReadBackend, SunQueryBackend, ZombieReadBackend,
};
use rsvz_model::model::{
    BattleStatus, CardSelection, CheckedCardSelection, FiniteF32, GameUi, Grid, GridItemId, GridItemKind,
    MAX_SEED_SLOTS, PlantId, PlantKind, SceneKind, SeedSlot, Wave, ZombieId, ZombieKind,
};

use super::handles::{
    PvzGridItemHandle, PvzGridItemIter, PvzPlantHandle, PvzPlantIter, PvzSeedHandle, PvzSeedIter, PvzZombieHandle,
    PvzZombieIter,
};
use crate::error::{Pvz1051Error, Result};
use crate::ops::plant::effective_seed_type as plant_effective_seed_type;
use crate::raw::kind::{game_ui_from_raw, plant_kind_from_raw, scene_from_raw, zombie_kind_from_raw};
use crate::raw::layout as ptrs;
use crate::runtime::Pvz1051Backend;

impl Backend for Pvz1051Backend {
    type Error = Pvz1051Error;
}

impl GameUiBackend for Pvz1051Backend {
    fn game_ui(&self) -> Result<GameUi> {
        game_ui_from_raw(self.raw_game_ui())
    }

    fn game_ui_unavailable(&self, error: &Self::Error) -> bool {
        matches!(error, Pvz1051Error::NullLawnApp)
    }
}

impl BoardReadinessBackend for Pvz1051Backend {
    fn level_intro_board_ready(&self) -> Result<bool> {
        Pvz1051Backend::level_intro_board_ready(self)
    }

    fn playing_board_ready(&self) -> Result<bool> {
        if self.game_ui()? != GameUi::Playing {
            return Ok(false);
        }
        Ok(self.board().is_ok())
    }
}

impl SceneBackend for Pvz1051Backend {
    fn scene(&self) -> Result<SceneKind> {
        let board = self.board()?;
        // SAFETY: `board` is non-null and points to the active Board.
        scene_from_raw(unsafe { ptrs::Board::scene(board.as_ptr()) })
    }
}

impl SunQueryBackend for Pvz1051Backend {
    fn sun(&self) -> Result<u32> {
        Ok({
            let board = self.board()?;
            // SAFETY: `board` is non-null and the field is a copied scalar.
            unsafe { ptrs::Board::sun(board.as_ptr()) }.max(0) as u32
        })
    }
}

impl ClockBackend for Pvz1051Backend {
    fn clock(&self) -> Result<i32> {
        let board = self.board()?;
        // SAFETY: `board` is non-null and the field is a copied scalar.
        Ok(unsafe { ptrs::Board::clock(board.as_ptr()) })
    }
}

impl CursorQueryBackend for Pvz1051Backend {
    fn cursor_type(&self) -> Result<i32> {
        let board = self.board()?;
        // SAFETY: `board` is non-null and owns a cursor object while playing.
        let cursor = unsafe { ptrs::Board::cursor_object(board.as_ptr()) };
        let cursor = NonNull::new(cursor).ok_or(Pvz1051Error::InvariantViolated("cursor object"))?;
        // SAFETY: cursor was checked non-null and the field is a copied scalar.
        Ok(unsafe { ptrs::CursorObject::cursor_type(cursor.as_ptr()) })
    }
}

impl rsvz_backend_api::CursorSeedReadBackend for Pvz1051Backend {
    fn cursor_seed_kind(&self) -> Result<i32> {
        // SAFETY: current Board and non-null cursor, native MouseDownWithPlant +0x28.
        let cursor = unsafe { ptrs::Board::cursor_object(self.board()?.as_ptr()) };
        if cursor.is_null() {
            return Err(Pvz1051Error::OperationRejected("no cursor object"));
        }
        Ok(unsafe { ptrs::CursorObject::seed_kind(cursor) })
    }
    fn cursor_imitater_kind(&self) -> Result<i32> {
        // SAFETY: same borrowed native cursor; imitater target is +0x2C.
        let cursor = unsafe { ptrs::Board::cursor_object(self.board()?.as_ptr()) };
        if cursor.is_null() {
            return Err(Pvz1051Error::OperationRejected("no cursor object"));
        }
        Ok(unsafe { ptrs::CursorObject::imitater_kind(cursor) })
    }
}

impl CurrentWaveBackend for Pvz1051Backend {
    fn current_wave(&self) -> Result<Wave> {
        let board = self.board()?;
        // SAFETY: `board` is non-null and the field is a copied scalar.
        Ok(Wave(unsafe { ptrs::Board::current_wave(board.as_ptr()) }))
    }
}

impl ZombieReadBackend for Pvz1051Backend {
    type ZombieHandle<'a> = PvzZombieHandle<'a>;
    type ZombieIter<'a> = PvzZombieIter<'a>;

    fn zombies(&self) -> Result<Self::ZombieIter<'_>> {
        Ok({
            let board = self.board()?;
            // SAFETY: this pool is embedded in the Board held by the current shared scope.
            let array = unsafe { NonNull::new_unchecked(ptrs::Board::zombies(board.as_ptr())) };
            // SAFETY: the pool remains alive throughout the returned iterator's backend borrow.
            let max = unsafe { ptrs::DataArray::max_used_count(array.as_ptr()) };
            PvzZombieIter {
                live_only: true,
                backend: self,
                next: 0,
                max,
            }
        })
    }

    fn zombie(&self, id: ZombieId) -> Result<Option<Self::ZombieHandle<'_>>> {
        let board = self.board()?;
        Ok(resolve_live_zombie_ptr(board, id).map(PvzZombieHandle::new))
    }

    fn zombie_id<'a>(&'a self, handle: Self::ZombieHandle<'a>) -> ZombieId {
        // SAFETY: handles are created only from occupied DataArray entries.
        ZombieId::from_raw(unsafe { ptrs::DataArray::<ptrs::Zombie>::item_id(handle.ptr.as_ptr()) })
    }

    fn zombie_kind<'a>(&'a self, handle: Self::ZombieHandle<'a>) -> Result<ZombieKind> {
        // SAFETY: handle came from a verified live zombie path.
        zombie_kind_from_raw(unsafe { ptrs::Zombie::zombie_type(handle.ptr.as_ptr()) })
    }

    fn zombie_hp<'a>(&'a self, handle: Self::ZombieHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live zombie path and reads a copied scalar.
        unsafe { ptrs::Zombie::body_health(handle.ptr.as_ptr()) }
    }

    fn zombie_row<'a>(&'a self, handle: Self::ZombieHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live zombie path and reads a copied scalar.
        unsafe { ptrs::Zombie::row(handle.ptr.as_ptr()) }
    }

    fn zombie_age<'a>(&'a self, handle: Self::ZombieHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live zombie path and reads a copied scalar.
        unsafe { ptrs::Zombie::spawn_age(handle.ptr.as_ptr()) }
    }

    fn zombie_is_alive<'a>(&'a self, handle: Self::ZombieHandle<'a>) -> bool {
        // SAFETY: handle came from a verified live zombie path.
        unsafe { ptrs::Zombie::is_alive(handle.ptr.as_ptr()) }
    }
}

impl PlantReadBackend for Pvz1051Backend {
    type PlantHandle<'a> = PvzPlantHandle<'a>;
    type PlantIter<'a> = PvzPlantIter<'a>;

    fn plants(&self) -> Result<Self::PlantIter<'_>> {
        Ok({
            let board = self.board()?;
            // SAFETY: this pool is embedded in the Board held by the current shared scope.
            let array = unsafe { NonNull::new_unchecked(ptrs::Board::plants(board.as_ptr())) };
            // SAFETY: the pool remains alive throughout the returned iterator's backend borrow.
            let max = unsafe { ptrs::DataArray::max_used_count(array.as_ptr()) };
            PvzPlantIter {
                backend: self,
                next: 0,
                max,
                live_only: true,
            }
        })
    }

    fn plant(&self, id: PlantId) -> Result<Option<Self::PlantHandle<'_>>> {
        let board = self.board()?;
        Ok(resolve_live_plant_ptr(board, id).map(PvzPlantHandle::new))
    }

    fn plant_id<'a>(&'a self, handle: Self::PlantHandle<'a>) -> PlantId {
        // SAFETY: handles are created only from occupied DataArray entries.
        PlantId::from_raw(unsafe { ptrs::DataArray::<ptrs::Plant>::item_id(handle.ptr.as_ptr()) })
    }

    fn plant_kind<'a>(&'a self, handle: Self::PlantHandle<'a>) -> Result<PlantKind> {
        plant_effective_seed_type(handle.ptr)
    }

    fn plant_raw_kind<'a>(&'a self, handle: Self::PlantHandle<'a>) -> Result<PlantKind> {
        // SAFETY: handle came from a verified live plant path and reads the packet kind scalar.
        plant_kind_from_raw(unsafe { ptrs::Plant::seed_type(handle.ptr.as_ptr()) })
    }

    fn plant_x<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live plant path and reads a copied scalar.
        unsafe { ptrs::Plant::x(handle.ptr.as_ptr()) }
    }

    fn plant_state<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live plant path and reads a copied scalar.
        unsafe { ptrs::Plant::state(handle.ptr.as_ptr()) }
    }

    fn plant_is_squished<'a>(&'a self, handle: Self::PlantHandle<'a>) -> bool {
        // SAFETY: handle came from a verified live plant path and reads a copied flag.
        unsafe { ptrs::Plant::is_squished(handle.ptr.as_ptr()) }
    }

    fn plant_on_bungee_state<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live plant path and reads a copied scalar.
        unsafe { ptrs::Plant::on_bungee_state(handle.ptr.as_ptr()) }
    }

    fn plant_is_sleeping<'a>(&'a self, handle: Self::PlantHandle<'a>) -> bool {
        // SAFETY: handle came from a verified live plant path and reads a copied flag.
        unsafe { ptrs::Plant::is_asleep(handle.ptr.as_ptr()) }
    }

    fn plant_wake_up_counter<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live plant path and reads a copied scalar.
        unsafe { ptrs::Plant::wake_up_counter(handle.ptr.as_ptr()) }
    }

    fn plant_state_countdown<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live plant path and reads mStateCountdown at +0x54.
        unsafe { ptrs::Plant::state_countdown(handle.ptr.as_ptr()) }
    }

    fn plant_effect_countdown<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live plant path and reads mDoSpecialCountdown at +0x50.
        unsafe { ptrs::Plant::do_special_countdown(handle.ptr.as_ptr()) }
    }

    fn plant_disappear_countdown<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live plant path and reads mDisappearCountdown at +0x4c.
        unsafe { ptrs::Plant::disappear_countdown(handle.ptr.as_ptr()) }
    }

    fn plant_eating_flash_counter<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live plant path and reads mEatenFlashCountdown at +0xb8.
        unsafe { ptrs::Plant::eaten_flash_countdown(handle.ptr.as_ptr()) }
    }

    fn plant_reanim_circulation<'a>(&'a self, handle: Self::PlantHandle<'a>) -> Result<Option<f32>> {
        let app = self.app();
        // SAFETY: `app` is current; effect-system ownership is nullable during initialization.
        let Some(effect_system) = NonNull::new(unsafe { ptrs::LawnApp::effect_system(app.as_ptr()) }) else {
            return Ok(None);
        };
        // SAFETY: the effect system is current; its holder may be unavailable during teardown.
        let Some(holder) = NonNull::new(unsafe { ptrs::EffectSystem::reanimation_holder(effect_system.as_ptr()) })
        else {
            return Ok(None);
        };
        // SAFETY: holder is current and owns the embedded reanimation DataArray.
        let reanimations = unsafe { ptrs::ReanimationHolder::reanimations(holder.as_ptr()) };
        // SAFETY: handle is current; PvZ stores the plant body reanimation's native slot index in
        // the u16 field at Plant+0x94.
        let index = u32::from(unsafe { ptrs::Plant::body_reanim_id(handle.ptr.as_ptr()) });
        // SAFETY: occupied_item_at checks scan/capacity bounds and the slot's nonzero key.
        let Some(reanimation) = (unsafe { ptrs::DataArray::occupied_item_at(reanimations, index) }) else {
            return Ok(None);
        };
        // SAFETY: reanimation is an occupied current slot; circulation is a copied scalar.
        Ok(Some(unsafe {
            ptrs::Reanimation::circulation_rate(reanimation.as_ptr())
        }))
    }

    fn plant_hp<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: handle came from a verified live plant path and reads a copied scalar.
        unsafe { ptrs::Plant::health(handle.ptr.as_ptr()) }
    }

    fn plant_row<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: the borrow-bound handle is valid for this scalar read.
        unsafe { ptrs::Plant::row(handle.ptr.as_ptr()) }
    }
    fn plant_col<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        // SAFETY: the borrow-bound handle is valid for this scalar read.
        unsafe { ptrs::Plant::col(handle.ptr.as_ptr()) }
    }

    fn plant_is_alive<'a>(&'a self, handle: Self::PlantHandle<'a>) -> bool {
        // SAFETY: handle came from a verified live plant path.
        unsafe { ptrs::Plant::is_alive(handle.ptr.as_ptr()) }
    }
}

impl ImitatorMorphBackend for Pvz1051Backend {
    fn imitator_morph_successor(&self, placeholder: PlantId) -> Result<Option<PlantId>> {
        Ok({
            crate::runtime::imitator_morph::successor(self.board()?.as_ptr(), placeholder.raw()).map(PlantId::from_raw)
        })
    }
}

fn resolve_live_plant_ptr(board: NonNull<ptrs::Board>, id: PlantId) -> Option<NonNull<ptrs::Plant>> {
    // SAFETY: `board` is non-null and the plants DataArray is embedded in Board.
    let array = unsafe { ptrs::Board::plants(board.as_ptr()) };
    // SAFETY: `array` is the current Board plant DataArray; try_to_get rejects zero keys and
    // validates scan/capacity bounds, occupancy, and the full generation ID.
    let Some(plant) = (unsafe { ptrs::DataArray::try_to_get(array, id.raw()) }) else {
        return None;
    };
    // SAFETY: `plant` came from the bounded occupied full-ID lookup above.
    if unsafe { !ptrs::Plant::is_alive(plant.as_ptr()) } {
        return None;
    }
    Some(plant)
}

pub(super) fn resolve_live_zombie_ptr(board: NonNull<ptrs::Board>, id: ZombieId) -> Option<NonNull<ptrs::Zombie>> {
    // SAFETY: `board` is non-null and the zombies DataArray is embedded in Board.
    let array = unsafe { ptrs::Board::zombies(board.as_ptr()) };
    // SAFETY: `array` is the current Board zombie DataArray; try_to_get rejects zero keys and
    // validates scan/capacity bounds, occupancy, and the full generation ID.
    let Some(zombie) = (unsafe { ptrs::DataArray::try_to_get(array, id.raw()) }) else {
        return None;
    };
    // SAFETY: `zombie` came from the bounded occupied full-ID lookup above.
    if unsafe { !ptrs::Zombie::is_alive(zombie.as_ptr()) } {
        return None;
    }
    Some(zombie)
}

impl GridItemReadBackend for Pvz1051Backend {
    type GridItemHandle<'a> = PvzGridItemHandle<'a>;
    type GridItemIter<'a> = PvzGridItemIter<'a>;

    fn grid_items(&self) -> Result<Self::GridItemIter<'_>> {
        Ok({
            let board = self.board()?;
            // SAFETY: this pool is embedded in the Board held by the current shared scope.
            let array = unsafe { NonNull::new_unchecked(ptrs::Board::grid_items(board.as_ptr())) };
            // SAFETY: the pool remains alive throughout the returned iterator's backend borrow.
            let max = unsafe { ptrs::DataArray::max_used_count(array.as_ptr()) };
            PvzGridItemIter {
                backend: self,
                next: 0,
                max,
            }
        })
    }

    fn grid_item_id<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> GridItemId {
        // SAFETY: handles are created only from occupied DataArray entries.
        GridItemId::from_raw(unsafe { ptrs::DataArray::<ptrs::GridItem>::item_id(handle.ptr.as_ptr()) })
    }

    fn grid_item_kind<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> Result<GridItemKind> {
        // SAFETY: handle came from a verified live grid-item path and reads a copied scalar.
        let raw = unsafe { ptrs::GridItem::grid_item_type(handle.ptr.as_ptr()) };
        crate::raw::kind::grid_item_kind_from_raw(raw).ok_or(Pvz1051Error::UnknownRawKind {
            category: "grid item",
            raw,
        })
    }

    fn grid_item_row<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> i32 {
        // SAFETY: the borrow-bound handle is valid for this scalar read.
        unsafe { ptrs::GridItem::grid_y(handle.ptr.as_ptr()) }
    }
    fn grid_item_col<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> i32 {
        // SAFETY: the borrow-bound handle is valid for this scalar read.
        unsafe { ptrs::GridItem::grid_x(handle.ptr.as_ptr()) }
    }

    fn grid_item_is_alive<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> bool {
        // SAFETY: handle came from a verified live grid-item path.
        !unsafe { ptrs::GridItem::is_dead(handle.ptr.as_ptr()) }
    }
}

impl SeedBankReadBackend for Pvz1051Backend {
    fn seed_kind_code<'a>(&'a self, seed: Self::SeedHandle<'a>) -> Result<i32> {
        // SAFETY: native enum on the current borrowed packet.
        Ok(unsafe { ptrs::SeedPacket::packet_type(seed.ptr.as_ptr()) })
    }

    fn seed_canvas_rect<'a>(&'a self, seed: Self::SeedHandle<'a>) -> Option<rsvz_model::Rect> {
        // SAFETY: current borrowed packet, verified MouseHitTest layout.
        Some(unsafe { ptrs::SeedPacket::canvas_rect(seed.ptr.as_ptr()) })
    }

    type SeedHandle<'a> = PvzSeedHandle<'a>;
    type SeedIter<'a> = PvzSeedIter<'a>;

    fn seeds(&self) -> Result<Self::SeedIter<'_>> {
        let bank = self.seed_bank()?;
        // SAFETY: `bank` is non-null and packet_count is a copied scalar.
        let count = unsafe { ptrs::SeedBank::packet_count(bank.as_ptr()) }.clamp(0, MAX_SEED_SLOTS as i32) as usize;
        Ok(PvzSeedIter {
            backend: self,
            next: 0,
            count,
        })
    }

    fn seed(&self, slot: SeedSlot) -> Result<Option<Self::SeedHandle<'_>>> {
        let mut packets = self.seeds()?;
        if slot.index() >= packets.count {
            return Ok(None);
        }
        packets.next = slot.index();
        packets.count = slot.index() + 1;
        Ok(packets.next())
    }

    fn seed_slot<'a>(&'a self, handle: Self::SeedHandle<'a>) -> SeedSlot {
        handle.slot
    }

    fn seed_selection<'a>(&'a self, handle: Self::SeedHandle<'a>) -> Result<CheckedCardSelection> {
        // SAFETY: seed handles are produced from the current SeedBank packet range.
        let packet_type = unsafe { ptrs::SeedPacket::packet_type(handle.ptr.as_ptr()) };
        // SAFETY: same verified packet handle.
        let imitater_type = unsafe { ptrs::SeedPacket::imitater_type(handle.ptr.as_ptr()) };
        let packet_type = crate::raw::kind::plant_kind_from_raw(packet_type)?;
        let imitator_target = if packet_type == PlantKind::Imitator {
            Some(crate::raw::kind::plant_kind_from_raw(imitater_type)?)
        } else {
            None
        };
        CardSelection::from_packet_parts(packet_type, imitator_target)
            .map_err(|_error| Pvz1051Error::AbiPreconditionFailed("invalid native seed packet selection"))
    }

    fn seed_is_usable<'a>(&'a self, handle: Self::SeedHandle<'a>) -> bool {
        rsvz_backend_api::SeedPacketBackend::seed_can_pick_up(self, handle).unwrap_or(false)
    }
}

impl CardSelectionReadBackend for Pvz1051Backend {
    fn selected_card_count(&self) -> Result<usize> {
        match self.game_ui()? {
            GameUi::LevelIntro => self.seed_chooser_selected_count(),
            GameUi::Playing => Ok(self.seeds()?.count()),
            _ => Err(Pvz1051Error::AbiPreconditionFailed(
                "card selection is unavailable outside level intro or playing",
            )),
        }
    }

    fn selected_card(&self, index: usize) -> Result<CheckedCardSelection> {
        match self.game_ui()? {
            GameUi::LevelIntro => self.seed_chooser_selected_card(index),
            GameUi::Playing => {
                let seed = self.seeds()?.nth(index).ok_or(Pvz1051Error::InvalidSeedSlot)?;
                self.seed_selection(seed)
            }
            _ => Err(Pvz1051Error::AbiPreconditionFailed(
                "card selection is unavailable outside level intro or playing",
            )),
        }
    }
}

impl SeedCooldownReadBackend for Pvz1051Backend {
    fn seed_cooldown_remaining<'a>(&'a self, seed: Self::SeedHandle<'a>) -> i32 {
        // SAFETY: seed handles are produced from the current SeedBank range.
        if !unsafe { ptrs::SeedPacket::is_refreshing(seed.ptr.as_ptr()) } {
            return 0;
        }
        // SAFETY: the same validated packet owns both copied scalar fields.
        let elapsed = unsafe { ptrs::SeedPacket::refresh_counter(seed.ptr.as_ptr()) };
        // SAFETY: same as above.
        let total = unsafe { ptrs::SeedPacket::refresh_time(seed.ptr.as_ptr()) };
        cooldown_remaining(total, elapsed)
    }
}

impl ChooserCooldownReadBackend for Pvz1051Backend {
    fn chooser_cooldown_remaining(&self, selection: CheckedCardSelection) -> Result<Option<i32>> {
        let chooser = self.seed_chooser()?;
        let packet = selection.packet_kind();
        // SAFETY: the active chooser owns one ChosenSeed for every valid packet type.
        let chosen = unsafe { ptrs::SeedChooserScreen::chosen_seed(chooser.as_ptr(), packet.code() as usize) };
        // SAFETY: `chosen` points into the active chooser's fixed array.
        if !unsafe { ptrs::ChosenSeed::refreshing(chosen) } {
            return Ok(Some(0));
        }
        // SAFETY: the same ChosenSeed owns this copied scalar.
        let elapsed = unsafe { ptrs::ChosenSeed::refresh_counter(chosen) };
        let kind = selection.effective_kind();
        Ok(Some(cooldown_remaining(plant_refresh_time(kind), elapsed)))
    }
}

const PLANT_DEFINITIONS: usize = 0x69f2b0;
const PLANT_DEFINITION_SIZE: usize = 0x24;
const PLANT_DEFINITION_REFRESH_TIME: usize = 0x14;

impl rsvz_backend_api::SeedDefinitionBackend for Pvz1051Backend {
    fn seed_base_cost(&self, selection: CheckedCardSelection) -> Result<i32> {
        let kind = selection.effective_kind();
        let address = PLANT_DEFINITIONS + kind.code() as usize * PLANT_DEFINITION_SIZE + 0x10;
        // SAFETY: checked selection indexes the same fixed gPlantDefs table as plant_refresh_time;
        // mSeedCost is the preceding i32 (AvZ AFodder::AutoSetCards, 0x69f2c0 + 0x24 * kind).
        Ok(unsafe { (address as *const i32).read() })
    }

    fn seed_base_recharge(&self, selection: CheckedCardSelection) -> Result<i32> {
        Ok(plant_refresh_time(selection.effective_kind()))
    }
}

fn plant_refresh_time(kind: PlantKind) -> i32 {
    let address = PLANT_DEFINITIONS + kind.code() as usize * PLANT_DEFINITION_SIZE + PLANT_DEFINITION_REFRESH_TIME;
    // SAFETY: these constants describe PvZ 1.0.0.1051's fixed gPlantDefs array.
    unsafe { (address as *const i32).read() }
}

fn cooldown_remaining(total: i32, elapsed: i32) -> i32 {
    // Native SeedPacket::Update activates only after the counter advances past `total`.
    total.saturating_sub(elapsed).saturating_add(1).max(0)
}

impl GridGeometryBackend for Pvz1051Backend {
    fn grid_to_pixel_x(&self, grid_x: i32, grid_y: i32) -> Result<i32> {
        let grid = Grid {
            row: grid_y,
            col: grid_x,
        };
        self.validate_grid(grid)?;
        // SAFETY: `validate_grid` checked board bounds and `board` existence was checked via
        // `field_info`; the ABI wrapper reads the current Board from LawnApp.
        Ok(unsafe { crate::raw::abi::board_grid_to_pixel_x(grid.row, grid.col) })
    }

    fn grid_to_pixel_y(&self, grid_x: i32, grid_y: i32) -> Result<i32> {
        let grid = Grid {
            row: grid_y,
            col: grid_x,
        };
        self.validate_grid(grid)?;
        // SAFETY: `validate_grid` checked board bounds; raw register mapping was objdump-verified.
        Ok(unsafe { crate::raw::abi::board_grid_to_pixel_y(grid.row, grid.col) })
    }

    fn pos_y_based_on_row(&self, pos_x: FiniteF32, row: i32) -> Result<f32> {
        let field = rsvz_model::FieldInfo::from_scene(self.scene()?);
        if !field.contains_row(row) {
            return Err(Pvz1051Error::InvalidGrid);
        }
        let board = self.board()?;
        // SAFETY: board, finite x, and row bounds were checked; result is returned in x87 ST0.
        Ok(unsafe { crate::raw::abi::board_get_pos_y_based_on_row(board.as_ptr(), pos_x.get(), row) })
    }
}

impl GridTerrainBackend for Pvz1051Backend {
    fn grid_row_kind(&self, row: u8) -> Result<rsvz_model::GridRowKind> {
        if row >= 6 {
            return Err(Pvz1051Error::InvalidGrid);
        }
        let board = self.board()?;
        // SAFETY: live Board, bounded six-row array at verified +0x5d8.
        let kind = unsafe { *ptrs::Board::row_types(board.as_ptr()).add(usize::from(row)) };
        rsvz_model::GridRowKind::try_from(kind).map_err(|_| Pvz1051Error::InvariantViolated("unknown row terrain"))
    }
    fn grid_square_kind(&self, grid: Grid) -> Result<rsvz_model::GridSquareKind> {
        self.validate_grid(grid)?;
        let board = self.board()?;
        // SAFETY: the same verified Board+0x168 matrix used by scene setup and
        // native IsPoolSquare; dynamic bounds precede the [col][row] access.
        let kind = unsafe { *ptrs::Board::block_types(board.as_ptr()).add(grid.col as usize * 6 + grid.row as usize) };
        rsvz_model::GridSquareKind::try_from(kind)
            .map_err(|_| Pvz1051Error::InvariantViolated("unknown grid-square kind"))
    }
    fn is_pool_square(&self, grid_x: i32, grid_y: i32) -> Result<bool> {
        let grid = Grid {
            row: grid_y,
            col: grid_x,
        };
        self.validate_grid(grid)?;
        let board = self.board()?;
        // SAFETY: dynamic grid bounds prevent the native fixed-array lookup from escaping Board;
        // EDX/EAX/ECX/AL were objdump-verified at 0x40ce00.
        Ok(unsafe { crate::raw::abi::board_is_pool_square(board.as_ptr(), grid_x, grid_y) } != 0)
    }

    fn row_can_have_zombies(&self, row: i32) -> Result<bool> {
        let _board = self.board()?;
        // SAFETY: 0x416110 explicitly returns false for every row outside 0..=5 before reading the
        // row-type array; ECX/EAX/AL were objdump-verified.
        Ok(unsafe { crate::raw::abi::board_row_can_have_zombies(row) } != 0)
    }
}

impl rsvz_backend_api::DeadObjectReclaimBackend for Pvz1051Backend {
    fn reclaim_dead_objects(&mut self) -> Result<()> {
        use rsvz_backend_api::BoardReadinessBackend;
        let ready = match self.game_ui()? {
            GameUi::Playing => self.playing_board_ready()?,
            GameUi::LevelIntro => self.level_intro_board_ready()?,
            _ => false,
        };
        if !ready {
            return Err(Pvz1051Error::OperationRejected(
                "dead-object reclamation requires a ready world",
            ));
        }
        let _board = self.board()?;
        // SAFETY: exclusive backend access at the outer host boundary excludes
        // both native iteration and all borrowed entities. Verified ESI/ret ABI.
        unsafe { crate::raw::abi::board_process_delete_queue() };
        // Dead plants also leave dead reanimations/attachments. Reclaim those
        // through the existing native queue before a replacement fills the pool.
        if !unsafe { ptrs::LawnApp::effect_system(self.app().as_ptr()) }.is_null() {
            // SAFETY: exclusive outer-frame access also excludes effect traversal.
            unsafe { crate::raw::abi::effect_system_process_delete_queue() };
        }
        Ok(())
    }
}

impl rsvz_backend_api::GridItemPoolBackend for Pvz1051Backend {
    fn grid_item_pool_active_count(&self) -> Result<u32> {
        let board = self.board()?;
        // SAFETY: current embedded pool; scalar header read.
        Ok(unsafe { ptrs::DataArray::active_count(ptrs::Board::grid_items(board.as_ptr())) })
    }
    fn grid_item_pool_capacity(&self) -> Result<u32> {
        let board = self.board()?;
        // SAFETY: current embedded pool; scalar header read.
        Ok(unsafe { ptrs::DataArray::max_size(ptrs::Board::grid_items(board.as_ptr())) })
    }
}

impl BattleStatusBackend for Pvz1051Backend {
    fn battle_status(&self) -> Result<BattleStatus> {
        let ui = self.game_ui()?;
        let level_complete = if ui == GameUi::Playing {
            let board = self.board()?;
            // SAFETY: `board` is non-null and the completion flag is a copied scalar.
            Some(unsafe { ptrs::Board::level_complete(board.as_ptr()) })
        } else {
            None
        };
        Ok(classify_battle_status(ui, level_complete))
    }
}

pub(super) fn classify_battle_status(ui: GameUi, level_complete: Option<bool>) -> BattleStatus {
    match ui {
        GameUi::ZombiesWon => BattleStatus::Lost,
        GameUi::Award | GameUi::Credit => BattleStatus::Ended,
        GameUi::Playing => match level_complete {
            Some(true) => BattleStatus::ObjectiveReached,
            Some(false) => BattleStatus::Running,
            None => BattleStatus::Unknown,
        },
        GameUi::Loading | GameUi::Menu | GameUi::LevelIntro | GameUi::Challenge => BattleStatus::Unknown,
    }
}

#[cfg(test)]
mod cooldown_tests {
    use super::{classify_battle_status, cooldown_remaining};
    use rsvz_model::{BattleStatus, GameUi};

    #[test]
    fn native_strict_refresh_threshold_has_one_tick_left_at_the_total() {
        assert_eq!(cooldown_remaining(750, 0), 751);
        assert_eq!(cooldown_remaining(750, 750), 1);
        assert_eq!(cooldown_remaining(750, 751), 0);
    }

    #[test]
    fn battle_status_classification_shares_playing_and_terminal_semantics() {
        assert_eq!(
            classify_battle_status(GameUi::Playing, Some(false)),
            BattleStatus::Running
        );
        assert_eq!(
            classify_battle_status(GameUi::Playing, Some(true)),
            BattleStatus::ObjectiveReached
        );
        assert_eq!(classify_battle_status(GameUi::Award, None), BattleStatus::Ended);
        assert_eq!(classify_battle_status(GameUi::Playing, None), BattleStatus::Unknown);
    }
}

impl rsvz_backend_api::PlantPresenceBackend for Pvz1051Backend {
    type PresentPlantIter<'a> = PvzPlantIter<'a>;
    fn present_plants(&self) -> Result<Self::PresentPlantIter<'_>> {
        let mut plants = self.plants()?;
        plants.live_only = false;
        Ok(plants)
    }
}

impl rsvz_backend_api::WorldIdentityBackend for Pvz1051Backend {
    fn world_identity(&self) -> Result<Option<rsvz_model::WorldId>> {
        // SAFETY: token proves LawnApp lifetime; this only checks its current Board pointer.
        if unsafe { ptrs::LawnApp::board(self.app().as_ptr()) }.is_null() {
            return Ok(None);
        }
        Ok(Some(rsvz_model::WorldId::from_raw(
            crate::patches::world_lifetime::generation()?,
        )))
    }
}

impl rsvz_backend_api::ZombiePresenceBackend for Pvz1051Backend {
    type PresentZombieIter<'a> = PvzZombieIter<'a>;
    fn present_zombies(&self) -> Result<Self::PresentZombieIter<'_>> {
        let mut zombies = self.zombies()?;
        zombies.live_only = false;
        Ok(zombies)
    }
}

impl rsvz_backend_api::PlantSlotEditBackend for Pvz1051Backend {
    fn validate_plant_slot_change(&self, source: PlantId, target: u32, expected_target: Option<PlantId>) -> Result<()> {
        use rsvz_backend_api::{BoardReadinessBackend, GameUiBackend};
        let ready = match self.game_ui()? {
            rsvz_model::GameUi::Playing => self.playing_board_ready()?,
            rsvz_model::GameUi::LevelIntro => self.level_intro_board_ready()?,
            _ => false,
        };
        if !ready || source.generation() == 0 || expected_target.is_some_and(|id| id.generation() == 0) {
            return Err(crate::error::Pvz1051Error::OperationRejected(
                "invalid relocation phase or identity",
            ));
        }
        let board = self.board()?;
        // SAFETY: commit=false only validates borrowed pool state; it cannot invalidate entities.
        unsafe {
            ptrs::DataArray::relocate_plant(
                ptrs::Board::plants(board.as_ptr()),
                source.raw(),
                target,
                expected_target.map_or(0, PlantId::raw),
                false,
            )
            .map(|_| ())
        }
    }

    fn plant_slot_id(&self, slot: u32) -> Result<Option<PlantId>> {
        let board = self.board()?;
        // SAFETY: Board remains valid for this shared token; the atom checks bounds.
        unsafe {
            ptrs::DataArray::plant_slot_id(ptrs::Board::plants(board.as_ptr()), slot)
                .map(|id| id.map(PlantId::from_raw))
        }
    }
    fn relocate_plant_slot(
        &mut self, source: PlantId, target: u32, expected_target: Option<PlantId>,
    ) -> Result<rsvz_model::PlantSlotChange> {
        use rsvz_backend_api::{BoardReadinessBackend, GameUiBackend};
        let ready = match self.game_ui()? {
            rsvz_model::GameUi::Playing => self.playing_board_ready()?,
            rsvz_model::GameUi::LevelIntro => self.level_intro_board_ready()?,
            _ => false,
        };
        if !ready || source.generation() == 0 || expected_target.is_some_and(|id| id.generation() == 0) {
            return Err(crate::error::Pvz1051Error::OperationRejected(
                "plant relocation requires a ready world and valid identities",
            ));
        }
        let board = self.board()?;
        // SAFETY: exclusive token at the physical update boundary excludes native
        // traversal and every borrowed entity. The pool atom preflights all edits.
        let (moved, swapped) = unsafe {
            ptrs::DataArray::relocate_plant(
                ptrs::Board::plants(board.as_ptr()),
                source.raw(),
                target,
                expected_target.map_or(0, PlantId::raw),
                true,
            )
        }?;
        if moved != source.raw() {
            let remap = |id: u32| {
                if id == source.raw() {
                    moved
                } else if expected_target.is_some_and(|old| old.raw() == id) {
                    swapped
                } else {
                    id
                }
            };
            // SAFETY: native Zombie+0x128 and CursorObject+0x38/3c/40 are the
            // persistent PlantID users audited in both sources and the 1051 layout.
            unsafe {
                let zombies = ptrs::Board::zombies(board.as_ptr());
                for index in 0..ptrs::DataArray::max_used_count(zombies) {
                    if let Some(zombie) = ptrs::DataArray::occupied_item_at(zombies, index) {
                        let field = zombie.as_ptr().cast::<u8>().add(0x128).cast::<u32>();
                        std::ptr::write_unaligned(field, remap(std::ptr::read_unaligned(field)));
                    }
                }
                let cursor = ptrs::Board::cursor_object(board.as_ptr());
                if !cursor.is_null() {
                    for offset in [0x38, 0x3c, 0x40] {
                        let field = cursor.cast::<u8>().add(offset).cast::<u32>();
                        std::ptr::write_unaligned(field, remap(std::ptr::read_unaligned(field)));
                    }
                }
            }
            crate::runtime::imitator_morph::begin_update_batch();
        }
        Ok(rsvz_model::PlantSlotChange {
            moved: rsvz_model::PlantIdMapping {
                previous: source,
                current: PlantId::from_raw(moved),
            },
            swapped: expected_target
                .filter(|_| swapped != 0)
                .map(|previous| rsvz_model::PlantIdMapping {
                    previous,
                    current: PlantId::from_raw(swapped),
                }),
        })
    }
}
