//! Build and run native PvZ-Emulator final runners.

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Args;

use crate::report::ReportFormat;
use crate::runner_config::{
    configure_generated_runner_command, default_thread_count, generate_default_seed_base, new_run_id,
};
use crate::{BuildProfile, PeBuildOptions, PeBuildToolchain, PePgoMode, ScriptRunnerOptions, wire};

const DEV_SCRIPTS_DIR: &str = "dev-scripts";

// The CLI owns this runner, including when presentation or process waiting fails.
struct RunnerChild(std::process::Child);
impl Drop for RunnerChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// Build and run a PvZ-Emulator final runner.
#[derive(Args, Debug, Clone)]
pub struct RunPeArgs {
    #[command(flatten)]
    diagnostic: crate::diagnostics::DiagnosticArgs,
    /// Standalone Rust script source prepared by the rsvz CLI.
    #[arg(
        value_name = "SCRIPT",
        conflicts_with_all = [
            "script_crate",
            "dev_script",
            "script_package",
            "script_features",
            "no_script_default_features"
        ],
        required_unless_present_any = ["script_crate", "dev_script"]
    )]
    script: Option<PathBuf>,
    /// External script crate directory or Cargo.toml to link into a generated runner.
    #[arg(long, value_name = "PATH", conflicts_with = "dev_script")]
    script_crate: Option<PathBuf>,
    /// Worktree-local script under dev-scripts, or an absolute script crate path.
    #[arg(long, value_name = "PATH", conflicts_with = "script_crate")]
    dev_script: Option<PathBuf>,
    /// Script name used for generated runner paths. Defaults to the dev script or package name.
    #[arg(long)]
    script_name: Option<String>,
    /// Package name when the script path belongs to a multi-package workspace.
    #[arg(long)]
    script_package: Option<String>,
    /// Feature to enable on the user script package. May be repeated.
    #[arg(long = "script-feature")]
    script_features: Vec<String>,
    /// Disable the user script package's default features.
    #[arg(long)]
    no_script_default_features: bool,
    /// Build release artifacts instead of debug artifacts.
    #[arg(long)]
    release: bool,
    /// Generate and build the runner without advancing runtime simulation.
    #[arg(long)]
    build_only: bool,
    /// Number of worker-local PE worlds.
    #[arg(long)]
    threads: Option<NonZeroUsize>,
    /// Use the same deterministic app seed for every worker.
    #[arg(long, conflicts_with = "base_seed")]
    seed: Option<u32>,
    /// Derive distinct deterministic worker seeds from this base seed.
    #[arg(long, conflicts_with = "seed")]
    base_seed: Option<u32>,
    /// Maximum wall-clock seconds for runtime execution.
    #[arg(long)]
    max_wall_secs: Option<u64>,
    /// Maximum PE logical frames per worker.
    #[arg(long)]
    max_sim_frames: Option<u64>,
    /// Maximum completed levels per worker.
    #[arg(long)]
    max_levels: Option<u64>,
    /// Write a script-published artifact as its complete root JSON value.
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Include detailed worker statistics in the report.
    #[arg(long, conflicts_with_all = ["json", "no_report"])]
    stats: bool,
    /// Print machine-readable JSON report.
    #[arg(long, conflicts_with_all = ["stats", "no_report"])]
    json: bool,
    /// Suppress report output.
    #[arg(long, conflicts_with_all = ["stats", "json", "profile"])]
    no_report: bool,
    /// Include low-overhead PE runtime frame profiling in the report.
    #[arg(long)]
    profile: bool,
    /// Record performance for this initial window, without stopping the script.
    #[arg(long)]
    performance_window_secs: Option<u64>,
    /// Toolchain mode for PE native dependencies.
    #[arg(long, value_enum, default_value_t = PeBuildToolchain::Clang)]
    pe_toolchain: PeBuildToolchain,
    /// Build with PGO instrumentation and write runtime .profraw files under this directory.
    #[arg(long, value_name = "DIR", conflicts_with = "pgo_use")]
    pgo_generate: Option<PathBuf>,
    /// Rebuild with LLVM PGO optimization using a merged .profdata file.
    #[arg(long, value_name = "FILE", conflicts_with = "pgo_generate")]
    pgo_use: Option<PathBuf>,
    /// RustVsZombies workspace root. Defaults to searching from the current directory, then the CLI executable path.
    #[arg(long, value_name = "PATH")]
    workspace_root: Option<PathBuf>,
}

impl RunPeArgs {
    pub fn take_bare_script(&mut self) -> Option<(PathBuf, Option<String>)> {
        self.script.take().map(|script| (script, self.script_name.clone()))
    }

