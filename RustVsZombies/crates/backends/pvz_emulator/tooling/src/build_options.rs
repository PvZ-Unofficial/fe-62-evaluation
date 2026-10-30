use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::ValueEnum;

use crate::GENERATED_RUNNER_TARGET_ENV;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    #[must_use]
    pub const fn from_release_flag(release: bool) -> Self {
        if release { Self::Release } else { Self::Debug }
    }

    pub(crate) const fn is_release(self) -> bool {
        matches!(self, Self::Release)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeBuildOptions {
    pub toolchain: PeBuildToolchain,
    pub pgo: PePgoMode,
}

impl PeBuildOptions {
    pub(crate) fn resolve(&self, profile: BuildProfile, root: &Path) -> Result<ResolvedPeBuildOptions> {
        let pgo = match &self.pgo {
            PePgoMode::None => PePgoMode::None,
            PePgoMode::Generate(path) => {
                require_clang(self.toolchain)?;
                PePgoMode::Generate(
                    fs::canonicalize(path)
                        .with_context(|| format!("规范化 PGO profile 目录失败: {}", path.display()))?,
                )
            }
            PePgoMode::Use(path) => {
                require_clang(self.toolchain)?;
                PePgoMode::Use(resolve_pgo_input(path, &root.join("target/rsvz/pgo"))?)
            }
        };

        let mut rustflags = inherited_encoded_rustflags()?;
        self.toolchain.append_lto_rustflags(&mut rustflags);
        match &pgo {
            PePgoMode::Generate(dir) => rustflags.push(format!("-Cprofile-generate={}", dir.display())),
            PePgoMode::Use(path) => {
                rustflags.push(format!("-Cprofile-use={}", path.display()));
            }
            PePgoMode::None => {}
        }

        Ok(ResolvedPeBuildOptions {
            toolchain: self.toolchain,
            pgo,
            rustflags,
            release: profile.is_release(),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PePgoMode {
    #[default]
    None,
    Generate(PathBuf),
    Use(PathBuf),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum PeBuildToolchain {
    Msvc,
    #[default]
    #[value(alias = "default")]
    Clang,
    ClangThinLto,
    ClangFullLto,
}

impl PeBuildToolchain {
    const fn uses_clang(self) -> bool {
        !matches!(self, Self::Msvc)
    }

    const fn lto(self) -> Option<&'static str> {
        match self {
            Self::ClangThinLto => Some("thin"),
            Self::ClangFullLto => Some("full"),
            Self::Msvc | Self::Clang => None,
        }
    }

    fn append_lto_rustflags(self, flags: &mut Vec<String>) {
        let Some(lto) = self.lto() else {
            return;
        };
        flags.push("-Clinker-plugin-lto".to_owned());
        if lto == "full" {
            flags.push("-Clto=fat".to_owned());
            flags.push("-Cembed-bitcode=yes".to_owned());
        }
        flags.push("-Clink-arg=/opt:lldlto=3".to_owned());
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedPeBuildOptions {
    pub toolchain: PeBuildToolchain,
    pub pgo: PePgoMode,
    pub rustflags: Vec<String>,
    release: bool,
}

impl ResolvedPeBuildOptions {
    pub(crate) fn pgo_generate_dir(&self) -> Option<&Path> {
        match &self.pgo {
            PePgoMode::Generate(path) => Some(path),
            PePgoMode::None | PePgoMode::Use(_) => None,
        }
    }

    pub(crate) fn apply_to_cargo_command(&self, command: &mut Command) {
        for name in [
            "PE_RS_TOOLCHAIN",
            "PE_RS_LLVM_LTO",
            "PE_RS_PGO_GENERATE",
            "PE_RS_PGO_USE",
        ] {
            command.env_remove(name);
        }
        if self.toolchain.uses_clang() {
            command.env("PE_RS_TOOLCHAIN", "clang-cl");
            if let Some(llvm_bin) = resolve_llvm_bin() {
                command.env("PE_RS_LLVM_BIN", &llvm_bin);
                let lld_link = llvm_bin.join(tool_exe("lld-link"));
                if lld_link.exists() {
                    command.env("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER", lld_link);
                }
            }
        }
        if let Some(lto) = self.toolchain.lto() {
            command.env("PE_RS_LLVM_LTO", lto);
            if self.release {
                command.env("CARGO_PROFILE_RELEASE_LTO", "off");
                command.env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "1");
            }
        }
        match &self.pgo {
            PePgoMode::None => {}
            PePgoMode::Generate(_) => {
                command.env("PE_RS_PGO_GENERATE", "1");
            }
            PePgoMode::Use(path) => {
                command.env("PE_RS_PGO_USE", path);
            }
        }
        if !self.rustflags.is_empty() {
            command.env("CARGO_ENCODED_RUSTFLAGS", self.rustflags.join("\u{1f}"));
            command.env_remove("RUSTFLAGS");
            command.env_remove(format!("CARGO_TARGET_{GENERATED_RUNNER_TARGET_ENV}_RUSTFLAGS"));
        }
    }
}

fn require_clang(toolchain: PeBuildToolchain) -> Result<()> {
    if !toolchain.uses_clang() {
        bail!("PE PGO requires a clang toolchain");
    }
    Ok(())
}

fn resolve_llvm_bin() -> Option<PathBuf> {
    env::var_os("PE_RS_LLVM_BIN")
        .map(PathBuf::from)
        .filter(|path| path.join(tool_exe("clang-cl")).exists())
        .or_else(|| {
            let path = PathBuf::from(r"C:\Program Files\LLVM\bin");
            path.join(tool_exe("clang-cl")).exists().then_some(path)
        })
}

fn tool_exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn resolve_pgo_input(path: &Path, cache_dir: &Path) -> Result<PathBuf> {
    let path = fs::canonicalize(path).with_context(|| format!("规范化 PGO profdata 失败: {}", path.display()))?;
    if !path.is_file() {
        bail!("PGO use input must be a file: {}", path.display());
    }
    let bytes = fs::read(&path).with_context(|| format!("读取 PGO profdata 失败: {}", path.display()))?;
    let fingerprint = format!("{:016x}", fnv1a_update(FNV_OFFSET, &bytes));
    fs::create_dir_all(cache_dir).with_context(|| format!("创建 PGO 输入缓存失败: {}", cache_dir.display()))?;
    let cached = cache_dir.join(format!("{fingerprint}.profdata"));
    // Change the compiler input path when data changes, not Rust's symbol identity.
    crate::write_if_changed(&cached, &bytes)?;
    Ok(cached.canonicalize()?)
}

fn inherited_encoded_rustflags() -> Result<Vec<String>> {
    let Some(encoded) = env::var_os("CARGO_ENCODED_RUSTFLAGS") else {
        return Ok(Vec::new());
    };
    let encoded = encoded
        .into_string()
        .map_err(|_| anyhow::anyhow!("CARGO_ENCODED_RUSTFLAGS must be Unicode"))?;
    Ok(encoded
        .split('\u{1f}')
        .filter(|flag| !flag.is_empty())
        .map(str::to_owned)
        .collect())
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a_update(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

pub(crate) fn stable_hash_hex(parts: &[&str]) -> String {
    let mut hash = FNV_OFFSET;
    for part in parts {
        hash = fnv1a_update(hash, part.as_bytes());
        hash = fnv1a_update(hash, &[0xff]);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pgo_input_changes_path_without_changing_rust_symbol_metadata() {
        let path = env::temp_dir().join(format!(
            "rsvz-pgo-fingerprint-{}-{}.profdata",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let root = path.with_extension("test");
        let resolve = || {
            PeBuildOptions {
                toolchain: PeBuildToolchain::ClangFullLto,
                pgo: PePgoMode::Use(path.clone()),
            }
            .resolve(BuildProfile::Release, &root)
            .unwrap()
        };
        fs::write(&path, b"abcdef").unwrap();
        let first = resolve();
        assert!(!first.rustflags.iter().any(|flag| flag.contains("metadata")));
        let PePgoMode::Use(first_input) = &first.pgo else {
            panic!("expected PGO use")
        };
        let modified = fs::metadata(first_input).unwrap().modified().unwrap();
        assert_eq!(first, resolve());
        assert_eq!(modified, fs::metadata(first_input).unwrap().modified().unwrap());
        fs::write(&path, b"abcdeg").unwrap();
        let second = resolve();
        assert_ne!(first.rustflags, second.rustflags);
        let PePgoMode::Use(second_input) = &second.pgo else {
            panic!("expected PGO use")
        };
        assert_ne!(first_input, second_input);
        assert_eq!(fs::read(first_input).unwrap(), b"abcdef");
        assert_eq!(fs::read(second_input).unwrap(), b"abcdeg");
        assert_eq!(fs::read(&path).unwrap(), b"abcdeg");
        assert!(root.starts_with(env::temp_dir()));
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn toolchain_mode_owns_its_lto_choice() {
        let default = PeBuildOptions::default()
            .resolve(BuildProfile::Debug, &env::temp_dir())
            .expect("default options");
        let full = PeBuildOptions {
            toolchain: PeBuildToolchain::ClangFullLto,
            pgo: PePgoMode::None,
        }
        .resolve(BuildProfile::Debug, &env::temp_dir())
        .expect("full LTO options");
        assert_eq!(default.toolchain, PeBuildToolchain::Clang);
        assert!(full.rustflags.starts_with(&default.rustflags));
        assert!(
            full.rustflags[default.rustflags.len()..]
                .iter()
                .any(|flag| flag == "-Clto=fat")
        );
    }

    #[test]
    fn pgo_generate_configures_rust_and_native_builds() {
        let options = PeBuildOptions {
            toolchain: PeBuildToolchain::Clang,
            pgo: PePgoMode::Generate(env::temp_dir()),
        }
        .resolve(BuildProfile::Release, &env::temp_dir())
        .expect("PGO generate options");
        let mut command = Command::new("cargo");
        options.apply_to_cargo_command(&mut command);
        let profile_flag = format!(
            "-Cprofile-generate={}",
            env::temp_dir().canonicalize().unwrap().display()
        );
        let command_env = |name: &str| {
            command
                .get_envs()
                .find(|(key, _)| *key == name)
                .and_then(|(_, value)| value)
        };
        assert_eq!(command_env("PE_RS_PGO_GENERATE"), Some(std::ffi::OsStr::new("1")));
        assert!(
            command_env("CARGO_ENCODED_RUSTFLAGS")
                .unwrap()
                .to_str()
                .unwrap()
                .split('\u{1f}')
                .any(|flag| flag == profile_flag)
        );
    }
}
