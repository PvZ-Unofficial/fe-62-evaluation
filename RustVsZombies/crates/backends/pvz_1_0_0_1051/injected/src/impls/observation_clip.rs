//! Paused presentation of validated PE observations. No game simulation is run.
use crate::{
    Pvz1051Backend,
    error::{Pvz1051Error, Result},
    raw::{abi, layout as p},
};
use rsvz_backend_api::{AdvancedPauseBackend, DeadObjectReclaimBackend};
use rsvz_model::model::pe_replay::{Entity, Frame, Objects};
use std::ptr::NonNull;

// Offsets are the same 1051 fields used by raw/layout and the native save schema.
trait NativeScalar: Copy {}
impl NativeScalar for i32 {}
impl NativeScalar for u32 {}
impl NativeScalar for f32 {}
impl NativeScalar for bool {}

unsafe fn w<T: NativeScalar>(ptr: *mut impl Sized, offset: usize, value: T) {
    unsafe {
        ptr.cast::<u8>().add(offset).cast::<T>().write_unaligned(value);
    }
}
unsafe fn r<T: Copy>(ptr: *const impl Sized, offset: usize) -> T {
    unsafe { ptr.cast::<u8>().add(offset).cast::<T>().read_unaligned() }
}
fn missing() -> Pvz1051Error {
    Pvz1051Error::OperationRejected("PE presentation object allocation failed")
}
unsafe fn pose(app: *mut p::LawnApp, id: u32, e: Entity<'_>, at: usize) -> Result<()> {
    unsafe {
        let effect = p::LawnApp::effect_system(app);
        if effect.is_null() {
            return Err(missing());
        }
        let holder = p::EffectSystem::reanimation_holder(effect);
        if holder.is_null() {
            return Err(missing());
        }
        if let Some(a) = p::DataArray::try_to_get(p::ReanimationHolder::reanimations(holder), id) {
            let a = a.as_ptr();
            if e.u(at + 4) == 0 {
                return Ok(());
            }
            let def: *const u8 = r(a, 12);
            if def.is_null() {
                return Err(missing());
            }
            let tracks: *const u8 = r(def, 0);
            let count: i32 = r(def, 4);
            if tracks.is_null() || count < 1 {
                return Err(missing());
            }
            for i in 0..count as usize {
                let length: i32 = r(tracks.add(i * 12), 8);
                if e.i(at + 3).checked_add(e.i(at + 4)).is_none_or(|end| end > length) {
                    return Err(Pvz1051Error::OperationRejected(
                        "PE animation outside native definition",
                    ));
                }
            }
            w(a, 4, e.f(at));
            w(a, 0x94, e.f(at + 1));
            w(a, 8, e.f(at + 2));
            w(a, 0x18, e.i(at + 3));
            w(a, 0x1c, e.i(at + 4));
            w(a, 0x10, e.i(at + 5));
            if at == 28 && e.i(1) == 17 {
                // 52C595..52C5F5 writes m00 at +24 and m02 at +2C;
                // backwards adds 90px before negating scale. PE scale is 1.
                w(a, 0x24, if e.digger_faces_right() { -1.0_f32 } else { 1.0_f32 });
                w(
                    a,
                    0x2c,
                    e.f(3) - e.i(5) as f32 + 15.0 + if e.digger_faces_right() { 90.0 } else { 0.0 },
                );
            }
        }
        Ok(())
    }
}

