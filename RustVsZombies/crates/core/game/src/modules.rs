//! Registration and execution identities for one script and one modifier.
//! The concrete backend still owns the sole physical update/hook. The loader
//! owns DLL handles; it may release one only after `unregister` succeeds.

use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_current::{CurrentBackend, DispatchInput, DispatchResult};
use rsvz_schedule::module::{ModuleSlot, with_module};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
};

pub type ModuleEntry = fn(&mut CurrentBackend, DispatchInput) -> DispatchResult;

/// Exported by modules built for the same native ABI and source/layout identity.
/// The loader validates target and artifact hashes before mapping, then checks
/// this fixed header before calling the C-ABI entries.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleDescriptor {
    pub magic: [u8; 8],
    pub abi_version: u32,
    pub descriptor_size: u32,
    pub sdk_sha256: [u8; 32],
    /// 0 = ordinary script, 1 = modifier.
    pub kind: u32,
    /// The module promises no unmanaged threads, callbacks or TLS destructors.
    /// Without this contract, Stop is supported but native unloading is refused.
    pub managed_unload: u32,
    pub attach: unsafe extern "C" fn(*const ExecutionHost, *const std::ffi::c_void, u32, u64) -> u32,
    pub dispatch: extern "C" fn(u32, u64) -> u32,
    pub terminal: unsafe extern "C" fn(u32, *mut u8, usize) -> usize,
    pub boundary: extern "C" fn() -> u32,
    pub detach: extern "C" fn() -> u32,
}

pub const MODULE_MAGIC: [u8; 8] = *b"RSVZMOD2";
pub const MODULE_ABI_VERSION: u32 = 5;

