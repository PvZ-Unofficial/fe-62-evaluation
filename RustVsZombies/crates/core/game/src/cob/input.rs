//! Select the single or batch result shape without converting inputs or allocating.
use super::{IntoCobTarget, IntoCobTargets};
use crate::model::CobTarget;

/// Implementation support for scripting overloads; custom batches still use IntoCobTargets.
pub trait FireInput: Sized {
    type Single: IntoCobTarget;
    type Batch: IntoCobTargets;
    type Output<S, B>;

    fn dispatch<S, B>(
        self, single: impl FnOnce(Self::Single) -> S, batch: impl FnOnce(Self::Batch) -> B,
    ) -> Self::Output<S, B>;
}

impl<T: IntoCobTargets> FireInput for T {
    type Single = CobTarget;
    type Batch = Self;
    type Output<S, B> = B;

    fn dispatch<S, B>(self, _single: impl FnOnce(Self::Single) -> S, batch: impl FnOnce(Self::Batch) -> B) -> B {
        batch(self)
    }
}

macro_rules! single_input {
    ($($ty:ty),+ $(,)?) => {$(
        impl FireInput for $ty {
            type Single = Self;
            type Batch = [Self; 0];
            type Output<S, B> = S;

            fn dispatch<S, B>(
                self, single: impl FnOnce(Self) -> S, _batch: impl FnOnce(Self::Batch) -> B,
            ) -> S {
                single(self)
            }
        }
    )+};
}

single_input!((i32, i32), (i32, f32), CobTarget);
