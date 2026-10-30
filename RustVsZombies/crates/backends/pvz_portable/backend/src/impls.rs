use crate::{PortableBackend, PortableBackendError, Result};
use rsvz_backend_api::backend::{
    AdvancedPauseBackend, BackendIdentityBackend, BattleEntryBackend, BattleStatusBackend, BoardReadinessBackend,
    BoardStateBackend, BoardSupportBackend, ClockBackend, CobRuleEditBackend, CurrentWaveBackend, CursorQueryBackend,
    DancerClockWriteBackend, DropRuleEditBackend, FastForwardBackend, GameSpeedHintBackend, GameUiBackend,
    GridGeometryBackend, GridTerrainBackend, KeyboardStateBackend, LawnMowerClearBackend, MaidCheatsBackend,
    MainMenuBackend, ProfileReadonlyBackend, RandomControlBackend, SceneBackend, SceneEditBackend,
    SpawnScheduleBackend, SunCostRuleEditBackend, SunMoneyBackend, SunProductionModeBackend, SunQueryBackend,
    SunWriteBackend, VisibilityEditBackend, WaveHealthBackend, WaveRefreshControlBackend, WaveTimingBackend,
    WorldResetBackend,
};
use rsvz_model::model::{
    AdvancedPauseOptions, BattleConfig, BattleStatus, CardSelection, CheckedCardSelection, ContactRect,
    FastForwardOptions, FastForwardPerformance, FastForwardStopReason, FiniteF32, GameUi, Grid, KeyCode, MaidCheat,
    NonNegativeI32, PlantKind, PositiveFiniteF32, RandomMode, RandomStreamKind, ResetCardCooldowns, SceneKind,
    SpawnWaveSlot, SunAmount, SunProductionMode, Wave, WorldResetConfig, ZombieKind,
};
#[cfg(windows)]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

macro_rules! read_raw {
    ($handle:expr, $($field:tt)+) => {{
        // SAFETY: Portable handles are created only from occupied native slots
        // under the current backend borrow; this copies one bindgen-verified field.
        unsafe { std::ptr::addr_of!((*$handle.as_ptr()).$($field)+).read() }
    }};
}

macro_rules! write_raw {
    ($handle:expr, $value:expr, $($field:tt)+) => {{
        // SAFETY: Portable handles are created only from occupied native slots
        // under the current backend borrow; this writes one bindgen-verified field.
        unsafe { std::ptr::addr_of_mut!((*$handle.as_mut_ptr()).$($field)+).write($value) }
    }};
}

mod assist_control;
mod grid_item;
mod item;
mod level_flow;
mod mower;
mod plant;
mod projectile;
mod seed;
mod user_card;
mod zombie;

fn invalid_kind(kind: &'static str, value: i32) -> PortableBackendError {
    PortableBackendError::InvalidKind { kind, value }
}

fn card_selection(packet: i32, imitater: i32) -> Result<CheckedCardSelection> {
    let packet = PlantKind::try_from_code(packet).map_err(|_| invalid_kind("plant", packet))?;
    let target = if packet == PlantKind::Imitator {
        Some(PlantKind::try_from_code(imitater).map_err(|_| invalid_kind("plant", imitater))?)
    } else {
        None
    };
    CardSelection::from_packet_parts(packet, target)
        .map_err(|_| PortableBackendError::OperationRejected("invalid native seed packet selection"))
}

fn card_parts(selection: CheckedCardSelection) -> (i32, i32) {
    (
        selection.packet_kind().code(),
        selection.imitator_target().map_or(-1, PlantKind::code),
    )
}

fn validate_grid(backend: &PortableBackend, grid: Grid) -> Result<()> {
    if rsvz_model::FieldInfo::from_scene(backend.scene()?).contains_grid(grid) {
        Ok(())
    } else {
        Err(PortableBackendError::OperationRejected(
            "grid is outside the current field",
        ))
    }
}

fn validate_row(backend: &PortableBackend, row: i32) -> Result<()> {
    if rsvz_model::FieldInfo::from_scene(backend.scene()?).contains_row(row) {
        Ok(())
    } else {
        Err(PortableBackendError::OperationRejected(
            "row is outside the current field",
        ))
    }
}