    #[must_use]
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace_root.as_deref()
    }

    pub fn use_prepared_script(&mut self, manifest: PathBuf, default_name: String) {
        self.script_crate = Some(manifest);
        self.script_name.get_or_insert(default_name);
    }
}

pub fn run(args: RunPeArgs, root: &Path, current_dir: &Path) -> Result<()> {
    args.diagnostic.validate(false, args.json, false)?;
    if args.script.is_some() {
        bail!("bare .rs scripts must be prepared by the rsvz CLI");
    }
    let script_source = resolve_script_source(root, &args)?;
    let profile = BuildProfile::from_release_flag(args.release);
    let pgo = match (args.pgo_generate.as_deref(), args.pgo_use.as_deref()) {
        (Some(dir), None) => {
            let dir = absolute(current_dir, dir);
            fs::create_dir_all(&dir).with_context(|| format!("创建 PGO profile 目录失败: {}", dir.display()))?;
            PePgoMode::Generate(dir)
        }
        (None, Some(path)) => PePgoMode::Use(absolute(current_dir, path)),
        (None, None) => PePgoMode::None,
        (Some(_), Some(_)) => unreachable!("clap rejects simultaneous PGO generate and use modes"),
    };
    let build_options = PeBuildOptions {
        toolchain: args.pe_toolchain,
        pgo,
    };
    let runner = crate::prepare_script_runner(
        root,
        ScriptRunnerOptions {
            current_dir,
            script_crate: &script_source.script_crate,
            script_name: script_source.script_name.as_deref(),
            script_package: args.script_package.as_deref(),
            script_features: &args.script_features,
            script_default_features: !args.no_script_default_features,
            profile,
            build_options: &build_options,
        },
    )?;
    let exe_path = crate::cargo_build_generated_runner(root, &runner)?;

    if args.build_only {
        crate::diagnostics::build_message(
            args.diagnostic.stream_json,
            format_args!("PE runner EXE: {}", exe_path.display()),
        );
        crate::diagnostics::build_message(
            args.diagnostic.stream_json,
            format_args!("PE runner manifest: {}", runner.manifest_path.display()),
        );
        return Ok(());
    }

    let report_format = if args.no_report {
        ReportFormat::None
    } else if args.json {
        ReportFormat::Json
    } else {
        ReportFormat::Text { stats: args.stats }
    };
    run_generated_runner(
        root,
        &exe_path,
        &args,
        current_dir,
        runner.pgo_generate_dir(),
        report_format,
    )
}

