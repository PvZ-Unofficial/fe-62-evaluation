//! Bounded in-memory, lossless Lite replay stream. See 经验流协议.md.
use super::*;
use std::io::{BufWriter, Write};

const NONE: u64 = u64::MAX;
const ENDPOINT: u16 = u16::MAX;
#[derive(Clone, Copy)]
pub(super) struct Meta {
    pub seed: u32,
    pub round: u32,
    pub clock: i32,
    pub wave: i32,
    pub time: i32,
    pub action: u16,
    pub demo: bool,
    pub explored: bool,
    pub probability: f32,
    pub plan: Option<super::plan::Plan>,
}
struct Prepared {
    meta: Meta,
    selected: u16,
    candidates: Option<Vec<u8>>,
}
#[derive(Clone, Copy)]
struct Anchor {
    id: u64,
    serial: u64,
    clock: i32,
}
struct Replay<W: Write> {
    out: W,
    every: i32,
    max_states: u64,
    with_plan: bool,
    seed: u32,
    next_id: u64,
    serial: u64,
    timed: u64,
    states: u64,
    bytes: u64,
    root_bytes: u64,
    gaps: u32,
    rejected: u32,
    last_clock: i32,
    last_timed: i32,
    action_clock: i32,
    head: Option<Anchor>,
    reward: f32,
    pending: Option<Prepared>,
    active: bool,
    capped: bool,
}
fn u16b(v: &mut Vec<u8>, n: u16) {
    v.extend(n.to_le_bytes());
}
fn u32b(v: &mut Vec<u8>, n: u32) {
    v.extend(n.to_le_bytes());
}
fn i32b(v: &mut Vec<u8>, n: i32) {
    v.extend(n.to_le_bytes());
}
fn u64b(v: &mut Vec<u8>, n: u64) {
    v.extend(n.to_le_bytes());
}
fn f32b(v: &mut Vec<u8>, n: f32) {
    v.extend(n.to_le_bytes());
}

