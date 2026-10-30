//! Bounded parser for the native 1051 SyncBoard format, without touching a Board.
//! Layout: original 4819D0..482B30; pool/list rules also cross-checked against
//! Portable's Legacy1051Save.cpp. Parsing is not yet a native-load certificate:
//! POD fields and effect graphs are checked in values; definition/frame bounds
//! come from the current native resources. The caller seals the validated copy.
mod values;
type Result<T> = std::result::Result<T, &'static str>;

const MAGIC: u32 = 0xfeeddead;
const BUILD_DATE: u32 = 0xb7eff6a8;
const POOLS: [(usize, usize, usize); 12] = [
    (1024, 348, 344),
    (1024, 332, 328),
    (1024, 148, 144),
    (1024, 216, 208),
    (32, 72, 68),
    (128, 236, 232),
    (1024, 44, 40),
    (1024, 176, 172),
    (1024, 160, 156),
    (1024, 160, 156),
    (1024, 236, 232),
    (1024, 780, 776),
];

fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset.checked_add(4).ok_or("checkpoint offset overflow")?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or("truncated checkpoint field")?
            .try_into()
            .unwrap(),
    ))
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(length).ok_or("checkpoint length overflow")?;
        let value = self.bytes.get(self.at..end).ok_or("truncated checkpoint")?;
        self.at = end;
        Ok(value)
    }
    fn word(&mut self) -> Result<u32> {
        word(self.take(4)?, 0)
    }
    fn signed(&mut self) -> Result<i32> {
        self.word().map(|value| value as i32)
    }
    fn block(&mut self, size: usize) -> Result<&'a [u8]> {
        if self.word()? as usize != size {
            return Err("native block size mismatch");
        }
        self.take(size)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Pool<'a> {
    pub bytes: &'a [u8],
    pub stride: usize,
    id_offset: usize,
    pub used: usize,
    active: usize,
}
impl<'a> Pool<'a> {
    const EMPTY: Self = Self {
        bytes: &[],
        stride: 0,
        id_offset: 0,
        used: 0,
        active: 0,
    };
    pub fn slot(&self, index: usize) -> Result<&'a [u8]> {
        if index >= self.used {
            return Err("pool index outside used slots");
        }
        Ok(&self.bytes[index * self.stride..(index + 1) * self.stride])
    }
    fn id(&self, index: usize) -> Result<u32> {
        word(self.slot(index)?, self.id_offset)
    }
    pub fn is_active(&self, index: usize) -> bool {
        self.id(index).is_ok_and(|id| id >> 16 != 0)
    }
    fn contains(&self, id: u32) -> bool {
        id >> 16 != 0 && self.id((id & 0xffff) as usize) == Ok(id)
    }
    fn read(reader: &mut Reader<'a>, capacity: usize, stride: usize, id_offset: usize) -> Result<Self> {
        let free = reader.word()? as usize;
        let used = reader.word()? as usize;
        let active = reader.word()? as usize;
        if used > capacity || active > used || free > used {
            return Err("invalid native pool counts");
        }
        let bytes = reader.block(used.checked_mul(stride).ok_or("pool size overflow")?)?;
        let pool = Self {
            bytes,
            stride,
            id_offset,
            used,
            active,
        };
        let mut count = 0;
        for index in 0..used {
            let id = pool.id(index)?;
            if id >> 16 != 0 {
                if (id & 0xffff) as usize != index {
                    return Err("pool identity/index mismatch");
                }
                count += 1;
            } else if id as usize > used {
                return Err("invalid pool free link");
            }
        }
        if count != active {
            return Err("pool active count mismatch");
        }
        let mut seen = [false; 1024];
        let mut index = free;
        while index < used {
            if pool.is_active(index) || seen[index] {
                return Err("cyclic or occupied pool free list");
            }
            seen[index] = true;
            index = pool.id(index)? as usize;
        }
        for (index, free) in seen[..used].iter().enumerate() {
            if pool.is_active(index) == *free {
                return Err("pool free list misses a slot");
            }
        }
        Ok(pool)
    }
}

fn id_list(reader: &mut Reader<'_>, pool: Pool<'_>, owners: &mut [bool; 1024]) -> Result<Vec<u32>> {
    let count = reader.word()? as usize;
    if count > pool.active {
        return Err("native reference list exceeds pool");
    }
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        let id = word(reader.block(4)?, 0)?;
        if !pool.contains(id) {
            return Err("native list references a missing object");
        }
        let index = (id & 0xffff) as usize;
        if owners[index] {
            return Err("native object has multiple list owners");
        }
        owners[index] = true;
        ids.push(id);
    }
    Ok(ids)
}
fn image(reader: &mut Reader<'_>) -> Result<()> {
    // ResourceId includes fonts and sounds: a mere <= RESOURCE_ID_MAX check is unsafe.
    if !matches!(reader.word()?, 0..=7 | 30..=364 | 529..=613 | 616..=626) {
        return Err("invalid native image identifier");
    }
    Ok(())
}

