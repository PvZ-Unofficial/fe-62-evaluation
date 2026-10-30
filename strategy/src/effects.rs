//! PE currently exposes damage events but no normal ash activation callbacks.
//! Observe only our own plants' countdown transitions, replacement on imitator morph, and visible landing.
use super::*;
use rsvz::core::model::{ActivationFact, PixelPos, PlantId};
#[derive(Clone, Copy)]
struct Pending {
    id: PlantId,
    kind: PlantKind,
    pos: Pos,
    copy: bool,
    last_effect: i32,
    last_state: i32,
    last_count: i32,
    x: i32,
    y: i32,
    confirmed: bool,
    reason: i32,
    planted_at: i32,
}
thread_local! {static PENDING:RefCell<[Option<Pending>;16]>=const{RefCell::new([None;16])};}
thread_local! {static LAST_ICE:std::cell::Cell<Option<i32>>=const{std::cell::Cell::new(None)};}
pub fn frozen_lower_bound(now: i32) -> i32 {
    LAST_ICE.with(|at| at.get().map_or(0, |at| (at + 299 - now).max(0)))
}
pub fn remaining(id: PlantId, now: i32) -> Option<i32> {
    PENDING.with_borrow(|slots| {
        slots
            .iter()
            .flatten()
            .find(|p| p.id == id)
            .map(|p| (p.planted_at + if p.copy { 420 } else { 100 } - now).max(1))
    })
}
pub fn clear() {
    PENDING.with_borrow_mut(|slots| slots.fill(None));
    LAST_ICE.with(|at| at.set(None));
}
pub fn planted(id: PlantId, card: CardSelection, pos: Pos, reason: i32) {
    let (kind, copy) = match card {
        CardSelection::Plant(k) => (k, false),
        CardSelection::Imitator(k) => (k, true),
    };
    if !matches!(kind, IceShroom | DoomShroom | CherryBomb | Jalapeno | Squash) {
        return;
    }
    PENDING.with_borrow_mut(|slots| {
        let slot = slots
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("at most sixteen pending effects");
        *slot = Some(Pending {
            id,
            kind,
            pos,
            copy,
            last_effect: -1,
            last_state: 0,
            last_count: -1,
            x: pos.1 * 80 - 40,
            y: 0,
            confirmed: false,
            reason,
            planted_at: rsvz::timing::clock(),
        });
    });
}
pub fn damaged(e: &rsvz::core::model::PlantEffectEvent) {
    PENDING.with_borrow_mut(|slots| {
        for slot in slots {
            if slot.is_some_and(|p| p.id == e.plant_id) {
                if e.outcome == PlantEffectOutcome::Activated {
                    slot.as_mut().unwrap().confirmed = true;
                } else if matches!(
                    e.outcome,
                    PlantEffectOutcome::Killed | PlantEffectOutcome::Squished | PlantEffectOutcome::Stolen
                ) {
                    *slot = None;
                }
            }
        }
    });
}
fn activation(kind: PlantKind) -> ActivationKind {
    match kind {
        IceShroom => ActivationKind::Ice,
        DoomShroom => ActivationKind::Doom,
        CherryBomb => ActivationKind::Cherry,
        Jalapeno => ActivationKind::Jalapeno,
        Squash => ActivationKind::Squash,
        _ => unreachable!(),
    }
}
pub fn poll() {
    let (now, w, _) = time();
    PENDING.with_borrow_mut(|slots| {
        for slot in slots {
            let Some(pending) = slot.as_mut() else {
                continue;
            };
            let observed = rsvz::with_frame(|b| {
                let mut value = b.plant(pending.id).filter(|p| p.is_alive()).map(|p| {
                    (
                        p.id(),
                        p.raw_kind(),
                        p.effect_countdown(),
                        p.state(),
                        p.state_countdown(),
                        p.x(),
                        p.y(),
                    )
                });
                if value.is_none() && pending.copy {
                    b.for_each_plant_at_anchor_grid(grid(pending.pos), |p| {
                        if p.is_alive() && p.raw_kind() == pending.kind {
                            value = Some((
                                p.id(),
                                p.raw_kind(),
                                p.effect_countdown(),
                                p.state(),
                                p.state_countdown(),
                                p.x(),
                                p.y(),
                            ));
                        }
                    });
                }
                value
            });
            let mut activated = pending.confirmed;
            if let Some((id, kind, effect, state, count, x, y)) = observed {
                pending.id = id;
                pending.x = x;
                pending.y = y;
                if kind != Imitator {
                    pending.copy = false;
                    if pending.kind == Squash {
                        activated |= state == 6 && count <= 5 || state == 7;
                    } else {
                        activated |= pending.last_effect > 0 && effect == 0;
                    }
                    pending.last_effect = effect;
                }
                pending.last_state = state;
                pending.last_count = count;
            } else {
                activated |= (1..=2).contains(&pending.last_effect);
                // Otherwise disappearance without a witnessed transition is not called activation.
            }
            if activated {
                if pending.kind == IceShroom {
                    LAST_ICE.with(|at| at.set(Some(now)));
                }
                let fact = ActivationFact {
                    refresh_countdown: 0,
                    initial_countdown: 0,
                    wave: w,
                    kind: activation(pending.kind),
                    grid: grid(pending.pos),
                    position: PixelPos {
                        x: pending.x,
                        y: pending.y,
                    },
                    target_x: pending.x,
                    main_counter: now - 1,
                };
                diagnostics::activation(&fact);
                CONTROL.with_borrow_mut(|control| control.activated(&fact, pending.reason));
                *slot = None;
            } else if observed.is_none() && !pending.copy {
                *slot = None;
            }
        }
    });
}
