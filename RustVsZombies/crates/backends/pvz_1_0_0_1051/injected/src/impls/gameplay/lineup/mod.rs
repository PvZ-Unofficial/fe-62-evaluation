use super::*;
use crate::impls::handles::PvzGridItemHandle;

mod grid_items;
mod scene;
use grid_items::*;
use scene::*;

impl BoardSupportBackend for Pvz1051Backend {
    fn ensure_board_supported(&self) -> Result<()> {
        match self.game_ui()? {
            GameUi::LevelIntro | GameUi::Playing => {}
            _ => {
                return Err(Pvz1051Error::UnsupportedToolMode(
                    "lineup automation requires level-intro or playing UI",
                ));
            }
        }
        let _board = self.board()?;
        let mode = self.raw_game_mode()?;
        if !is_survival_mode(mode) {
            return Err(Pvz1051Error::UnsupportedToolMode(
                "lineup automation currently supports Survival modes only",
            ));
        }
        Ok(())
    }
}

impl LawnMowerClearBackend for Pvz1051Backend {
    fn clear_lawn_mowers(&mut self) -> Result<()> {
        let board = self.board()?;
        clear_lawn_mowers(board);
        Ok(())
    }
}

impl SceneEditBackend for Pvz1051Backend {
    fn set_scene(&mut self, scene: SceneKind) -> Result<()> {
        self.ensure_board_supported()?;
        ensure_lineup_scene_switch_supported(scene)?;
        let current = self.scene()?;
        if current == scene {
            return Ok(());
        }

        let board = self.board()?;
        match self.game_ui()? {
            GameUi::LevelIntro => {}
            GameUi::Playing => {
                // SAFETY: `board` is non-null and points to the active Board; current_wave is a
                // copied scalar. Scene switching is limited to the opening wave-0 setup window.
                if unsafe { ptrs::Board::current_wave(board.as_ptr()) } > 0 {
                    return Err(Pvz1051Error::AbiPreconditionFailed(
                        "lineup scene switching is not allowed after zombies have spawned",
                    ));
                }
            }
            _ => {
                return Err(Pvz1051Error::UnsupportedToolMode(
                    "lineup scene switching requires level-intro or opening playing UI",
                ));
            }
        }

        let switch_state = prepare_lineup_scene_switch(self, board, scene)?;

        // SAFETY: UI, Board, Survival mode, regular scene kind, and all later scene-switch
        // dependencies were checked above. 0x40A160 is Board::LoadBackgroundImages; after writing
        // mBackground it loads the resources needed by the target scene.
        unsafe {
            *ptrs::Board::scene_mut(board.as_ptr()) = scene.code();
            crate::raw::abi::board_load_background_images(board.as_ptr());
        }
        write_lineup_scene_terrain(board, scene);
        if let Some(effect_system) = switch_state.effect_system {
            clear_pool_wave_particles(effect_system, board);
        }
        reset_scene_lawn_items(board, switch_state);
        play_lineup_scene_music(scene);
        Ok(())
    }
}

impl rsvz_backend_api::SceneLayoutBackend for Pvz1051Backend {
    fn validate_scene_layout(&self, scene: SceneKind) -> Result<()> {
        use rsvz_backend_api::BoardReadinessBackend;
        let ready = match self.game_ui()? {
            GameUi::Playing => self.playing_board_ready()?,
            GameUi::LevelIntro => self.level_intro_board_ready()?,
            _ => false,
        };
        if !ready {
            return Err(Pvz1051Error::OperationRejected("scene layout requires a ready world"));
        }
        let app = self.app();
        // SAFETY: copied native pointers; all dependencies checked before mutation.
        if unsafe {
            ptrs::LawnApp::music(app.as_ptr()).is_null()
                || (!scene.has_pool() && ptrs::LawnApp::effect_system(app.as_ptr()).is_null())
        } {
            return Err(Pvz1051Error::OperationRejected("scene resources are not ready"));
        }
        Ok(())
    }
    fn set_scene_layout(&mut self, scene: SceneKind) -> Result<()> {
        self.validate_scene_layout(scene)?;
        let board = self.board()?;
        // SAFETY: ready world and valid scene enum; exact native background load.
        unsafe {
            *ptrs::Board::scene_mut(board.as_ptr()) = scene.code();
            crate::raw::abi::board_load_background_images(board.as_ptr());
        }
        write_lineup_scene_terrain(board, scene);
        if !scene.has_pool() {
            // SAFETY: the effect-system dependency was validated above.
            let effects =
                NonNull::new(unsafe { ptrs::LawnApp::effect_system(self.app().as_ptr()) }).expect("validated effects");
            clear_pool_wave_particles(effects, board);
        }
        let music = match scene.code() {
            5 => 2,
            6.. => 8,
            code => code + 1,
        };
        // SAFETY: valid native tune and music object. No stage reset or mower action.
        unsafe { crate::raw::abi::lawn_app_play_music(music) };
        Ok(())
    }
}

