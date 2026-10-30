use crate::{
    Pvz1051Backend,
    error::{Pvz1051Error, Result},
    patches::leases::BoolPatchId,
    raw::layout as ptrs,
};
use rsvz_backend_api::ProfileProgressBackend;
use std::ptr::NonNull;

fn profile(backend: &Pvz1051Backend) -> Result<NonNull<ptrs::PlayerInfo>> {
    // SAFETY: the token owns LawnApp; profile can be absent during initial setup.
    NonNull::new(unsafe { ptrs::LawnApp::player_info(backend.app().as_ptr()) }).ok_or(Pvz1051Error::NullUserData)
}

impl rsvz_backend_api::HiddenPagesBackend for Pvz1051Backend {
    fn hidden_pages_visible(&self) -> Result<bool> {
        BoolPatchId::HiddenPagesVisible.enabled()
    }
    fn set_hidden_pages_visible(&self, visible: bool) -> Result<()> {
        crate::patches::leases::set_bool_patch(BoolPatchId::HiddenPagesVisible, visible)
    }
}

impl rsvz_backend_api::TemporaryUnlockBackend for Pvz1051Backend {
    fn temporary_unlock_enabled(&self) -> Result<bool> {
        BoolPatchId::TemporaryUnlock.enabled()
    }
    fn set_temporary_unlock_enabled(&mut self, enabled: bool) -> Result<()> {
        use rsvz_backend_api::GameUiBackend;
        let menu = self.game_ui()? == rsvz_model::GameUi::Menu;
        if menu {
            profile(self)?;
        }
        crate::patches::leases::set_bool_patch(BoolPatchId::TemporaryUnlock, enabled)?;
        if menu {
            self.sync_game_selector(true)?;
        }
        Ok(())
    }
}

impl ProfileProgressBackend for Pvz1051Backend {
    fn ensure_profile_writable(&self) -> Result<()> {
        profile(self)?;
        if BoolPatchId::ProfileReadonly.enabled()? {
            return Err(Pvz1051Error::OperationRejected("profile is read-only"));
        }
        Ok(())
    }
    fn finished_adventure(&self) -> Result<u32> {
        Ok(unsafe { ptrs::PlayerInfo::finished_adventure(profile(self)?.as_ptr()) })
    }
    fn set_finished_adventure(&self, value: u32) -> Result<()> {
        self.ensure_profile_writable()?;
        unsafe { ptrs::PlayerInfo::set_finished_adventure(profile(self)?.as_ptr(), value) };
        Ok(())
    }
    fn challenge_record(&self, index: u32) -> Result<u32> {
        if index >= 100 {
            return Err(Pvz1051Error::AbiPreconditionFailed(
                "challenge record index exceeds profile array",
            ));
        }
        Ok(unsafe { ptrs::PlayerInfo::challenge_record(profile(self)?.as_ptr(), index as usize) })
    }
    fn set_challenge_record(&self, index: u32, value: u32) -> Result<()> {
        if index >= 100 {
            return Err(Pvz1051Error::AbiPreconditionFailed(
                "challenge record index exceeds profile array",
            ));
        }
        self.ensure_profile_writable()?;
        unsafe { ptrs::PlayerInfo::set_challenge_record(profile(self)?.as_ptr(), index as usize, value) };
        Ok(())
    }
    fn purchase(&self, index: u32) -> Result<u32> {
        if index >= 80 {
            return Err(Pvz1051Error::AbiPreconditionFailed(
                "purchase index exceeds profile array",
            ));
        }
        Ok(unsafe { ptrs::PlayerInfo::purchase(profile(self)?.as_ptr(), index as usize) })
    }
    fn set_purchase(&self, index: u32, value: u32) -> Result<()> {
        if index >= 80 {
            return Err(Pvz1051Error::AbiPreconditionFailed(
                "purchase index exceeds profile array",
            ));
        }
        self.ensure_profile_writable()?;
        unsafe { ptrs::PlayerInfo::set_purchase(profile(self)?.as_ptr(), index as usize, value) };
        Ok(())
    }
    fn sync_game_selector(&mut self, show_loading: bool) -> Result<()> {
        self.ensure_game_ui(rsvz_model::GameUi::Menu)?;
        let selector = NonNull::new(unsafe { ptrs::LawnApp::game_selector(self.app().as_ptr()) })
            .ok_or(Pvz1051Error::GameNotReady)?;
        // SAFETY: exclusive access, live main menu; ECX/stack/ret verified above.
        unsafe { crate::raw::abi::game_selector_sync_profile(selector.as_ptr(), i32::from(show_loading)) };
        Ok(())
    }
}
