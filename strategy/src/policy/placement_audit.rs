//! Opt-in, single-decision read-only placement audit. Never supplies model inputs.
use super::*;
use rsvz::core::backend::*;
use std::sync::OnceLock;

pub(super) fn inspect(decision: u64, actions: &[Action], c: &Context) {
    static TARGET: OnceLock<Option<u64>> = OnceLock::new();
    let target = TARGET.get_or_init(|| {
        std::env::var("SIX_POLICY_AUDIT_PLACEMENT_DECISION")
            .ok()
            .map(|v| v.parse().expect("placement audit decision"))
    });
    if *target != Some(decision) {
        return;
    }
    let ids: Vec<_> = actions.iter().filter(|a| a.kind == 4).map(|a| a.id()).collect();
    let ice = rsvz::with_backend(|b| (b.ice_path_x(1), b.ice_path_countdown(1)));
    rsvz::log(
        rsvz::LogLevel::Info,
        format_args!(
            "policy_placement_audit seed={} decision={} clock={} wave={} time={} cherry_allowed={} ice_required={} ready={} cd={} sun={} cherry_ids={:?} row2_ice={:?}",
            diagnostics::seed(),
            decision,
            rsvz::timing::clock(),
            c.wave,
            c.time,
            c.cherry,
            c.ice_required,
            ready(sel(CherryBomb)),
            cd(sel(CherryBomb)),
            rsvz::board::sun(),
            ids,
            ice
        ),
    );
    for col in 4..=9 {
        let p = (2, col);
        rsvz::log(
            rsvz::LogLevel::Info,
            format_args!(
                "policy_placement_grid decision={} row=2 col={} main={:?} candidate={} plantability={:?}",
                decision,
                col,
                main_kind(p),
                combat::candidate(sel(CherryBomb), p),
                rsvz::core::cards::plantability(sel(CherryBomb), grid(p))
            ),
        );
    }
    rsvz::with_frame(|b| {
        for p in b.plants().filter(|p| p.is_alive()) {
            rsvz::log(
                rsvz::LogLevel::Info,
                format_args!(
                    "policy_placement_plant decision={} kind={:?} grid={:?} hp={} state={}",
                    decision,
                    p.raw_kind(),
                    p.grid(),
                    p.hp(),
                    p.state()
                ),
            );
        }
    });
}