/// Only execution identity and start/stop coordination cross this boundary.
/// No Rust ownership, game objects, scheduler state or user callback is transferred.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ExecutionHost {
    pub read_restore: unsafe extern "C" fn(*mut rsvz_model::RestoreReceipt) -> bool,
    pub publish_restore: extern "C" fn(rsvz_model::RestoreReceipt),
    pub execution: extern "C" fn(u32, u32, u64) -> u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionId {
    pub module: ModuleSlot,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionState {
    Running,
    StopRequested,
    Stopped,
    CleanupFailed,
}

/// Contains only SDK-owned bytes and strings, never module vtables or str slices.
pub struct TerminalResult {
    pub execution: ExecutionId,
    pub artifact_json: Option<Vec<u8>>,
    pub error: Option<String>,
}

#[derive(Default)]
struct PendingBoundaries {
    replaced: bool,
    rounds: u64,
}
impl PendingBoundaries {
    fn observe(&mut self, replaced: bool, rounds: u64) {
        self.replaced |= replaced;
        self.rounds = self.rounds.saturating_add(rounds);
    }
    fn take(&mut self) -> (bool, u64) {
        let pending = std::mem::take(self);
        (pending.replaced, pending.rounds)
    }
}

struct Module {
    id: ExecutionId,
    entry: ModuleDescriptor,
    services: *const std::ffi::c_void,
    state: ExecutionState,
    terminal: Option<TerminalResult>,
    pending: PendingBoundaries,
    unload_requested: bool,
}

thread_local! {
    static MODULES: RefCell<[Option<Module>; 2]> = const { RefCell::new([None, None]) };
    static NEXT_EXECUTION: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
    static DISPATCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static START_BLOCKS: std::cell::Cell<[u32; 2]> = const { std::cell::Cell::new([0; 2]) };
    static REMOTE: std::cell::Cell<Option<ExecutionHost>> = const { std::cell::Cell::new(None) };
    static LOCAL_SOURCE: std::cell::Cell<ModuleSlot> = const { std::cell::Cell::new(ModuleSlot::Script) };
    static LOCAL_DIAGNOSTIC_END: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static LOCAL_TERMINAL: RefCell<(Option<Vec<u8>>, Option<String>)> = const { RefCell::new((None, None)) };
    static REPLAY_RECEIPT: std::cell::Cell<Option<rsvz_model::RestoreReceipt>> = const { std::cell::Cell::new(None) };
    static BOUNDARY_OWNER: std::cell::Cell<Option<ModuleSlot>> = const { std::cell::Cell::new(None) };
}

static HOST: ExecutionHost = ExecutionHost {
    execution: execution_request,
    read_restore,
    publish_restore,
};

extern "C" fn execution_request(command: u32, source: u32, generation: u64) -> u64 {
    let module = match source {
        0 => ModuleSlot::Script,
        1 => ModuleSlot::Modifier,
        _ => return u64::MAX,
    };
    let id = ExecutionId { module, generation };
    match command {
        0 => current_execution(module).map_or(0, |id| id.generation),
        1 => state(id).map_or(u64::MAX, |s| match s {
            ExecutionState::Running => 0,
            ExecutionState::StopRequested => 1,
            ExecutionState::Stopped => 2,
            ExecutionState::CleanupFailed => 3,
        }),
        2 => {
            if request_stop(id).is_ok() {
                0
            } else {
                u64::MAX
            }
        }
        3 => {
            if request_unload(id).is_ok() {
                0
            } else {
                u64::MAX
            }
        }
        4 => u64::from(start_blocked(module)),
        5 => {
            let mut blocks = START_BLOCKS.get();
            let Some(next) = blocks[index(module)].checked_add(1) else {
                return u64::MAX;
            };
            blocks[index(module)] = next;
            START_BLOCKS.set(blocks);
            0
        }
        6 => {
            let mut blocks = START_BLOCKS.get();
            let Some(next) = blocks[index(module)].checked_sub(1) else {
                return u64::MAX;
            };
            blocks[index(module)] = next;
            START_BLOCKS.set(blocks);
            0
        }
        7 => {
            if BOUNDARY_OWNER.get().is_some_and(|owner| owner != module) {
                return u64::MAX;
            }
            BOUNDARY_OWNER.set(Some(module));
            0
        }
        8 => {
            if BOUNDARY_OWNER.get() == Some(module) {
                BOUNDARY_OWNER.set(None);
            }
            0
        }
        _ => u64::MAX,
    }
}

fn remote(command: u32, source: ModuleSlot, generation: u64) -> Option<u64> {
    REMOTE
        .get()
        .map(|host| (host.execution)(command, index(source) as u32, generation))
}

pub(crate) fn claim_native_boundary() -> RuntimeResult<()> {
    let source = rsvz_schedule::module::current_module();
    if remote(7, source, 0).unwrap_or_else(|| execution_request(7, index(source) as u32, 0)) != 0 {
        return Err(RuntimeError::new("native boundary is owned by the other execution"));
    }
    Ok(())
}
pub(crate) fn release_native_boundary() {
    let source = rsvz_schedule::module::current_module();
    if remote(8, source, 0).is_none() {
        execution_request(8, index(source) as u32, 0);
    }
}

pub fn execution_boundary(backend: &mut CurrentBackend) -> u32 {
    with_module(LOCAL_SOURCE.get(), || {
        match catch_unwind(AssertUnwindSafe(|| {
            let saved = crate::saved_game::poll_at_boundary(backend)?;
            crate::replay::poll_at_boundary(backend).map(|replay| saved | replay)
        })) {
            Ok(result) => u32::from(result.unwrap_or(true)),
            Err(payload) => {
                drop(payload);
                crate::session::fail_script(RuntimeError::new("Replay boundary panicked"));
                1
            }
        }
    })
}

// Native callbacks run outside dispatch_execution's source scope.
pub(crate) fn with_local_source<R>(body: impl FnOnce() -> R) -> R {
    with_module(LOCAL_SOURCE.get(), body)
}

/// Called by the module's fixed C entry, at a resident-host safe point.
/// # Safety
/// The table must be supplied by the validated resident host. Its callbacks
/// obey the execution ABI and remain callable until this DLL is detached.
pub unsafe fn attach_execution(host: ExecutionHost, source: u32, generation: u64) -> u32 {
    if source > 1 || generation == 0 {
        return 1;
    }
    let source = if source == 0 {
        ModuleSlot::Script
    } else {
        ModuleSlot::Modifier
    };
    REMOTE.set(Some(host));
    LOCAL_SOURCE.set(source);
    LOCAL_TERMINAL.with_borrow_mut(|value| *value = (None, None));
    LOCAL_DIAGNOSTIC_END.with_borrow_mut(Vec::clear);
    with_module(source, || {
        crate::session::reset_session_control();
        crate::lifecycle::reset_session_resources();
        crate::frame::set_runtime_dispatch_state(crate::session::RuntimeDispatchState::new());
    });
    0
}

/// All payload conversion and destruction happens inside the producing DLL.
pub fn dispatch_execution(entry: ModuleEntry, backend: &mut CurrentBackend, input: DispatchInput) -> u32 {
    with_module(LOCAL_SOURCE.get(), || {
        let result = catch_unwind(AssertUnwindSafe(|| entry(backend, input)));
        match result {
            Ok(DispatchResult::Continue) => 0,
            Ok(DispatchResult::Idle) => 1,
            Ok(DispatchResult::SkipUpdate) => 2,
            Ok(DispatchResult::EnterBattle) => 3,
            terminal => {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    let Ok(DispatchResult::Stop { artifact, error, .. }) = terminal else {
                        return Err("module dispatch panicked; DLL remains mapped".to_owned());
                    };
                    LOCAL_DIAGNOSTIC_END.with_borrow_mut(|bytes| {
                        *bytes = serde_json::to_vec(&CurrentBackend::diagnostic_finish()).unwrap_or_default();
                    });
                    let completed = crate::lifecycle::with_lifecycle(|state| state.cleanup_complete());
                    let json = artifact
                        .map(|artifact| {
                            let mut bytes = Vec::new();
                            artifact.write_json(&mut bytes).map(|()| bytes)
                        })
                        .transpose()
                        .map_err(|error| error.to_string())?;
                    let error = error.map(|error| error.to_string());
                    LOCAL_TERMINAL.with_borrow_mut(|value| *value = (json, error));
                    if !completed {
                        return Err("module cleanup incomplete; DLL remains mapped".into());
                    }
                    crate::frame::reset_runtime_state_preserving_backend();
                    crate::session::reset_session_control();
                    crate::diagnostics::reset_logger();
                    Ok(())
                }));
                match result {
                    Ok(Ok(())) => 4,
                    failure => {
                        let message = match failure {
                            Ok(Err(error)) => error,
                            _ => "module cleanup panicked; DLL remains mapped".into(),
                        };
                        LOCAL_TERMINAL.with_borrow_mut(|value| match &mut value.1 {
                            Some(error) => {
                                error.push_str("; ");
                                error.push_str(&message);
                            }
                            None => value.1 = Some(message),
                        });
                        5
                    }
                }
            }
        }
    })
}

