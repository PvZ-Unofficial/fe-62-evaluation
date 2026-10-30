//! Versioned, one-way PE observation clips. These are not simulation checkpoints.
use std::ops::Range;

pub const MAGIC: &[u8; 8] = b"RSVZPE01";
pub const HEADER_WORDS: usize = 56;
pub const STRIDES: [usize; 4] = [24, 36, 16, 5];
pub const MAX_BYTES: usize = 256 * 1024 * 1024;

pub fn word(bytes: &[u8], index: usize) -> u32 {
    u32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
}
pub fn put(bytes: &mut Vec<u8>, value: impl IntoWord) {
    bytes.extend_from_slice(&value.word().to_le_bytes());
}
pub trait IntoWord {
    fn word(self) -> u32;
}
impl IntoWord for u32 {
    fn word(self) -> u32 {
        self
    }
}
impl IntoWord for i32 {
    fn word(self) -> u32 {
        self as u32
    }
}
impl IntoWord for bool {
    fn word(self) -> u32 {
        u32::from(self)
    }
}
impl IntoWord for f32 {
    fn word(self) -> u32 {
        self.to_bits()
    }
}

#[derive(Clone, Copy)]
pub struct Entity<'a>(pub &'a [u8]);
impl Entity<'_> {
    /// Native Digger IsWalkingBackwards: direction is phase-based, also while frozen.
    pub fn digger_faces_right(self) -> bool {
        self.u(25) != 0 || matches!(self.i(10), 33 | 36 | 37) || (matches!(self.i(10), 1..=3) && self.u(35) != 0)
    }
    pub fn u(self, i: usize) -> u32 {
        word(self.0, i)
    }
    pub fn i(self, i: usize) -> i32 {
        self.u(i) as i32
    }
    pub fn f(self, i: usize) -> f32 {
        f32::from_bits(self.u(i))
    }
}

#[derive(Clone, Copy)]
pub struct Frame<'a> {
    bytes: &'a [u8],
}
impl<'a> Frame<'a> {
    pub fn header(self) -> Entity<'a> {
        Entity(&self.bytes[..HEADER_WORDS * 4])
    }
    pub fn entities(self, group: usize) -> impl Iterator<Item = Entity<'a>> {
        let begin = HEADER_WORDS
            + (0..group)
                .map(|g| self.header().u(7 + g) as usize * STRIDES[g])
                .sum::<usize>();
        let end = begin + self.header().u(7 + group) as usize * STRIDES[group];
        self.bytes[begin * 4..end * 4]
            .chunks_exact(STRIDES[group] * 4)
            .map(Entity)
    }
    pub fn parse(bytes: &'a [u8]) -> Result<Self, &'static str> {
        if bytes.len() < HEADER_WORDS * 4 || bytes.len() % 4 != 0 || word(bytes, 0) as usize != bytes.len() {
            return Err("invalid PE frame length");
        }
        let value = Self { bytes };
        let h = value.header();
        if h.i(1) < 0 || !(0..=20).contains(&h.i(3)) || h.u(6) != 3 {
            return Err("prototype requires a Fog Endless frame");
        }
        for slot in 0..10 {
            let at = 11 + slot * 3;
            if !(0..49).contains(&h.i(at))
                || !(-1..49).contains(&h.i(at + 1))
                || (h.i(at) == 48 && !(0..48).contains(&h.i(at + 1)))
            {
                return Err("invalid PE seed packet");
            }
        }
        let mut words = HEADER_WORDS;
        for (g, stride) in STRIDES.iter().enumerate() {
            let count = h.u(7 + g) as usize;
            if count > if g == 3 { 128 } else { 1024 } {
                return Err("PE entity capacity exceeded");
            }
            words += count * stride;
        }
        if words * 4 != bytes.len() {
            return Err("PE frame sections do not match length");
        }
        for g in 0..4 {
            let mut seen = [false; 1024];
            for e in value.entities(g) {
                let slot = (e.u(0) & 0xffff) as usize;
                if e.u(0) >> 16 == 0 || slot >= if g == 3 { 128 } else { 1024 } || seen[slot] {
                    return Err("invalid or duplicate PE entity identity");
                }
                seen[slot] = true;
                if !(0..6).contains(&e.i(2)) {
                    return Err("invalid PE row");
                }
                let valid_kind = match g {
                    0 => (0..49).contains(&e.i(1)),
                    1 => (0..33).contains(&e.i(1)),
                    2 => (0..13).contains(&e.i(1)),
                    _ => (1..4).contains(&e.i(1)),
                };
                if !valid_kind {
                    return Err("unsupported PE entity kind");
                }
                if (g == 0 || g == 3) && !(0..9).contains(&e.i(3)) {
                    return Err("invalid PE column");
                }
                let floats: &[usize] = match g {
                    0 => &[18, 19, 20],
                    1 => &[3, 4, 12, 19, 21, 28, 29, 30],
                    2 => &[3, 4, 5, 6, 7, 8, 9, 15],
                    _ => &[],
                };
                if floats.iter().any(|&i| !e.f(i).is_finite() || e.f(i).abs() > 100000.0) {
                    return Err("invalid PE floating point value");
                }
                if g == 0 && (!(0..=50).contains(&e.i(7)) || !(-1..=48).contains(&e.i(8))) {
                    return Err("invalid PE plant state");
                }
                if g == 1 && !(0..=95).contains(&e.i(10)) {
                    return Err("invalid PE zombie phase");
                }
                if g < 2 {
                    let at = if g == 0 { 21 } else { 31 };
                    if !(0.0..=1.0).contains(&e.f(at - 3)) || !(-1.0..=1.0).contains(&e.f(at - 2)) {
                        return Err("invalid PE animation progress");
                    }
                    if e.u(at) > 10000 || e.u(at + 1) > 10000 || e.u(at + 2) > 5 {
                        return Err("invalid PE animation span");
                    }
                }
            }
        }
        Ok(value)
    }
}

