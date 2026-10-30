//! File-format preflight. This validates metadata only, never native checkpoint bytes.
use crate::runtime::{RuntimeError, RuntimeResult};
use serde::Deserialize;
use std::{collections::HashSet, fs::File, io::BufReader, path::Path};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    format: String,
    version: u32,
    #[serde(default)]
    pub(super) input: Option<Input>,
    pub backend: String,
    pub(super) interval_cs: u32,
    pub(super) records: Vec<Record>,
    pub(super) ticks: Vec<(i32, u32, u8, [i32; 4])>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    pub file: String,
    pub base_sequence: u64,
    pub terminal_clock: i32,
    #[serde(default)]
    pub world_ended: bool,
    #[serde(default)]
    pub terminal_snapshot: bool,
    pub terminal_sequence: u64,
    pub ticks: Vec<(i32, u64)>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    #[serde(default)]
    pub(super) input_sequence: u64,
    pub(super) file: String,
    pub(super) mode: i32,
    pub(super) clock: i32,
    pub(super) dancer: u32,
    pub(super) maid: u8,
    pub(super) cursor: [i32; 4],
}

impl Manifest {
    pub fn read(path: &Path) -> RuntimeResult<Self> {
        let file = File::open(path).map_err(super::io)?;
        let manifest: Self = serde_json::from_reader(BufReader::new(file))
            .map_err(|error| RuntimeError::new(format!("回放清单无效：{error}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> RuntimeResult<()> {
        if self.format != "rsvz-replay" || !matches!(self.version, 1 | 2) {
            return Err(RuntimeError::new("不兼容的回放格式或版本"));
        }
        if self.backend.is_empty() || self.interval_cs == 0 || self.interval_cs > i32::MAX as u32 {
            return Err(RuntimeError::new("回放兼容身份或记录精度无效"));
        }
        if self.records.is_empty() {
            return Err(RuntimeError::new("回放中没有记录"));
        }
        if let Some(input) = &self.input {
            if self.version != 2
                || input.file != "input.bin"
                || input.base_sequence > input.terminal_sequence
                || self
                    .records
                    .windows(2)
                    .any(|pair| pair[0].clock > pair[1].clock || pair[0].input_sequence > pair[1].input_sequence)
                || self.records.iter().any(|record| {
                    record.input_sequence < input.base_sequence
                        || record.input_sequence > input.terminal_sequence
                        || record.clock > input.terminal_clock
                })
                || input
                    .ticks
                    .windows(2)
                    .any(|pair| pair[0].0 >= pair[1].0 || pair[0].1 > pair[1].1)
                || input.ticks.iter().any(|tick| {
                    tick.1 < input.base_sequence || tick.1 > input.terminal_sequence || tick.0 > input.terminal_clock
                })
                || input.ticks.len() != self.ticks.len()
                || input.ticks.iter().zip(&self.ticks).any(|(a, b)| a.0 != b.0)
            {
                return Err(RuntimeError::new("invalid ordered input metadata"));
            }
        } else if self.records.iter().any(|record| record.input_sequence != 0) {
            return Err(RuntimeError::new("missing ordered input metadata"));
        }
        if let Some(input) = &self.input
            && input.terminal_snapshot
            && self.records.last().is_none_or(|record| {
                record.clock != input.terminal_clock || record.input_sequence != input.terminal_sequence
            })
        {
            return Err(RuntimeError::new("terminal snapshot does not match the input boundary"));
        }
        let mut names = HashSet::with_capacity(self.records.len());
        for record in &self.records {
            let serial = record.file.strip_suffix(".dat");
            if !serial.is_some_and(|serial| {
                (12..=20).contains(&serial.len())
                    && serial.bytes().all(|byte| byte.is_ascii_digit())
                    && serial.parse::<u64>().is_ok()
            }) || !names.insert(record.file.as_str())
            {
                return Err(RuntimeError::new("回放记录文件名无效或重复"));
            }
            if record.maid > 3 || record.mode < 0 {
                return Err(RuntimeError::new("回放记录状态无效"));
            }
            // Typed deserialization also checks each scalar and the exact cursor tuple.
            // Coordinates may be outside the lawn; do not turn that into a game-mode policy.
            let _ = (record.clock, record.dancer, record.cursor);
        }
        if self.ticks.iter().any(|tick| tick.2 > 3) || self.ticks.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err(RuntimeError::new("回放逐帧数据无效或重复"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid() -> serde_json::Value {
        serde_json::json!({"format":"rsvz-replay","version":1,"backend":"test-backend",
            "interval_cs":10,"records":[{"file":"000000000000.dat","mode":42,
            "clock":-1,"dancer":0,"maid":0,"cursor":[-20,700,0,0]}],
            "ticks":[[-1,0,0,[-20,700,0,0]],[0,1,3,[10,20,1,2]]]})
    }
    fn parse(value: serde_json::Value) -> RuntimeResult<Manifest> {
        let manifest: Manifest = serde_json::from_value(value).map_err(|e| RuntimeError::new(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }
    #[test]
    fn malformed_metadata_is_rejected_before_any_native_load() {
        assert!(parse(valid()).is_ok());
        for filename in [
            "../escape.dat",
            "C:/escape.dat",
            "000000000000.dat:stream",
            "18446744073709551616.dat",
        ] {
            let mut value = valid();
            value["records"][0]["file"] = filename.into();
            assert!(parse(value).is_err());
        }
        let mut value = valid();
        value["records"]
            .as_array_mut()
            .unwrap()
            .push(valid()["records"][0].clone());
        assert!(parse(value).is_err());
        for (field, bad) in [("version", 3), ("interval_cs", 0), ("interval_cs", u32::MAX)] {
            let mut value = valid();
            value[field] = bad.into();
            assert!(parse(value).is_err());
        }
        let mut value = valid();
        value["ticks"][1][0] = (-1).into();
        assert!(parse(value).is_err());
        let mut value = valid();
        value["records"][0]["maid"] = 4.into();
        assert!(parse(value).is_err());
        let mut value = valid();
        value["records"][0]["cursor"] = serde_json::json!([0, 0]);
        assert!(parse(value).is_err());
        let mut value = valid();
        value["unexpected"] = true.into();
        assert!(parse(value).is_err());
    }
}
