//! 1051's fixed two-source physical subscriptions. Entity operations stay local.
use crate::Pvz1051Backend;
use crate::error::{Pvz1051Error, Result};
use rsvz_backend_api::backend::NativeEventSink;
use rsvz_backend_api::event::ModuleEventSink;
use rsvz_backend_api::{KeyboardStateBackend, NativeEventBackend, OptionalOverlayBackend};
use rsvz_model::*;
use std::cell::Cell;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleServices {
    pub diagnostic_launch: unsafe extern "C" fn(u32, *mut u8, usize) -> usize,
    pub diagnostic_dialog: unsafe extern "C" fn(*const u8, usize) -> u32,
    pub diagnostic_exit: extern "C" fn(),
    pub click_set: extern "C" fn(u32, bool) -> u32,
    pub click_priority: extern "C" fn(bool) -> u32,
    pub click_active: extern "C" fn() -> u32,
    pub click_take: unsafe extern "C" fn(u32, *mut i32) -> u32,
    pub preview: extern "C" fn(u32, i32, bool) -> u32,

    pub damage_rule: unsafe extern "C" fn(*mut PlantDamageRule) -> u32,
    pub event_world_reset: extern "C" fn(),
    pub resize_enabled: extern "C" fn() -> bool,
    pub resize_set: extern "C" fn(value: bool) -> u32,
    pub resize_maintain: extern "C" fn() -> u32,
    pub view_original_size: unsafe extern "C" fn(*mut i32) -> bool,
    pub view_scale: unsafe extern "C" fn(*mut f64) -> u32,
    pub world_reset: extern "C" fn(u32, u32, u32, bool) -> u32,
    pub battle_start: extern "C" fn() -> u32,
    pub reset_stage: extern "C" fn() -> i32,
    pub reset_isolation: extern "C" fn() -> bool,
    pub view_enabled: extern "C" fn() -> bool,
    pub view_width: extern "C" fn() -> i32,
    pub view_left: extern "C" fn(width: i32) -> i32,
    pub view_left_aligned: extern "C" fn() -> bool,
    pub view_set_left_aligned: extern "C" fn(value: bool) -> u32,
    pub view_set: extern "C" fn(value: bool) -> u32,
    pub view_maintain: extern "C" fn() -> u32,
    pub view_set_scale: extern "C" fn(value: f64) -> u32,
    pub limit_set_sunlight_limit: extern "C" fn(value: u32) -> u32,
    pub limit_set_money_limit: extern "C" fn(value: u32) -> u32,
    pub natural_spawn: extern "C" fn(seed: u32) -> u32,

    pub canvas_cursor: unsafe extern "C" fn(x: i32, y: i32, hand: i32) -> i32,
    pub canvas_digit_bounds: unsafe extern "C" fn(*const u16, i32, i32, *mut i32, *mut i32) -> i32,
    pub canvas_rect: unsafe extern "C" fn(x: i32, y: i32, w: i32, h: i32, argb: u32) -> i32,
    pub canvas_clip: unsafe extern "C" fn(enabled: i32, x: i32, y: i32, w: i32, h: i32) -> i32,
    pub canvas_text: unsafe extern "C" fn(
        x: i32,
        y: i32,
        text: *const u8,
        length: i32,
        family: *const u16,
        pixels: i32,
        bold: i32,
        argb: u32,
        measure: i32,
        width: *mut u32,
        height: *mut u32,
    ) -> i32,

    pub bool_patch: extern "C" fn(patch: crate::patches::leases::BoolPatchId, enabled: bool) -> u32,
    pub bool_owned: extern "C" fn(patch: crate::patches::leases::BoolPatchId) -> bool,
    pub plant_special_countdown_rule: extern "C" fn(value: PlantSpecialCountdownRule) -> u32,
    pub kernel_pult_projectile_rule: extern "C" fn(value: KernelPultProjectileRule) -> u32,
    pub plant_damage_rule: extern "C" fn(value: PlantDamageRule) -> u32,
    pub maid_cheat: extern "C" fn(value: MaidCheat) -> u32,
    pub jack_explosion_rule: extern "C" fn(value: ZombieExplosionRule) -> u32,
    pub pepper_explosion_rule: extern "C" fn(value: ZombieExplosionRule) -> u32,
    pub zombie_damage_rule: extern "C" fn(value: ZombieDamageRule) -> u32,

    pub ff_start: extern "C" fn(options: FastForwardOptions) -> u32,
    pub ff_stop: extern "C" fn(reason: FastForwardStopReason) -> u32,
    pub ff_active: extern "C" fn() -> bool,
    pub chooser_start: extern "C" fn(options: SeedChooserFastForwardOptions) -> u32,
    pub chooser_stop: extern "C" fn() -> (),
    pub pause_set: extern "C" fn(enabled: bool, options: AdvancedPauseOptions) -> u32,
    pub pause_options: extern "C" fn() -> AdvancedPauseOptions,
    pub pause_active: extern "C" fn() -> bool,
    pub world_generation: extern "C" fn() -> u64,
    pub world_mutated: extern "C" fn() -> (),
    pub morph_successor: extern "C" fn(placeholder: u32) -> u32,
    pub morph_reset: extern "C" fn() -> (),
    pub random_set: extern "C" fn(mode: RandomMode) -> u32,
    pub wave_random_set: extern "C" fn(enabled: bool, seed: u32) -> u32,
    pub random_seed: extern "C" fn(kind: RandomStreamKind) -> u32,
    pub random_locked: extern "C" fn(kind: RandomStreamKind) -> bool,
    pub random_fixed: extern "C" fn(kind: RandomStreamKind) -> u32,
    pub random_note_reset: extern "C" fn(seed: u32) -> (),
    pub sun_credit: extern "C" fn(enabled: bool) -> u32,
    pub game_speed: extern "C" fn(speed: f64) -> u32,
    pub prepare_reset: extern "C" fn() -> u32,
    pub house_set: extern "C" fn(enabled: bool) -> u32,
    pub house_enabled: extern "C" fn() -> bool,
    pub house_take: extern "C" fn() -> bool,

    pub key: extern "C" fn(u32, Option<extern "C" fn(u32)>) -> u32,
    pub paint: extern "C" fn(u32, Option<extern "C" fn(u32, u32)>) -> u32,
    pub event: unsafe extern "C" fn(u32, *const ModuleEventSink) -> u32,
}
static SERVICES: ModuleServices = ModuleServices {
    diagnostic_launch: host_diagnostic_launch,
    diagnostic_dialog: host_diagnostic_dialog,
    diagnostic_exit: host_diagnostic_exit,
    click_set: host_click_set,
    click_priority: host_click_priority,
    click_active: host_click_active,
    click_take: host_click_take,
    preview: host_preview,
    damage_rule: host_damage_rule,
    event_world_reset: host_event_world_reset,
    resize_enabled: host_resize_enabled,
    resize_set: host_resize_set,
    resize_maintain: host_resize_maintain,
    view_original_size: host_view_original_size,
    view_scale: host_view_scale,
    world_reset: host_world_reset,
    battle_start: host_battle_start,
    reset_stage: host_reset_stage,
    reset_isolation: host_reset_isolation,
    view_enabled: host_view_enabled,
    view_width: host_view_width,
    view_left: host_view_left,
    view_left_aligned: host_view_left_aligned,
    view_set_left_aligned: host_view_set_left_aligned,
    view_set: host_view_set,
    view_maintain: host_view_maintain,
    view_set_scale: host_view_set_scale,
    limit_set_sunlight_limit: host_limit_set_sunlight_limit,
    limit_set_money_limit: host_limit_set_money_limit,
    natural_spawn: host_natural_spawn,

    canvas_cursor: crate::runtime::hook::paint::rsvz_canvas_cursor,
    canvas_digit_bounds: crate::runtime::hook::paint::rsvz_canvas_digit_bounds,
    canvas_rect: crate::runtime::hook::paint::rsvz_canvas_rect,
    canvas_clip: crate::runtime::hook::paint::rsvz_canvas_clip,
    canvas_text: crate::runtime::hook::paint::rsvz_canvas_text,

    bool_patch: host_bool_patch,
    bool_owned: host_bool_owned,
    plant_special_countdown_rule: host_plant_special_countdown_rule,
    kernel_pult_projectile_rule: host_kernel_pult_projectile_rule,
    plant_damage_rule: host_plant_damage_rule,
    maid_cheat: host_maid_cheat,
    jack_explosion_rule: host_jack_explosion_rule,
    pepper_explosion_rule: host_pepper_explosion_rule,
    zombie_damage_rule: host_zombie_damage_rule,

    ff_start: host_ff_start,
    ff_stop: host_ff_stop,
    ff_active: host_ff_active,
    chooser_start: host_chooser_start,
    chooser_stop: host_chooser_stop,
    pause_set: host_pause_set,
    pause_options: host_pause_options,
    pause_active: host_pause_active,
    world_generation: host_world_generation,
    world_mutated: host_world_mutated,
    morph_successor: host_morph_successor,
    morph_reset: host_morph_reset,
    random_set: host_random_set,
    wave_random_set: host_wave_random_set,
    random_seed: host_random_seed,
    random_locked: host_random_locked,
    random_fixed: host_random_fixed,
    random_note_reset: host_random_note_reset,
    sun_credit: host_sun_credit,
    game_speed: host_game_speed,
    prepare_reset: host_prepare_reset,
    house_set: host_house_set,
    house_enabled: host_house_enabled,
    house_take: host_house_take,
    key: host_key,
    paint: host_paint,
    event: host_event,
};
thread_local! {
    static CLICK_OWNER: Cell<Option<u32>> = const { Cell::new(None) };
    static PREVIEW_OWNER: Cell<Option<u32>> = const { Cell::new(None) };
    static REMOTE: Cell<Option<ModuleServices>> = const { Cell::new(None) };
    static SOURCE: Cell<u32> = const { Cell::new(0) };
    static KEY: Cell<Option<fn(u32)>> = const { Cell::new(None) };
    static EVENT: Cell<Option<NativeEventSink>> = const { Cell::new(None) };
    static KEYS: Cell<[Option<extern "C" fn(u32)>; 2]> = const { Cell::new([None;2]) };
    static PAINTS: Cell<[Option<extern "C" fn(u32, u32)>; 2]> = const { Cell::new([None;2]) };
    static EVENTS: Cell<[Option<ModuleEventSink>; 2]> = const { Cell::new([None;2]) };
    static EVENT_INSTALLED: Cell<bool> = const { Cell::new(false) };
    static TOKEN: Cell<u32> = const { Cell::new(1) };
}
pub(crate) fn services() -> Option<ModuleServices> {
    REMOTE.get()
}
impl Pvz1051Backend {
    pub fn module_services() -> *const std::ffi::c_void {
        (&raw const SERVICES).cast()
    }
    /// # Safety
    /// `services` is this backend's validated resident table, alive until shutdown.
    pub unsafe fn attach_module_services(services: *const std::ffi::c_void, source: u32) -> u32 {
        if services.is_null() || source > 1 {
            return 1;
        }
        REMOTE.set(Some(unsafe { *services.cast::<ModuleServices>() }));
        SOURCE.set(source);
        0
    }
    /// # Safety
    /// Resident host has excluded every other module/paint callback and native
    /// update; LawnApp lives on this thread and all borrows end before return.
    pub unsafe fn dispatch_module<R>(
        flags: u32, rounds: u64, call: impl FnOnce(&mut Self, crate::DispatchInput) -> R,
    ) -> R {
        call(
            &mut token(),
            crate::DispatchInput {
                stop_requested: flags & 1 != 0,
                completed_rounds: rounds,
                opening_ready: flags & 4 != 0,
                script_dispatch_allowed: flags & 8 != 0,
            },
        )
    }
}
pub(crate) fn checked(code: u32) -> Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(Pvz1051Error::OperationRejected(
            "resident physical subscription rejected",
        ))
    }
}
pub(crate) fn key(sink: Option<fn(u32)>) -> Option<Result<()>> {
    REMOTE.get().map(|host| {
        checked((host.key)(
            SOURCE.get(),
            sink.map(|_| deliver_key as extern "C" fn(u32)),
        ))?;
        KEY.set(sink);
        Ok(())
    })
}
extern "C" fn deliver_key(event: u32) {
    if let Some(sink) = KEY.get() {
        sink(event);
    }
}
fn broadcast_key(event: u32) {
    if crate::diagnostics::modal_active() {
        return;
    }
    for sink in KEYS.get().into_iter().flatten() {
        sink(event);
    }
}
extern "C" fn host_key(source: u32, sink: Option<extern "C" fn(u32)>) -> u32 {
    if source > 1 {
        return 1;
    }
    let mut next = KEYS.get();
    next[source as usize] = sink;
    if token()
        .set_key_event_sink(next.iter().any(Option::is_some).then_some(broadcast_key))
        .is_err()
    {
        return 1;
    }
    KEYS.set(next);
    0
}
pub(crate) fn paint(enabled: bool) -> Option<Result<()>> {
    REMOTE
        .get()
        .map(|host| checked((host.paint)(SOURCE.get(), enabled.then_some(deliver_paint))))
}
extern "C" fn deliver_paint(width: u32, height: u32) {
    crate::runtime::hook::paint::dispatch_module(width, height);
}
unsafe extern "C" {
    fn rsvz_canvas_source(source: u32);
    fn rsvz_canvas_clear_source(source: u32);
}
fn broadcast_paint(_: &mut Pvz1051Backend) {
    if crate::diagnostics::modal_active() {
        return;
    }
    let (w, h) = crate::runtime::hook::paint::viewport().unwrap_or((0, 0));
    for (source, sink) in PAINTS.get().into_iter().enumerate() {
        if let Some(sink) = sink {
            unsafe {
                rsvz_canvas_source(source as u32);
            }
            sink(w, h);
        }
    }
    unsafe {
        rsvz_canvas_source(0);
    }
}
extern "C" fn host_paint(source: u32, sink: Option<extern "C" fn(u32, u32)>) -> u32 {
    if source > 1 {
        return 1;
    }
    let mut next = PAINTS.get();
    next[source as usize] = sink;
    if token()
        .set_paint_entry(next.iter().any(Option::is_some).then_some(broadcast_paint))
        .is_err()
    {
        return 1;
    }
    if sink.is_none() {
        unsafe {
            rsvz_canvas_clear_source(source);
        }
    }
    PAINTS.set(next);
    0
}
pub(crate) fn event(sink: Option<NativeEventSink>) -> Option<Result<()>> {
    REMOTE.get().map(|host| {
        let wire = sink.map(|sink| ModuleEventSink {
            interest: sink.interest.raw(),
            begin_logic_frame: deliver_begin,
            begin_plant_effect: deliver_effect,
            adopt_decision: deliver_adopt,
            finish_plant_effect: deliver_finish,
            emit_activation: deliver_activation,
            emit_home_entry: deliver_home,
            emit_gargantuar_spawned: deliver_spawn,
            emit_imp_thrown: deliver_imp,
            emit_gargantuar_ash_hit: deliver_ash,
            end_logic_frame: deliver_end,
        });
        checked(unsafe { (host.event)(SOURCE.get(), wire.as_ref().map_or(std::ptr::null(), |wire| wire)) })?;
        EVENT.set(sink);
        Ok(())
    })
}
extern "C" fn deliver_begin(epoch: u64, clock: i32) {
    if let Some(s) = EVENT.get() {
        (s.begin_logic_frame)(epoch, clock);
    }
}
extern "C" fn deliver_effect(fact: PlantEffectAttemptFact, token: EventToken) -> BeginPlantEffect {
    EVENT.get().map_or(BeginPlantEffect::FAIL_OPEN, |s| {
        s.begin_with_token
            .map_or_else(|| (s.begin_plant_effect)(fact), |f| f(fact, token))
    })
}
extern "C" fn deliver_adopt(token: EventToken, decision: EventDecision) {
    if let Some(f) = EVENT.get().and_then(|s| s.adopt_decision) {
        f(token, decision);
    }
}
extern "C" fn deliver_finish(token: EventToken, outcome: PlantEffectOutcome) {
    if let Some(s) = EVENT.get() {
        (s.finish_plant_effect)(token, outcome);
    }
}
extern "C" fn deliver_activation(fact: ActivationFact) {
    if let Some(s) = EVENT.get() {
        (s.emit_activation)(fact);
    }
}
extern "C" fn deliver_home(fact: HomeEntryFact) {
    if let Some(s) = EVENT.get() {
        (s.emit_home_entry)(fact);
    }
}
extern "C" fn deliver_spawn(fact: GargantuarSpawnedFact) {
    if let Some(s) = EVENT.get() {
        (s.emit_gargantuar_spawned)(fact);
    }
}
extern "C" fn deliver_imp(fact: ImpThrownFact) {
    if let Some(s) = EVENT.get() {
        (s.emit_imp_thrown)(fact);
    }
}
extern "C" fn deliver_ash(fact: GargantuarAshHitFact) {
    if let Some(s) = EVENT.get() {
        (s.emit_gargantuar_ash_hit)(fact);
    }
}
extern "C" fn deliver_end(status: EventFrameStatus) {
    if let Some(s) = EVENT.get() {
        (s.end_logic_frame)(status);
    }
}
fn broadcast_begin(epoch: u64, clock: i32) {
    for s in EVENTS.get().into_iter().flatten() {
        (s.begin_logic_frame)(epoch, clock);
    }
}
fn broadcast_effect(fact: PlantEffectAttemptFact) -> BeginPlantEffect {
    let raw = TOKEN.get();
    TOKEN.set(raw.wrapping_add(1).max(1));
    let token = EventToken::from_raw(raw);
    let mut decision = EventDecision::Apply;
    let mut valid = false;
    for s in EVENTS.get().into_iter().flatten() {
        let result = (s.begin_plant_effect)(fact, token);
        valid |= result.token.is_valid();
        if result.decision == EventDecision::SuppressByMeasurement {
            decision = result.decision;
        }
    }
    if !valid {
        return BeginPlantEffect::FAIL_OPEN;
    }
    for s in EVENTS.get().into_iter().flatten() {
        (s.adopt_decision)(token, decision);
    }
    BeginPlantEffect { token, decision }
}
fn broadcast_finish(token: EventToken, outcome: PlantEffectOutcome) {
    for s in EVENTS.get().into_iter().flatten() {
        (s.finish_plant_effect)(token, outcome);
    }
}
fn broadcast_activation(fact: ActivationFact) {
    for s in EVENTS.get().into_iter().flatten() {
        (s.emit_activation)(fact);
    }
}
fn broadcast_home(fact: HomeEntryFact) {
    for s in EVENTS.get().into_iter().flatten() {
        (s.emit_home_entry)(fact);
    }
}
fn broadcast_spawn(fact: GargantuarSpawnedFact) {
    for s in EVENTS.get().into_iter().flatten() {
        (s.emit_gargantuar_spawned)(fact);
    }
}
fn broadcast_imp(fact: ImpThrownFact) {
    for s in EVENTS.get().into_iter().flatten() {
        (s.emit_imp_thrown)(fact);
    }
}
fn broadcast_ash(fact: GargantuarAshHitFact) {
    for s in EVENTS.get().into_iter().flatten() {
        (s.emit_gargantuar_ash_hit)(fact);
    }
}
fn broadcast_end(status: EventFrameStatus) {
    for s in EVENTS.get().into_iter().flatten() {
        (s.end_logic_frame)(status);
    }
}
unsafe extern "C" fn host_event(source: u32, sink: *const ModuleEventSink) -> u32 {
    if source > 1 {
        return 1;
    }
    let mut next = EVENTS.get();
    next[source as usize] = unsafe { sink.as_ref().copied() };
    let interest = next.iter().flatten().fold(0, |mask, sink| mask | sink.interest);
    let combined = NativeEventSink {
        interest: EventInterest::from_raw(interest),
        begin_with_token: None,
        adopt_decision: None,
        begin_logic_frame: broadcast_begin,
        begin_plant_effect: broadcast_effect,
        finish_plant_effect: broadcast_finish,
        emit_activation: broadcast_activation,
        emit_home_entry: broadcast_home,
        emit_gargantuar_spawned: broadcast_spawn,
        emit_imp_thrown: broadcast_imp,
        emit_gargantuar_ash_hit: broadcast_ash,
        emit_gargantuar_motion: None,
        end_logic_frame: broadcast_end,
    };
    let mut backend = token();
    let result = if interest == 0 {
        backend.remove_native_event_sink()
    } else if EVENT_INSTALLED.get() {
        backend.update_native_event_sink(combined)
    } else {
        backend.install_native_event_sink(combined)
    };
    if result.is_err() {
        return 1;
    }
    EVENT_INSTALLED.set(interest != 0);
    EVENTS.set(next);
    0
}

