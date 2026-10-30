#![expect(
    clippy::inline_asm_x86_att_syntax,
    reason = "the verified 1051 ABI macro emits AT&T templates intentionally"
)]

mod replay;
pub(crate) use replay::*;
mod app;
mod board;
mod challenge;
mod effect;
mod objects;
mod plant;
mod seed;
mod widget;
mod zombie;

pub(crate) use app::*;
pub(crate) use board::*;
pub(crate) use challenge::*;
pub(crate) use effect::*;
pub(crate) use objects::*;
pub(crate) use plant::*;
pub(crate) use seed::*;
pub(crate) use widget::*;
pub(crate) use zombie::*;