pub struct Clip {
    pub bytes: Vec<u8>,
    pub frames: Vec<Range<usize>>,
    pub seed: u32,
}
impl Clip {
    pub fn decode(bytes: Vec<u8>) -> Result<Self, &'static str> {
        if bytes.len() < 16 || bytes.len() > MAX_BYTES || &bytes[..8] != MAGIC {
            return Err("invalid PE clip header or size");
        }
        let seed = word(&bytes, 2);
        let count = word(&bytes, 3) as usize;
        if count == 0 || count > 8001 {
            return Err("prototype supports at most 8001 frames");
        }
        let mut frames = Vec::with_capacity(count);
        let mut offset = 16;
        let mut last = None;
        for _ in 0..count {
            if offset + 4 > bytes.len() {
                return Err("truncated PE clip");
            }
            let end = offset
                .checked_add(word(&bytes[offset..], 0) as usize)
                .ok_or("PE length overflow")?;
            if end > bytes.len() {
                return Err("truncated PE frame");
            }
            let frame = Frame::parse(&bytes[offset..end])?;
            let clock = frame.header().i(1);
            if last.is_some_and(|previous| clock != previous + 1) {
                return Err("PE clip clocks are not consecutive");
            }
            last = Some(clock);
            frames.push(offset..end);
            offset = end;
        }
        if offset != bytes.len() {
            return Err("unexpected PE trailing bytes");
        }
        Ok(Self { bytes, frames, seed })
    }
    pub fn frame(&self, index: usize) -> Frame<'_> {
        Frame {
            bytes: &self.bytes[self.frames[index].clone()],
        }
    }
}

/// Playback-owned correspondence, not native addresses or cached entity facts.
pub struct Objects {
    pub ids: [[(u32, u32); 1024]; 4],
}
impl Default for Objects {
    fn default() -> Self {
        Self {
            ids: [[(0, 0); 1024]; 4],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_digger_direction_comes_from_phase() {
        let mut bytes = vec![0u8; 36 * 4];
        bytes[10 * 4..11 * 4].copy_from_slice(&37u32.to_le_bytes());
        bytes[13 * 4..14 * 4].copy_from_slice(&600u32.to_le_bytes());
        assert!(Entity(&bytes).digger_faces_right());
        bytes[10 * 4..11 * 4].copy_from_slice(&38u32.to_le_bytes());
        assert!(!Entity(&bytes).digger_faces_right());
        bytes[10 * 4..11 * 4].copy_from_slice(&33u32.to_le_bytes());
        assert!(Entity(&bytes).digger_faces_right());
    }
    #[test]
    fn capacities_and_clock_gaps_are_rejected() {
        let mut frame = vec![0; HEADER_WORDS * 4];
        frame[..4].copy_from_slice(&((HEADER_WORDS * 4) as u32).to_le_bytes());
        frame[24..28].copy_from_slice(&3u32.to_le_bytes());
        frame[28..32].copy_from_slice(&1025u32.to_le_bytes());
        assert!(Frame::parse(&frame).is_err());
        frame[28..32].fill(0);
        let mut b = Vec::from(*MAGIC);
        put(&mut b, 1u32);
        put(&mut b, 2u32);
        b.extend_from_slice(&frame);
        frame[4..8].copy_from_slice(&2u32.to_le_bytes());
        b.extend_from_slice(&frame);
        assert!(Clip::decode(b).is_err());
    }
    #[test]
    fn malformed_clip_is_rejected() {
        assert!(Clip::decode(vec![]).is_err());
        let mut b = Vec::from(*MAGIC);
        put(&mut b, 260004u32);
        put(&mut b, 1u32);
        let mut frame = vec![0; HEADER_WORDS * 4];
        frame[..4].copy_from_slice(&((HEADER_WORDS * 4) as u32).to_le_bytes());
        frame[24..28].copy_from_slice(&3u32.to_le_bytes());
        b.extend(frame);
        assert!(Clip::decode(b.clone()).is_ok());
        b.push(0);
        assert!(Clip::decode(b).is_err());
    }
}