// Used only inside the resident host's serialized native/module ingress.
pub(crate) fn token() -> Pvz1051Backend {
    unsafe { Pvz1051Backend::claim_runtime_owner_token() }
}

extern "C" fn host_ff_start(options: FastForwardOptions) -> u32 {
    u32::from(crate::runtime::hook::start_fast_forward(options).is_err())
}
extern "C" fn host_ff_stop(reason: FastForwardStopReason) -> u32 {
    u32::from(crate::runtime::hook::stop_fast_forward(reason).is_err())
}
extern "C" fn host_ff_active() -> bool {
    crate::runtime::hook::fast_forward_active()
}
extern "C" fn host_chooser_start(options: SeedChooserFastForwardOptions) -> u32 {
    u32::from(crate::runtime::hook::request_seed_chooser_fast_forward(options).is_err())
}
extern "C" fn host_chooser_stop() -> () {
    crate::runtime::hook::stop_seed_chooser_fast_forward()
}
extern "C" fn host_pause_set(enabled: bool, options: AdvancedPauseOptions) -> u32 {
    u32::from(crate::runtime::hook::set_advanced_pause(&token(), enabled, options).is_err())
}
extern "C" fn host_pause_options() -> AdvancedPauseOptions {
    crate::runtime::hook::advanced_pause_options()
}
extern "C" fn host_pause_active() -> bool {
    crate::runtime::hook::advanced_pause_active()
}
extern "C" fn host_world_generation() -> u64 {
    crate::patches::world_lifetime::generation().unwrap_or(0)
}
extern "C" fn host_world_mutated() -> () {
    crate::patches::world_lifetime::note_mutation()
}
extern "C" fn host_morph_successor(placeholder: u32) -> u32 {
    token()
        .board()
        .ok()
        .and_then(|b| crate::runtime::imitator_morph::successor(b.as_ptr(), placeholder))
        .unwrap_or(0)
}
extern "C" fn host_morph_reset() -> () {
    crate::runtime::imitator_morph::reset()
}
extern "C" fn host_random_set(mode: RandomMode) -> u32 {
    u32::from(crate::patches::random::configure(mode).is_err())
}
extern "C" fn host_wave_random_set(enabled: bool, seed: u32) -> u32 {
    u32::from(crate::patches::random::configure_wave_seed(enabled.then_some(seed)).is_err())
}
extern "C" fn host_random_seed(kind: RandomStreamKind) -> u32 {
    crate::patches::random::seed(kind)
}
extern "C" fn host_random_locked(kind: RandomStreamKind) -> bool {
    crate::patches::random::locked(kind)
}
extern "C" fn host_random_fixed(kind: RandomStreamKind) -> u32 {
    crate::patches::random::fixed(kind)
}
extern "C" fn host_random_note_reset(seed: u32) -> () {
    crate::patches::random::note_reset_seed(seed)
}
extern "C" fn host_sun_credit(enabled: bool) -> u32 {
    u32::from(crate::patches::set_direct_credit(enabled).is_err())
}
extern "C" fn host_game_speed(speed: f64) -> u32 {
    u32::from(crate::runtime::hook::set_game_speed_multiplier(speed).is_err())
}
extern "C" fn host_prepare_reset() -> u32 {
    u32::from(crate::runtime::hook::prepare_world_reset().is_err())
}
extern "C" fn host_house_set(enabled: bool) -> u32 {
    u32::from(crate::patches::house_entry::set(enabled).is_err())
}
extern "C" fn host_house_enabled() -> bool {
    crate::patches::house_entry::enabled()
}
extern "C" fn host_house_take() -> bool {
    crate::patches::house_entry::take()
}