// Prefix is physically shared; candidate tails use exact-bit delta coding.
pub(super) fn encode_candidates(candidates: &[(Action, [f32; FEATURES])]) -> Vec<u8> {
    assert!(!candidates.is_empty());
    let mut out = Vec::new();
    u16b(&mut out, candidates.len() as u16);
    for x in &candidates[0].1[..104] {
        f32b(&mut out, *x);
    }
    let mut prev = [0u32; 88];
    for (a, x) in candidates {
        debug_assert!(
            x[..104]
                .iter()
                .zip(&candidates[0].1[..104])
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        u16b(&mut out, a.id() as u16);
        let mut mask = [0u8; 11];
        for i in 0..88 {
            if x[104 + i].to_bits() != prev[i] {
                mask[i / 8] |= 1 << (i % 8);
            }
        }
        out.extend(mask);
        for i in 0..88 {
            let bits = x[104 + i].to_bits();
            if bits != prev[i] {
                u32b(&mut out, bits);
                prev[i] = bits;
            }
        }
    }
    out
}
impl<W: Write> Replay<W> {
    fn new(mut out: W, every: i32, max_states: u64, worker: u32, with_plan: bool) -> Self {
        out.write_all(if with_plan { b"PVZEXP02" } else { b"PVZEXP01" })
            .expect("replay header");
        for n in [192, 4, every as u32, worker] {
            out.write_all(&n.to_le_bytes()).expect("replay header");
        }
        if with_plan {
            out.write_all(&(super::plan::WIDTH as u32).to_le_bytes())
                .expect("plan header");
        }
        Self {
            out,
            every,
            max_states,
            with_plan,
            seed: 0,
            next_id: 0,
            serial: 0,
            timed: 0,
            states: 0,
            bytes: if with_plan { 28 } else { 24 },
            root_bytes: if with_plan { 28 } else { 24 },
            gaps: 0,
            rejected: 0,
            last_clock: 0,
            last_timed: -1,
            action_clock: -1,
            head: None,
            reward: 0.,
            pending: None,
            active: false,
            capped: false,
        }
    }
    fn record(&mut self, tag: u8, data: &[u8]) {
        self.out.write_all(&[tag]).expect("replay tag");
        self.out
            .write_all(&(data.len() as u32).to_le_bytes())
            .expect("replay length");
        self.out.write_all(data).expect("replay payload");
        self.bytes += 5 + data.len() as u64;
    }
    fn begin(&mut self, seed: u32, clock: i32) {
        if self.active {
            self.end(self.last_clock, 2);
        }
        self.seed = seed;
        self.serial = 0;
        self.timed = 0;
        self.states = 0;
        self.root_bytes = self.bytes;
        self.gaps = 0;
        self.rejected = 0;
        self.last_clock = clock;
        self.last_timed = -1;
        self.action_clock = -1;
        self.head = None;
        self.reward = 0.;
        self.pending = None;
        self.active = true;
        self.capped = false;
    }
    fn wants(&self, action: u16, clock: i32) -> bool {
        self.active
            && !self.capped
            && (action != 0
                || self.head.is_none()
                || self.action_clock == clock
                || self.head.is_some_and(|a| clock - a.clock >= self.every))
    }
    fn count(&mut self, clock: i32) {
        assert!(clock >= self.last_clock, "replay clock reversed within a root");
        if clock % 5 == 0 && self.last_timed != clock {
            self.timed += 1;
            self.last_timed = clock;
        }
        self.last_clock = clock;
    }
    fn transition(&mut self, to: u64, clock: i32, kind: u8) {
        if let Some(from) = self.head.take() {
            let mut p = Vec::new();
            u64b(&mut p, from.id);
            u64b(&mut p, to);
            u32b(&mut p, (self.serial - from.serial) as u32);
            i32b(&mut p, clock - from.clock);
            f32b(&mut p, self.reward);
            p.push(kind);
            self.record(2, &p);
        }
        self.reward = 0.;
    }
    fn state(&mut self, p: Prepared, success: bool, endpoint: bool) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.states += 1;
        let m = p.meta;
        let mut b = Vec::new();
        u64b(&mut b, id);
        u32b(&mut b, m.seed);
        u32b(&mut b, m.round);
        i32b(&mut b, m.clock);
        i32b(&mut b, m.wave);
        i32b(&mut b, m.time);
        u64b(&mut b, self.serial);
        u16b(&mut b, if endpoint { ENDPOINT } else { p.selected });
        u16b(&mut b, m.action);
        b.push(u8::from(m.demo) | u8::from(m.explored) << 1 | u8::from(success) << 2 | u8::from(endpoint) << 3);
        f32b(&mut b, m.probability);
        b.extend(p.candidates.expect("anchor features"));
        self.record(1, &b);
        if self.with_plan {
            let plan = m.plan.expect("PVZEXP02 state requires Plan32");
            assert!(plan.iter().all(|v| v.is_finite()), "nonfinite replay Plan32");
            let mut payload = Vec::new();
            u64b(&mut payload, id);
            for value in plan {
                f32b(&mut payload, value);
            }
            self.record(5, &payload);
        }
        if self.states % 128 == 0 {
            self.out.flush().expect("replay flush");
        }
        id
    }
    fn finish_action(&mut self, success: bool) {
        let Some(p) = self.pending.take() else {
            return;
        };
        if !self.active {
            return;
        }
        let clock = p.meta.clock;
        let nonwait = p.meta.action != 0;
        self.count(clock);
        if !success {
            self.rejected += 1;
        }
        if p.candidates.is_some() {
            let id = self.state(p, success, false);
            self.transition(id, clock, if success { 0 } else { 3 });
            if success {
                self.head = Some(Anchor {
                    id,
                    serial: self.serial,
                    clock,
                });
            }
        } else if !success {
            self.transition(NONE, clock, 3);
        }
        if success && nonwait {
            self.action_clock = clock;
        }
        self.serial += 1;
        if self.max_states > 0 && self.states >= self.max_states && !self.capped {
            self.transition(NONE, clock, 4);
            self.capped = true;
        }
    }
    fn reward(&mut self, rounds: u32, clock: i32) {
        if !self.active {
            return;
        }
        let mut b = Vec::new();
        u32b(&mut b, self.seed);
        u32b(&mut b, rounds);
        i32b(&mut b, clock);
        f32b(&mut b, 1.);
        self.record(3, &b);
        if self.head.is_some() {
            self.reward += 1.;
        }
        self.last_clock = clock;
        self.out.flush().expect("replay reward flush");
    }
    fn gap(&mut self, clock: i32) {
        if !self.active {
            return;
        }
        self.count(clock);
        self.gaps += 1;
        self.transition(NONE, clock, 3);
        self.serial += 1;
    }
    fn endpoint(&mut self, p: Prepared, kind: u8) {
        if !self.active {
            return;
        }
        let clock = p.meta.clock;
        if self.head.is_some() {
            let id = self.state(p, false, true);
            self.transition(id, clock, kind);
        }
    }
    fn end(&mut self, clock: i32, kind: u8) {
        if !self.active {
            return;
        }
        self.transition(NONE, clock, kind);
        let mut b = Vec::new();
        u32b(&mut b, self.seed);
        i32b(&mut b, clock);
        b.push(kind);
        for n in [self.serial, self.timed, self.states, self.bytes - self.root_bytes] {
            u64b(&mut b, n);
        }
        u32b(&mut b, self.gaps);
        u32b(&mut b, self.rejected);
        self.record(4, &b);
        self.out.flush().expect("replay end flush");
        self.active = false;
        self.pending = None;
    }
}
struct Runtime {
    replay: Option<Replay<BufWriter<std::fs::File>>>,
}
impl Default for Runtime {
    fn default() -> Self {
        let replay = std::env::var("SIX_REPLAY").ok().map(|p| {
            assert!(
                std::env::var("SIX_POLICY").map_or(true, |v| matches!(v.as_str(), "rule" | "mlp-lite")),
                "replay supports rule or mlp-lite only"
            );
            assert!(
                std::env::var_os("SIX_POLICY_RULE_AFTER_DECISION").is_none()
                    && std::env::var_os("SIX_POLICY_WAVE_WINDOW").is_none(),
                "replay mixture is per-root; within-root control switching is not supported"
            );
            let path = process_path(&p);
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .expect("new replay file");
            Replay::new(
                BufWriter::with_capacity(256 * 1024, file),
                setting("SIX_REPLAY_EVERY", 50).max(1) as i32,
                setting("SIX_REPLAY_MAX_STATES", 0) as u64,
                rsvz::session::session_shard().index,
                setting("SIX_REPLAY_PLAN", 0) != 0,
            )
        });
        Self { replay }
    }
}
thread_local! {static REPLAY:RefCell<Runtime>=RefCell::new(Runtime::default());}
pub(super) fn plan_enabled() -> bool {
    REPLAY.with_borrow(|r| r.replay.as_ref().is_some_and(|r| r.with_plan))
}
pub(super) fn enabled() -> bool {
    REPLAY.with_borrow(|r| r.replay.is_some())
}
pub(super) fn demo() -> bool {
    enabled() && std::env::var("SIX_POLICY").map_or(true, |s| s == "rule")
}
pub(super) fn begin(seed: u32, clock: i32) {
    REPLAY.with_borrow_mut(|r| {
        if let Some(r) = &mut r.replay {
            r.begin(seed, clock);
        }
    });
}
pub(super) fn wants(action: Action, clock: i32) -> bool {
    REPLAY.with_borrow(|r| r.replay.as_ref().is_some_and(|r| r.wants(action.id() as u16, clock)))
}
pub(super) fn prepare(meta: Meta, selected: usize, candidates: Option<Vec<u8>>) {
    REPLAY.with_borrow_mut(|r| {
        if let Some(r) = &mut r.replay {
            assert!(r.pending.is_none(), "overlapping replay actions");
            r.pending = Some(Prepared {
                meta,
                selected: selected as u16,
                candidates,
            });
        }
    });
}
pub(super) fn finish(success: bool) {
    REPLAY.with_borrow_mut(|r| {
        if let Some(r) = &mut r.replay {
            r.finish_action(success);
        }
    });
}
pub(super) fn gap(clock: i32) {
    REPLAY.with_borrow_mut(|r| {
        if let Some(r) = &mut r.replay {
            r.gap(clock);
        }
    });
}
pub(super) fn reward(rounds: u32, clock: i32) {
    REPLAY.with_borrow_mut(|r| {
        if let Some(r) = &mut r.replay {
            r.reward(rounds, clock);
        }
    });
}
pub(super) fn endpoint(meta: Meta, candidates: Vec<u8>, kind: u8) {
    REPLAY.with_borrow_mut(|r| {
        if let Some(r) = &mut r.replay {
            r.endpoint(
                Prepared {
                    meta,
                    selected: ENDPOINT,
                    candidates: Some(candidates),
                },
                kind,
            );
        }
    });
}
pub(super) fn end(clock: i32, kind: u8) {
    REPLAY.with_borrow_mut(|r| {
        if let Some(r) = &mut r.replay {
            r.end(clock, kind);
        }
    });
}
pub(super) fn exit() {
    REPLAY.with_borrow_mut(|r| {
        if let Some(r) = &mut r.replay {
            r.end(r.last_clock, 2);
            r.out.flush().expect("replay exit flush");
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    fn meta(clock: i32, action: u16) -> Meta {
        Meta {
            seed: 7,
            round: 0,
            clock,
            wave: 1,
            time: clock,
            action,
            demo: false,
            explored: false,
            probability: 1.,
            plan: None,
        }
    }
    fn payload() -> Vec<u8> {
        encode_candidates(&[(Action { kind: 0, pos: (0, 0) }, [0.; FEATURES])])
    }
    fn take(r: &mut Replay<Vec<u8>>, clock: i32, action: u16, success: bool) {
        let save = r.wants(action, clock);
        r.pending = Some(Prepared {
            meta: meta(clock, action),
            selected: 0,
            candidates: save.then(payload),
        });
        r.finish_action(success);
    }
    fn records(bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
        let mut pos = if &bytes[..8] == b"PVZEXP02" { 28 } else { 24 };
        let mut result = Vec::new();
        while pos < bytes.len() {
            let tag = bytes[pos];
            let n = u32::from_le_bytes(bytes[pos + 1..pos + 5].try_into().unwrap()) as usize;
            pos += 5;
            result.push((tag, bytes[pos..pos + n].to_vec()));
            pos += n;
        }
        result
    }
    #[test]
    fn same_frame_actions_wait_sampling_and_round_reward_remain_ordered() {
        let mut r = Replay::new(Vec::new(), 50, 0, 0, false);
        r.begin(7, 0);
        take(&mut r, 0, 111, true);
        take(&mut r, 0, 0, true);
        for clock in (5..50).step_by(5) {
            take(&mut r, clock, 0, true);
        }
        r.reward(1, 49);
        take(&mut r, 50, 0, true);
        r.end(54, 1);
        let rows = records(&r.out);
        let t: Vec<_> = rows.iter().filter(|r| r.0 == 2).map(|r| &r.1).collect();
        assert_eq!(t.len(), 3);
        assert_eq!(i32::from_le_bytes(t[0][20..24].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(t[0][16..20].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(t[1][16..20].try_into().unwrap()), 10);
        assert_eq!(i32::from_le_bytes(t[1][20..24].try_into().unwrap()), 50);
        assert_eq!(f32::from_le_bytes(t[1][24..28].try_into().unwrap()), 1.);
        assert_eq!(t[1][28], 0);
        assert_eq!(t[2][28], 1);
        assert_eq!(r.serial, 12);
        assert_eq!(r.timed, 11);
    }
    #[test]
    fn rejected_action_and_gap_are_not_terminal_and_ids_survive_roots() {
        let mut r = Replay::new(Vec::new(), 50, 0, 0, false);
        r.begin(7, 0);
        take(&mut r, 0, 0, true);
        take(&mut r, 5, 111, false);
        assert!(r.head.is_none());
        assert_eq!(r.rejected, 1);
        take(&mut r, 10, 0, true);
        r.gap(15);
        r.end(20, 2);
        let before = r.next_id;
        r.begin(8, 0);
        take(&mut r, 0, 0, true);
        assert!(r.next_id > before);
        r.end(5, 1);
        let rows = records(&r.out);
        let t: Vec<_> = rows.iter().filter(|r| r.0 == 2).map(|r| r.1[28]).collect();
        assert_eq!(t, [3, 3, 1]);
        let states: Vec<_> = rows.iter().filter(|r| r.0 == 1).map(|r| &r.1).collect();
        assert_eq!(states[1][40] & 4, 0);
    }
    #[test]
    fn capped_endpoint_bootstraps_but_exit_without_endpoint_does_not() {
        let mut r = Replay::new(Vec::new(), 50, 0, 0, false);
        r.begin(7, 0);
        take(&mut r, 0, 0, true);
        r.endpoint(
            Prepared {
                meta: meta(20, 0),
                selected: ENDPOINT,
                candidates: Some(payload()),
            },
            2,
        );
        r.end(20, 2);
        let rows = records(&r.out);
        let t = &rows.iter().find(|r| r.0 == 2).unwrap().1;
        assert_eq!(t[28], 2);
        assert_ne!(u64::from_le_bytes(t[8..16].try_into().unwrap()), NONE);
        r.begin(8, 0);
        take(&mut r, 0, 0, true);
        r.end(10, 2);
        let rows = records(&r.out);
        let t = &rows.iter().rev().find(|r| r.0 == 2).unwrap().1;
        assert_eq!(t[28], 2);
        assert_eq!(u64::from_le_bytes(t[8..16].try_into().unwrap()), NONE);
    }
    #[test]
    fn delta_encoding_is_exact_including_signed_zero_and_unmodified_features() {
        let mut x = [0.; FEATURES];
        x[0] = 1.;
        x[105] = -0.;
        x[110] = 7.25;
        let mut y = x;
        y[110] = -9.;
        y[191] = f32::from_bits(1);
        let data = encode_candidates(&[
            (Action { kind: 0, pos: (0, 0) }, x),
            (Action { kind: 1, pos: (1, 1) }, y),
        ]);
        let mut pos = 2 + 104 * 4;
        let mut tail = [0u32; 88];
        for expected in [x, y] {
            pos += 2;
            let mask = &data[pos..pos + 11];
            pos += 11;
            for i in 0..88 {
                if mask[i / 8] & (1 << (i % 8)) != 0 {
                    tail[i] = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                    pos += 4;
                }
                assert_eq!(tail[i], expected[104 + i].to_bits());
            }
        }
        assert_eq!(pos, data.len());
    }
    #[test]
    fn v2_adds_one_finite_plan_record_without_changing_state_payload() {
        let mut v1 = Replay::new(Vec::new(), 50, 0, 0, false);
        let mut v2 = Replay::new(Vec::new(), 50, 0, 0, true);
        for r in [&mut v1, &mut v2] {
            r.begin(7, 0);
            let mut m = meta(0, 0);
            m.plan = Some(std::array::from_fn(|i| i as f32 / 32.));
            r.pending = Some(Prepared {
                meta: m,
                selected: 0,
                candidates: Some(payload()),
            });
            r.finish_action(true);
            r.end(5, 1);
        }
        assert_eq!(&v1.out[..8], b"PVZEXP01");
        assert_eq!(&v2.out[..8], b"PVZEXP02");
        assert_eq!(u32::from_le_bytes(v2.out[24..28].try_into().unwrap()), 32);
        let a = records(&v1.out);
        let b = records(&v2.out);
        assert_eq!(
            a.iter().find(|r| r.0 == 1).unwrap().1,
            b.iter().find(|r| r.0 == 1).unwrap().1
        );
        let plans: Vec<_> = b.iter().filter(|r| r.0 == 5).collect();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].1.len(), 136);
        assert_eq!(b[0].0, 1);
        assert_eq!(b[1].0, 5);
        for i in 0..32 {
            assert_eq!(
                f32::from_le_bytes(plans[0].1[8 + i * 4..12 + i * 4].try_into().unwrap()),
                i as f32 / 32.
            );
        }
    }
    #[test]
    #[should_panic(expected = "PVZEXP02 state requires Plan32")]
    fn v2_missing_plan_is_not_silently_zero() {
        let mut r = Replay::new(Vec::new(), 50, 0, 0, true);
        r.begin(7, 0);
        take(&mut r, 0, 0, true);
    }
}