fn contact_rect(raw: pvzp_rs::raw::pvzp_rs_rect_i32) -> Result<ContactRect> {
    ContactRect::checked_from_pos_size(raw.x, raw.y, raw.width, raw.height).ok_or(
        PortableBackendError::OperationRejected("native contact rectangle overflow"),
    )
}

impl BackendIdentityBackend for PortableBackend {
    fn backend_name(&self) -> &'static str {
        "pvz-portable"
    }

    fn backend_version(&self) -> &'static str {
        "1.0.0.1051-source"
    }
}

impl GameUiBackend for PortableBackend {
    fn game_ui(&self) -> Result<GameUi> {
        let raw = pvzp_rs::game_ui()?;
        GameUi::try_from_code(raw).map_err(|_| invalid_kind("game-ui", raw))
    }

    fn game_ui_unavailable(&self, error: &Self::Error) -> bool {
        matches!(error, PortableBackendError::Native(pvzp_rs::Error::NotFound { .. }))
    }
}

impl BoardReadinessBackend for PortableBackend {
    fn level_intro_board_ready(&self) -> Result<bool> {
        Ok(self.game_ui()? == GameUi::LevelIntro && self.world().is_ok())
    }

    fn playing_board_ready(&self) -> Result<bool> {
        Ok(self.game_ui()? == GameUi::Playing && self.world().is_ok())
    }
}

impl SceneBackend for PortableBackend {
    fn scene(&self) -> Result<SceneKind> {
        let raw = self.world()?.scene();
        SceneKind::try_from_code(raw).map_err(|_| invalid_kind("scene", raw))
    }
}

impl ClockBackend for PortableBackend {
    fn clock(&self) -> Result<i32> {
        i32::try_from(self.world()?.main_counter())
            .map_err(|_| PortableBackendError::NumericOutOfRange("board main counter"))
    }
}

impl CursorQueryBackend for PortableBackend {
    fn cursor_type(&self) -> Result<i32> {
        self.world()?.cursor_type().map_err(Into::into)
    }
}
impl rsvz_backend_api::CursorSeedReadBackend for PortableBackend {
    fn cursor_seed_kind(&self) -> Result<i32> {
        self.world()?.cursor_seed_kind().map_err(Into::into)
    }
    fn cursor_imitater_kind(&self) -> Result<i32> {
        self.world()?.cursor_imitater_kind().map_err(Into::into)
    }
}

impl CurrentWaveBackend for PortableBackend {
    fn current_wave(&self) -> Result<Wave> {
        Ok(Wave(self.world()?.current_wave()))
    }
}

impl SunQueryBackend for PortableBackend {
    fn sun(&self) -> Result<u32> {
        Ok(self.world()?.sun().max(0) as u32)
    }
}

impl GridGeometryBackend for PortableBackend {
    fn grid_to_pixel_x(&self, grid_x: i32, grid_y: i32) -> Result<i32> {
        self.world()?.grid_to_pixel_x(grid_x, grid_y).map_err(Into::into)
    }

    fn grid_to_pixel_y(&self, grid_x: i32, grid_y: i32) -> Result<i32> {
        self.world()?.grid_to_pixel_y(grid_x, grid_y).map_err(Into::into)
    }

    fn pos_y_based_on_row(&self, pos_x: FiniteF32, row: i32) -> Result<f32> {
        self.world()?.pos_y_based_on_row(pos_x.get(), row).map_err(Into::into)
    }
}

impl GridTerrainBackend for PortableBackend {
    fn grid_row_kind(&self, row: u8) -> Result<rsvz_model::GridRowKind> {
        let kind = self.world()?.grid_row_kind(i32::from(row))?;
        rsvz_model::GridRowKind::try_from(kind).map_err(|_| invalid_kind("grid row", kind))
    }
    fn grid_square_kind(&self, grid: Grid) -> Result<rsvz_model::GridSquareKind> {
        validate_grid(self, grid)?;
        let kind = self.world()?.grid_square_kind(grid.row, grid.col)?;
        rsvz_model::GridSquareKind::try_from(kind).map_err(|_| invalid_kind("grid square", kind))
    }
    fn is_pool_square(&self, grid_x: i32, grid_y: i32) -> Result<bool> {
        self.world()?.is_pool_square(grid_x, grid_y).map_err(Into::into)
    }

