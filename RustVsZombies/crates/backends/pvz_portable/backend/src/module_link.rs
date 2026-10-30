//! Portable's fixed two-source physical subscriptions. Entity operations stay local.
use crate::{PortableBackend, PortableBackendError, Result};
use rsvz_backend_api::backend::NativeEventSink;
use rsvz_backend_api::event::ModuleEventSink;
use rsvz_backend_api::{KeyboardStateBackend, NativeEventBackend, OptionalOverlayBackend};
use rsvz_model::*;
use std::cell::{Cell, RefCell};

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

    pub key: extern "C" fn(u32, Option<extern "C" fn(u32)>) -> u32,
    pub paint: extern "C" fn(u32, Option<extern "C" fn(u32, u32)>) -> u32,
    pub event: unsafe extern "C" fn(u32, *const ModuleEventSink) -> u32,
    pub subscription_error: unsafe extern "C" fn(*mut u8, usize) -> usize,
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
    key: host_key,
    paint: host_paint,
    event: host_event,
    subscription_error: host_subscription_error,
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
    static SUBSCRIPTION_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
    static TOKEN: Cell<u32> = const { Cell::new(1) };
}
pub(crate) fn services() -> Option<ModuleServices> {
    REMOTE.get()
}
pub(crate) fn attached() -> bool {
    REMOTE.get().is_some()
}
impl PortableBackend {
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
            &mut Self::new(),
            crate::DispatchInput {
                stop_requested: flags & 1 != 0,
                world_replaced: flags & 2 != 0,
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
    } else if code == 0x1002 && REMOTE.get().is_some() {
        let host = REMOTE.get().unwrap();
        let mut bytes = [0u8; 4096];
        let length = unsafe { (host.subscription_error)(bytes.as_mut_ptr(), bytes.len()) };
        Err(PortableBackendError::SubscriptionRejected(
            String::from_utf8_lossy(&bytes[..length.min(bytes.len())]).into_owned(),
        ))
    } else {
        Err(PortableBackendError::OperationRejected(
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
    if PortableBackend::new()
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
extern "C" fn deliver_paint(_width: u32, _height: u32) {
    unsafe { crate::paint::dispatch() };
}
fn broadcast_paint(_: &mut PortableBackend) {
    if crate::diagnostics::modal_active() {
        return;
    }
    for (source, sink) in PAINTS.get().into_iter().enumerate() {
        if let Some(sink) = sink {
            unsafe {
                pvzp_rs::raw::pvzp_rs_paint_source(source as u32);
            }
            sink(0, 0);
        }
    }
    unsafe {
        pvzp_rs::raw::pvzp_rs_paint_source(0);
    }
}
extern "C" fn host_paint(source: u32, sink: Option<extern "C" fn(u32, u32)>) -> u32 {
    if source > 1 {
        return 1;
    }
    let mut next = PAINTS.get();
    next[source as usize] = sink;
    if PortableBackend::new()
        .set_paint_entry(next.iter().any(Option::is_some).then_some(broadcast_paint))
        .is_err()
    {
        return 1;
    }
    if sink.is_none() {
        unsafe {
            pvzp_rs::raw::pvzp_rs_paint_clear_source(source);
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
    let mut backend = PortableBackend::new();
    let result = if interest == 0 {
        backend.remove_native_event_sink()
    } else if crate::event::installed_interest().is_some() {
        backend.update_native_event_sink(combined)
    } else {
        backend.install_native_event_sink(combined)
    };
    if let Err(error) = result {
        let message = format!(
            "resident event subscription rejected: source={source}, operation={}, previous={:#x?}, requested={:#x?}, combined={interest:#x}, installed={:#x?}, native_frame={:?}, board_available={}; cause: {error}",
            if sink.is_null() { "remove" } else { "set" },
            EVENTS.get().map(|s| s.map(|s| s.interest)),
            next.map(|s| s.map(|s| s.interest)),
            crate::event::installed_interest(),
            crate::event::frame_counter(),
            unsafe { pvzp_rs::World::current() }.is_ok(),
        );
        PortableBackend::diagnostic_fallback(&message);
        SUBSCRIPTION_ERROR.with_borrow_mut(|slot| *slot = message);
        return 0x1002;
    }
    EVENTS.set(next);
    0
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
    if PortableBackend::new().set_board_click_capture(enabled).is_err() {
        return 1;
    }
    CLICK_OWNER.set(enabled.then_some(source));
    0
}
extern "C" fn host_click_priority(enabled: bool) -> u32 {
    use rsvz_backend_api::BoardClickBackend;
    u32::from(PortableBackend::new().set_board_click_native_priority(enabled).is_err())
}
extern "C" fn host_click_active() -> u32 {
    use rsvz_backend_api::BoardClickBackend;
    PortableBackend::new().board_click_capture_active().map_or(2, u32::from)
}
unsafe extern "C" fn host_click_take(source: u32, output: *mut i32) -> u32 {
    use rsvz_backend_api::BoardClickBackend;
    if output.is_null() {
        return 2;
    }
    if CLICK_OWNER.get() != Some(source) {
        return 0;
    }
    match PortableBackend::new().take_board_click() {
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
    if PortableBackend::new().set_lawn_preview(preview).is_err() {
        return 1;
    }
    PREVIEW_OWNER.set(preview.map(|_| source));
    0
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

// Copies bytes only: no Rust allocation or ownership crosses the DLL boundary.
unsafe extern "C" fn host_subscription_error(output: *mut u8, capacity: usize) -> usize {
    SUBSCRIPTION_ERROR.with_borrow(|message| {
        let length = message.len().min(capacity);
        if !output.is_null() {
            unsafe { std::ptr::copy_nonoverlapping(message.as_ptr(), output, length) };
        }
        length
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wire(interest: u32) -> ModuleEventSink {
        ModuleEventSink {
            interest,
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
        }
    }
    #[test]
    fn destruction_with_two_sources_allows_removal_and_reentry() {
        crate::event::clear_native_event_sink();
        EVENTS.set([None, None]);
        let script = wire(EventInterest::HOME_ENTRY.raw());
        let modifier = wire(EventInterest::BITE.raw());
        for _ in 0..3 {
            assert_eq!(unsafe { host_event(0, &script) }, 0);
            assert_eq!(unsafe { host_event(1, &modifier) }, 0);
            assert_eq!(
                crate::event::installed_interest(),
                Some(script.interest | modifier.interest)
            );
            // The native Board destruction callback revokes physical callbacks first.
            crate::event::clear_native_event_sink();
            assert_eq!(unsafe { host_event(0, std::ptr::null()) }, 0);
            assert_eq!(crate::event::installed_interest(), Some(modifier.interest));
            assert_eq!(unsafe { host_event(1, std::ptr::null()) }, 0);
            assert_eq!(crate::event::installed_interest(), None);
        }
    }
    #[test]
    fn rejection_preserves_cause_and_does_not_commit_requested_membership() {
        crate::event::clear_native_event_sink();
        EVENTS.set([None, None]);
        let invalid = wire(EventInterest::GARGANTUAR_MOTION.raw());
        assert_eq!(unsafe { host_event(0, &invalid) }, 0x1002);
        assert!(EVENTS.get().iter().all(Option::is_none));
        let mut bytes = [0; 4096];
        let len = unsafe { host_subscription_error(bytes.as_mut_ptr(), bytes.len()) };
        let message = std::str::from_utf8(&bytes[..len]).unwrap();
        assert!(message.contains("Gargantuar motion events"));
        assert!(message.contains("source=0"));
        assert!(message.contains("native_frame=None"));
    }
}
