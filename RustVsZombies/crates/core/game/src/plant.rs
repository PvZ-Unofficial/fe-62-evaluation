//! Backend-neutral plant runtime proxy helpers.

use rsvz_current::CurrentBackend;

use rsvz_backend_api::backend::{PlantReadBackend, PlantRemoveBackend};
use rsvz_model::PlantId;
use rsvz_model::{Grid, GridPlantView, PlantKind};

use crate::live_value::LiveValue;
use crate::live_value::{live_or_missing, read_or_abort};

/// Id-only plant handle that re-resolves the object for every read or action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Plant {
    id: PlantId,
}

impl Plant {
    /// Creates a handle from a cross-frame plant ID.
    #[must_use]
    pub const fn from_id(id: PlantId) -> Self {
        Self { id }
    }

    /// Returns this handle's cross-frame plant ID.
    #[must_use]
    pub const fn id(&self) -> PlantId {
        self.id
    }
}

impl Plant
where
    CurrentBackend: PlantReadBackend,
{
    /// Checks whether this plant ID currently resolves to a live object.
    pub fn is_alive(&self) -> bool {
        read_or_abort(
            CurrentBackend::with_backend_shared(|backend| read_or_abort(backend.plant(self.id), "plant").is_some()),
            "failed to resolve plant id",
        )
    }

    /// Reads HP as a convenience live value.
    ///
    /// Stale or missing IDs produce `LiveValue::missing()`.
    ///
    /// A backend read failure ends the current callback through its error boundary.
    #[must_use]
    pub fn hp(&self) -> LiveValue<i32> {
        live_or_missing(
            CurrentBackend::with_backend_shared(|backend| {
                read_or_abort(backend.plant(self.id), "plant").map(|plant| backend.plant_hp(plant))
            }),
            "failed to read plant hp",
        )
    }

    /// Reads grid as a convenience live value.
    ///
    /// Stale or missing IDs produce `LiveValue::missing()`.
    ///
    /// A backend read failure ends the current callback through its error boundary.
    #[must_use]
    pub fn grid(&self) -> LiveValue<Grid> {
        live_or_missing(
            CurrentBackend::with_backend_shared(|backend| {
                read_or_abort(backend.plant(self.id), "plant")
                    .map(|plant| crate::plant::grid_from_handle(backend, plant))
            }),
            "failed to read plant grid",
        )
    }

    /// Reads kind as a convenience live value.
    ///
    /// Stale or missing IDs produce `LiveValue::missing()`.
    ///
    /// A backend read failure ends the current callback through its error boundary.
    #[must_use]
    pub fn kind(&self) -> LiveValue<PlantKind> {
        live_or_missing(
            CurrentBackend::with_backend_shared(|backend| {
                read_or_abort(backend.plant(self.id), "plant")
                    .map(|plant| read_or_abort(backend.plant_kind(plant), "plant_kind"))
            }),
            "failed to read plant kind",
        )
    }
}

impl Plant
where
    CurrentBackend: PlantRemoveBackend,
{
    /// Removes this plant if the ID currently resolves to a live object.
    ///
    /// Returns `true` when applied and `false` for a stale ID; operation failures end the callback.
    pub fn remove_by_id(&self) -> bool {
        crate::modifier::remove_plant_by_id(self.id)
    }
}

/// Visits scalar views of all currently live plants without allocating a collection.
pub fn for_each_plant(mut visit: impl FnMut(GridPlantView))
where
    CurrentBackend: PlantReadBackend,
{
    read_or_abort(
        CurrentBackend::with_backend_shared(|backend| {
            for plant in crate::live_value::read_or_abort(backend.plants(), "plants") {
                visit(crate::plant::view_from_handle(backend, plant));
            }
        }),
        "failed to visit entities",
    )
}

pub(crate) fn grid_from_handle<'a>(
    backend: &'a CurrentBackend, handle: <CurrentBackend as PlantReadBackend>::PlantHandle<'a>,
) -> Grid
where
    CurrentBackend: PlantReadBackend,
{
    Grid {
        row: backend.plant_row(handle),
        col: backend.plant_col(handle),
    }
}

pub(crate) fn view_from_handle<'a>(
    backend: &'a CurrentBackend, handle: <CurrentBackend as PlantReadBackend>::PlantHandle<'a>,
) -> GridPlantView
where
    CurrentBackend: PlantReadBackend,
{
    GridPlantView {
        id: backend.plant_id(handle),
        kind: crate::live_value::read_or_abort(backend.plant_kind(handle), "plant_kind"),
        grid: grid_from_handle(backend, handle),
        hp: backend.plant_hp(handle),
    }
}

/// 查找锚点为 `grid` 的第一株存活 `kind` 植物。
///
/// 元组使用 1-based 坐标，Grid 使用 0-based 坐标；无效元组或没有匹配植物返回 None。
pub fn find_plant_at_kind<G: crate::logic::IntoGrid>(grid: G, kind: PlantKind) -> Option<PlantId>
where
    CurrentBackend: PlantReadBackend,
{
    let grid = grid.try_into_grid().ok()?;
    crate::live_value::read_or_abort(
        crate::access::with_backend(|backend| -> Result<_, rsvz_current::CurrentBackendError> {
            let mut found = None;
            backend.for_each_plant_at_anchor_grid(grid, |plant| {
                if crate::live_value::read_or_abort(backend.plant_kind(plant), "plant_kind") == kind && found.is_none()
                {
                    found = Some(backend.plant_id(plant));
                }
                Ok(())
            })?;
            Ok(found)
        }),
        "failed to read find_plant_at_kind",
    )
}