    fn row_can_have_zombies(&self, row: i32) -> Result<bool> {
        self.world()?.row_can_have_zombies(row).map_err(Into::into)
    }
}

impl rsvz_backend_api::GridItemPoolBackend for PortableBackend {
    fn grid_item_pool_active_count(&self) -> Result<u32> {
        Ok(self.world()?.grid_items().active_count())
    }
    fn grid_item_pool_capacity(&self) -> Result<u32> {
        Ok(self.world()?.grid_items().capacity())
    }
}

impl BattleStatusBackend for PortableBackend {
    fn battle_status(&self) -> Result<BattleStatus> {
        match self.game_ui()? {
            GameUi::ZombiesWon => Ok(BattleStatus::Lost),
            GameUi::Award | GameUi::Credit => Ok(BattleStatus::Ended),
            GameUi::Playing if self.world()?.level_complete() => Ok(BattleStatus::ObjectiveReached),
            GameUi::Playing => Ok(BattleStatus::Running),
            GameUi::Loading | GameUi::Menu | GameUi::LevelIntro | GameUi::Challenge => Ok(BattleStatus::Unknown),
        }
    }
}

impl SunMoneyBackend for PortableBackend {
    fn can_take_sun_money(&self, amount: SunAmount) -> Result<bool> {
        self.world()?.can_take_sun(amount.get()).map_err(Into::into)
    }

    fn take_sun_money(&self, amount: SunAmount) -> Result<bool> {
        self.world()?.take_sun(amount.get()).map_err(Into::into)
    }
}

impl BoardSupportBackend for PortableBackend {
    fn ensure_board_supported(&self) -> Result<()> {
        self.world().map(|_| ())
    }
}

impl rsvz_backend_api::NativeGameModeBackend for PortableBackend {
    fn native_game_mode(&self) -> Result<i32> {
        pvzp_rs::native_game_mode().map_err(Into::into)
    }
}

impl rsvz_backend_api::SceneLayoutBackend for PortableBackend {
    fn validate_scene_layout(&self, scene: SceneKind) -> Result<()> {
        self.world()?.validate_scene_layout(scene.code()).map_err(Into::into)
    }
    fn set_scene_layout(&mut self, scene: SceneKind) -> Result<()> {
        // SAFETY: exclusive backend at the outer host boundary excludes borrowed views.
        unsafe { self.world()?.set_scene_layout(scene.code()) }.map_err(Into::into)
    }
}

impl rsvz_backend_api::DeadObjectReclaimBackend for PortableBackend {
    fn reclaim_dead_objects(&mut self) -> Result<()> {
        // SAFETY: mutable backend access excludes entity borrows and is only
        // provided by the physical host at its outer update boundary.
        unsafe { self.world()?.process_delete_queue() }.map_err(Into::into)
    }
}

impl LawnMowerClearBackend for PortableBackend {
    fn clear_lawn_mowers(&mut self) -> Result<()> {
        // SAFETY: exclusive backend receiver excludes all mower borrows.
        unsafe { self.world()?.clear_lawn_mowers() }.map_err(Into::into)
    }
}

impl BattleEntryBackend for PortableBackend {
    fn enter_game(&mut self, config: BattleConfig) -> Result<()> {
        let BattleConfig::Endless(config) = config;
        if !matches!(
            config.scene,
            SceneKind::Day | SceneKind::Night | SceneKind::Pool | SceneKind::Fog | SceneKind::Roof
        ) {
            return Err(PortableBackendError::Unsupported("survival endless scene"));
        }
        pvzp_rs::enter_endless(config.scene as i32).map_err(Into::into)
    }

    fn start_battle(&mut self) -> Result<()> {
        self.ensure_seed_chooser_ready_for_auto_selection()?;
        pvzp_rs::start_battle().map_err(Into::into)
    }
}

impl MainMenuBackend for PortableBackend {
    fn back_to_main_menu(&mut self) -> Result<()> {
        pvzp_rs::back_to_main_menu().map_err(Into::into)
    }
}

