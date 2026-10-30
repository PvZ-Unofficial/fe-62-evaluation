use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

use rsvz_backend_api::artifact::SessionArtifact;
use serde::Serialize;

pub(super) fn write_control<T: Serialize>(path: &Path, run_id: &str, value: &T) -> Result<(), String> {
    write_atomic(path, run_id, |writer| {
        serde_json::to_writer(&mut *writer, value).map_err(|error| error.to_string())?;
        writer.write_all(b"\n").map_err(|error| error.to_string())
    })
}

pub(super) fn write_artifact(path: &Path, run_id: &str, artifact: &SessionArtifact) -> Result<(), String> {
    write_atomic(path, run_id, |writer| {
        artifact.write_json(writer).map_err(|error| error.to_string())
    })
}

fn write_atomic(
    path: &Path, run_id: &str, write: impl FnOnce(&mut dyn Write) -> Result<(), String>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("output path has no UTF-8 file name")?;
    let temporary = parent.join(format!(".{name}.{run_id}.tmp"));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let result = (|| {
        let mut writer = BufWriter::new(file);
        write(&mut writer)?;
        writer.flush().map_err(|error| error.to_string())?;
        drop(writer);
        if path.exists() {
            return Err(format!("output path already exists: {}", path.display()));
        }
        fs::rename(&temporary, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _cleanup = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_publication_only_removes_its_own_temporary_file() {
        let dir = std::env::temp_dir().join(format!("rsvz-portable-output-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let target = dir.join("report.json");
        let temporary = dir.join(".report.json.run.tmp");
        fs::write(&temporary, b"another writer").unwrap();
        assert!(write_atomic(&target, "run", |_| Ok(())).is_err());
        assert_eq!(fs::read(&temporary).unwrap(), b"another writer");
        fs::remove_file(&temporary).unwrap();
        assert!(write_atomic(&target, "run", |_| Err("write failed".into())).is_err());
        assert!(!temporary.exists());
        assert!(!target.exists());
        fs::remove_dir(&dir).unwrap();
    }
}
