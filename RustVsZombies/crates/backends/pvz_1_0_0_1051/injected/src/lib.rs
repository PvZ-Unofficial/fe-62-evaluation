//! PvZ 1.0.0.1051 injected backend.

#[cfg(not(all(
    target_os = "windows",
    target_arch = "x86",
    target_env = "msvc",
    target_pointer_width = "32"
)))]
compile_error!("PvZ 1.0.0.1051 injected backend requires i686-pc-windows-msvc");

mod access;
mod dispatch;
pub(crate) mod error;
pub mod host;
mod module_link;
pub(crate) mod impls {
    mod base;
    mod cob;
    mod contact;
    pub(crate) mod event;
    mod gameplay;
    mod handles;
    mod hud;
    mod input;
    mod item;
    mod modifier;
    mod observation_clip;
    mod plant_state;
    mod profile;
    mod replay;
    mod replay_format;
    pub(crate) mod reset;
    mod seed_edit;
    mod state;
    mod surface;
    mod timing;
    mod world_edits;
}
pub(crate) mod ops {
    pub(crate) mod plant;
    pub(crate) mod seed;
}
pub(crate) mod patches {
    //! 1051-local byte patch helpers.

    pub(crate) mod board_click;
    pub(crate) mod chooser_repick;
    mod event;
    pub(crate) mod house_entry;
    mod imitator_morph;
    pub(crate) mod keyboard;
    pub(crate) mod leases;
    pub(crate) mod memory;
    pub(crate) mod mods;
    pub(crate) mod mower;
    pub(crate) mod paint;
    mod pause;
    mod performance;
    pub(crate) mod pickup_limits;
    pub(crate) mod rake;
    pub(crate) mod random;
    mod spec;
    mod sun;
    pub(crate) mod variant;
    pub(crate) mod window_resize;
    pub(crate) mod window_view;
    pub(crate) mod world_lifetime;

    pub(crate) use event::EventHookGuard;
    pub(crate) use imitator_morph::ImitatorMorphHookGuard;
    pub(crate) use pause::advanced_pause_patch;
    pub(crate) use performance::performance_patches;
    pub(crate) use random::{
        ResetRandomGuard, configure as configure_random, configure_wave_seed as configure_wave_random_seed,
        effective_seed as effective_random_seed, fixed as random_fixed, locked as random_locked,
        note_reset_seed as note_random_reset_seed, restore as restore_random, seed as random_seed,
    };
    pub(crate) use spec::PatchSet;
    pub(crate) use sun::{restore as restore_sun_production, set_direct_credit};
}
pub(crate) mod raw {
    pub(crate) mod abi;
    pub(crate) mod kind;
    pub(crate) mod layout;
    pub(crate) mod types;
}
pub(crate) mod runtime {
    mod advanced_pause;
    pub(crate) mod backend;
    mod fast_forward;
    pub(crate) mod hook;
    pub(crate) mod imitator_morph;
    mod seed_chooser;

    pub use backend::Pvz1051Backend;
}

pub use dispatch::{DispatchEntry, DispatchInput, DispatchResult};
pub use error::Pvz1051Error;
pub use runtime::Pvz1051Backend;

impl Pvz1051Backend {
    #[doc(hidden)]
    pub fn initialize_module_host(context: *mut std::ffi::c_void, dispatch: DispatchEntry) -> u32 {
        host::initialize_shared(context, dispatch)
    }

    #[doc(hidden)]
    pub fn request_module_host_unload(context: *mut std::ffi::c_void) -> u32 {
        host::request_unload(context)
    }

    /// Displays a runtime log record through the 1051 backend's emergency UI.
    pub fn default_log_output(message: &str) {
        runtime::hook::report_runtime_error(message);
    }
}

mod diagnostics;