pub(super) struct Parsed<'a> {
    pub board: &'a [u8],
    pub pools: [Pool<'a>; 12],
    pub tracks: Vec<&'a [u8]>,
    pub cursor: &'a [u8],
    pub preview: &'a [u8],
    pub advice: &'a [u8],
    pub seeds: &'a [u8],
    pub challenge: &'a [u8],
    pub music: &'a [u8],
}

/// Definition counts come from the matching game resources, not from the file.
/// These callbacks may load known native resources; they must not edit a Board.
pub(super) fn parse<'a>(
    bytes: &'a [u8], mut emitter_count: impl FnMut(i32) -> Result<usize>,
    mut track_count: impl FnMut(i32, &[u8]) -> Result<usize>, trail_count: usize,
) -> Result<Parsed<'a>> {
    let mut reader = Reader { bytes, at: 0 };
    let header = reader.block(12)?;
    if word(header, 0)? != MAGIC || word(header, 4)? != 2 || word(header, 8)? != BUILD_DATE {
        return Err("incompatible 1051 native checkpoint header");
    }
    let board = reader.block(0x564c)?;
    let mut pools = [Pool::EMPTY; 12];
    for (pool, (capacity, stride, id_offset)) in pools.iter_mut().zip(POOLS) {
        *pool = Pool::read(&mut reader, capacity, stride, id_offset)?;
    }
    let mut emitters = [false; 1024];
    let mut particles = [false; 1024];
    for index in 0..pools[6].used {
        if !pools[6].is_active(index) {
            continue;
        }
        let definition = reader.signed()?;
        let definitions = if definition == -1 {
            0
        } else {
            emitter_count(definition)?
        };
        let ids = id_list(&mut reader, pools[7], &mut emitters)?;
        for _ in ids {
            let emitter = reader.word()? as usize;
            if emitter >= definitions {
                return Err("particle emitter definition out of range");
            }
            image(&mut reader)?;
            id_list(&mut reader, pools[8], &mut particles)?;
        }
    }
    for (pool, owners) in [(pools[7], &emitters), (pools[8], &particles)] {
        for (index, owned) in owners[..pool.used].iter().enumerate() {
            if pool.is_active(index) != *owned {
                return Err("unowned native effect object");
            }
        }
    }
    let mut tracks: Vec<&[u8]> = Vec::with_capacity(pools[9].active);
    for index in 0..pools[9].used {
        if !pools[9].is_active(index) {
            continue;
        }
        let count = track_count(reader.signed()?, pools[9].slot(index)?)?;
        if count == 0 {
            if word(pools[9].slot(index)?, 88)? != 0 {
                return Err("track pointer without a native track allocation");
            }
            tracks.push(&[]);
            continue;
        }
        tracks.push(reader.block(count.checked_mul(96).ok_or("track size overflow")?)?);
        for _ in 0..count {
            image(&mut reader)?;
        }
    }
    for index in 0..pools[10].used {
        if pools[10].is_active(index) && reader.word()? as usize >= trail_count {
            return Err("trail definition out of range");
        }
    }
    let parsed = Parsed {
        board,
        pools,
        tracks,
        cursor: reader.block(76)?,
        preview: reader.block(44)?,
        advice: reader.block(796)?,
        seeds: reader.block(848)?,
        challenge: reader.block(188)?,
        music: reader.block(76)?,
    };
    if reader.word()? != MAGIC || reader.at != bytes.len() {
        return Err("invalid checkpoint terminator or trailing bytes");
    }
    parsed.validate_values()?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn push_word(bytes: &mut Vec<u8>, word: u32) {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    fn block(bytes: &mut Vec<u8>, value: &[u8]) {
        push_word(bytes, value.len() as u32);
        bytes.extend_from_slice(value);
    }
    fn empty() -> Vec<u8> {
        let mut out = Vec::new();
        let mut header = Vec::new();
        for word in [MAGIC, 2, BUILD_DATE] {
            push_word(&mut header, word);
        }
        block(&mut out, &header);
        block(&mut out, &vec![0; 0x564c]);
        for _ in POOLS {
            for word in [0, 0, 0, 0] {
                push_word(&mut out, word);
            }
        }
        for size in [76, 44, 796, 848, 188, 76] {
            block(&mut out, &vec![0; size]);
        }
        push_word(&mut out, MAGIC);
        out
    }
    #[test]
    fn exact_framing_and_pool_bounds_are_required() {
        let bytes = empty();
        assert!(parse(&bytes, |_| Err("unexpected"), |_, _| Err("unexpected"), 1).is_ok());
        for end in [0, 3, 16, bytes.len() - 1] {
            assert!(parse(&bytes[..end], |_| Ok(0), |_, _| Ok(0), 1).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(parse(&trailing, |_| Ok(0), |_, _| Ok(0), 1).is_err());
        let mut oversized = bytes;
        let used = 16 + 4 + 0x564c + 4;
        oversized[used..used + 4].copy_from_slice(&1025u32.to_le_bytes());
        assert!(parse(&oversized, |_| Ok(0), |_, _| Ok(0), 1).is_err());
    }
    #[test]
    #[ignore = "uses an existing disposable 1051 save; actual resource counts need in-game verification"]
    fn previously_captured_native_save_matches_wire_schema() {
        let path = std::env::var("RSVZ_REPLAY_FIXTURE").expect("set disposable fixture path");
        let bytes = std::fs::read(path).unwrap();
        let mut r = Reader { bytes: &bytes, at: 0 };
        r.block(12).unwrap();
        r.block(0x564c).unwrap();
        let mut pools = [Pool::EMPTY; 12];
        for (pool, (capacity, stride, offset)) in pools.iter_mut().zip(POOLS) {
            *pool = Pool::read(&mut r, capacity, stride, offset).unwrap();
        }
        let mut emitters = [false; 1024];
        let mut particles = [false; 1024];
        for index in 0..pools[6].used {
            if !pools[6].is_active(index) {
                continue;
            }
            r.signed().unwrap();
            for _ in id_list(&mut r, pools[7], &mut emitters).unwrap() {
                r.word().unwrap();
                r.word().unwrap();
                id_list(&mut r, pools[8], &mut particles).unwrap();
            }
        }
        let mut definitions = std::collections::BTreeMap::new();
        for index in 0..pools[9].used {
            if !pools[9].is_active(index) {
                continue;
            }
            let kind = r.signed().unwrap();
            let length = word(r.bytes, r.at).unwrap() as usize;
            assert!(
                length >= 96 && length % 96 == 0,
                "fixture must contain loaded, nonempty animation definitions"
            );
            let count = length / 96;
            r.block(length).unwrap();
            r.take(count * 4).unwrap();
            if let Some(previous) = definitions.insert(kind, count) {
                assert_eq!(previous, count);
            }
        }
        parse(
            &bytes,
            |_| Ok(usize::MAX),
            |kind, _| Ok(*definitions.get(&kind).unwrap()),
            1,
        )
        .unwrap();
        eprintln!(
            "verified native fixture framing/scalars: {} animation definitions, {} active plants",
            definitions.len(),
            pools[1].active
        );
    }

    #[test]
    fn unused_imitater_field_is_not_a_seed_type() {
        let bytes = empty();
        let mut parsed = parse(&bytes, |_| Ok(0), |_, _| Ok(0), 1).unwrap();
        let mut seeds = vec![0; 848];
        seeds[96..100].copy_from_slice(&306133440u32.to_le_bytes());
        parsed.seeds = &seeds;
        assert!(parsed.validate_values().is_ok());
        let mut imitater = seeds.clone();
        imitater[92..96].copy_from_slice(&48u32.to_le_bytes());
        parsed.seeds = &imitater;
        assert_eq!(parsed.validate_values(), Err("native enum/count out of range"));
    }

    #[test]
    fn image_references_cannot_select_fonts_or_sounds() {
        for value in [8u32, 9, 29, 365, 528, 614, 615, 627, u32::MAX] {
            let bytes = value.to_le_bytes();
            assert!(image(&mut Reader { bytes: &bytes, at: 0 }).is_err());
        }
        for value in [0u32, 7, 30, 364, 529, 613, 616, 625, 626] {
            let bytes = value.to_le_bytes();
            assert!(image(&mut Reader { bytes: &bytes, at: 0 }).is_ok());
        }
    }
    #[test]
    fn scalar_and_attachment_checks_do_not_dereference_file_pointers() {
        let bytes = empty();
        let mut parsed = parse(&bytes, |_| Ok(0), |_, _| Ok(0), 1).unwrap();
        let mut attachment = vec![0; 780];
        attachment[0..4].copy_from_slice(&0x10000u32.to_le_bytes());
        attachment[4..8].copy_from_slice(&3u32.to_le_bytes());
        attachment[768..772].copy_from_slice(&1u32.to_le_bytes());
        attachment[776..780].copy_from_slice(&0x10000u32.to_le_bytes());
        parsed.pools[11] = Pool {
            bytes: &attachment,
            stride: 780,
            id_offset: 776,
            used: 1,
            active: 1,
        };
        assert_eq!(parsed.validate_values(), Err("cyclic native attachment graph"));
        let mut plant = vec![0; 332];
        plant[192..196].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        plant[328..332].copy_from_slice(&0x10000u32.to_le_bytes());
        parsed.pools[11] = Pool::EMPTY;
        parsed.pools[1] = Pool {
            bytes: &plant,
            stride: 332,
            id_offset: 328,
            used: 1,
            active: 1,
        };
        assert_eq!(parsed.validate_values(), Err("non-finite native scalar"));
    }

    #[test]
    fn free_list_cannot_cycle_or_contain_active_slots() {
        let mut bytes = Vec::new();
        for word in [0, 1, 0, 4, 0] {
            push_word(&mut bytes, word);
        }
        assert!(Pool::read(&mut Reader { bytes: &bytes, at: 0 }, 1, 4, 0).is_err());
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&0x10000u32.to_le_bytes());
        assert!(Pool::read(&mut Reader { bytes: &bytes, at: 0 }, 1, 4, 0).is_err());
        bytes[..4].copy_from_slice(&1u32.to_le_bytes());
        assert!(Pool::read(&mut Reader { bytes: &bytes, at: 0 }, 1, 4, 0).is_ok());
    }
}