impl FastForwardBackend for PortableBackend {
    fn start_fast_forward(&self, options: FastForwardOptions) -> Result<()> {
        let performance = match options.performance {
            FastForwardPerformance::Off => 0,
            FastForwardPerformance::Basic => 1,
            FastForwardPerformance::Aggressive => 2,
        };
        pvzp_rs::set_fast_forward(true, performance, options.suppress_window_update).map_err(Into::into)
    }

    fn stop_fast_forward(&self, _reason: FastForwardStopReason) -> Result<()> {
        pvzp_rs::set_fast_forward(false, 0, false).map_err(Into::into)
    }

    fn fast_forward_active(&self) -> bool {
        pvzp_rs::fast_forward_active()
    }
}

impl AdvancedPauseBackend for PortableBackend {
    fn release_advanced_pause_for_step(&self, options: AdvancedPauseOptions) -> Result<()> {
        pvzp_rs::release_advanced_pause_for_step(options.play_sound).map_err(Into::into)
    }

    fn set_advanced_pause_with_options(&self, enabled: bool, options: AdvancedPauseOptions) -> Result<()> {
        let color = options.mask_color;
        let rgba = u32::from(color.red) << 24
            | u32::from(color.green) << 16
            | u32::from(color.blue) << 8
            | u32::from(color.alpha);
        pvzp_rs::set_advanced_pause(
            enabled,
            options.draw_mask,
            rgba,
            options.play_sound,
            options.refresh_cursor_preview,
        )
        .map_err(Into::into)
    }

    fn advanced_pause_options(&self) -> AdvancedPauseOptions {
        let (draw_mask, color, refresh_cursor_preview) = pvzp_rs::modifier::advanced_pause_options();
        AdvancedPauseOptions {
            draw_mask,
            refresh_cursor_preview,
            play_sound: false,
            mask_color: rsvz_model::AdvancedPauseMaskColor::rgba(
                (color >> 24) as u8,
                (color >> 16) as u8,
                (color >> 8) as u8,
                color as u8,
            ),
        }
    }

    fn advanced_pause_active(&self) -> bool {
        pvzp_rs::advanced_pause_active()
    }
}

thread_local! { static KEY_SINK: std::cell::Cell<Option<fn(u32)>> = const {std::cell::Cell::new(None)}; }
unsafe extern "C" fn key_event(event: u32) {
    if let Some(sink) = KEY_SINK.get() {
        sink(event);
    }
}
impl KeyboardStateBackend for PortableBackend {
    fn set_key_event_sink(&self, sink: Option<fn(u32)>) -> Result<()> {
        if let Some(result) = crate::module_link::key(sink) {
            return result;
        }
        KEY_SINK.set(sink);
        // SAFETY: fixed record-only ingress, revoked before the shared SDK unloads.
        unsafe {
            pvzp_rs::raw::pvzp_rs_set_key_sink(sink.map(|_| key_event as unsafe extern "C" fn(u32)));
        }
        Ok(())
    }
    fn key_is_down(&self, key: KeyCode) -> bool {
        // SAFETY: this reads process-independent copied keyboard state for one virtual-key code.
        #[cfg(windows)]
        unsafe {
            GetAsyncKeyState(i32::from(key.raw_virtual_key())) < 0
        }
        #[cfg(unix)]
        {
            pvzp_rs::key_is_down(key.raw_virtual_key().into())
        }
    }

    fn input_focused(&self) -> Result<bool> {
        pvzp_rs::input_focused().map_err(Into::into)
    }
}

impl RandomControlBackend for PortableBackend {
    fn set_random_mode(&mut self, mode: RandomMode) -> Result<()> {
        let (mode, value) = match mode {
            RandomMode::Seeded(seed) => (0, seed),
            RandomMode::Locked(value) => (1, value),
        };
        pvzp_rs::set_random_mode(mode, value).map_err(Into::into)
    }

    fn set_wave_spawn_random_seed(&mut self, base_seed: Option<u32>) -> Result<()> {
        pvzp_rs::set_wave_spawn_random_seed(base_seed).map_err(Into::into)
    }