extern "C" fn host_bool_patch(patch: crate::patches::leases::BoolPatchId, enabled: bool) -> u32 {
    u32::from(crate::patches::leases::set_bool_patch(patch, enabled).is_err())
}
extern "C" fn host_bool_owned(patch: crate::patches::leases::BoolPatchId) -> bool {
    crate::patches::leases::bool_patch_owned(patch)
}
extern "C" fn host_plant_special_countdown_rule(value: PlantSpecialCountdownRule) -> u32 {
    u32::from(crate::patches::leases::set_plant_special_countdown_rule(value).is_err())
}
extern "C" fn host_kernel_pult_projectile_rule(value: KernelPultProjectileRule) -> u32 {
    u32::from(crate::patches::leases::set_kernel_pult_projectile_rule(value).is_err())
}
extern "C" fn host_plant_damage_rule(value: PlantDamageRule) -> u32 {
    u32::from(crate::patches::leases::set_plant_damage_rule(value).is_err())
}
extern "C" fn host_maid_cheat(value: MaidCheat) -> u32 {
    u32::from(crate::patches::leases::set_maid_cheat(value).is_err())
}
extern "C" fn host_jack_explosion_rule(value: ZombieExplosionRule) -> u32 {
    u32::from(crate::patches::leases::set_jack_explosion_rule(value).is_err())
}
extern "C" fn host_pepper_explosion_rule(value: ZombieExplosionRule) -> u32 {
    u32::from(crate::patches::leases::set_pepper_explosion_rule(value).is_err())
}
extern "C" fn host_zombie_damage_rule(value: ZombieDamageRule) -> u32 {
    u32::from(crate::patches::leases::set_zombie_damage_rule(value).is_err())
}

