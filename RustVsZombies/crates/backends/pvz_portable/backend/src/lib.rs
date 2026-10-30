//! PvZ-Portable backend adapter for RustVsZombies.

mod access;
mod dispatch;
mod error;
mod event;
mod ffi;
mod handles;
pub mod host;
mod impls;
mod input_profile;
mod module_link;
mod observation_clip;
mod paint;
mod replay;
mod replay_format;
mod runtime;

pub use dispatch::{DispatchEntry, DispatchInput, DispatchResult};
pub use error::{PortableBackendError, Result};
#[doc(hidden)]
pub use ffi::{plugin_abi_version, plugin_initialize, plugin_shutdown};
pub use runtime::PortableBackend;

impl PortableBackend {
    #[doc(hidden)]
    pub fn module_host_abi_version() -> u32 {
        ffi::plugin_abi_version()
    }

    #[doc(hidden)]
    pub fn initialize_module_host(dispatch: DispatchEntry) -> i32 {
        ffi::plugin_initialize_shared(dispatch)
    }

    #[doc(hidden)]
    pub fn shutdown_module_host() -> i32 {
        ffi::plugin_shutdown()
    }

    /// Writes a runtime error through the native Portable host.
    pub fn default_log_output(message: &str) {
        #[cfg(not(test))]
        ffi::log(message);
        #[cfg(test)]
        eprintln!("{message}");
    }
}

mod diagnostics;