fn resolve_run_config(args: &RunPeArgs, current_dir: &Path, runtime_dir: &Path) -> wire::PeRunConfig {
    let seed_mode = match (args.seed, args.base_seed) {
        (Some(seed), None) => wire::SeedMode::Fixed { seed },
        (None, Some(seed)) => wire::SeedMode::Base { seed },
        (None, None) => wire::SeedMode::Base {
            seed: generate_default_seed_base(),
        },
        (Some(_), Some(_)) => unreachable!("clap rejects simultaneous seed modes"),
    };
    let run_id = new_run_id();
    wire::PeRunConfig {
        diagnostics: args
            .diagnostic
            .launch(&run_id, &runtime_dir.join("diagnostics"), current_dir, false),
        schema_version: wire::SCHEMA_VERSION,
        raw_result_path: runtime_dir.join(format!("{run_id}.result.json")),
        run_id,
        threads: args.threads.unwrap_or_else(default_thread_count).get(),
        seed_mode,
        limits: wire::RunLimits {
            max_wall_ms: args.max_wall_secs.map(|seconds| seconds.saturating_mul(1000)),
            max_sim_frames: args.max_sim_frames,
            max_levels: args.max_levels,
        },
        profile: args.profile,
        performance_window_ms: args.performance_window_secs.map(|seconds| seconds.saturating_mul(1000)),
        output_path: args.output.as_deref().map(|path| absolute(current_dir, path)),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScriptSource {
    script_crate: PathBuf,
    script_name: Option<String>,
}

fn resolve_script_source(root: &Path, args: &RunPeArgs) -> Result<ScriptSource> {
    if let Some(script_crate) = args.script_crate.as_deref() {
        return Ok(ScriptSource {
            script_crate: script_crate.to_path_buf(),
            script_name: args.script_name.clone(),
        });
    }

    let dev_script = args.dev_script.as_deref().context("missing dev script")?;
    let script_crate = if dev_script.is_absolute() {
        dev_script.to_path_buf()
    } else {
        root.join(DEV_SCRIPTS_DIR).join(dev_script)
    };
    let script_name = match args.script_name.clone() {
        Some(script_name) => script_name,
        None => dev_script_name(dev_script)?,
    };
    Ok(ScriptSource {
        script_crate,
        script_name: Some(script_name),
    })
}

fn run_generated_runner(
    root: &Path, exe_path: &Path, args: &RunPeArgs, current_dir: &Path, pgo_generate: Option<&Path>,
    report_format: ReportFormat,
) -> Result<()> {
    let mut command = Command::new(exe_path);
    command.current_dir(root);
    if let Some(dir) = pgo_generate {
        command.env("LLVM_PROFILE_FILE", dir.join("rsvz-pe-%m-%p.profraw"));
    }
    let runtime_dir = root.join("target").join("rsvz").join("runtime").join("pe");
    let config = resolve_run_config(args, current_dir, &runtime_dir);
    let config_path = configure_generated_runner_command(&mut command, &runtime_dir, &config)?;
    if args.diagnostic.stream_json {
        command.stdout(std::process::Stdio::from(std::io::stderr()));
    }
    let mut child = RunnerChild(
        command
            .spawn()
            .with_context(|| format!("start {}", exe_path.display()))?,
    );
    let mut follower = crate::diagnostics::Follower::new(&config.diagnostics, args.diagnostic.stream_json);
    let status = loop {
        follower.poll()?;
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    };
    remove_if_exists(&config_path)?;

    let result_path = &config.raw_result_path;
    let raw = fs::read_to_string(result_path).with_context(|| {
        format!(
            "generated PE runner 未发布有效结果（exit code: {:?}）: {}",
            status.code(),
            result_path.display()
        )
    })?;
    let result: wire::PeRunResult = serde_json::from_str(&raw).context("解析 generated PE raw result JSON 失败")?;
    result
        .validate_for(&config)
        .map_err(anyhow::Error::msg)
        .context("校验 generated PE raw result 失败")?;
    remove_if_exists(result_path)?;

    if matches!(result.terminal, wire::Terminal::Success {}) && !status.success() {
        bail!(
            "generated PE runner published success but exited unsuccessfully: {:?}",
            status.code()
        );
    }
    let ends: Vec<_> = result
        .workers
        .iter()
        .flat_map(|w| w.diagnostics.iter().cloned())
        .collect();
    follower.finish(&ends, &result)?;
    if args.diagnostic.stream_json {
        match result.terminal {
            wire::Terminal::Success {} => Ok(()),
            wire::Terminal::Failure { message } | wire::Terminal::Cancelled { message, .. } => bail!("{message}"),
        }
    } else {
        crate::report::render_result(&result, report_format)
    }
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("删除本次 PE runtime 文件失败: {}", path.display())),
    }
}

fn dev_script_name(path: &Path) -> Result<String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .with_context(|| format!("无法从 --dev-script 推断 script name: {}", path.display()))?;
    Ok(name.to_owned())
}

fn absolute(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

#[cfg(test)]
mod tests {
    use clap::FromArgMatches;

    use super::*;

    fn args(extra: &[&str]) -> Result<RunPeArgs, clap::Error> {
        let matches = RunPeArgs::augment_args(clap::Command::new("run-pe")).try_get_matches_from(
            ["run-pe", "--dev-script", "lowdsl/pe24"]
                .into_iter()
                .chain(extra.iter().copied()),
        )?;
        RunPeArgs::from_arg_matches(&matches)
    }

    #[test]
    fn run_config_resolves_cli_defaults_seeds_paths_and_limits() {
        let current_dir = Path::new("C:/invocation");
        let config = |extra: &[&str]| resolve_run_config(&args(extra).unwrap(), current_dir, current_dir);
        let default = config(&[]);
        assert_eq!(default.threads, default_thread_count().get());
        assert!(matches!(default.seed_mode, wire::SeedMode::Base { .. }));

        let fixed = config(&["--threads", "1", "--seed", "10"]);
        assert_eq!(fixed.threads, 1);
        assert_eq!(fixed.seed_mode, wire::SeedMode::Fixed { seed: 10 });
        assert_eq!(
            config(&["--base-seed", "20"]).seed_mode,
            wire::SeedMode::Base { seed: 20 }
        );
        assert_eq!(
            config(&["--output", "result.json"]).output_path,
            Some(current_dir.join("result.json"))
        );
        for (seconds, milliseconds) in [("2", 2000), ("18446744073709551615", u64::MAX)] {
            let config = config(&["--max-wall-secs", seconds, "--performance-window-secs", seconds]);
            assert_eq!(config.limits.max_wall_ms, Some(milliseconds));
            assert_eq!(config.performance_window_ms, Some(milliseconds));
        }
        assert!(args(&["--threads", "0"]).is_err());
        assert!(args(&["--seed", "1", "--base-seed", "2"]).is_err());
    }
}
