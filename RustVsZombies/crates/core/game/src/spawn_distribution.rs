//! Pure limited spawn-table generation. Ordinary `set_zombies` defaults are
//! unchanged. Callers supply actual huge-wave facts and choose their defaults.
use rsvz_model::{SPAWN_SLOTS_PER_WAVE, SpawnList, ZombieKind};

/// PT reference weights after its normal/conehead adjustments; huge waves use
/// Giga=6000. The tool chooses its ordinary-wave defaults separately.
pub const PT_WEIGHTS: [u32; 33] = [
    400, 0, 1000, 2000, 3000, 1000, 3500, 2000, 1000, 0, 0, 2000, 2000, 2000, 1500, 1000, 2000, 1000, 1000, 1, 1000,
    1000, 1500, 1500, 0, 0, 4000, 3000, 1000, 2000, 2000, 2000, 6000,
];

pub enum Distribution<'a> {
    /// Ascending native IDs, advancing the cycle before each slot as PT does.
    Uniform,
    Weighted {
        ordinary: &'a [u32; 33],
        huge: &'a [u32; 33],
        giga_allowed: &'a [bool],
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DistributionError {
    #[error("spawn schedule needs at least one wave")]
    EmptyWaves,
    #[error("Giga mask length differs from the actual wave count")]
    GigaMaskLength,
    #[error("no eligible filler type has positive weight for wave {wave}")]
    NoEligibleType { wave: usize },
    #[error("native zombie type is outside 0..=32")]
    InvalidType,
}

fn filler(kind: usize) -> bool {
    !matches!(kind, 1 | 19 | 20)
}

/// Flag/Yeti/Bungee restrictions are part of this explicitly named algorithm:
/// flag slot 0, Bungee slots 9..12 on actual huge waves, and one global Yeti that
/// cannot replace those slots. Weighted mode also reserves huge slots 1..8 for
/// normals. There is no "at least one Giga per allowed wave" redraw guarantee.
pub fn limited_schedule(
    types: &[ZombieKind], huge_waves: &[bool], distribution: Distribution<'_>, seed: u32,
) -> Result<SpawnList, DistributionError> {
    if huge_waves.is_empty() {
        return Err(DistributionError::EmptyWaves);
    }
    let mut selected = [false; 33];
    for kind in types {
        let index = usize::try_from(kind.code()).map_err(|_| DistributionError::InvalidType)?;
        if index >= 33 {
            return Err(DistributionError::InvalidType);
        }
        selected[index] = true;
    }
    if let Distribution::Weighted { giga_allowed, .. } = &distribution {
        if giga_allowed.len() != huge_waves.len() {
            return Err(DistributionError::GigaMaskLength);
        }
    }
    // Validate every wave before allocating or emitting any result.
    for (wave, &huge_wave) in huge_waves.iter().enumerate() {
        let eligible = (0..33).any(|kind| {
            selected[kind]
                && filler(kind)
                && match &distribution {
                    Distribution::Uniform => true,
                    Distribution::Weighted {
                        ordinary,
                        huge,
                        giga_allowed,
                    } => {
                        (kind != 32 || giga_allowed[wave])
                            && if huge_wave { huge[kind] > 0 } else { ordinary[kind] > 0 }
                    }
                }
        });
        if !eligible {
            return Err(DistributionError::NoEligibleType { wave: wave + 1 });
        }
    }
    let weighted = matches!(distribution, Distribution::Weighted { .. });
    let reserved = |wave: usize, slot: usize| {
        huge_waves[wave] && ((slot == 0 && (selected[1] || weighted)) || ((9..=12).contains(&slot) && selected[20]))
    };
    let mut rng = Mt19937::new(seed);
    // PT's Yeti choice is independent of table filling. Select its rank among
    // unreserved slots directly instead of retrying forever on impossible input.
    let yeti_slot = if selected[19] {
        let count = huge_waves
            .iter()
            .enumerate()
            .map(|(wave, _)| (0..SPAWN_SLOTS_PER_WAVE).filter(|&slot| !reserved(wave, slot)).count())
            .sum::<usize>();
        let mut rank = rng.below(count as u64) as usize;
        let mut chosen = None;
        'outer: for wave in 0..huge_waves.len() {
            for slot in 0..SPAWN_SLOTS_PER_WAVE {
                if !reserved(wave, slot) {
                    if rank == 0 {
                        chosen = Some((wave, slot));
                        break 'outer;
                    }
                    rank -= 1;
                }
            }
        }
        chosen
    } else {
        None
    };
    let mut cyclic = 0usize;
    let rows = huge_waves.iter().enumerate().map(|(wave, &huge_wave)| {
        let mut cumulative = [0u64; 33];
        let mut total = 0;
        if let Distribution::Weighted {
            ordinary,
            huge,
            giga_allowed,
        } = &distribution
        {
            let weights = if huge_wave { *huge } else { *ordinary };
            for kind in 0..33 {
                if selected[kind] && filler(kind) && (kind != 32 || giga_allowed[wave]) {
                    total += u64::from(weights[kind]);
                }
                cumulative[kind] = total;
            }
        }
        let mut row = [ZombieKind::Normal; SPAWN_SLOTS_PER_WAVE];
        for slot in &mut row {
            let kind = if weighted {
                let draw = rng.below(total);
                cumulative.partition_point(|&limit| limit <= draw)
            } else {
                loop {
                    cyclic = (cyclic + 1) % 33;
                    if selected[cyclic] && filler(cyclic) {
                        break cyclic;
                    }
                }
            };
            *slot = ZombieKind::try_from_code(kind as i32).expect("validated native kind");
        }
        if huge_wave {
            if selected[1] || weighted {
                row[0] = ZombieKind::Flag;
            }
            if weighted {
                row[1..9].fill(ZombieKind::Normal);
            }
            if selected[20] {
                row[9..13].fill(ZombieKind::Bungee);
            }
        }
        if let Some((yeti_wave, slot)) = yeti_slot {
            if wave == yeti_wave {
                row[slot] = ZombieKind::Yeti;
            }
        }
        row
    });
    let mut result = SpawnList::from_waves(rows);
    // Preserve selected zero-weight or special types in the allowed-type table,
    // and retain the intrinsic flag/normal types of weighted huge waves.
    let extra = weighted && huge_waves.iter().any(|huge| *huge);
    result.set_allowed_types(
        (0..33)
            .filter(|&kind| selected[kind] || (extra && kind <= 1))
            .map(|kind| ZombieKind::try_from_code(kind as i32).expect("native kind")),
    );
    Ok(result)
}

