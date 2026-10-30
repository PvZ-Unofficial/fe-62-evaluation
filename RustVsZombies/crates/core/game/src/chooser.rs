//! Restore an explicit complete deck through native chooser actions. Product
//! triggers, saved preferences and arbitration with a script belong to callers.
use crate::{
    access::{operation_error, with_backend, with_exclusive_backend},
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::{
    BattleEntryBackend, CardAppendSelectionBackend, CardSelectionReadBackend, GameUiBackend, SeedBankEditBackend,
    SeedChooserControlBackend, SeedChooserFastForwardBackend, WorldIdentityBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{CardSelection, GameUi, SeedChooserFastForwardOptions, WorldId};
use rsvz_schedule::{
    module::{ModuleSlot, current_module, with_module},
    state_hook::StateHookHandle,
};
use std::{
    cell::Cell,
    rc::{Rc, Weak},
};

thread_local! {
    static ACTIVE: std::cell::RefCell<Weak<Cell<bool>>> = const { std::cell::RefCell::new(Weak::new()) };
}
pub(crate) fn start_held() -> bool {
    ACTIVE.with_borrow(|active| active.upgrade().is_some_and(|active| active.get()))
}

/// Read the current complete chooser/battle deck in native bank order. This is
/// a one-shot capture, never a watcher that silently replaces a saved record.
pub fn capture_complete_deck() -> RuntimeResult<Vec<CardSelection>>
where
    CurrentBackend: CardSelectionReadBackend + SeedBankEditBackend,
{
    with_backend(|backend| {
        let count = backend.selected_card_count().map_err(operation_error)?;
        if count == 0 || count != usize::from(backend.seed_packet_count().map_err(operation_error)?) {
            return Err(RuntimeError::new("no complete valid deck is currently selected"));
        }
        let mut cards = Vec::with_capacity(count);
        for index in 0..count {
            cards.push(backend.selected_card(index).map_err(operation_error)?.selection());
        }
        crate::logic::cards::validate_card_selection(&cards).map_err(|error| RuntimeError::new(error.to_string()))?;
        Ok(cards)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Check,
    Clear,
    Select,
    Land,
    Done,
}

/// Call once per service dispatch. A native phase change or source stop cancels
/// the request; no stored preference is modified and no fallback deck is chosen.
pub struct CardRestore {
    cards: Vec<CardSelection>,
    world: WorldId,
    source: ModuleSlot,
    phase: Phase,
    active: Rc<Cell<bool>>,
    cleanup: Option<StateHookHandle>,
    fast_forward: bool,
    skipping: Rc<Cell<bool>>,
    start_battle: bool,
}

impl CardRestore {
    pub fn prepare(cards: Vec<CardSelection>, fast_forward: bool, start_battle: bool) -> RuntimeResult<Self>
    where
        CurrentBackend:
            GameUiBackend + WorldIdentityBackend + SeedChooserControlBackend + SeedChooserFastForwardBackend,
    {
        crate::logic::cards::validate_card_selection(&cards).map_err(|error| RuntimeError::new(error.to_string()))?;
        if cards.is_empty() {
            return Err(RuntimeError::new("automatic selection requires a complete deck"));
        }
        if start_held() {
            return Err(RuntimeError::new("another card restoration is pending"));
        }
        let world = with_backend(|backend| {
            if backend.game_ui().map_err(operation_error)? != GameUi::LevelIntro {
                return Err(RuntimeError::new("card restoration requires the seed chooser"));
            }
            backend
                .world_identity()
                .map_err(operation_error)?
                .ok_or_else(|| RuntimeError::new("no current world"))
        })?;
        let active = Rc::new(Cell::new(true));
        let skipping = Rc::new(Cell::new(false));
        let cleanup_active = Rc::clone(&active);
        let cleanup_skipping = Rc::clone(&skipping);
        let cleanup = crate::state_hook::register_native_cleanup(i32::MAX, move || {
            if cleanup_skipping.get() {
                with_backend(|backend| -> RuntimeResult<()> {
                    if backend.world_identity().map_err(operation_error)? == Some(world) {
                        backend.stop_seed_chooser_fast_forward().map_err(operation_error)?;
                    }
                    Ok(())
                })?;
                cleanup_skipping.set(false);
            }
            cleanup_active.set(false);
            Ok(())
        });
        ACTIVE.with_borrow_mut(|current| *current = Rc::downgrade(&active));
        Ok(Self {
            cards,
            world,
            source: current_module(),
            phase: Phase::Check,
            active,
            cleanup: Some(cleanup),
            fast_forward,
            skipping,
            start_battle,
        })
    }

    /// End only this plan's UI acceleration, retaining its automatic-start hold.
    /// A caller can report a failed restore and leave the chooser interactive.
    pub fn stop_fast_forward(&mut self) -> RuntimeResult<()>
    where
        CurrentBackend: SeedChooserControlBackend + WorldIdentityBackend,
    {
        if current_module() != self.source {
            return Err(RuntimeError::new("card restoration belongs to another source"));
        }
        if self.skipping.get() {
            with_backend(|backend| -> RuntimeResult<()> {
                if backend.world_identity().map_err(operation_error)? == Some(self.world) {
                    backend.stop_seed_chooser_fast_forward().map_err(operation_error)?;
                }
                Ok(())
            })?;
            self.skipping.set(false);
        }
        self.fast_forward = false;
        Ok(())
    }

    pub fn cancel(&mut self) -> RuntimeResult<()>
    where
        CurrentBackend: SeedChooserControlBackend + WorldIdentityBackend,
    {
        if current_module() != self.source {
            return Err(RuntimeError::new("card restoration belongs to another source"));
        }
        self.stop_fast_forward()?;
        self.active.set(false);
        if let Some(cleanup) = self.cleanup.take() {
            crate::state_hook::remove_state_hook(cleanup);
        }
        Ok(())
    }

    pub fn advance(&mut self) -> RuntimeResult<bool>
    where
        CurrentBackend: SeedChooserControlBackend
            + SeedChooserFastForwardBackend
            + SeedBankEditBackend
            + CardAppendSelectionBackend
            + GameUiBackend
            + WorldIdentityBackend
            + BattleEntryBackend,
    {
        if current_module() != self.source {
            return Err(RuntimeError::new("card restoration belongs to another source"));
        }
        if self.phase == Phase::Done {
            return Ok(true);
        }
        if !self.active.get() {
            return Err(RuntimeError::new("card restoration was cancelled"));
        }
        let ready = with_backend(|backend| {
            if backend.game_ui().map_err(operation_error)? != GameUi::LevelIntro
                || backend.world_identity().map_err(operation_error)? != Some(self.world)
            {
                return Err(RuntimeError::new("world or screen changed during card restoration"));
            }
            Ok(backend.chooser_readiness().map_err(operation_error)?
                == rsvz_backend_api::opening::SeedChooserReadiness::Ready)
        })?;
        if usize::from(with_backend(|backend| backend.seed_packet_count()).map_err(operation_error)?)
            != self.cards.len()
        {
            self.stop_fast_forward()?;
            return Err(RuntimeError::new(
                "bank size changed during card restoration; choose a matching deck explicitly",
            ));
        }
        if self.fast_forward && !self.skipping.get() {
            with_backend(|backend| backend.request_seed_chooser_fast_forward(SeedChooserFastForwardOptions::default()))
                .map_err(operation_error)?;
            self.skipping.set(true);
        }
        if !ready {
            return Ok(false);
        }
        match self.phase {
            Phase::Check => {
                with_backend(|backend| {
                    if usize::from(backend.seed_packet_count().map_err(operation_error)?) != self.cards.len() {
                        return Err(RuntimeError::new("recorded deck does not match current bank size"));
                    }
                    for &selection in &self.cards {
                        if !backend
                            .chooser_card_available(selection.checked().expect("validated deck"))
                            .map_err(operation_error)?
                        {
                            return Err(RuntimeError::new("recorded deck contains an unavailable plant"));
                        }
                    }
                    let count = backend.selected_card_count().map_err(operation_error)?;
                    let mut matches = count == self.cards.len();
                    for index in 0..count {
                        matches &= self.cards.get(index).copied()
                            == Some(backend.selected_card(index).map_err(operation_error)?.selection());
                    }
                    if matches {
                        self.phase = Phase::Land;
                        return Ok(());
                    }
                    for index in 0..count {
                        if backend.chooser_card_locked(index).map_err(operation_error)? {
                            return Err(RuntimeError::new("current deck contains a card locked by Crazy Dave"));
                        }
                    }
                    for index in (0..count).rev() {
                        backend.deselect_card(index).map_err(operation_error)?;
                    }
                    self.phase = Phase::Clear;
                    Ok(())
                })?;
            }
            Phase::Clear => {
                if with_backend(|backend| backend.selected_card_count()).map_err(operation_error)? != 0 {
                    return Err(RuntimeError::new("card selection changed while clearing the old deck"));
                }
                self.phase = Phase::Select;
            }
            Phase::Select => {
                with_backend(|backend| {
                    if backend.selected_card_count().map_err(operation_error)? != 0 {
                        return Err(RuntimeError::new("card selection changed before restoring the deck"));
                    }
                    for &selection in &self.cards {
                        if !backend
                            .chooser_card_available(selection.checked().expect("validated deck"))
                            .map_err(operation_error)?
                        {
                            return Err(RuntimeError::new("recorded plant availability changed"));
                        }
                    }
                    for &selection in &self.cards {
                        backend
                            .select_card(selection.checked().expect("validated deck"))
                            .map_err(operation_error)?;
                    }
                    backend.finish_card_selection().map_err(operation_error)
                })?;
                self.phase = Phase::Land;
            }
            Phase::Land => {
                with_backend(|backend| {
                    if backend.selected_card_count().map_err(operation_error)? != self.cards.len() {
                        return Err(RuntimeError::new("restored deck changed while cards were landing"));
                    }
                    for (index, &expected) in self.cards.iter().enumerate() {
                        if backend.selected_card(index).map_err(operation_error)?.selection() != expected {
                            return Err(RuntimeError::new("restored card order changed"));
                        }
                    }
                    Ok(())
                })?;
                self.stop_fast_forward()?;
                if self.start_battle {
                    with_exclusive_backend(|backend| backend.start_battle()).map_err(operation_error)?;
                }
                self.phase = Phase::Done;
                // Keep the start hold while the caller owns a completed manual
                // selection, so ordinary Opening cannot unexpectedly start it.
                return Ok(true);
            }
            Phase::Done => unreachable!(),
        }
        Ok(false)
    }
}

impl Drop for CardRestore {
    fn drop(&mut self) {
        // The lifecycle hook retains pending native cleanup if the caller drops
        // an unfinished plan; only the automatic-start hold is released here.
        // cancel() explicitly stops fast-forward at an installed backend scope.
        if !self.skipping.get() {
            self.active.set(false);
            if let Some(cleanup) = self.cleanup.take() {
                with_module(self.source, || crate::state_hook::remove_state_hook(cleanup));
            }
        }
    }
}