/// The caller provides writable storage; ownership never crosses the ABI.
/// # Safety
/// A non-null output must address at least `capacity` writable bytes.
pub unsafe extern "C" fn copy_terminal(kind: u32, output: *mut u8, capacity: usize) -> usize {
    if kind == 2 {
        return LOCAL_DIAGNOSTIC_END.with_borrow(|bytes| {
            if !output.is_null() && capacity >= bytes.len() {
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
                }
            }
            bytes.len()
        });
    }
    LOCAL_TERMINAL.with_borrow(|value| {
        let bytes = if kind == 0 {
            value.0.as_deref()
        } else {
            value.1.as_ref().map(|s| s.as_bytes())
        };
        let Some(bytes) = bytes else {
            return 0;
        };
        if !output.is_null() && capacity >= bytes.len() {
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len()) };
        }
        bytes.len()
    })
}

/// Prevents starting/restarting one existing execution slot while the caller
/// performs an exclusive operation. It does not choose or stop an execution.
pub struct StartBlock {
    source: ModuleSlot,
    _game_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
pub fn start_blocked(source: ModuleSlot) -> bool {
    if let Some(value) = remote(4, source, 0) {
        return value != 0;
    }
    START_BLOCKS.get()[index(source)] != 0
}
pub(crate) fn dispatching() -> bool {
    REMOTE.get().is_some() || DISPATCHING.get()
}
pub fn block_start(source: ModuleSlot) -> RuntimeResult<StartBlock> {
    if let Some(value) = remote(5, source, 0) {
        if value != 0 {
            return Err(RuntimeError::new("execution start-block count exhausted"));
        }
        return Ok(StartBlock {
            source,
            _game_thread: std::marker::PhantomData,
        });
    }
    let mut blocks = START_BLOCKS.get();
    blocks[index(source)] = blocks[index(source)]
        .checked_add(1)
        .ok_or_else(|| RuntimeError::new("execution start-block count exhausted"))?;
    START_BLOCKS.set(blocks);
    Ok(StartBlock {
        source,
        _game_thread: std::marker::PhantomData,
    })
}
impl Drop for StartBlock {
    fn drop(&mut self) {
        if remote(6, self.source, 0).is_some() {
            return;
        }
        let mut blocks = START_BLOCKS.get();
        blocks[index(self.source)] -= 1;
        START_BLOCKS.set(blocks);
    }
}

fn index(module: ModuleSlot) -> usize {
    match module {
        ModuleSlot::Script => 0,
        ModuleSlot::Modifier => 1,
    }
}

fn next_id(module: ModuleSlot) -> RuntimeResult<ExecutionId> {
    let generation = NEXT_EXECUTION.get();
    NEXT_EXECUTION.set(
        generation
            .checked_add(1)
            .ok_or_else(|| RuntimeError::new("module execution identity exhausted"))?,
    );
    Ok(ExecutionId { module, generation })
}

/// Descriptor has already passed artifact, ABI, layout and backend validation.
/// The loader retains the DLL until unregister and final detach both succeed.
/// # Safety
/// Descriptor callbacks and the backend service table must obey the validated
/// module ABI, and their DLLs must remain mapped through final detach.
pub unsafe fn register_external(
    module: ModuleSlot, descriptor: ModuleDescriptor, services: *const std::ffi::c_void,
) -> RuntimeResult<ExecutionId> {
    if start_blocked(module) || DISPATCHING.get() {
        return Err(RuntimeError::new(
            "module registration requires an unblocked safe point",
        ));
    }
    let id = next_id(module)?;
    MODULES.with_borrow_mut(|modules| {
        if modules[index(module)].is_some() {
            return Err(RuntimeError::new("module slot is occupied"));
        }
        modules[index(module)] = Some(Module {
            id,
            entry: descriptor,
            services,
            state: ExecutionState::Running,
            terminal: None,
            pending: PendingBoundaries::default(),
            unload_requested: false,
        });
        Ok(())
    })?;
    CurrentBackend::diagnostic_clear_end(index(module));
    if unsafe { (descriptor.attach)(&raw const HOST, services, index(module) as u32, id.generation) } != 0 {
        MODULES
            .with_borrow_mut(|modules| modules[index(module)].as_mut().unwrap().state = ExecutionState::CleanupFailed);
        return Err(RuntimeError::new("module attach failed; DLL remains mapped"));
    }
    Ok(id)
}

pub fn state(id: ExecutionId) -> RuntimeResult<ExecutionState> {
    if let Some(value) = remote(1, id.module, id.generation) {
        return match value {
            0 => Ok(ExecutionState::Running),
            1 => Ok(ExecutionState::StopRequested),
            2 => Ok(ExecutionState::Stopped),
            3 => Ok(ExecutionState::CleanupFailed),
            _ => Err(RuntimeError::new("stale module execution")),
        };
    }
    MODULES.with_borrow(|modules| {
        modules[index(id.module)]
            .as_ref()
            .filter(|module| module.id == id)
            .map(|module| module.state)
            .ok_or_else(|| RuntimeError::new("stale module execution"))
    })
}

/// Identifies the currently loaded module without exposing its entry or DLL.
pub fn current_execution(source: ModuleSlot) -> Option<ExecutionId> {
    if let Some(generation) = remote(0, source, 0) {
        return (generation != 0).then_some(ExecutionId {
            module: source,
            generation,
        });
    }
    MODULES.with_borrow(|modules| modules[index(source)].as_ref().map(|module| module.id))
}

/// A peer dispatch needs to observe resolved Opening declarations before the
/// current source performs native selection/start. This grants no precedence.
pub(crate) fn has_running_peer() -> bool {
    let source = rsvz_schedule::module::current_module();
    if REMOTE.get().is_some() {
        let peer = if source == ModuleSlot::Script {
            ModuleSlot::Modifier
        } else {
            ModuleSlot::Script
        };
        return current_execution(peer)
            .and_then(|id| state(id).ok())
            .is_some_and(|state| matches!(state, ExecutionState::Running | ExecutionState::StopRequested));
    }
    let peer = 1 - index(source);
    MODULES.with_borrow(|modules| {
        modules[peer]
            .as_ref()
            .is_some_and(|module| matches!(module.state, ExecutionState::Running | ExecutionState::StopRequested))
    })
}

pub fn request_stop(id: ExecutionId) -> RuntimeResult<()> {
    if let Some(value) = remote(2, id.module, id.generation) {
        return if value == 0 {
            Ok(())
        } else {
            Err(RuntimeError::new("cannot stop this execution"))
        };
    }
    MODULES.with_borrow_mut(|modules| {
        let module = modules[index(id.module)]
            .as_mut()
            .filter(|module| module.id == id)
            .ok_or_else(|| RuntimeError::new("stale module execution"))?;
        if module.state == ExecutionState::Running {
            module.state = ExecutionState::StopRequested;
        }
        if module.state == ExecutionState::CleanupFailed {
            return Err(RuntimeError::new("module cleanup failed; DLL remains mapped"));
        }
        Ok(())
    })
}

/// Request teardown followed by physical unloading. The loader must still verify
/// its managed-resource contract, successful cleanup and an inactive callback stack.
/// Plain stopping never implies this request, regardless of the module's role.
pub fn request_unload(id: ExecutionId) -> RuntimeResult<()> {
    if let Some(value) = remote(3, id.module, id.generation) {
        return if value == 0 {
            Ok(())
        } else {
            Err(RuntimeError::new("cannot unload this execution"))
        };
    }
    request_stop(id)?;
    MODULES.with_borrow_mut(|modules| {
        modules[index(id.module)]
            .as_mut()
            .expect("validated execution")
            .unload_requested = true;
    });
    Ok(())
}

pub fn unload_requested(id: ExecutionId) -> RuntimeResult<bool> {
    MODULES.with_borrow(|modules| {
        modules[index(id.module)]
            .as_ref()
            .filter(|module| module.id == id)
            .map(|module| module.unload_requested)
            .ok_or_else(|| RuntimeError::new("stale module execution"))
    })
}

pub fn take_terminal(id: ExecutionId) -> RuntimeResult<Option<TerminalResult>> {
    MODULES.with_borrow_mut(|modules| {
        modules[index(id.module)]
            .as_mut()
            .filter(|module| module.id == id)
            .map(|module| module.terminal.take())
            .ok_or_else(|| RuntimeError::new("stale module execution"))
    })
}

/// Leaves the DLL loaded and establishes a new execution identity. No level reset.
pub fn rerun(id: ExecutionId) -> RuntimeResult<ExecutionId> {
    if start_blocked(id.module) {
        return Err(RuntimeError::new(
            "execution slot is temporarily blocked from restarting",
        ));
    }
    if DISPATCHING.get() {
        return Err(RuntimeError::new("rerun modules at a host safe point"));
    }
    let (entry, services) = MODULES.with_borrow(|modules| {
        let module = modules[index(id.module)]
            .as_ref()
            .filter(|module| module.id == id)
            .ok_or_else(|| RuntimeError::new("stale module execution"))?;
        if module.state != ExecutionState::Stopped || module.unload_requested {
            return Err(RuntimeError::new("module is not ready to restart"));
        }
        Ok((module.entry, module.services))
    })?;
    let next = next_id(id.module)?;
    CurrentBackend::diagnostic_clear_end(index(id.module));
    if unsafe { (entry.attach)(&raw const HOST, services, index(id.module) as u32, next.generation) } != 0 {
        MODULES.with_borrow_mut(|modules| {
            modules[index(id.module)].as_mut().unwrap().state = ExecutionState::CleanupFailed;
        });
        return Err(RuntimeError::new("module restart failed; DLL remains mapped"));
    }
    MODULES.with_borrow_mut(|modules| {
        let module = modules[index(id.module)]
            .as_mut()
            .expect("validated module remains registered");
        module.id = next;
        module.state = ExecutionState::Running;
        module.terminal = None;
        module.pending = PendingBoundaries::default();
    });
    Ok(next)
}

/// The native loader must call this before FreeLibrary, never from module code.
pub fn unregister(id: ExecutionId) -> RuntimeResult<()> {
    if DISPATCHING.get() {
        return Err(RuntimeError::new("cannot unload from an active module callback"));
    }
    MODULES.with_borrow_mut(|modules| {
        let slot = &mut modules[index(id.module)];
        let module = slot
            .as_ref()
            .filter(|module| module.id == id)
            .ok_or_else(|| RuntimeError::new("stale module execution"))?;
        if module.state != ExecutionState::Stopped {
            return Err(RuntimeError::new("module cleanup has not completed; keep DLL mapped"));
        }
        *slot = None;
        Ok(())
    })
}

/// One physical ingress; script then modifier, with no update between modules.
/// A stopped script does not stop the host or the modifier. A global host stop
/// completes only when every registered module has proved successful cleanup.
pub fn dispatch(backend: &mut CurrentBackend, input: DispatchInput) -> DispatchResult {
    if DISPATCHING.replace(true) {
        return DispatchResult::SkipUpdate;
    }
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            DISPATCHING.set(false);
        }
    }
    let _restore = Restore;
    let mut skip_update = false;
    let mut enter_battle = false;
    for source in [ModuleSlot::Script, ModuleSlot::Modifier] {
        let next = MODULES.with_borrow_mut(|modules| {
            let module = modules[index(source)].as_mut()?;
            if !matches!(module.state, ExecutionState::Running | ExecutionState::StopRequested) {
                return None;
            }
            module.pending.observe(input.world_replaced(), input.completed_rounds());
            if source == ModuleSlot::Script
                && !input.script_dispatch_allowed()
                && !input.stop_requested
                && module.state != ExecutionState::StopRequested
            {
                return None;
            }
            let (replaced, rounds) = module.pending.take();
            let mut request = input.with_boundaries(replaced, rounds);
            request.stop_requested |= module.state == ExecutionState::StopRequested;
            Some((module.id, module.entry, request))
        });
        let Some((id, entry, request)) = next else {
            continue;
        };
        let descriptor = entry;
        let flags = u32::from(request.stop_requested)
            | (u32::from(request.world_replaced()) << 1)
            | (u32::from(request.opening_ready()) << 2)
            | (u32::from(request.script_dispatch_allowed()) << 3);
        let status = (descriptor.dispatch)(flags, request.completed_rounds());
        match status {
            0 | 1 => {}
            2 => skip_update = true,
            3 => enter_battle = true,
            _ => {
                // Final Replay cleanup can release its owner while restoring
                // Board. Consume that module's one-update barrier before unload.
                if status == 4 {
                    skip_update |= (descriptor.boundary)() != 0;
                }
                let read = |kind| {
                    let length = unsafe { (descriptor.terminal)(kind, std::ptr::null_mut(), 0) };
                    if length == 0 {
                        return None;
                    }
                    let mut bytes = vec![0; length];
                    unsafe { (descriptor.terminal)(kind, bytes.as_mut_ptr(), length) };
                    Some(bytes)
                };
                let artifact_json = read(0);
                if let Some(bytes) = read(2) {
                    if let Ok(end) = serde_json::from_slice(&bytes) {
                        CurrentBackend::diagnostic_publish_end(index(source), end);
                    }
                }
                let error = read(1).map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
                MODULES.with_borrow_mut(|modules| {
                    let module = modules[index(source)].as_mut().expect("active module");
                    module.state = if status == 4 {
                        ExecutionState::Stopped
                    } else {
                        ExecutionState::CleanupFailed
                    };
                    module.terminal = Some(TerminalResult {
                        execution: id,
                        artifact_json,
                        error,
                    });
                });
            }
        }
    }
    let owner = BOUNDARY_OWNER.get();
    let external =
        owner.and_then(|source| MODULES.with_borrow(|modules| Some(modules[index(source)].as_ref()?.entry.boundary)));
    skip_update |= if let Some(boundary) = external {
        boundary() != 0
    } else {
        crate::saved_game::poll_at_boundary(backend).unwrap_or(true)
            | crate::replay::poll_at_boundary(backend).unwrap_or(true)
    };
    if input.stop_requested {
        let clean = MODULES.with_borrow(|modules| {
            modules
                .iter()
                .flatten()
                .all(|module| module.state == ExecutionState::Stopped)
        });
        if clean {
            CurrentBackend::diagnostic_drive_exit(true);
            return DispatchResult::Stop {
                cleanup_complete: true,
                artifact: None,
                error: None,
            };
        }
        return DispatchResult::SkipUpdate;
    }
    let executing = MODULES.with_borrow(|modules| {
        modules
            .iter()
            .flatten()
            .any(|module| matches!(module.state, ExecutionState::Running | ExecutionState::StopRequested))
    });
    if !executing {
        let clean = MODULES.with_borrow(|modules| {
            modules
                .iter()
                .flatten()
                .all(|module| module.state == ExecutionState::Stopped)
        });
        CurrentBackend::diagnostic_drive_exit(clean);
        // Final-source Replay cleanup can restore Board while returning Stop.
        // Its one-update barrier must survive the transition to an idle host.
        return if skip_update {
            DispatchResult::SkipUpdate
        } else {
            DispatchResult::Idle
        };
    }
    if enter_battle {
        return DispatchResult::EnterBattle;
    }
    if skip_update {
        DispatchResult::SkipUpdate
    } else {
        DispatchResult::Continue
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn native_callback_source_is_local_and_restores_its_caller() {
        let previous = LOCAL_SOURCE.replace(ModuleSlot::Modifier);
        with_module(ModuleSlot::Script, || {
            with_local_source(|| assert_eq!(rsvz_schedule::module::current_module(), ModuleSlot::Modifier));
            assert_eq!(rsvz_schedule::module::current_module(), ModuleSlot::Script);
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_local_source(|| panic!("native callback failed"));
            }));
            assert!(result.is_err());
            assert_eq!(rsvz_schedule::module::current_module(), ModuleSlot::Script);
        });
        LOCAL_SOURCE.set(previous);
    }

    #[test]
    fn start_blocks_are_scoped_to_the_requested_slot_and_nest() {
        for source in [ModuleSlot::Script, ModuleSlot::Modifier] {
            assert!(!start_blocked(source));
            let outer = block_start(source).unwrap();
            {
                let _inner = block_start(source).unwrap();
                assert!(start_blocked(source));
            }
            assert!(start_blocked(source));
            drop(outer);
            assert!(!start_blocked(source));
        }
    }
    #[test]
    fn unsafe_script_intervals_retain_boundaries_independently_of_the_modifier() {
        let mut script = PendingBoundaries::default();
        let mut modifier = PendingBoundaries::default();
        for (replaced, rounds) in [(true, 1), (false, 0), (false, 2)] {
            script.observe(replaced, rounds);
            modifier.observe(replaced, rounds);
            assert_eq!(modifier.take(), (replaced, rounds));
        }
        assert_eq!(script.take(), (true, 3));
        assert_eq!(
            script.take(),
            (false, 0),
            "a resumed module must consume a boundary only once"
        );
        assert_eq!(modifier.take(), (false, 0));
    }
}

unsafe extern "C" fn read_restore(output: *mut rsvz_model::RestoreReceipt) -> bool {
    if output.is_null() {
        return false;
    }
    if let Some(receipt) = REPLAY_RECEIPT.get() {
        unsafe {
            output.write(receipt);
        }
        true
    } else {
        false
    }
}
extern "C" fn publish_restore(receipt: rsvz_model::RestoreReceipt) {
    REPLAY_RECEIPT.set(Some(receipt));
}
pub(crate) fn last_restore() -> Option<rsvz_model::RestoreReceipt> {
    if let Some(host) = REMOTE.get() {
        let mut value = std::mem::MaybeUninit::uninit();
        if unsafe { (host.read_restore)(value.as_mut_ptr()) } {
            Some(unsafe { value.assume_init() })
        } else {
            None
        }
    } else {
        REPLAY_RECEIPT.get()
    }
}
pub(crate) fn publish_restore_receipt(receipt: rsvz_model::RestoreReceipt) {
    if let Some(host) = REMOTE.get() {
        (host.publish_restore)(receipt);
    } else {
        publish_restore(receipt);
    }
}