/// Standard MT19937 state; local to one generation, never the game's RNG.
pub(crate) struct Mt19937 {
    state: [u32; 624],
    index: usize,
}
impl Mt19937 {
    pub(crate) fn new(seed: u32) -> Self {
        let mut state = [0; 624];
        state[0] = seed;
        for index in 1..624 {
            state[index] = 1812433253u32
                .wrapping_mul(state[index - 1] ^ (state[index - 1] >> 30))
                .wrapping_add(index as u32);
        }
        Self { state, index: 624 }
    }
    fn next(&mut self) -> u32 {
        if self.index == 624 {
            for index in 0..624 {
                let y = (self.state[index] & 0x80000000) | (self.state[(index + 1) % 624] & 0x7fffffff);
                self.state[index] =
                    self.state[(index + 397) % 624] ^ (y >> 1) ^ if y & 1 != 0 { 0x9908b0df } else { 0 };
            }
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c5680;
        y ^= (y << 15) & 0xefc60000;
        y ^= y >> 18;
        y
    }
    pub(crate) fn below(&mut self, upper: u64) -> u64 {
        debug_assert!(upper > 0);
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let value = (u64::from(self.next()) << 32) | u64::from(self.next());
            if value >= threshold {
                return value % upper;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mt_matches_standard_reference_sequence() {
        let mut rng = Mt19937::new(5489);
        assert_eq!(
            (0..5).map(|_| rng.next()).collect::<Vec<_>>(),
            [3499211612, 581869302, 3890346734, 3586334585, 545404204]
        );
    }
    #[test]
    fn uniform_matches_copied_pt_reference_golden() {
        // Generated by the PT C++ branch, source SHA256
        // 6e0b6cf90b51ea0f955db4ba4b6322f055625f5581f0a492b98a182d1d945398.
        let flags: Vec<_> = (1..=20).map(|wave| wave % 10 == 0).collect();
        let types = [
            ZombieKind::Normal,
            ZombieKind::Flag,
            ZombieKind::Conehead,
            ZombieKind::Bobsled,
            ZombieKind::Bungee,
            ZombieKind::GigaGargantuar,
        ];
        let table = limited_schedule(&types, &flags, Distribution::Uniform, 1).unwrap();
        let expected: Vec<i32> = include_str!("spawn_distribution_tests/pt_uniform_20.csv")
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(expected.len(), 1000);
        assert_eq!(
            table.waves().flatten().map(|kind| kind.code()).collect::<Vec<_>>(),
            expected
        );
    }
    #[test]
    fn actual_wave_count_and_huge_wave_facts_are_not_a_repeated_twenty_wave_template() {
        for count in [1, 5, 10, 20, 25, 100] {
            let flags: Vec<_> = (0..count).map(|wave| wave == count - 1).collect();
            let table = limited_schedule(
                &[
                    ZombieKind::Normal,
                    ZombieKind::Flag,
                    ZombieKind::Bobsled,
                    ZombieKind::Bungee,
                ],
                &flags,
                Distribution::Uniform,
                42,
            )
            .unwrap();
            assert_eq!(table.wave_count(), count);
            for wave in 0..count {
                let row = table.wave(wave).unwrap();
                assert_eq!(row.len(), 50);
                assert!(row.contains(&ZombieKind::Bobsled));
                if flags[wave] {
                    assert_eq!(row[0], ZombieKind::Flag);
                    assert_eq!(&row[9..13], &[ZombieKind::Bungee; 4]);
                } else {
                    assert!(!row.contains(&ZombieKind::Flag));
                    assert!(!row.contains(&ZombieKind::Bungee));
                }
            }
        }
    }
    #[test]
    fn low_weight_allowed_giga_is_not_forced_and_weight_tables_are_independent() {
        let types = [ZombieKind::Normal, ZombieKind::Conehead, ZombieKind::GigaGargantuar];
        let flags = vec![false; 100];
        let allowed = vec![true; 100];
        let mut ordinary = [0; 33];
        ordinary[0] = 1;
        ordinary[2] = 3;
        let table = limited_schedule(
            &types,
            &flags,
            Distribution::Weighted {
                ordinary: &ordinary,
                huge: &PT_WEIGHTS,
                giga_allowed: &allowed,
            },
            5489,
        )
        .unwrap();
        let normals = table
            .waves()
            .flatten()
            .filter(|&&kind| kind == ZombieKind::Normal)
            .count();
        assert!((1100..=1400).contains(&normals));
        assert!(!table.waves().flatten().any(|&kind| kind == ZombieKind::GigaGargantuar));
        ordinary[0] = 999999999;
        ordinary[2] = 0;
        ordinary[32] = 1;
        let table = limited_schedule(
            &types,
            &[false],
            Distribution::Weighted {
                ordinary: &ordinary,
                huge: &PT_WEIGHTS,
                giga_allowed: &[true],
            },
            1,
        )
        .unwrap();
        assert!(table.wave(0).unwrap().iter().all(|&kind| kind == ZombieKind::Normal));
    }
    #[test]
    fn weighted_rejects_impossible_waves_and_keeps_special_slots() {
        let types = [
            ZombieKind::Normal,
            ZombieKind::Flag,
            ZombieKind::Yeti,
            ZombieKind::Bungee,
            ZombieKind::GigaGargantuar,
        ];
        let flags = [false, true, false];
        let mask = [false, true, false];
        let mut weights = [0; 33];
        weights[32] = 999999999;
        assert_eq!(
            limited_schedule(
                &types,
                &flags,
                Distribution::Weighted {
                    ordinary: &weights,
                    huge: &PT_WEIGHTS,
                    giga_allowed: &mask
                },
                1
            )
            .err(),
            Some(DistributionError::NoEligibleType { wave: 1 })
        );
        weights[0] = 1;
        let table = limited_schedule(
            &types,
            &flags,
            Distribution::Weighted {
                ordinary: &weights,
                huge: &PT_WEIGHTS,
                giga_allowed: &mask,
            },
            1,
        )
        .unwrap();
        assert_eq!(
            table
                .waves()
                .flatten()
                .filter(|&&kind| kind == ZombieKind::Yeti)
                .count(),
            1
        );
        assert_eq!(table.wave(1).unwrap()[0], ZombieKind::Flag);
        assert!(
            table.wave(1).unwrap()[9..13]
                .iter()
                .all(|kind| *kind == ZombieKind::Bungee)
        );
        assert!(
            table
                .wave(0)
                .unwrap()
                .iter()
                .chain(table.wave(2).unwrap())
                .all(|kind| matches!(kind, ZombieKind::Normal | ZombieKind::Yeti))
        );
    }
}