extern "C" fn host_resize_enabled() -> bool {
    crate::patches::window_resize::enabled()
}
extern "C" fn host_resize_set(value: bool) -> u32 {
    u32::from(crate::patches::window_resize::set(value).is_err())
}
extern "C" fn host_resize_maintain() -> u32 {
    u32::from(crate::patches::window_resize::maintain().is_err())
}
extern "C" fn host_view_enabled() -> bool {
    crate::patches::window_view::enabled()
}
extern "C" fn host_view_width() -> i32 {
    crate::patches::window_view::width()
}
extern "C" fn host_view_left(width: i32) -> i32 {
    crate::patches::window_view::left(width)
}
extern "C" fn host_view_left_aligned() -> bool {
    crate::patches::window_view::left_aligned()
}
extern "C" fn host_view_set_left_aligned(value: bool) -> u32 {
    u32::from(crate::patches::window_view::set_left_aligned(value).is_err())
}
extern "C" fn host_view_set(value: bool) -> u32 {
    u32::from(crate::patches::window_view::set(value).is_err())
}
extern "C" fn host_view_maintain() -> u32 {
    u32::from(crate::patches::window_view::maintain().is_err())
}
extern "C" fn host_view_set_scale(value: f64) -> u32 {
    u32::from(crate::patches::window_view::set_scale(value).is_err())
}
extern "C" fn host_limit_set_sunlight_limit(value: u32) -> u32 {
    u32::from(crate::patches::pickup_limits::set_sunlight_limit(value).is_err())
}
extern "C" fn host_limit_set_money_limit(value: u32) -> u32 {
    u32::from(crate::patches::pickup_limits::set_money_limit(value).is_err())
}
extern "C" fn host_natural_spawn(seed: u32) -> u32 {
    u32::from(rsvz_backend_api::CurrentNaturalSpawnBackend::generate_current_natural_spawn(&token(), seed).is_err())
}