    fn random_seed(&self, stream: RandomStreamKind) -> Result<u32> {
        let stream = match stream {
            RandomStreamKind::Battle => 0,
            RandomStreamKind::Level => 1,
        };
        pvzp_rs::random_seed(stream).map_err(Into::into)
    }
    fn random_locked(&self, stream: RandomStreamKind) -> bool {
        pvzp_rs::random_locked(matches!(stream, RandomStreamKind::Level))
    }
    fn random_fixed(&self, stream: RandomStreamKind) -> u32 {
        pvzp_rs::random_fixed(matches!(stream, RandomStreamKind::Level))
    }
}

impl SunProductionModeBackend for PortableBackend {
    fn set_sun_production_mode(&mut self, mode: SunProductionMode) -> Result<()> {
        let mode = match mode {
            SunProductionMode::DirectCredit => 0,
            SunProductionMode::NativePickup => 1,
        };
        pvzp_rs::set_sun_production_mode(mode).map_err(Into::into)
    }
}

impl WorldResetBackend for PortableBackend {
    fn reset_world(&mut self, config: WorldResetConfig) -> Result<()> {
        let mut world = self.world()?;
        world
            .reset(
                config.completed_rounds,
                config.seed,
                config.initial_sun,
                config.card_cooldowns == ResetCardCooldowns::Ready,
            )
            .map_err(Into::into)
    }
}

impl GameSpeedHintBackend for PortableBackend {
    fn game_speed_multiplier(&self) -> Result<f64> {
        pvzp_rs::capabilities::game_speed_multiplier().map_err(Into::into)
    }
    fn set_game_speed_multiplier(&self, value: f64) -> Result<()> {
        pvzp_rs::capabilities::set_game_speed_multiplier(value).map_err(Into::into)
    }
    fn set_game_speed(&self, speed: PositiveFiniteF32) -> Result<()> {
        pvzp_rs::set_game_speed(speed.get()).map_err(Into::into)
    }
}

impl SunWriteBackend for PortableBackend {
    fn set_sun(&self, value: u32) -> Result<()> {
        self.world()?.set_sun(value).map_err(Into::into)
    }
}

impl DancerClockWriteBackend for PortableBackend {
    fn set_dancer_clock(&self, value: u32) -> Result<()> {
        pvzp_rs::set_app_counter(value).map_err(Into::into)
    }
}

impl SunCostRuleEditBackend for PortableBackend {
    fn set_sun_cost_ignored(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_sun_cost_ignored(enabled).map_err(Into::into)
    }

    fn sun_cost_ignored(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::sun_cost_ignored())
    }
}

impl VisibilityEditBackend for PortableBackend {
    fn set_fog_revealed(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_fog_revealed(enabled).map_err(Into::into)
    }

    fn fog_revealed(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::fog_revealed())
    }

    fn set_vase_contents_visible(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_vase_contents_visible(enabled).map_err(Into::into)
    }

    fn vase_contents_visible(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::vase_contents_visible())
    }
}

impl CobRuleEditBackend for PortableBackend {
    fn cob_fixed_delay(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::cob_fixed_delay())
    }

    fn set_cob_fixed_delay(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_cob_fixed_delay(enabled).map_err(Into::into)
    }

    fn cob_recharge_shortened(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::cob_recharge_shortened())
    }

    fn set_cob_recharge_shortened(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_cob_recharge_shortened(enabled).map_err(Into::into)
    }

    fn cob_drift_fixed(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::cob_drift_fixed())
    }

    fn set_cob_drift_fixed(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_cob_drift_fixed(enabled).map_err(Into::into)
    }
}

impl DropRuleEditBackend for PortableBackend {
    fn item_drop_disabled(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::item_drop_disabled())
    }

    fn set_item_drop_disabled(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_item_drop_disabled(enabled).map_err(Into::into)
    }

    fn natural_sun_drop_disabled(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::natural_sun_drop_disabled())
    }

    fn set_natural_sun_drop_disabled(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_natural_sun_drop_disabled(enabled).map_err(Into::into)
    }

    fn set_natural_sun_generated(&self, count: NonNegativeI32) -> Result<()> {
        self.world()?.set_natural_sun_generated(count.get()).map_err(Into::into)
    }

    fn set_natural_sun_countdown(&self, countdown: NonNegativeI32) -> Result<()> {
        self.world()?
            .set_natural_sun_countdown(countdown.get())
            .map_err(Into::into)
    }
}