impl Pvz1051Backend {
    pub fn present_observation_frame(&mut self, frame: Frame<'_>, objects: &mut Objects) -> Result<()> {
        if !self.advanced_pause_active() {
            return Err(Pvz1051Error::OperationRejected(
                "PE presentation requires advanced pause",
            ));
        }
        let board = self.board()?.as_ptr();
        let app = self.app().as_ptr();
        // SAFETY: exclusive backend token, validated frame and native pool identities.
        // Pointers are used only inside this call, never retained in Objects.
        unsafe {
            for group in 0..4 {
                let mut live = [0u32; 1024];
                for e in frame.entities(group) {
                    live[(e.u(0) & 0xffff) as usize] = e.u(0);
                }
                for (slot, pair) in objects.ids[group].iter_mut().enumerate() {
                    if pair.0 != 0 && pair.0 != live[slot] {
                        match group {
                            0 => {
                                if let Some(x) = p::DataArray::try_to_get(p::Board::plants(board), pair.1) {
                                    abi::plant_die(x.as_ptr());
                                }
                            }
                            1 => {
                                if let Some(x) = p::DataArray::try_to_get(p::Board::zombies(board), pair.1) {
                                    abi::zombie_die_no_loot(x.as_ptr());
                                }
                            }
                            2 => {
                                if let Some(x) = p::DataArray::try_to_get(p::Board::projectiles(board), pair.1) {
                                    w(x.as_ptr(), 0x50, true);
                                }
                            }
                            _ => {
                                if let Some(x) = p::DataArray::try_to_get(p::Board::grid_items(board), pair.1) {
                                    abi::grid_item_die(x.as_ptr());
                                }
                            }
                        }
                        *pair = (0, 0);
                    }
                }
            }
        }
        self.reclaim_dead_objects()?;
        unsafe {
            let h = frame.header();
            p::Board::set_sun(board, h.i(5));
            w(board, 0x5568, h.i(1));
            w(board, 0x557c, h.i(3));
            for slot in 0..10 {
                let packet = p::SeedBank::packet(p::Board::seed_bank(board), slot);
                let i = 11 + slot * 3;
                p::SeedPacket::set_packet_type(packet, h.i(i));
                p::SeedPacket::set_imitater_type(packet, h.i(i + 1));
                p::SeedPacket::set_visible(packet, true);
                let kind = rsvz_model::PlantKind::try_from_code(if h.i(i) == 48 { h.i(i + 1) } else { h.i(i) })
                    .map_err(|_| missing())?;
                let selection = if h.i(i) == 48 {
                    rsvz_model::CardSelection::Imitator(kind)
                } else {
                    rsvz_model::CardSelection::Plant(kind)
                }
                .checked()
                .map_err(|_| missing())?;
                let full = rsvz_backend_api::SeedDefinitionBackend::seed_base_recharge(self, selection)?;
                p::SeedPacket::set_refresh_time(packet, full);
                p::SeedPacket::set_refresh_counter(packet, (full - h.i(i + 2)).max(0));
                p::SeedPacket::set_refreshing(packet, h.i(i + 2) > 0);
                p::SeedPacket::set_active(packet, h.i(i + 2) == 0);
            }
            for row in 0..6 {
                w(board, 0x60c + row * 4, h.i(41 + row * 2));
                w(board, 0x624 + row * 4, h.i(42 + row * 2));
            }
            for e in frame.entities(0) {
                let pair = &mut objects.ids[0][(e.u(0) & 0xffff) as usize];
                let pool = p::Board::plants(board);
                let existing = if pair.0 == e.u(0) {
                    p::DataArray::try_to_get(pool, pair.1)
                } else {
                    None
                };
                let plant = match existing {
                    Some(v) => v,
                    None => NonNull::new(abi::board_new_plant(e.i(2), e.i(3), e.i(1), e.i(8))).ok_or_else(missing)?,
                };
                let q = plant.as_ptr();
                *pair = (e.u(0), p::DataArray::item_id(q));
                for (offset, i) in [
                    (8, 4),
                    (12, 5),
                    (0x40, 6),
                    (0x3c, 7),
                    (0x50, 10),
                    (0x54, 11),
                    (0x58, 12),
                    (0x90, 13),
                    (0xb4, 14),
                    (0x80, 15),
                    (0x84, 16),
                ] {
                    w(q, offset, e.i(i));
                }
                w(q, 0x143, e.u(9) != 0);
                w(q, 0x142, e.u(17) != 0);
                pose(app, r(q, 0x94), e, 18)?;
            }
            for e in frame.entities(1) {
                let pair = &mut objects.ids[1][(e.u(0) & 0xffff) as usize];
                let pool = p::Board::zombies(board);
                let existing = if pair.0 == e.u(0) {
                    p::DataArray::try_to_get(pool, pair.1)
                } else {
                    None
                };
                let z = match existing {
                    Some(v) => v,
                    None => NonNull::new(abi::board_add_zombie_in_row(board, e.i(1), e.i(2), e.i(18)))
                        .ok_or_else(missing)?,
                };
                let q = z.as_ptr();
                *pair = (e.u(0), p::DataArray::item_id(q));
                for (offset, i) in [(0x2c, 3), (0x30, 4), (0x34, 12), (0x84, 19), (0x124, 21)] {
                    w(q, offset, e.f(i));
                }
                for (offset, i) in [
                    (8, 5),
                    (12, 6),
                    (0xc8, 7),
                    (0xd0, 8),
                    (0xdc, 9),
                    (0x28, 10),
                    (0xb4, 13),
                    (0xac, 14),
                    (0xb0, 15),
                    (0x60, 17),
                    (0x6c, 18),
                    (0x68, 20),
                ] {
                    w(q, offset, e.i(i));
                }
                for (offset, i) in [(0x51, 16), (0xb8, 25), (0xba, 26), (0xb9, 27), (0xbc, 35)] {
                    w(q, offset, e.u(i) != 0);
                }
                pose(app, p::Zombie::body_reanim_id(q), e, 28)?;
            }
            for e in frame.entities(2) {
                let pair = &mut objects.ids[2][(e.u(0) & 0xffff) as usize];
                let pool = p::Board::projectiles(board);
                let existing = if pair.0 == e.u(0) {
                    p::DataArray::try_to_get(pool, pair.1)
                } else {
                    None
                };
                let z = match existing {
                    Some(v) => v,
                    None => NonNull::new(abi::board_replay_projectile(
                        board,
                        e.i(12),
                        e.i(13),
                        400000 + e.i(2) * 10000,
                        e.i(2),
                        e.i(1),
                    ))
                    .ok_or_else(missing)?,
                };
                let q = z.as_ptr();
                *pair = (e.u(0), p::DataArray::item_id(q));
                if r::<i32>(q, 0x5c) != e.i(1) || r::<i32>(q, 0x1c) != e.i(2) {
                    w(q, 0x50, true);
                    return Err(Pvz1051Error::OperationRejected(
                        "projectile constructor ABI readback mismatch",
                    ));
                }
                for (offset, i) in [
                    (0x30, 3),
                    (0x34, 4),
                    (0x38, 5),
                    (0x3c, 6),
                    (0x40, 7),
                    (0x44, 8),
                    (0x48, 9),
                    (0x4c, 15),
                ] {
                    w(q, offset, e.f(i));
                }
                for (offset, i) in [(0x58, 10), (0x60, 11), (8, 12), (12, 13)] {
                    w(q, offset, e.i(i));
                }
                w(q, 0x18, e.u(14) != 0);
            }
            for e in frame.entities(3) {
                let pair = &mut objects.ids[3][(e.u(0) & 0xffff) as usize];
                let pool = p::Board::grid_items(board);
                let existing = if pair.0 == e.u(0) {
                    p::DataArray::try_to_get(pool, pair.1)
                } else {
                    None
                };
                let z = match existing {
                    Some(v) => v,
                    None => NonNull::new(match e.i(1) {
                        2 => abi::board_add_crater(e.i(2), e.i(3)),
                        3 => abi::board_add_ladder(e.i(2), e.i(3)),
                        _ => return Err(Pvz1051Error::OperationRejected("grave replay not implemented")),
                    })
                    .ok_or_else(missing)?,
                };
                let q = z.as_ptr();
                *pair = (e.u(0), p::DataArray::item_id(q));
                w(q, 0x18, e.i(4));
            }
        }
        Ok(())
    }
}
