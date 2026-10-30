use crate::{PeBackend, PeBackendError, Result};
use rsvz_backend_api::backend::{PlantReadBackend, SceneBackend};
use rsvz_model::model::{
    CheckedCardSelection, ContactRect, Grid, GridItemKind, MaidCheat, PlantKind, PlantWeapon, ZombieKind, ZombiePhase,
};
use std::ptr::NonNull;

fn normalize_plant_state(state: i32) -> i32 {
    match state {
        // PE orders launch/armed as 0x25/0x26; PvZ 1051 and the backend-neutral
        // cob logic use firing/ready as 0x26/0x25.
        0x25 => 0x26,
        0x26 => 0x25,
        state => state,
    }
}

macro_rules! read_pe_raw_field {
    ($ptr:expr, $($field:tt)+) => {{
        let ptr = $ptr;
        // SAFETY: Call sites pass PE current-frame object or card-slot pointers.
        // Field reads copy scalar values immediately and never build safe references
        // into PE world-owned storage.
        unsafe { ::std::ptr::addr_of!((*ptr).$($field)+).read() }
    }};
}

macro_rules! write_pe_raw_field {
    ($ptr:expr, $value:expr, $($field:tt)+) => {{
        let ptr = $ptr;
        // SAFETY: Call sites pass a current-world, borrow-bound pool object and
        // write one address-stable scalar field without retaining a reference.
        unsafe { ::std::ptr::addr_of_mut!((*ptr).$($field)+).write($value) }
    }};
}

fn pe_plant_kind_from_handle(handle: crate::handles::PePlantHandle<'_>) -> Result<PlantKind> {
    pe_raw_plant_kind(read_pe_raw_field!(handle.as_ptr(), type_))
}

fn pe_grid_item_kind_from_handle(handle: crate::handles::PeGridItemHandle<'_>) -> Result<GridItemKind> {
    pe_raw_grid_item_kind(read_pe_raw_field!(handle.as_ptr(), type_))
}

fn pe_zombie_kind_from_handle(handle: crate::handles::PeZombieHandle<'_>) -> Result<ZombieKind> {
    pe_raw_zombie_kind(read_pe_raw_field!(handle.as_ptr(), type_))
}

fn pe_contact_rect(raw: pe_rs::raw::pe_rs_rect) -> Result<ContactRect> {
    ContactRect::checked_from_pos_size(raw.x, raw.y, raw.width, raw.height)
        .ok_or(PeBackendError::OperationRejected("invalid PE contact rectangle"))
}

const fn pe_plant_weapon(weapon: PlantWeapon) -> pe_rs::PlantWeapon {
    match weapon {
        PlantWeapon::Primary => pe_rs::PlantWeapon::Primary,
        PlantWeapon::Secondary => pe_rs::PlantWeapon::Secondary,
    }
}

fn pe_u32_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

const fn canonical_relation_id(value: i32) -> u32 {
    if value == -1 { 0 } else { value as u32 }
}

fn pe_card_selection_from_raw(ptr: *const pe_rs::raw::pe_rs_card) -> Result<Option<CheckedCardSelection>> {
    let plant_type = pe_raw_plant_type(read_pe_raw_field!(ptr, type_))?;
    if matches!(plant_type, pe_rs::PlantType::None) {
        return Ok(None);
    }
    let imitater_type = pe_raw_plant_type(read_pe_raw_field!(ptr, imitater_type))?;
    crate::convert::kind::card_from_pe(plant_type, imitater_type).map(Some)
}

fn validate_grid(backend: &PeBackend, grid: Grid) -> Result<()> {
    let field = rsvz_model::FieldInfo::from_scene(backend.scene()?);
    if !field.contains_grid(grid) {
        return Err(PeBackendError::InvalidGrid);
    }
    Ok(())
}