impl MaidCheatsBackend for PortableBackend {
    fn maid_cheat(&self) -> Result<MaidCheat> {
        match pvzp_rs::modifier::maid_cheat()? {
            0 => Ok(MaidCheat::Stop),
            1 => Ok(MaidCheat::CallPartner),
            2 => Ok(MaidCheat::Dancing),
            3 => Ok(MaidCheat::Move),
            value => Err(invalid_kind("maid cheat", value)),
        }
    }

    fn set_maid_cheat(&self, cheat: MaidCheat) -> Result<()> {
        let raw = match cheat {
            MaidCheat::Stop => 0,
            MaidCheat::CallPartner => 1,
            MaidCheat::Dancing => 2,
            MaidCheat::Move => 3,
        };
        pvzp_rs::modifier::set_maid_cheat(raw).map_err(Into::into)
    }
}

impl ProfileReadonlyBackend for PortableBackend {
    fn set_profile_readonly(&self, readonly: bool) -> Result<()> {
        pvzp_rs::modifier::set_profile_readonly(readonly).map_err(Into::into)
    }
}

impl BoardStateBackend for PortableBackend {
    fn natural_sun_generated(&self) -> Result<i32> {
        Ok(self.world()?.natural_sun_generated())
    }

    fn natural_sun_countdown(&self) -> Result<i32> {
        Ok(self.world()?.natural_sun_countdown())
    }

    fn dancer_clock(&self) -> Result<u32> {
        pvzp_rs::app_counter().map_err(Into::into)
    }

    fn ice_path_x(&self, row: u32) -> Result<i32> {
        self.world()?.ice_path_x(row).map_err(Into::into)
    }

    fn ice_path_countdown(&self, row: u32) -> Result<u32> {
        self.world()?.ice_path_countdown(row).map_err(Into::into)
    }

    fn spawn_allowed(&self, raw_zombie_kind: u32) -> Result<bool> {
        self.world()?.spawn_allowed(raw_zombie_kind).map_err(Into::into)
    }

    fn spawn_entry(&self, wave: u32, slot: u32) -> Result<i32> {
        self.world()?.spawn_entry(wave, slot).map_err(Into::into)
    }
}

impl WaveTimingBackend for PortableBackend {
    fn total_waves(&self) -> Result<i32> {
        Ok(self.world()?.total_waves())
    }
    fn refresh_countdown(&self) -> Result<i32> {
        Ok(self.world()?.refresh_countdown())
    }
    fn initial_countdown(&self) -> Result<i32> {
        Ok(self.world()?.initial_countdown())
    }
    fn huge_wave_countdown(&self) -> Result<i32> {
        Ok(self.world()?.huge_wave_countdown())
    }
    fn level_end_countdown(&self) -> Result<i32> {
        Ok(self.world()?.level_end_countdown())
    }
}

impl WaveRefreshControlBackend for PortableBackend {
    fn commit_timer_only_wave_refresh(&self, expected_current_wave: Wave, initial_countdown: i32) -> Result<()> {
        self.world()?
            .commit_wave_refresh(expected_current_wave.0, initial_countdown)
            .map_err(Into::into)
    }
}

impl WaveHealthBackend for PortableBackend {
    fn zombie_refresh_health_threshold(&self) -> Result<i32> {
        self.world()?.zombie_refresh_health_threshold().map_err(Into::into)
    }

    fn zombie_health_wave_start(&self) -> Result<i32> {
        self.world()?.zombie_health_wave_start().map_err(Into::into)
    }

    fn total_zombies_health_in_wave(&self, wave: i32) -> Result<i32> {
        self.world()?.total_zombie_health_in_wave(wave).map_err(Into::into)
    }
}