unsafe extern "C" fn host_damage_rule(output: *mut PlantDamageRule) -> u32 {
    if output.is_null() {
        return 1;
    }
    match crate::impls::event::damage_rule() {
        Ok(rule) => {
            unsafe {
                output.write(rule);
            }
            0
        }
        Err(_) => 1,
    }
}
extern "C" fn host_event_world_reset() {
    crate::impls::event::note_world_reset();
}

pub(crate) fn source() -> u32 {
    SOURCE.get()
}
extern "C" fn host_click_set(source: u32, enabled: bool) -> u32 {
    use rsvz_backend_api::BoardClickBackend;
    if source > 1 || (enabled && CLICK_OWNER.get().is_some_and(|owner| owner != source)) {
        return 1;
    }
    if !enabled && CLICK_OWNER.get().is_some_and(|owner| owner != source) {
        return 0;
    }
    if token().set_board_click_capture(enabled).is_err() {
        return 1;
    }
    CLICK_OWNER.set(enabled.then_some(source));
    0
}
extern "C" fn host_click_priority(enabled: bool) -> u32 {
    use rsvz_backend_api::BoardClickBackend;
    u32::from(token().set_board_click_native_priority(enabled).is_err())
}
extern "C" fn host_click_active() -> u32 {
    use rsvz_backend_api::BoardClickBackend;
    token().board_click_capture_active().map_or(2, u32::from)
}
unsafe extern "C" fn host_click_take(source: u32, output: *mut i32) -> u32 {
    use rsvz_backend_api::BoardClickBackend;
    if output.is_null() {
        return 2;
    }
    if CLICK_OWNER.get() != Some(source) {
        return 0;
    }
    match token().take_board_click() {
        Ok(Some((x, y, button))) => {
            unsafe {
                output.write(x);
                output.add(1).write(y);
                output.add(2).write(button);
            }
            1
        }
        Ok(None) => 0,
        Err(_) => 2,
    }
}
extern "C" fn host_preview(source: u32, kind: i32, imitate: bool) -> u32 {
    use rsvz_backend_api::BoardClickBackend;
    if source > 1 {
        return 1;
    }
    let preview = match kind {
        -1 => None,
        -2 => Some(LawnPreview::Shovel),
        0..=50 => match PlantPlacementSelection::new(kind as u8, imitate) {
            Ok(plant) => Some(LawnPreview::Plant(plant)),
            Err(_) => return 1,
        },
        _ => return 1,
    };
    if preview.is_none() && PREVIEW_OWNER.get().is_some_and(|owner| owner != source) {
        return 0;
    }
    if token().set_lawn_preview(preview).is_err() {
        return 1;
    }
    PREVIEW_OWNER.set(preview.map(|_| source));
    0
}

