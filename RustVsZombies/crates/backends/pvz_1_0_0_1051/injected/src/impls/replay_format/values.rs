//! Raw scalar offsets follow the 1051 POD readers in Legacy1051Save.cpp.
use super::{Parsed, Result, word};

fn booleans(bytes: &[u8], offsets: impl IntoIterator<Item = usize>) -> Result<()> {
    for offset in offsets {
        if bytes.get(offset).is_none_or(|value| *value > 1) {
            return Err("invalid native boolean");
        }
    }
    Ok(())
}
fn floats(bytes: &[u8], offsets: impl IntoIterator<Item = usize>) -> Result<()> {
    for offset in offsets {
        if !f32::from_bits(word(bytes, offset)?).is_finite() {
            return Err("non-finite native scalar");
        }
    }
    Ok(())
}
fn range(bytes: &[u8], offset: usize, min: i32, max: i32) -> Result<()> {
    if !(min..=max).contains(&(word(bytes, offset)? as i32)) {
        return Err("native enum/count out of range");
    }
    Ok(())
}
const BOOLS: [&[usize]; 12] = [
    &[
        24, 80, 81, 112, 120, 136, 184, 185, 186, 187, 188, 189, 190, 191, 236, 260, 332,
    ],
    &[24, 320, 321, 322, 323, 324, 325],
    &[24, 80, 112],
    &[24, 56, 80, 200, 201, 202],
    &[48, 49],
    &[32, 72],
    &[28, 29, 36],
    &[56, 76],
    &[],
    &[20, 100, 124, 144],
    &[164, 212],
    &[772],
];
const FLOATS: [&[usize]; 12] = [
    &[44, 48, 52, 132, 284, 288, 292],
    &[192, 196],
    &[48, 52, 56, 60, 64, 68, 72, 76, 104, 108, 128],
    &[36, 40, 44, 48, 52, 64, 68, 100],
    &[8, 12, 56],
    &[36, 40, 44, 48],
    &[],
    &[24, 28, 32, 48, 52, 80],
    &[12, 16, 20, 24, 28, 32, 36, 44, 48],
    &[4, 8, 148],
    &[],
    &[],
];