impl PeBackend {
    pub(crate) fn pe_grid_item_is_live<'a>(&self, handle: crate::handles::PeGridItemHandle<'a>) -> bool {
        !read_pe_raw_field!(handle.as_ptr(), is_disappeared)
    }

    fn visit_plant_map_grid<'a, F>(
        &'a self, grid: Grid, visited: &mut [*mut pe_rs::raw::pe_rs_plant], visited_len: &mut usize, visit: &mut F,
    ) -> Result<()>
    where
        F: FnMut(crate::handles::PePlantHandle<'a>) -> Result<()>,
    {
        let status = self.with_current_world(|world| {
            world
                .scene()
                .grid_plant_status_at(grid.col, grid.row)
                .map(|status| status.as_ptr())
        })?;
        let status = match status {
            Ok(status) => Some(status),
            // Out-of-lawn queries still follow the trait's full-pool semantics.
            Err(pe_rs::Error::InvalidArgument { .. }) => None,
            Err(error) => return Err(error.into()),
        };
        if status.is_none() || self.with_current_world(|world| world.scene().plant_map_incomplete(grid.row))?? {
            for plant in self.plants()? {
                let anchor = Grid {
                    row: self.plant_row(plant),
                    col: self.plant_col(plant),
                };
                if rsvz_model::plant_occupies_grid(anchor, self.plant_kind(plant)?, grid) {
                    visit(plant)?;
                }
            }
            return Ok(());
        }
        let status = status.expect("the incomplete-map branch handled missing entries");
        // SAFETY: `status` is the current world's grid-map entry. Each field is
        // copied independently; no four-pointer snapshot is constructed.
        let pumpkin = unsafe { std::ptr::addr_of!((*status).pumpkin).read() };
        self.visit_plant_map_ptr(pumpkin, visited, visited_len, visit)?;
        // SAFETY: same current grid-map entry and scalar pointer read as above.
        let base = unsafe { std::ptr::addr_of!((*status).base).read() };
        self.visit_plant_map_ptr(base, visited, visited_len, visit)?;
        // SAFETY: same current grid-map entry and scalar pointer read as above.
        let content = unsafe { std::ptr::addr_of!((*status).content).read() };
        self.visit_plant_map_ptr(content, visited, visited_len, visit)?;
        // SAFETY: same current grid-map entry and scalar pointer read as above.
        let coffee = unsafe { std::ptr::addr_of!((*status).coffee_bean).read() };
        self.visit_plant_map_ptr(coffee, visited, visited_len, visit)?;
        Ok(())
    }

    fn visit_plant_map_ptr<'a, F>(
        &'a self, ptr: *mut pe_rs::raw::pe_rs_plant, visited: &mut [*mut pe_rs::raw::pe_rs_plant],
        visited_len: &mut usize, visit: &mut F,
    ) -> Result<()>
    where
        F: FnMut(crate::handles::PePlantHandle<'a>) -> Result<()>,
    {
        let Some(ptr) = NonNull::new(ptr) else {
            return Ok(());
        };
        let raw = ptr.as_ptr();
        let handle = crate::handles::PePlantHandle::new(ptr);
        if !self.pe_plant_is_live(handle) || visited[..*visited_len].contains(&raw) {
            return Ok(());
        }
        if *visited_len < visited.len() {
            visited[*visited_len] = raw;
            *visited_len += 1;
        }
        visit(handle)
    }

    pub(crate) fn pe_plant_is_live<'a>(&self, handle: crate::handles::PePlantHandle<'a>) -> bool {
        !read_pe_raw_field!(handle.as_ptr(), is_dead)
            && !read_pe_raw_field!(handle.as_ptr(), is_smashed)
            && read_pe_raw_field!(handle.as_ptr(), edible)
                != pe_rs::raw::pvz_emulator_object_plant_edible_status_invisible_and_not_edible
            && !(read_pe_raw_field!(handle.as_ptr(), type_) == pe_rs::raw::pvz_emulator_object_plant_type_squash
                && read_pe_raw_field!(handle.as_ptr(), status)
                    == pe_rs::raw::pvz_emulator_object_plant_status_squash_crushed)
    }

    pub(crate) fn pe_zombie_is_live<'a>(&self, handle: crate::handles::PeZombieHandle<'a>) -> bool {
        let status = read_pe_raw_field!(handle.as_ptr(), status);
        !read_pe_raw_field!(handle.as_ptr(), is_dead)
            && status != pe_rs::raw::pvz_emulator_object_zombie_status_dying
            && status != pe_rs::raw::pvz_emulator_object_zombie_status_dying_from_instant_kill
            && status != pe_rs::raw::pvz_emulator_object_zombie_status_dying_from_lawnmower
    }
}