unsafe extern "C" fn host_view_original_size(output: *mut i32) -> bool {
    if output.is_null() {
        return false;
    }
    let Some((width, height)) = crate::patches::window_view::original_size() else {
        return false;
    };
    unsafe {
        output.write(width);
        output.add(1).write(height);
    }
    true
}
unsafe extern "C" fn host_view_scale(output: *mut f64) -> u32 {
    if output.is_null() {
        return 1;
    }
    match crate::patches::window_view::scale() {
        Ok(value) => {
            unsafe {
                output.write(value);
            }
            0
        }
        Err(_) => 1,
    }
}
extern "C" fn host_world_reset(rounds: u32, seed: u32, sun: u32, ready: bool) -> u32 {
    use rsvz_backend_api::WorldResetBackend;
    u32::from(
        token()
            .reset_world(WorldResetConfig {
                completed_rounds: rounds,
                seed,
                initial_sun: sun,
                card_cooldowns: if ready {
                    ResetCardCooldowns::Ready
                } else {
                    ResetCardCooldowns::PreserveNative
                },
            })
            .is_err(),
    )
}
extern "C" fn host_battle_start() -> u32 {
    use rsvz_backend_api::BattleEntryBackend;
    u32::from(token().start_battle().is_err())
}
extern "C" fn host_reset_stage() -> i32 {
    crate::impls::reset::pending_world_stage().unwrap_or(-1)
}
extern "C" fn host_reset_isolation() -> bool {
    crate::impls::reset::profile_isolation_active()
}