impl SceneEditBackend for PortableBackend {
    fn set_scene(&mut self, scene: SceneKind) -> Result<()> {
        if self.scene()? == scene {
            return Ok(());
        }
        if !matches!(self.game_ui()?, GameUi::LevelIntro | GameUi::Playing) || self.current_wave()?.0 > 0 {
            return Err(PortableBackendError::OperationRejected(
                "scene switch requires level intro or opening wave",
            ));
        }
        // SAFETY: exclusive backend receiver excludes surviving native entity views.
        unsafe { self.world()?.set_scene(scene.code()) }.map_err(Into::into)
    }
}

impl SpawnScheduleBackend for PortableBackend {
    fn refresh_spawn_preview(&self) -> Result<()> {
        Ok(self.world()?.refresh_spawn_preview()?)
    }
    fn spawn_wave_count(&self) -> Result<usize> {
        let count = self.world()?.total_waves();
        let count = usize::try_from(count).map_err(|_| PortableBackendError::NumericOutOfRange("spawn wave count"))?;
        if (1..=pvzp_rs::SPAWN_WAVE_CAPACITY).contains(&count) {
            Ok(count)
        } else {
            Err(PortableBackendError::OperationRejected(
                "spawn wave count is outside Portable range",
            ))
        }
    }

    fn set_spawn_type_allowed(&self, kind: ZombieKind, allowed: bool) -> Result<()> {
        self.world()?
            .set_spawn_allowed(kind.code(), allowed)
            .map_err(Into::into)
    }

    fn set_spawn_slot(&self, wave: usize, slot: SpawnWaveSlot, kind: Option<ZombieKind>) -> Result<()> {
        if wave >= self.spawn_wave_count()? {
            return Err(PortableBackendError::OperationRejected(
                "spawn wave is outside current board range",
            ));
        }
        let wave = u32::try_from(wave).map_err(|_| PortableBackendError::NumericOutOfRange("spawn wave"))?;
        let slot = u32::try_from(slot.index()).map_err(|_| PortableBackendError::NumericOutOfRange("spawn slot"))?;
        self.world()?
            .set_spawn_entry(wave, slot, kind.map_or(-1, ZombieKind::code))
            .map_err(Into::into)
    }

    fn pick_spawn_list(&self) -> Result<()> {
        self.world()?.pick_spawn_list().map_err(Into::into)
    }
}

impl rsvz_backend_api::SpawnScheduleReadBackend for PortableBackend {
    fn spawn_type_allowed(&self, kind: ZombieKind) -> Result<bool> {
        Ok(self.world()?.spawn_allowed(kind.code() as u32)?)
    }
    fn spawn_slot(&self, wave: usize, slot: SpawnWaveSlot) -> Result<Option<ZombieKind>> {
        if wave >= self.spawn_wave_count()? {
            return Err(PortableBackendError::OperationRejected("spawn wave out of range"));
        }
        let kind = self.world()?.spawn_entry(wave as u32, slot.index() as u32)?;
        if kind == -1 {
            Ok(None)
        } else {
            ZombieKind::try_from_code(kind)
                .map(Some)
                .map_err(|_| invalid_kind("zombie", kind))
        }
    }
    fn spawn_huge_wave(&self, wave: usize) -> Result<bool> {
        if wave >= self.spawn_wave_count()? {
            return Err(PortableBackendError::OperationRejected("spawn wave out of range"));
        }
        Ok(self.world()?.spawn_huge_wave(wave as u32)?)
    }
}

impl rsvz_backend_api::CurrentNaturalSpawnBackend for PortableBackend {
    fn board_spawn_seed(&self) -> Result<u32> {
        Ok(self.world()?.board_spawn_seed()?)
    }
    fn generate_current_natural_spawn(&self, seed: u32) -> Result<()> {
        Ok(self.world()?.generate_current_spawn(seed)?)
    }
}

impl rsvz_backend_api::CraterLifetimeRuleBackend for PortableBackend {
    fn craters_expire_immediately(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::craters_expire_immediately())
    }
    fn set_craters_expire_immediately(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_craters_expire_immediately(enabled).map_err(Into::into)
    }
}