fn pe_raw_plant_type(value: pe_rs::raw::pvz_emulator_object_plant_type) -> Result<pe_rs::PlantType> {
    pe_rs::PlantType::from_raw(value).ok_or(PeBackendError::NumericOutOfRange("plant.type"))
}

fn pe_raw_zombie_type(value: pe_rs::raw::pvz_emulator_object_zombie_type) -> Result<pe_rs::ZombieType> {
    pe_rs::ZombieType::from_raw(value).ok_or(PeBackendError::NumericOutOfRange("zombie.type"))
}

fn pe_raw_grid_item_type(value: pe_rs::raw::pvz_emulator_object_griditem_type) -> Result<pe_rs::GridItemType> {
    pe_rs::GridItemType::from_raw(value).ok_or(PeBackendError::NumericOutOfRange("griditem.type"))
}

fn pe_raw_plant_kind(value: pe_rs::raw::pvz_emulator_object_plant_type) -> Result<PlantKind> {
    crate::convert::kind::plant_from_pe(pe_raw_plant_type(value)?)
}

fn pe_raw_grid_item_kind(value: pe_rs::raw::pvz_emulator_object_griditem_type) -> Result<GridItemKind> {
    crate::convert::kind::grid_item_from_pe(pe_raw_grid_item_type(value)?)
}

fn pe_raw_zombie_kind(value: pe_rs::raw::pvz_emulator_object_zombie_type) -> Result<ZombieKind> {
    crate::convert::kind::zombie_from_pe(pe_raw_zombie_type(value)?)
}

fn pe_zombie_phase_from_raw(raw: i32) -> Result<ZombiePhase> {
    ZombiePhase::try_from_code(raw).map_err(|_error| PeBackendError::InvalidKind {
        kind: "zombie phase",
        raw,
    })
}

#[cfg(test)]
mod zombie_phase_tests {
    use super::pe_zombie_phase_from_raw;
    use rsvz_model::model::ZombiePhase;

    #[test]
    fn pe_phase_values_map_at_the_backend_boundary() {
        for raw in 0..=95 {
            assert!(pe_zombie_phase_from_raw(raw).is_ok(), "missing PE phase {raw}");
        }
        assert_eq!(
            pe_zombie_phase_from_raw(70).expect("smashing"),
            ZombiePhase::GargantuarSmashing
        );
        assert_eq!(pe_zombie_phase_from_raw(91).expect("yeti"), ZombiePhase::YetiRunning);
        assert!(pe_zombie_phase_from_raw(96).is_err());
    }
}

fn validate_target_row(backend: &PeBackend, row: i32) -> Result<()> {
    let field = rsvz_model::FieldInfo::from_scene(backend.scene()?);
    if !field.contains_row(row) {
        return Err(PeBackendError::InvalidGrid);
    }
    Ok(())
}

fn maid_cheat_to_pe(cheat: MaidCheat) -> pe_rs::MaidCheat {
    match cheat {
        MaidCheat::Stop => pe_rs::MaidCheat::Stop,
        MaidCheat::CallPartner => pe_rs::MaidCheat::CallPartner,
        MaidCheat::Dancing => pe_rs::MaidCheat::Dancing,
        MaidCheat::Move => pe_rs::MaidCheat::Move,
    }
}

fn maid_cheat_from_pe(cheat: pe_rs::MaidCheat) -> MaidCheat {
    match cheat {
        pe_rs::MaidCheat::Stop => MaidCheat::Stop,
        pe_rs::MaidCheat::CallPartner => MaidCheat::CallPartner,
        pe_rs::MaidCheat::Dancing => MaidCheat::Dancing,
        pe_rs::MaidCheat::Move => MaidCheat::Move,
    }
}

mod gameplay;
mod item;
mod modifier;
mod opening;
mod plant;
mod state;
mod zombie;

mod paint;
