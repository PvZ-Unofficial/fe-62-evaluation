//! Optional operation audit, independent of diagnostics and policy state.
use super::*;

struct Audit {
    mode: u32,
    seed: u32,
    round: u32,
    count: u64,
    hashes: [u64; 2],
}
impl Default for Audit {
    fn default() -> Self {
        Self {
            mode: setting("SIX_ACTION_AUDIT", 0),
            seed: 0,
            round: 1,
            count: 0,
            hashes: [0xcbf29ce484222325, 0x84222325cbf29ce4],
        }
    }
}
impl Audit {
    fn feed(&mut self, fields: &[i64]) {
        self.count += 1;
        for value in fields {
            for byte in value.to_le_bytes() {
                self.hashes[0] = (self.hashes[0] ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                self.hashes[1] = self.hashes[1]
                    .rotate_left(7)
                    .wrapping_add(u64::from(byte))
                    .wrapping_mul(0x9e3779b185ebca87);
            }
        }
    }
}
thread_local! { static AUDIT: RefCell<Audit> = RefCell::new(Audit::default()); }

pub fn begin(seed: u32) {
    AUDIT.with_borrow_mut(|a| {
        let mode = a.mode;
        *a = Audit {
            mode,
            seed,
            ..Audit::default()
        };
    });
}
pub fn next_round() {
    AUDIT.with_borrow_mut(|a| a.round += 1);
}
pub fn record(op: i32, card: CardSelection, pos: Pos) {
    AUDIT.with_borrow_mut(|a| {
        if a.mode == 0 {
            return;
        }
        let (kind, copy) = match card {
            CardSelection::Plant(k) => (k, 0),
            CardSelection::Imitator(k) => (k, 1),
        };
        let clock = rsvz::timing::clock();
        let fields = [
            a.round as i64,
            clock as i64,
            op as i64,
            kind as i64,
            copy,
            pos.0 as i64,
            pos.1 as i64,
        ];
        a.feed(&fields);
        if a.mode >= 2 {
            rsvz::log(
                rsvz::LogLevel::Info,
                format_args!("six_two_action seed={} values={:?}", a.seed, fields),
            );
        }
    });
}
pub fn finish() {
    AUDIT.with_borrow(|a| {
        if a.mode != 0 {
            rsvz::log(
                rsvz::LogLevel::Info,
                format_args!(
                    "six_two_action_digest seed={} count={} hash={:016x}{:016x}",
                    a.seed, a.count, a.hashes[0], a.hashes[1]
                ),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audit_distinguishes_order_time_card_and_copy_without_reading_the_game() {
        let first = [1, 100, 1, 14, 0, 2, 3];
        let second = [1, 101, 1, 14, 1, 2, 3];
        let mut a = Audit::default();
        a.feed(&first);
        a.feed(&second);
        let mut b = Audit::default();
        b.feed(&second);
        b.feed(&first);
        assert_eq!(a.count, 2);
        assert_ne!(a.hashes, b.hashes);
        let mut c = Audit::default();
        c.feed(&first);
        c.feed(&first);
        assert_ne!(a.hashes, c.hashes);
    }
}
