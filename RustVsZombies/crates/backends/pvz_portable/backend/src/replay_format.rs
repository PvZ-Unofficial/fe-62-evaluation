//! SAVE4 v2/v3 preflight over bytes; this never constructs native objects or edits a Board.
mod schema;
type Result<T> = std::result::Result<T, &'static str>;
enum Field {
    I(&'static str),
    U(&'static str),
    F(&'static str),
    B(&'static str),
    Q(&'static str),
    Text(&'static str, usize),
    Image(&'static str),
    Repeat(&'static [Field], usize),
}
#[derive(Clone, Copy, Default)]
struct Deferred {
    effects: u32,
    effect_count: u32,
    motion: u32,
    motion_count: u32,
    magnet: u32,
    magnet_active: u32,
    trail: u32,
    trail_count: u32,
    trail_scalar: usize,
}
fn array_index(name: &str, prefix: &str) -> Option<u32> {
    name.strip_prefix(prefix)?.split_once(']')?.0.parse().ok()
}
impl Deferred {
    fn float(&mut self, name: &str, value: u32) -> Result<()> {
        let invalid = !f32::from_bits(value).is_finite();
        let field = if let Some(index) = array_index(name, "mEffectArray[") {
            Some((&mut self.effects, index))
        } else if let Some(index) = array_index(name, "mMotionTrailFrames[") {
            Some((&mut self.motion, index))
        } else if let Some(index) = array_index(name, "mMagnetItems[") {
            Some((&mut self.magnet, index))
        } else if name == "point" {
            let index = (self.trail_scalar / 2) as u32;
            self.trail_scalar += 1;
            Some((&mut self.trail, index))
        } else {
            None
        };
        if let Some((mask, index)) = field {
            if index >= 32 {
                return Err("invalid SAVE4 fixed-array index");
            }
            if invalid {
                *mask |= 1 << index;
            }
            return Ok(());
        }
        if invalid {
            Err("non-finite SAVE4 scalar")
        } else {
            Ok(())
        }
    }
    fn integer(&mut self, name: &str, value: i32) -> Result<()> {
        match name {
            "mNumEffects" => self.effect_count = value as u32,
            "mMotionTrailCount" => self.motion_count = value as u32,
            "mNumTrailPoints" => {
                if !(0..=20).contains(&value) {
                    return Err("SAVE4 trail point count out of range");
                }
                self.trail_count = value as u32;
            }
            _ => {}
        }
        if name.ends_with(".mItemType")
            && let Some(index) = array_index(name, "mMagnetItems[")
        {
            if !(0..=21).contains(&value) || index >= 5 {
                return Err("invalid SAVE4 magnet item");
            }
            if value != 0 {
                self.magnet_active |= 1 << index;
            }
        }
        Ok(())
    }
    fn finish(&self) -> Result<()> {
        if self.effects & ((1u32 << self.effect_count) - 1) != 0
            || self.motion & ((1u32 << self.motion_count) - 1) != 0
            || self.trail & ((1u32 << self.trail_count) - 1) != 0
            || self.magnet & self.magnet_active != 0
        {
            Err("non-finite active SAVE4 array element")
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy)]
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    deferred: Deferred,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            at: 0,
            deferred: Deferred::default(),
        }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(count).ok_or("SAVE4 size overflow")?;
        let bytes = self.bytes.get(self.at..end).ok_or("truncated SAVE4 data")?;
        self.at = end;
        Ok(bytes)
    }
    fn word(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn blob(&mut self) -> Result<&'a [u8]> {
        let size = self.word()? as usize;
        self.take(size)
    }
    fn finish(&self) -> Result<()> {
        if self.at == self.bytes.len() {
            self.deferred.finish()
        } else {
            Err("unexpected SAVE4 trailing data")
        }
    }
    fn fields(&mut self, fields: &[Field], kind: u32, visit: &mut impl FnMut(&str, u32)) -> Result<()> {
        for field in fields {
            match field {
                Field::I(name) => {
                    let value = self.word()?;
                    integer(name, value as i32, kind)?;
                    self.deferred.integer(name, value as i32)?;
                    visit(name, value);
                }
                Field::U(name) => {
                    let value = self.word()?;
                    visit(name, value);
                }
                Field::F(name) => {
                    let value = self.word()?;
                    self.deferred.float(name, value)?;
                    visit(name, value);
                }
                Field::B(name) => {
                    let value = self.take(1)?[0];
                    if value > 1 {
                        return Err("invalid SAVE4 boolean");
                    }
                    visit(name, u32::from(value));
                }
                Field::Q(name) => {
                    let _ = name;
                    self.take(8)?;
                }
                Field::Text(name, size) => {
                    let _ = name;
                    if !self.take(*size)?.contains(&0) {
                        return Err("unterminated SAVE4 text");
                    }
                }
                Field::Image(name) => {
                    let value = self.word()?;
                    image(value)?;
                    visit(name, value);
                }
                Field::Repeat(fields, count) => {
                    for _ in 0..*count {
                        self.fields(fields, kind, visit)?;
                    }
                }
            }
        }
        Ok(())
    }
}
fn image(value: u32) -> Result<()> {
    if matches!(value,0..=7|30..=364|529..=613|616..=626) {
        Ok(())
    } else {
        Err("SAVE4 image is not an image resource")
    }
}
fn integer(name: &str, value: i32, kind: u32) -> Result<()> {
    let bounds = match name {
        "mBackground" => (0, 9),
        "mNumWaves" | "mCurrentWave" => (0, 100),
        "mZombieType" => {
            if kind == 2 {
                (0, 32)
            } else {
                (-1, 32)
            }
        }
        "mZombiePhase" => (0, 95),
        "mGridSquareType" => (0, 4),
        "mPlantRow" => (0, 3),

        "mNumPackets" => (0, 10),
        "mNumEffects" => (0, 16),
        "mMotionTrailCount" => (0, 12),
        "mPlantCol" => (0, 8),
        "mCursorType" => (0, 17),
        "mSeedBankIndex" => (-1, 9),
        "mSeedType" => {
            if kind == 3 {
                (0, 52)
            } else {
                (-1, 74)
            }
        }
        "mPacketType" | "mSlotMachiningNextSeed" | "mUsableSeedType" | "mLastConveyorSeedType" => (-1, 74),
        "mImitaterType" => (-1, 48),
        "mType" => {
            if kind == 5 {
                (0, 27)
            } else {
                (-1, 74)
            }
        }
        "mCoinMotion" => (0, 6),
        "mMotionType" => (0, 9),
        "mProjectileType" => (0, 13),
        "mGridItemType" => (0, 12),
        "mMowerState" | "mMowerType" => (0, 3),
        "mZombieHeight" => (0, 10),
        "mHelmType" => (0, 9),
        "mShieldType" => (0, 3),
        "mChallengeState" => (0, 14),
        "mScaryPotType" => (0, 3),
        "mMessageStyle" | "mMessageStyleNext" => (0, 17),
        "mHelpIndex" => (-1, 65),
        "mTutorialState" => (0, 30),
        "mDebugTextMode" => (0, 4),
        "mCurMusicTune" => (-1, 13),
        "mCurMusicFileMain" | "mCurMusicFileDrums" | "mCurMusicFileHihats" => (-1, 4),
        _ => return Ok(()),
    };
    if (bounds.0..=bounds.1).contains(&value) {
        Ok(())
    } else {
        #[cfg(test)]
        eprintln!("invalid SAVE4 scalar {name}={value}, chunk {kind}");
        Err("SAVE4 enum or count out of range")
    }
}
fn fields(bytes: &[u8], description: &[Field], kind: u32, visit: &mut impl FnMut(&str, u32)) -> Result<()> {
    let mut reader = Reader::new(bytes);
    reader.fields(description, kind, visit)?;
    reader.finish()
}
fn tlvs<'a>(bytes: &'a [u8], mut visit: impl FnMut(u32, &'a [u8]) -> Result<()>) -> Result<()> {
    let mut reader = Reader::new(bytes);
    while reader.at < bytes.len() {
        let id = reader.word()?;
        let data = reader.blob()?;
        visit(id, data)?;
    }
    reader.finish()
}
fn object(bytes: &[u8], base: bool, tail: &[Field], kind: u32, visit: &mut impl FnMut(&str, u32)) -> Result<()> {
    let mut seen = 0;
    tlvs(bytes, |id, data| {
        let (flag, description) = match id {
            1 if base => (1, schema::BASE),
            100 => (2, tail),
            _ => return Err("unexpected SAVE4 object field"),
        };
        if seen & flag != 0 {
            return Err("duplicate SAVE4 object field");
        }
        seen |= flag;
        fields(data, description, kind, visit)
    })?;
    if seen == if base { 3 } else { 2 } {
        Ok(())
    } else {
        Err("missing SAVE4 object field")
    }
}
#[derive(Default)]
struct Pool<'a> {
    ids: Vec<u32>,
    items: Vec<&'a [u8]>,
    active: usize,
}
impl<'a> Pool<'a> {
    fn contains(&self, id: u32) -> bool {
        id >> 16 != 0 && self.ids.get((id & 0xffff) as usize) == Some(&id)
    }
    fn read(bytes: &'a [u8], capacity: usize, ids_only: bool) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let free = r.word()? as usize;
        let used = r.word()? as usize;
        let active = r.word()? as usize;
        let next = r.word()?;
        let stored_capacity = r.word()? as usize;
        if stored_capacity != capacity || used > capacity || active > used || free > used || !(1..65536).contains(&next)
        {
            return Err("invalid SAVE4 pool header");
        }
        let mut out = Self {
            ids: Vec::with_capacity(used),
            items: Vec::with_capacity(used),
            active,
        };
        let mut count = 0;
        for index in 0..used {
            let id = r.word()?;
            let alive = id >> 16 != 0;
            if alive {
                if (id & 0xffff) as usize != index {
                    return Err("SAVE4 pool identity mismatch");
                }
                count += 1;
            } else if id as usize > used {
                return Err("SAVE4 free link outside pool");
            }
            let item = if ids_only { &[][..] } else { r.blob()? };
            if !ids_only && (alive == item.is_empty()) {
                return Err("SAVE4 active slot/payload mismatch");
            }
            out.ids.push(id);
            out.items.push(item);
        }
        r.finish()?;
        if count != active {
            return Err("SAVE4 active pool count mismatch");
        }
        let mut visited = [false; 1024];
        let mut next = free;
        while next < used {
            if visited[next] || out.ids[next] >> 16 != 0 {
                return Err("cyclic SAVE4 free list");
            }
            visited[next] = true;
            next = out.ids[next] as usize;
        }
        for (index, seen) in visited[..used].iter().enumerate() {
            if (out.ids[index] >> 16 != 0) == *seen {
                return Err("incomplete SAVE4 free list");
            }
        }
        Ok(out)
    }
}
fn one_field(bytes: &[u8]) -> Result<&[u8]> {
    let mut r = Reader::new(bytes);
    if r.word()? != 1 {
        return Err("missing SAVE4 effect field");
    }
    let result = r.blob()?;
    r.finish()?;
    Ok(result)
}
fn ids(r: &mut Reader<'_>, pool: &Pool<'_>, seen: &mut [bool; 1024]) -> Result<Vec<u32>> {
    let count = r.word()? as usize;
    if count > pool.active {
        return Err("SAVE4 ID list exceeds pool");
    }
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        let id = r.word()?;
        if !pool.contains(id) {
            return Err("SAVE4 missing effect reference");
        }
        let index = (id & 0xffff) as usize;
        if seen[index] {
            return Err("SAVE4 effect has multiple owners");
        }
        seen[index] = true;
        result.push(id);
    }
    Ok(result)
}

/// Definition queries return the matching game's actual counts; no game object
/// construction or load is permitted from these callbacks.
pub(super) fn validate(
    bytes: &[u8], mut emitter_count: impl FnMut(i32) -> Result<usize>,
    mut animation: impl FnMut(i32) -> Result<(usize, usize)>,
) -> Result<i32> {
    let mut header = Reader::new(bytes);
    if header.take(12)? != b"PVZP_SAVE4\0\0" {
        return Err("incompatible Portable native save header");
    }
    let version = header.word()?;
    if !matches!(version, 2 | 3) {
        return Err("incompatible SAVE4 version");
    }
    let length = header.word()? as usize;
    let checksum = header.word()?;
    let payload = header.take(length)?;
    header.finish()?;
    if crc32fast::hash(payload) != checksum {
        return Err("SAVE4 payload checksum mismatch");
    }
    let mut chunks = [None; 22];
    tlvs(payload, |id, data| {
        if !(1..=if version == 3 { 21 } else { 20 }).contains(&id) || chunks[id as usize].is_some() {
            return Err("unexpected or repeated SAVE4 chunk");
        }
        let mut chunk = Reader::new(data);
        if chunk.word()? != 1 {
            return Err("incompatible SAVE4 chunk version");
        }
        chunks[id as usize] = Some(one_field(chunk.take(data.len() - 4)?)?);
        Ok(())
    })?;
    if chunks[1..=if version == 3 { 21 } else { 20 }]
        .iter()
        .any(Option::is_none)
    {
        return Err("incomplete SAVE4 checkpoint");
    }
    let chunk = |id: usize| chunks[id].expect("checked complete chunk set");
    let mut clock = 0;
    let mut wave_count = 0usize;
    let mut wave_table = None;
    let mut board = Reader::new(chunk(1));
    let board_fields = board.blob()?;
    board.finish()?;
    let mut board_seen = [false; 105];
    tlvs(board_fields, |id, data| {
        let entry = schema::BOARD
            .iter()
            .find(|(key, _)| *key == id)
            .ok_or("unknown SAVE4 Board field")?;
        if board_seen[id as usize] {
            return Err("duplicate SAVE4 Board field");
        }
        board_seen[id as usize] = true;
        if id == 18 {
            wave_table = Some(data);
        }
        fields(data, entry.1, 1, &mut |name, value| {
            if name == "mMainCounter" {
                clock = value as i32;
            }
            if name == "mNumWaves" {
                wave_count = value as usize;
            }
        })
    })?;
    if schema::BOARD.iter().any(|(id, _)| !board_seen[*id as usize]) {
        return Err("incomplete SAVE4 Board fields");
    }
    // The writer includes all 100 physical waves, even when only N are initialized.
    // Slots after the first -1 in a wave are likewise not native spawn input.
    let table = wave_table.ok_or("missing SAVE4 wave table")?;
    for wave in table[..wave_count * 200].chunks_exact(200) {
        for entry in wave.chunks_exact(4) {
            let kind = i32::from_le_bytes(entry.try_into().unwrap());
            if kind == -1 {
                break;
            }
            if !(0..33).contains(&kind) {
                return Err("invalid active SAVE4 spawn kind");
            }
        }
    }
    // Native pool order mirrors the effect-reference topology, not chunk order.
    let kinds = [2usize, 3, 4, 5, 6, 7, 10, 8, 9, 11, 12, 13];
    let capacities = [1024, 1024, 1024, 1024, 32, 128, 1024, 1024, 1024, 1024, 1024, 1024];
    let mut pools: Vec<Pool<'_>> = Vec::with_capacity(12);
    for (index, id) in kinds.into_iter().enumerate() {
        pools.push(Pool::read(chunk(id), capacities[index], matches!(id, 8 | 9))?);
    }
    let tails = [
        schema::ZOMBIE,
        schema::PLANT,
        schema::PROJECTILE,
        schema::COIN,
        schema::MOWER,
        schema::GRID,
    ];
    for (index, tail) in tails.into_iter().enumerate() {
        for (&id, item) in pools[index].ids.iter().zip(&pools[index].items) {
            if id >> 16 != 0 {
                object(item, index < 4, tail, kinds[index] as u32, &mut |_, _| {})?;
            }
        }
    }
    let mut emitters = [false; 1024];
    let mut particles = [false; 1024];
    for (&id, item) in pools[6].ids.iter().zip(&pools[6].items) {
        if id >> 16 == 0 {
            continue;
        }
        let mut r = Reader::new(one_field(item)?);
        let definition = r.word()? as i32;
        let count = if definition == -1 {
            0
        } else {
            emitter_count(definition)?
        };
        let emitter_ids = ids(&mut r, &pools[7], &mut emitters)?;
        for _ in emitter_ids {
            if r.word()? as usize >= count {
                return Err("SAVE4 emitter definition out of range");
            }
            let particle_ids = ids(&mut r, &pools[8], &mut particles)?;
            // center, color, image, then the fixed native emitter scalars.
            r.fields(
                &[
                    Field::F("x"),
                    Field::F("y"),
                    Field::Repeat(&[Field::I("color")], 4),
                    Field::Image("image"),
                    Field::F("spawn"),
                    Field::Repeat(&[Field::I("counter")], 3),
                    Field::F("time"),
                    Field::F("last"),
                    Field::B("dead"),
                    Field::B("additive"),
                    Field::F("scale"),
                    Field::Repeat(&[Field::I("value")], 3),
                    Field::Repeat(&[Field::F("interpolation")], 18),
                ],
                10,
                &mut |_, _| {},
            )?;
            for _ in particle_ids {
                r.fields(schema::PARTICLE, 10, &mut |_, _| {})?;
            }
        }
        r.fields(
            &[
                Field::I("effect"),
                Field::B("dead"),
                Field::B("attachment"),
                Field::I("order"),
                Field::B("no_update"),
            ],
            10,
            &mut |_, _| {},
        )?;
        r.finish()?;
    }
    for (pool, seen) in [(&pools[7], &emitters), (&pools[8], &particles)] {
        for (index, id) in pool.ids.iter().enumerate() {
            if (*id >> 16 != 0) != seen[index] {
                return Err("unowned SAVE4 effect object");
            }
        }
    }
    let mut graph = vec![Vec::<usize>::new(); 2048];
    for (index, (&id, item)) in pools[9].ids.iter().zip(&pools[9].items).enumerate() {
        if id >> 16 == 0 {
            continue;
        }
        let mut r = Reader::new(one_field(item)?);
        let definition = r.word()? as i32;
        let (tracks, frames) = animation(definition)?;
        let mut kind = -1;
        let mut start = 0;
        let mut length = 0;
        let mut pose = -1;
        r.fields(schema::REANIMATION, 11, &mut |name, value| match name {
            "mReanimationType" => kind = value as i32,
            "mFrameStart" => start = value as i32,
            "mFrameCount" => length = value as i32,
            "mFrameBasePose" => pose = value as i32,
            _ => {}
        })?;
        if kind != definition
            || start < 0
            || length < 0
            || pose < -2
            || (tracks == 0 && (start != 0 || length != 0))
            || (tracks > 0
                && (start as usize >= frames
                    || (start as usize)
                        .checked_add(length as usize)
                        .is_none_or(|end| end > frames)
                    || pose >= 0 && pose as usize >= frames))
        {
            return Err("SAVE4 animation frame range exceeds definition");
        }
        for _ in 0..tracks {
            r.fields(schema::TRACK, 11, &mut |name, id| {
                if name == "mAttachmentID" && pools[11].contains(id) {
                    graph[1024 + index].push((id & 0xffff) as usize)
                }
            })?;
        }
        r.finish()?;
    }
    for (&id, item) in pools[10].ids.iter().zip(&pools[10].items) {
        if id >> 16 == 0 {
            continue;
        }
        let mut r = Reader::new(one_field(item)?);
        if r.word()? != 0 {
            return Err("SAVE4 trail definition out of range");
        }
        r.fields(
            &[
                Field::Repeat(&[Field::F("point")], 40),
                Field::I("mNumTrailPoints"),
                Field::B("dead"),
                Field::Repeat(&[Field::I("value")], 3),
                Field::Repeat(&[Field::F("interpolation")], 6),
                Field::B("attachment"),
                Field::Repeat(&[Field::I("color")], 4),
            ],
            12,
            &mut |_, _| {},
        )?;
        r.finish()?;
    }
    for (index, (&id, item)) in pools[11].ids.iter().zip(&pools[11].items).enumerate() {
        if id >> 16 == 0 {
            continue;
        }
        let mut count = 0usize;
        let mut effects = [(0u32, 0u32); 16];
        object(item, false, schema::ATTACHMENT, 13, &mut |name, value| {
            if name == "mNumEffects" {
                count = value as usize;
            }
            if let Some(tail) = name.strip_prefix("mEffectArray[") {
                if let Some((slot, field)) = tail.split_once(']') {
                    if let Ok(slot) = slot.parse::<usize>() {
                        if slot < 16 {
                            if field == ".mEffectID" {
                                effects[slot].0 = value;
                            }
                            if field == ".mEffectType" {
                                effects[slot].1 = value;
                            }
                        }
                    }
                }
            }
        })?;
        for (id, kind) in &effects[..count] {
            match kind {
                2 if pools[9].contains(*id) => graph[index].push(1024 + (*id & 0xffff) as usize),
                3 if pools[11].contains(*id) => graph[index].push((*id & 0xffff) as usize),
                0..=4 => {}
                _ => return Err("invalid SAVE4 attachment kind"),
            }
        }
    }
    acyclic(&graph)?;
    let mut packet_count = 0;
    let mut mouse_claim = [u32::MAX, 0, 0, 0]; // seed index, cursor kind, dropped card, cannon
    for (id, base, tail) in [
        (14, true, schema::CURSOR),
        (15, true, schema::PREVIEW),
        (16, false, schema::ADVICE),
        (17, true, schema::SEED_BANK),
        (19, false, schema::CHALLENGE),
        (20, false, schema::MUSIC),
    ] {
        let mut r = Reader::new(chunk(id));
        object(r.blob()?, base, tail, id as u32, &mut |name, value| {
            if id == 17 && name == "mNumPackets" {
                packet_count = value;
            }
            if id == 14 {
                match name {
                    "mSeedBankIndex" => mouse_claim[0] = value,
                    "mCursorType" => mouse_claim[1] = value,
                    "mCoinID" => mouse_claim[2] = value,
                    "mCobCannonPlantID" => mouse_claim[3] = value,
                    _ => {}
                }
            }
        })?;
        r.finish()?;
    }
    let mut seeds = Reader::new(chunk(18));
    if seeds.word()? != 10 {
        return Err("SAVE4 seed bank capacity mismatch");
    }
    for _ in 0..10 {
        object(seeds.blob()?, true, schema::SEED, 18, &mut |_, _| {})?;
    }
    seeds.finish()?;
    if version == 3 {
        validate_input(chunk(21), packet_count, mouse_claim)?;
    }
    Ok(clock)
}
fn validate_input(bytes: &[u8], packet_count: u32, mouse_claim: [u32; 4]) -> Result<()> {
    let mut reader = Reader::new(bytes);
    reader.take(8)?; // event watermark
    let mut held_ids = [None; 11];
    let mut seed_indices = [-1; 11];
    let mut coin_ids = [0; 11];
    let mut cob_ids = [0; 11];
    let boolean = |reader: &mut Reader<'_>| -> Result<bool> {
        match reader.take(1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("noncanonical input boolean"),
        }
    };
    for index in 0..11 {
        let device = u64::from_le_bytes(reader.take(8)?.try_into().unwrap());
        let finger = u64::from_le_bytes(reader.take(8)?.try_into().unwrap());
        let held = boolean(&mut reader)?;
        boolean(&mut reader)?;
        for _ in 0..2 {
            if !(-1048576..=1048576).contains(&(reader.word()? as i32)) {
                return Err("invalid touch coordinate");
            }
        }
        let target = reader.word()?;
        if target > 21 {
            return Err("invalid touch down target");
        }
        if held {
            if held_ids[..index].contains(&Some((device, finger))) {
                return Err("duplicate touch identity");
            }
            held_ids[index] = Some((device, finger));
        }
        let mut cursor = [0; 10];
        for word in &mut cursor {
            *word = reader.word()?;
        }
        let seed = cursor[0] as i32;
        if seed >= packet_count as i32
            || (seed >= 0 && mouse_claim[1] == 1 && cursor[0] == mouse_claim[0])
            || (cursor[4] != 0 && mouse_claim[1] == 2 && cursor[4] == mouse_claim[2])
            || (cursor[7] != 0 && mouse_claim[1] == 8 && cursor[7] == mouse_claim[3])
        {
            return Err("touch selection conflicts with saved seed bank or mouse");
        }
        if !(-1..10).contains(&seed)
            || !matches!(cursor[3], 0 | 1 | 2 | 6 | 8)
            || !(-1..=52).contains(&(cursor[1] as i32))
            || !(-1..=52).contains(&(cursor[2] as i32))
            || cursor[5] != 0
            || cursor[6] != 0
            || cursor[9] != 0
            || (cursor[3] != 1 && seed != -1)
            || (cursor[3] != 2 && cursor[4] != 0)
            || (cursor[3] != 8 && cursor[7] != 0)
        {
            return Err("invalid touch cursor state");
        }
        match cursor[3] {
            1 if seed < 0 || cursor[1] == u32::MAX => return Err("invalid touch seed selection"),
            2 if cursor[4] >> 16 == 0 || cursor[4] & 0xffff >= 1024 || cursor[1] == u32::MAX => {
                return Err("invalid touch coin selection");
            }
            8 if cursor[7] >> 16 == 0 || cursor[7] & 0xffff >= 1024 => return Err("invalid touch cannon ID"),
            _ => {}
        }
        if (seed >= 0 && seed_indices[..index].contains(&seed))
            || (cursor[4] != 0 && coin_ids[..index].contains(&cursor[4]))
            || (cursor[7] != 0 && cob_ids[..index].contains(&cursor[7]))
        {
            return Err("duplicate touch claim");
        }
        seed_indices[index] = seed;
        coin_ids[index] = cursor[4];
        cob_ids[index] = cursor[7];
    }
    if reader.word()? > 30 {
        return Err("invalid touch cannon protection");
    }
    for _ in 0..4 {
        if !(-1048576..=1048576).contains(&(reader.word()? as i32)) {
            return Err("invalid pointer coordinate");
        }
    }
    let down = reader.word()?;
    let actual = reader.word()?;
    let button = reader.word()? as i32;
    boolean(&mut reader)?;
    boolean(&mut reader)?;
    let touch_mouse = boolean(&mut reader)?;
    let captured = boolean(&mut reader)?;
    let device = u64::from_le_bytes(reader.take(8)?.try_into().unwrap());
    let finger = u64::from_le_bytes(reader.take(8)?.try_into().unwrap());
    if down & !7 != 0
        || actual & !7 != 0
        || !(-1..=2).contains(&button)
        || (captured && down == 0)
        || (touch_mouse && (!captured || down != 1 || held_ids.contains(&Some((device, finger)))))
    {
        return Err("invalid pointer ownership");
    }
    reader.finish()
}

pub(super) fn input_sequence(bytes: &[u8]) -> Result<u64> {
    let mut header = Reader::new(bytes);
    header.take(12)?;
    let version = header.word()?;
    let length = header.word()? as usize;
    header.word()?;
    let payload = header.take(length)?;
    let mut sequence = 0;
    tlvs(payload, |id, data| {
        if id == 21 {
            let mut chunk = Reader::new(data);
            chunk.word()?;
            let value = one_field(chunk.take(data.len() - 4)?)?;
            sequence = u64::from_le_bytes(value.get(..8).ok_or("truncated input watermark")?.try_into().unwrap());
        }
        Ok(())
    })?;
    if version == 2 && sequence != 0 {
        return Err("legacy checkpoint has input watermark");
    }
    Ok(sequence)
}

fn acyclic(graph: &[Vec<usize>]) -> Result<()> {
    let mut state = vec![0u8; graph.len()];
    let mut stack = Vec::new();
    for root in 0..graph.len() {
        if state[root] != 0 {
            continue;
        }
        stack.push((root, false));
        while let Some((node, exit)) = stack.pop() {
            if exit {
                state[node] = 2;
                continue;
            }
            if state[node] == 1 {
                return Err("cyclic SAVE4 effect graph");
            }
            if state[node] == 2 {
                continue;
            }
            state[node] = 1;
            stack.push((node, true));
            for &child in &graph[node] {
                stack.push((child, false));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn word(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    fn blob(out: &mut Vec<u8>, value: &[u8]) {
        word(out, value.len() as u32);
        out.extend_from_slice(value);
    }
    fn tlv(out: &mut Vec<u8>, key: u32, value: &[u8]) {
        word(out, key);
        blob(out, value);
    }
    fn zeros(description: &[Field]) -> Vec<u8> {
        let mut out = Vec::new();
        for field in description {
            match field {
                Field::I(_) | Field::U(_) | Field::F(_) => word(&mut out, 0),
                Field::Image(_) => word(&mut out, 626),
                Field::B(_) => out.push(0),
                Field::Q(_) => out.extend_from_slice(&[0; 8]),
                Field::Text(_, count) => out.resize(out.len() + count, 0),
                Field::Repeat(fields, count) => {
                    for _ in 0..*count {
                        out.extend_from_slice(&zeros(fields));
                    }
                }
            }
        }
        out
    }
    fn obj(base: bool, tail: &[Field]) -> Vec<u8> {
        let mut out = Vec::new();
        if base {
            tlv(&mut out, 1, &zeros(schema::BASE));
        }
        tlv(&mut out, 100, &zeros(tail));
        out
    }
    fn wrap(payload: &[u8]) -> Vec<u8> {
        let mut out = b"PVZP_SAVE4\0\0".to_vec();
        word(&mut out, 2);
        word(&mut out, payload.len() as u32);
        word(&mut out, crc32fast::hash(payload));
        out.extend_from_slice(payload);
        out
    }
    fn empty() -> Vec<u8> {
        let mut payload = Vec::new();
        for id in 1..=20 {
            let mut field = Vec::new();
            match id {
                1 => {
                    let mut board = Vec::new();
                    for (key, fields) in schema::BOARD {
                        tlv(&mut board, *key, &zeros(fields));
                    }
                    blob(&mut field, &board);
                }
                2..=13 => {
                    let capacity = match id {
                        6 => 32,
                        7 => 128,
                        _ => 1024,
                    };
                    for n in [0, 0, 0, 1001, capacity] {
                        word(&mut field, n);
                    }
                }
                18 => {
                    word(&mut field, 10);
                    for _ in 0..10 {
                        blob(&mut field, &obj(true, schema::SEED));
                    }
                }
                _ => {
                    let (base, tail) = match id {
                        14 => (true, schema::CURSOR),
                        15 => (true, schema::PREVIEW),
                        16 => (false, schema::ADVICE),
                        17 => (true, schema::SEED_BANK),
                        19 => (false, schema::CHALLENGE),
                        20 => (false, schema::MUSIC),
                        _ => unreachable!(),
                    };
                    blob(&mut field, &obj(base, tail));
                }
            }
            let mut chunk = Vec::new();
            word(&mut chunk, 1);
            tlv(&mut chunk, 1, &field);
            tlv(&mut payload, id, &chunk);
        }
        wrap(&payload)
    }
    #[test]
    fn complete_current_schema_parses_without_native_access() {
        let bytes = empty();
        assert_eq!(
            validate(&bytes, |_| panic!("no particles"), |_| panic!("no animations")),
            Ok(0)
        );
        for end in [0, 12, 23, bytes.len() - 1] {
            assert!(validate(&bytes[..end], |_| Ok(0), |_| Ok((0, 0))).is_err());
        }
        let mut corrupt = bytes.clone();
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        assert_eq!(
            validate(&corrupt, |_| Ok(0), |_| Ok((0, 0))),
            Err("SAVE4 payload checksum mismatch")
        );
        let mut payload = bytes[24..].to_vec();
        let mut first = Reader::new(&payload);
        let _id = first.word().unwrap();
        let _data = first.blob().unwrap();
        let end = first.at;
        let duplicate = payload[..end].to_vec();
        payload.extend_from_slice(&duplicate);
        assert!(validate(&wrap(&payload), |_| Ok(0), |_| Ok((0, 0))).is_err());
    }
    #[test]
    fn touch_input_rejects_native_invalid_states_with_valid_crc() {
        let mut input = vec![0; 830];
        for slot in 0..11 {
            let cursor = 8 + slot * 70 + 30;
            input[cursor..cursor + 12].fill(0xff); // no bank index, seed or imitater
        }
        let fixture = |input: &[u8]| {
            let old = empty();
            let mut payload = Vec::new();
            tlvs(&old[24..], |id, data| {
                if id == 17 {
                    let mut tail = zeros(schema::SEED_BANK);
                    tail[..4].copy_from_slice(&10u32.to_le_bytes());
                    let mut object = Vec::new();
                    tlv(&mut object, 1, &zeros(schema::BASE));
                    tlv(&mut object, 100, &tail);
                    let mut field = Vec::new();
                    blob(&mut field, &object);
                    let mut chunk = Vec::new();
                    word(&mut chunk, 1);
                    tlv(&mut chunk, 1, &field);
                    tlv(&mut payload, id, &chunk);
                } else {
                    tlv(&mut payload, id, data);
                }
                Ok(())
            })
            .unwrap();
            let mut chunk = Vec::new();
            word(&mut chunk, 1);
            tlv(&mut chunk, 1, input);
            tlv(&mut payload, 21, &chunk);
            let mut bytes = wrap(&payload);
            bytes[12..16].copy_from_slice(&3u32.to_le_bytes());
            bytes
        };
        let check = |input: &[u8]| validate(&fixture(input), |_| Ok(0), |_| Ok((0, 0)));
        assert_eq!(check(&input), Ok(0));
        for (offset, value, error) in [
            (38, 0, "invalid touch cursor state"),
            (54, 0x10000, "invalid touch cursor state"),
            (66, 0x10000, "invalid touch cursor state"),
            (34, 22, "invalid touch down target"),
        ] {
            let mut invalid = input.clone();
            invalid[offset..offset + 4].copy_from_slice(&u32::to_le_bytes(value));
            assert_eq!(check(&invalid), Err(error));
        }
        input[50..54].copy_from_slice(&2u32.to_le_bytes());
        input[54..58].copy_from_slice(&0x10000u32.to_le_bytes());
        assert_eq!(check(&input), Err("invalid touch coin selection"));
        input[42..46].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(check(&input), Ok(0));
    }
    #[test]
    #[ignore = "requires an existing disposable native SAVE4 fixture; resource bounds are tested in-game separately"]
    fn previously_captured_native_save_matches_wire_schema() {
        let path = std::env::var("RSVZ_REPLAY_FIXTURE").expect("set disposable fixture path");
        let bytes = std::fs::read(path).unwrap();
        let mut reanimations = None;
        tlvs(&bytes[24..], |id, data| {
            if id == 11 {
                let mut r = Reader::new(data);
                assert_eq!(r.word()?, 1);
                reanimations = Some(Pool::read(one_field(r.take(data.len() - 4)?)?, 1024, false)?);
            }
            Ok(())
        })
        .unwrap();
        let mut tracks = std::collections::BTreeMap::new();
        let fixed = zeros(schema::REANIMATION).len();
        let track_size = zeros(schema::TRACK).len();
        let pool = reanimations.unwrap();
        for (&id, item) in pool.ids.iter().zip(&pool.items) {
            if id >> 16 == 0 {
                continue;
            }
            let data = one_field(item).unwrap();
            let kind = u32::from_le_bytes(data[..4].try_into().unwrap()) as i32;
            assert!(data.len() >= 4 + fixed);
            assert_eq!((data.len() - 4 - fixed) % track_size, 0);
            let count = (data.len() - 4 - fixed) / track_size;
            if let Some(previous) = tracks.insert(kind, count) {
                assert_eq!(previous, count);
            }
        }
        // This verifies framing/scalars against actual writer output only. It is
        // deliberately not evidence for native resource sizes or safe restoration.
        validate(
            &bytes,
            |_| Ok(usize::MAX),
            |kind| Ok((*tracks.get(&kind).unwrap(), usize::MAX)),
        )
        .unwrap();
    }

    #[test]
    fn capacities_and_recursive_effects_are_rejected() {
        let mut bytes = Vec::new();
        for word_value in [0, 1025, 0, 1001, 1024] {
            word(&mut bytes, word_value);
        }
        assert!(Pool::read(&bytes, 1024, false).is_err());
        assert!(acyclic(&[vec![1], vec![0]]).is_err());
        assert!(acyclic(&[vec![1, 2], vec![2], vec![]]).is_ok());
        assert!(image(9).is_err());
        assert!(image(626).is_ok());
    }
}