impl GridItemCreateBackend for Pvz1051Backend {
    fn add_ladder<'a>(&'a self, grid: Grid) -> Result<PvzGridItemHandle<'a>> {
        let _board = grid_item_create_board(self, grid)?;
        Ok(PvzGridItemHandle::new(add_ladder(grid)?))
    }

    fn add_crater<'a>(&'a self, grid: Grid) -> Result<PvzGridItemHandle<'a>> {
        let _board = grid_item_create_board(self, grid)?;
        Ok(PvzGridItemHandle::new(add_crater(grid)?))
    }

    fn add_gravestone<'a>(&'a self, grid: Grid) -> Result<PvzGridItemHandle<'a>> {
        let board = grid_item_create_board(self, grid)?;
        Ok(PvzGridItemHandle::new(add_gravestone(board, grid)?))
    }
}

impl rsvz_backend_api::ChallengeGravePlacementBackend for Pvz1051Backend {
    fn spawn_challenge_grave(&self, grid: Grid) -> Result<()> {
        let board = grid_item_create_board(self, grid)?;
        // SAFETY: current Board's owned Challenge pointer, checked before the
        // native action can remove any plants or allocate a grave.
        let challenge =
            NonNull::new(unsafe { ptrs::Board::challenge(board.as_ptr()) }).ok_or(Pvz1051Error::NullChallenge)?;
        // SAFETY: verified native ABI; current UI, grid and pool capacity were
        // validated. The native atom keeps its plant-removal and particle effects.
        unsafe { crate::raw::abi::challenge_spawn_grave(challenge.as_ptr(), grid.row, grid.col) };
        Ok(())
    }
}

impl GridItemEditBackend for Pvz1051Backend {
    fn remove_grid_item<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> Result<()> {
        // SAFETY: `handle` came from an occupied, non-dead DataArray slot.
        unsafe { crate::raw::abi::grid_item_die(handle.ptr.as_ptr()) };
        Ok(())
    }
}

impl rsvz_backend_api::RakePlacementBackend for Pvz1051Backend {
    fn place_rake(&self, grid: Grid) -> Result<bool> {
        let board = grid_item_create_board(self, grid)?;
        // SAFETY: PlaceRake dereferences profile even with the stock guard patched.
        if unsafe { ptrs::LawnApp::player_info(self.app().as_ptr()).is_null() } {
            return Err(Pvz1051Error::OperationRejected("rake requires a player profile"));
        }
        // SAFETY: current embedded grid pool, read allocation count only.
        let pool = unsafe { ptrs::Board::grid_items(board.as_ptr()) };
        let before = unsafe { ptrs::DataArray::active_count(pool) };
        crate::patches::rake::with_position(grid.row as usize, grid.col as usize, || {
            // SAFETY: verified stack ABI; UI/grid/pool/profile checked before patches.
            unsafe { crate::raw::abi::board_place_rake(board.as_ptr()) };
            // Native mode guards may return before allocation. No update/collection
            // occurs inside this synchronous call, so an increment proves placement.
            Ok(unsafe { ptrs::DataArray::active_count(pool) } > before)
        })
    }
}
