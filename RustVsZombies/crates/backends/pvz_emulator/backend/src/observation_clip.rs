//! Opt-in observation encoder. Does not copy the world or advance random streams.
use crate::{PeBackend, PeBackendError, Result};
use rsvz_model::model::pe_replay::{HEADER_WORDS, MAX_BYTES, put};

impl PeBackend {
    /// Appends one bounded Fog observation at the caller's frame boundary.
    /// This stream is for presentation, not for restoring PE or Rust execution.
    pub fn append_observation_frame(&self, out: &mut Vec<u8>, round: u32, wave: i32, time: i32) -> Result<()> {
        // Worst-case fixed native capacities; reject before writing a partial frame.
        if out.len() + 400_000 > out.capacity().min(MAX_BYTES) {
            return Err(PeBackendError::OperationRejected("PE clip buffer capacity exhausted"));
        }
        self.with_current_world(|world| {
            let s = world.scene();
            let start = out.len();
            out.resize(start + HEADER_WORDS * 4, 0);
            let mut h = [0u32; HEADER_WORDS];
            h[1] = s.main_counter();
            h[2] = round;
            h[3] = wave as u32;
            h[4] = time as u32;
            h[6] = 3;
            // SAFETY: all reads remain inside this world borrow; no native update occurs.
            unsafe {
                h[5] = (*s.sun_data().as_ptr()).sun;
                for slot in 0..10 {
                    {
                        let card = s.card_at(slot)?;
                        let c = card.as_ptr().read();
                        let i = 11 + slot as usize * 3;
                        h[i] = c.type_ as u32;
                        h[i + 1] = c.imitater_type as u32;
                        h[i + 2] = c.cold_down;
                    }
                }
                let ice = s.ice_path_data().as_ptr();
                for row in 0..6 {
                    h[41 + row * 2] = (*ice).x.0[row] as u32;
                    h[42 + row * 2] = (*ice).countdown.0[row];
                }
                h[53] = (*s.spawn_data().as_ptr()).countdown.next_wave;
                h[54] = s.dancer_clock();
                let pool = s.plants();
                for i in 0..pool.max_used_count() {
                    let Some(v) = pool.get(i as i32) else { continue };
                    let p = v.as_ptr().read();
                    if p.is_dead {
                        continue;
                    }
                    h[7] += 1;
                    macro_rules! w {($($v:expr),* $(,)?)=>{$(put(out,$v);)*};}
                    w!(
                        pool.get_id(v).expect("occupied replay slot"),
                        p.type_,
                        p.row,
                        p.col,
                        p.x,
                        p.y,
                        p.hp,
                        p.status,
                        p.imitater_target,
                        p.is_sleeping,
                        p.countdown.effect,
                        p.countdown.status,
                        p.countdown.generate,
                        p.countdown.launch,
                        p.countdown.eaten,
                        p.cannon.x,
                        p.cannon.y,
                        p.is_smashed,
                        p.reanim.progress,
                        p.reanim.prev_progress,
                        p.reanim.fps,
                        p.reanim.begin_frame,
                        p.reanim.n_frames,
                        p.reanim.type_
                    );
                }
                let pool = s.zombies();
                for i in 0..pool.max_used_count() {
                    let Some(v) = pool.get(i as i32) else { continue };
                    let z = v.as_ptr().read();
                    if z.is_dead {
                        continue;
                    }
                    h[8] += 1;
                    macro_rules! w {($($v:expr),* $(,)?)=>{$(put(out,$v);)*};}
                    w!(
                        pool.get_id(v).expect("occupied replay slot"),
                        z.type_,
                        z.row,
                        z.x,
                        z.y,
                        z.int_x,
                        z.int_y,
                        z.hp,
                        z.accessory_1.hp,
                        z.accessory_2.hp,
                        z.status,
                        z.action,
                        z.dx,
                        z.countdown.freeze,
                        z.countdown.slow,
                        z.countdown.butter,
                        z.is_eating,
                        z.time_since_spawn,
                        z.spawn_wave,
                        z.dy,
                        z.countdown.action,
                        z.reanim.prev_fps,
                        z.bungee_target,
                        z.master_id,
                        z.has_balloon,
                        z.is_hypno,
                        z.is_not_dying,
                        z.is_blown,
                        z.reanim.progress,
                        z.reanim.prev_progress,
                        z.reanim.fps,
                        z.reanim.begin_frame,
                        z.reanim.n_frames,
                        z.reanim.type_,
                        z.reanim.n_repeated,
                        z.has_item_or_walk_left
                    );
                }
                let pool = s.projectiles();
                for i in 0..pool.max_used_count() {
                    let Some(v) = pool.get(i as i32) else { continue };
                    let p = v.as_ptr().read();
                    if p.is_disappeared {
                        continue;
                    }
                    h[9] += 1;
                    macro_rules! w {($($v:expr),* $(,)?)=>{$(put(out,$v);)*};}
                    w!(
                        pool.get_id(v).expect("occupied replay slot"),
                        p.type_,
                        p.row,
                        p.x,
                        p.y,
                        p.dy1,
                        p.dx,
                        p.dy2,
                        p.ddy,
                        p.dddy,
                        p.motion_type,
                        p.time_since_created,
                        p.int_x,
                        p.int_y,
                        p.is_visible,
                        p.shadow_y
                    );
                }
                let pool = s.griditems();
                for i in 0..pool.max_used_count() {
                    let Some(v) = pool.get(i as i32) else { continue };
                    let g = v.as_ptr().read();
                    if g.is_disappeared {
                        continue;
                    }
                    h[10] += 1;
                    for v in [
                        pool.get_id(v).expect("occupied replay slot"),
                        g.type_ as u32,
                        g.row,
                        g.col,
                        g.countdown as u32,
                    ] {
                        put(out, v);
                    }
                }
            }
            h[0] = (out.len() - start) as u32;
            for (i, value) in h.into_iter().enumerate() {
                out[start + i * 4..start + i * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
            Ok::<_, pe_rs::Error>(())
        })??;
        Ok(())
    }
}