pub(crate) fn diagnostic_launch() -> Option<rsvz_backend_api::diagnostics::DiagnosticLaunch> {
    REMOTE.get().and_then(|host| {
        let length = unsafe { (host.diagnostic_launch)(SOURCE.get(), std::ptr::null_mut(), 0) };
        if length == 0 || length > 65536 {
            return None;
        }
        let mut bytes = vec![0; length];
        let written = unsafe { (host.diagnostic_launch)(SOURCE.get(), bytes.as_mut_ptr(), bytes.len()) };
        if written != length {
            return None;
        }
        serde_json::from_slice(&bytes).ok()
    })
}
pub(crate) fn diagnostic_dialog(message: &str) -> rsvz_backend_api::diagnostics::DialogAction {
    let action = match REMOTE.get() {
        Some(host) => unsafe { (host.diagnostic_dialog)(message.as_ptr(), message.len()) },
        None => crate::diagnostics::show_dialog(message) as u32,
    };
    rsvz_backend_api::diagnostics::DialogAction::from_code(action)
}
pub(crate) fn diagnostic_exit() {
    if let Some(host) = REMOTE.get() {
        (host.diagnostic_exit)();
    } else {
        crate::diagnostics::request_exit();
    }
}
unsafe extern "C" fn host_diagnostic_launch(source: u32, output: *mut u8, capacity: usize) -> usize {
    std::panic::catch_unwind(|| {
        let mut launch = crate::diagnostics::local_launch();
        launch.source = if source == 1 { "modifier" } else { "script" }.into();
        let Ok(bytes) = serde_json::to_vec(&launch) else {
            return 0;
        };
        if !output.is_null() && capacity >= bytes.len() {
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
            }
        }
        bytes.len()
    })
    .unwrap_or(0)
}
unsafe extern "C" fn host_diagnostic_dialog(bytes: *const u8, length: usize) -> u32 {
    std::panic::catch_unwind(|| {
        if bytes.is_null() || length > 65536 {
            return 0;
        }
        let Ok(message) = std::str::from_utf8(unsafe { std::slice::from_raw_parts(bytes, length) }) else {
            return 0;
        };
        crate::diagnostics::show_dialog(message) as u32
    })
    .unwrap_or(0)
}
extern "C" fn host_diagnostic_exit() {
    let _ = std::panic::catch_unwind(crate::diagnostics::request_exit);
}
