#![cfg(feature = "pvz-emulator")]

use rsvz_game::modules::{self, ExecutionState, ModuleDescriptor};
use rsvz_pvz_emulator_backend::{
    DispatchInput, DispatchResult, PeWorldConfig,
    runner_internal::{PeWorldOwner, PeWorldRun},
};
use rsvz_schedule::module::ModuleSlot;
use std::cell::Cell;

// These C-ABI fixtures test resident-host coordination, not in-process business-state isolation.
thread_local! {
    static CALLS: Cell<[usize; 2]> = const { Cell::new([0; 2]) };
    static GENERATIONS: Cell<[u64; 2]> = const { Cell::new([0; 2]) };
    static FAIL_CLEANUP: Cell<bool> = const { Cell::new(false) };
}
unsafe extern "C" fn attach(
    _: *const modules::ExecutionHost, _: *const std::ffi::c_void, source: u32, generation: u64,
) -> u32 {
    let mut values = GENERATIONS.get();
    values[source as usize] = generation;
    GENERATIONS.set(values);
    0
}
fn dispatch_slot(source: usize, flags: u32) -> u32 {
    if flags & 1 != 0 {
        return if FAIL_CLEANUP.get() { 5 } else { 4 };
    }
    let mut calls = CALLS.get();
    calls[source] += 1;
    CALLS.set(calls);
    0
}
extern "C" fn script(flags: u32, _: u64) -> u32 {
    dispatch_slot(0, flags)
}
extern "C" fn modifier(flags: u32, _: u64) -> u32 {
    dispatch_slot(1, flags)
}
unsafe extern "C" fn terminal(_: u32, _: *mut u8, _: usize) -> usize {
    0
}
extern "C" fn boundary() -> u32 {
    0
}
fn descriptor(source: ModuleSlot) -> ModuleDescriptor {
    ModuleDescriptor {
        magic: modules::MODULE_MAGIC,
        abi_version: modules::MODULE_ABI_VERSION,
        descriptor_size: std::mem::size_of::<ModuleDescriptor>() as u32,
        sdk_sha256: [0; 32],
        kind: u32::from(source == ModuleSlot::Modifier),
        managed_unload: 1,
        attach,
        dispatch: if source == ModuleSlot::Script { script } else { modifier },
        terminal,
        boundary,
        detach: boundary,
    }
}
fn register(source: ModuleSlot) -> modules::ExecutionId {
    // Test entries have static lifetime and obey the fixed execution ABI.
    unsafe { modules::register_external(source, descriptor(source), std::ptr::null()) }.unwrap()
}
fn step(world: &mut PeWorldRun) {
    let result = world.with_backend(|backend| {
        modules::dispatch(
            backend,
            DispatchInput {
                stop_requested: false,
                completed_rounds: 0,
                shard: rsvz_model::SessionShard {
                    index: 0,
                    count: 1,
                    seed_base: 0,
                },
            },
        )
    });
    assert!(matches!(result, DispatchResult::Continue | DispatchResult::Idle));
}

#[test]
fn execution_boundary_retains_cleanup_details_when_unload_is_refused() {
    fn failed(_: &mut rsvz_pvz_emulator_backend::PeBackend, _: DispatchInput) -> DispatchResult {
        let error = rsvz_backend_api::error::UserCancelled {
            exit_game: false,
            cause: Some(rsvz::RuntimeError::from_source(
                rsvz_backend_api::error::CleanupFailure {
                    primary: Some(rsvz::RuntimeError::new("original failure")),
                    cleanup: rsvz::RuntimeError::new("cleanup detail"),
                },
            )),
        };
        DispatchResult::Stop {
            cleanup_complete: false,
            artifact: None,
            error: Some(rsvz::RuntimeError::from_source(error)),
        }
    }
    rsvz_game::lifecycle::reset_session_resources();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    let status = world.with_backend(|backend| {
        modules::dispatch_execution(
            failed,
            backend,
            DispatchInput {
                stop_requested: false,
                completed_rounds: 0,
                shard: Default::default(),
            },
        )
    });
    assert_eq!(status, 5);
    let length = unsafe { modules::copy_terminal(1, std::ptr::null_mut(), 0) };
    let mut bytes = vec![0; length];
    unsafe {
        modules::copy_terminal(1, bytes.as_mut_ptr(), bytes.len());
    }
    let message = String::from_utf8(bytes).unwrap();
    for expected in [
        "cancelled by user",
        "original failure",
        "cleanup detail",
        "DLL remains mapped",
    ] {
        assert!(message.contains(expected), "{message}");
    }
}

#[test]
fn external_modules_stop_rerun_and_reject_stale_requests_in_both_start_orders() {
    for first_source in [ModuleSlot::Script, ModuleSlot::Modifier] {
        CALLS.set([0; 2]);
        let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
            .unwrap()
            .install_current()
            .unwrap();
        let first = register(first_source);
        step(&mut world);
        let second = register(if first_source == ModuleSlot::Script {
            ModuleSlot::Modifier
        } else {
            ModuleSlot::Script
        });
        let (script, modifier) = if first_source == ModuleSlot::Script {
            (first, second)
        } else {
            (second, first)
        };
        step(&mut world);
        assert!(CALLS.get().iter().all(|calls| *calls > 0));
        assert!(modules::unregister(modifier).is_err());
        let before = CALLS.get();
        modules::request_stop(modifier).unwrap();
        step(&mut world);
        assert_eq!(modules::state(modifier).unwrap(), ExecutionState::Stopped);
        assert_eq!(CALLS.get()[1], before[1]);
        assert!(CALLS.get()[0] > before[0]);
        let rerun = modules::rerun(modifier).unwrap();
        assert_ne!(rerun.generation, modifier.generation);
        assert_eq!(GENERATIONS.get()[1], rerun.generation);
        assert!(modules::request_stop(modifier).is_err());
        assert!(modules::request_unload(modifier).is_err());
        step(&mut world);
        assert!(CALLS.get()[1] > before[1]);
        modules::request_unload(rerun).unwrap();
        assert!(modules::unload_requested(rerun).unwrap());
        assert!(modules::unregister(rerun).is_err());
        step(&mut world);
        assert!(modules::rerun(rerun).is_err());
        modules::unregister(rerun).unwrap();
        let before = CALLS.get()[0];
        step(&mut world);
        assert!(CALLS.get()[0] > before);
        modules::request_stop(script).unwrap();
        step(&mut world);
        modules::unregister(script).unwrap();
    }
}

#[test]
fn failed_external_cleanup_keeps_the_module_registered() {
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    let id = register(ModuleSlot::Modifier);
    FAIL_CLEANUP.set(true);
    modules::request_unload(id).unwrap();
    step(&mut world);
    assert_eq!(modules::state(id).unwrap(), ExecutionState::CleanupFailed);
    assert!(modules::unregister(id).is_err());
    assert!(modules::rerun(id).is_err());
    FAIL_CLEANUP.set(false);
}