impl rsvz_backend_api::ZombieBehaviorRulesBackend for PortableBackend {
    fn chilled_effects_disabled(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::chilled_effects_disabled())
    }
    fn set_chilled_effects_disabled(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_chilled_effects_disabled(enabled).map_err(Into::into)
    }
    fn butter_effects_disabled(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::butter_effects_disabled())
    }
    fn set_butter_effects_disabled(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_butter_effects_disabled(enabled).map_err(Into::into)
    }
    fn imp_throw_disabled(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::imp_throw_disabled())
    }
    fn set_imp_throw_disabled(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_imp_throw_disabled(enabled).map_err(Into::into)
    }
    fn zombie_walking_stopped(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::zombie_walking_stopped())
    }
    fn set_zombie_walking_stopped(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_zombie_walking_stopped(enabled).map_err(Into::into)
    }
}

impl rsvz_backend_api::IceTrailRuleBackend for PortableBackend {
    fn ice_trails_disabled(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::ice_trails_disabled())
    }
    fn set_ice_trails_disabled(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_ice_trails_disabled(enabled).map_err(Into::into)
    }
    fn set_ice_trail_countdown(&self, row: u8, value: rsvz_model::NonNegativeI32) -> Result<()> {
        self.world()?;
        pvzp_rs::modifier::set_ice_trail_countdown(row, value.get()).map_err(Into::into)
    }
}

impl rsvz_backend_api::WorldIdentityBackend for PortableBackend {
    fn world_identity(&self) -> Result<Option<rsvz_model::WorldId>> {
        pvzp_rs::capabilities::world_generation()
            .map(|generation| generation.map(rsvz_model::WorldId::from_raw))
            .map_err(Into::into)
    }
}

impl rsvz_backend_api::WindowRulesBackend for PortableBackend {
    fn window_left_aligned(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::window_left_aligned())
    }
    fn set_window_left_aligned(&self, enabled: bool) -> Result<()> {
        if crate::paint::active() {
            return Err(PortableBackendError::OperationRejected(
                "change viewport outside the draw stack",
            ));
        }
        pvzp_rs::modifier::set_window_left_aligned(enabled).map_err(Into::into)
    }
    fn window_expanded(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::window_expanded())
    }
    fn set_window_expanded(&self, enabled: bool) -> Result<()> {
        if crate::paint::active() {
            return Err(PortableBackendError::OperationRejected(
                "change viewport outside the draw stack",
            ));
        }
        pvzp_rs::modifier::set_window_expanded(enabled).map_err(Into::into)
    }
    fn window_scale(&self) -> Result<f64> {
        Ok(pvzp_rs::modifier::window_scale())
    }
    fn set_window_scale(&self, scale: f64) -> Result<()> {
        if crate::paint::active() {
            return Err(PortableBackendError::OperationRejected(
                "change window scale outside the draw stack",
            ));
        }
        pvzp_rs::modifier::set_window_scale(scale).map_err(Into::into)
    }
    fn window_view_width(&self) -> Result<i32> {
        Ok(pvzp_rs::modifier::window_view_width())
    }
    fn window_resizable(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::window_resizable())
    }
    fn set_window_resizable(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_window_resizable(enabled).map_err(Into::into)
    }
    fn background_run_rule(&self) -> Result<rsvz_model::BackgroundRunRule> {
        use rsvz_model::BackgroundRunRule::*;
        match pvzp_rs::modifier::background_run_rule() {
            0 => Ok(Native),
            1 => Ok(Enabled),
            2 => Ok(Disabled),
            value => Err(invalid_kind("background rule", value)),
        }
    }
    fn set_background_run_rule(&self, rule: rsvz_model::BackgroundRunRule) -> Result<()> {
        use rsvz_model::BackgroundRunRule::*;
        pvzp_rs::modifier::set_background_run_rule(match rule {
            Native => 0,
            Enabled => 1,
            Disabled => 2,
        })
        .map_err(Into::into)
    }
    fn pause_dialog_disabled(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::pause_dialog_disabled())
    }
    fn set_pause_dialog_disabled(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_pause_dialog_disabled(enabled).map_err(Into::into)
    }
}

impl rsvz_backend_api::IceTrailClearBackend for PortableBackend {
    fn clear_ice_trails(&self) -> Result<()> {
        pvzp_rs::modifier::clear_ice_trails().map_err(Into::into)
    }
}