impl Parsed<'_> {
    pub(super) fn validate_values(&self) -> Result<()> {
        booleans(
            self.board,
            [0, 1120, 21644, 21645, 21656, 21672, 21992]
                .into_iter()
                .chain(22012..22020)
                .chain(21360..21460)
                .chain(21572..21638),
        )?;
        floats(
            self.board,
            [1132, 22052]
                .into_iter()
                .chain((0..6).flat_map(|row| [1268 + 16 * row, 1272 + 16 * row, 1276 + 16 * row])),
        )?;
        range(self.board, 21480, 0, 9)?; // background
        range(self.board, 21504, 0, 100)?; // native wave-table capacity
        range(self.board, 21528, 0, 100)?;
        range(self.board, 21536, 0, 30)?;
        range(self.board, 21640, -1, 65)?;
        range(self.board, 21652, 0, 4)?;
        for wave in 0..word(self.board, 21504)? as usize {
            for slot in 0..50 {
                let value = word(self.board, 1360 + (wave * 50 + slot) * 4)? as i32;
                if value == -1 {
                    break;
                }
                if !(0..33).contains(&value) {
                    return Err("invalid active native spawn kind");
                }
            }
        }
        for offset in (4..220).step_by(4) {
            range(self.board, offset, 0, 4)?;
        }
        for offset in (1140..1164).step_by(4) {
            range(self.board, offset, 0, 3)?;
        }
        for (kind, pool) in self.pools.iter().enumerate() {
            for index in 0..pool.used {
                if !pool.is_active(index) {
                    continue;
                }
                let data = pool.slot(index)?;
                booleans(data, BOOLS[kind].iter().copied())?;
                floats(data, FLOATS[kind].iter().copied())?;
                match kind {
                    0 => {
                        range(data, 36, 0, 32)?;
                        range(data, 40, 0, 95)?;
                        range(data, 28, -1, 5)?;
                        range(data, 100, 0, 10)?;
                        range(data, 196, 0, 9)?;
                        range(data, 216, 0, 3)?;
                    }
                    1 => {
                        range(data, 36, 0, 52)?;
                        range(data, 40, 0, 8)?;
                        range(data, 28, 0, 5)?;
                        range(data, 312, -1, 48)?;
                        for offset in (200..300).step_by(20) {
                            range(data, offset + 16, 0, 21)?;
                            if word(data, offset + 16)? != 0 {
                                floats(data, (offset..offset + 16).step_by(4))?;
                            }
                        }
                    }
                    2 => {
                        range(data, 88, 0, 9)?;
                        range(data, 92, 0, 13)?;
                    }
                    3 => {
                        range(data, 88, 0, 27)?;
                        range(data, 92, 0, 6)?;
                        range(data, 104, -1, 74)?;
                    }
                    4 => {
                        range(data, 20, 0, 5)?;
                        range(data, 44, 0, 3)?;
                        range(data, 52, 0, 3)?;
                    }
                    5 => {
                        range(data, 8, 0, 12)?;
                        range(data, 228, 0, 12)?;
                        floats(data, (84..84 + word(data, 228)? as usize * 12).step_by(4))?;
                    }
                    7 => floats(data, (100..172).step_by(4))?,
                    8 => floats(data, (60..156).step_by(4))?,
                    9 => {
                        range(data, 0, 0, 142)?;
                        range(data, 16, 0, 5)?;
                        range(data, 24, 0, i32::MAX)?;
                        range(data, 28, 0, i32::MAX)?;
                        floats(data, (36..72).step_by(4))?;
                    }
                    10 => {
                        range(data, 160, 0, 20)?;
                        floats(
                            data,
                            (0..word(data, 160)? as usize * 8)
                                .step_by(4)
                                .chain((188..212).step_by(4)),
                        )?;
                    }
                    11 => {
                        range(data, 768, 0, 16)?;
                        for n in 0..word(data, 768)? as usize {
                            let offset = n * 48;
                            range(data, offset + 4, 0, 4)?;
                            floats(data, (offset + 8..offset + 44).step_by(4))?;
                            booleans(data, [offset + 44, offset + 45])?;
                        }
                    }
                    _ => {}
                }
            }
        }
        for tracks in &self.tracks {
            for track in tracks.chunks_exact(96) {
                booleans(track, 92..96)?;
                floats(track, (8..40).step_by(4).chain([52, 56, 60]))?;
                // Native SyncReanimation repairs text, image override and holder;
                // blend image/font pointers must be null, as native save asserts.
                if word(track, 40)? != 0 || word(track, 44)? != 0 {
                    return Err("unserialized blend image/font pointer");
                }
            }
        }
        booleans(self.cursor, [24])?;
        booleans(self.preview, [24])?;
        booleans(self.seeds, [24])?;
        range(self.cursor, 36, -1, 9)?;
        range(self.cursor, 40, -1, 74)?;
        range(self.cursor, 44, -1, 48)?;
        range(self.cursor, 48, 0, 17)?;
        range(self.seeds, 36, 0, 10)?;
        for index in 0..10 {
            let packet = &self.seeds[40 + index * 80..120 + index * 80];
            booleans(packet, [24, 72, 73])?;
            floats(packet, [68])?;
            range(packet, 52, -1, 74)?;
            // Original 1051 leaves this field unspecified for ordinary cards;
            // native cost/draw/use paths consult it only for SEED_IMITATER.
            if word(packet, 52)? == 48 {
                range(packet, 56, -1, 48)?;
            }
            range(packet, 64, -1, 74)?;
        }
        if !self.advice[4..132].contains(&0) || !self.advice[664..792].contains(&0) {
            return Err("unterminated native advice text");
        }
        booleans(self.challenge, 20..77)?;
        range(self.challenge, 104, -1, 74)?;
        range(self.challenge, 84, 0, 14)?;
        range(self.advice, 140, 0, 17)?;
        range(self.advice, 792, 0, 17)?;
        booleans(self.music, [64])?;
        floats(self.music, [28, 32])?;
        range(self.music, 8, -1, 13)?;
        for offset in [12, 16, 20] {
            range(self.music, offset, -1, 4)?;
        }
        self.validate_effect_graph()
    }

    // Attachments can point to attachments and to animations whose tracks own
    // further attachments. A cycle would recurse indefinitely in native draw/free.
    fn validate_effect_graph(&self) -> Result<()> {
        let attachments = self.pools[11];
        let reanimations = self.pools[9];
        let mut track_index = [usize::MAX; 1024];
        let mut rank = 0;
        for (index, target) in track_index[..reanimations.used].iter_mut().enumerate() {
            if reanimations.is_active(index) {
                *target = rank;
                rank += 1;
            }
        }
        let mut marks = [0u8; 2048];
        let mut pending = Vec::new();
        for root in 0..2048 {
            if marks[root] != 0 {
                continue;
            }
            pending.push((root, false));
            while let Some((node, exit)) = pending.pop() {
                if exit {
                    marks[node] = 2;
                    continue;
                }
                if marks[node] == 1 {
                    return Err("cyclic native attachment graph");
                }
                if marks[node] == 2 {
                    continue;
                }
                marks[node] = 1;
                pending.push((node, true));
                if node < 1024 {
                    if !attachments.is_active(node) {
                        continue;
                    }
                    let data = attachments.slot(node)?;
                    for n in 0..word(data, 768)? as usize {
                        let id = word(data, n * 48)?;
                        match word(data, n * 48 + 4)? {
                            2 if reanimations.contains(id) => pending.push((1024 + (id & 0xffff) as usize, false)),
                            3 if attachments.contains(id) => pending.push(((id & 0xffff) as usize, false)),
                            _ => {}
                        }
                    }
                } else {
                    let index = node - 1024;
                    if track_index[index] == usize::MAX {
                        continue;
                    }
                    for track in self.tracks[track_index[index]].chunks_exact(96) {
                        let id = word(track, 64)?;
                        if attachments.contains(id) {
                            pending.push(((id & 0xffff) as usize, false));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
