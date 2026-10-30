use std::env;
use std::ffi::OsString;
use std::fs;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::Args;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{FALSE, TRUE};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{CreateEventW, DETACHED_PROCESS, SetEvent};

use crate::{BuildProfile, PortableToolchain, wire};

#[path = "../../../../tools/live_process.rs"]
mod live_process;

#[derive(Args, Debug, Clone)]
pub struct RunPortableArgs {
    #[command(flatten)]
    diagnostic: crate::diagnostics::DiagnosticArgs,
    /// Own a disposable game, verify DLL unload and close only this child on completion/failure.
    #[arg(long, conflicts_with = "no_wait", requires = "profile_template")]
    close_game_on_finish: bool,
    /// Template directory containing userdata (copied into a temporary savedir).
    #[arg(long, requires = "close_game_on_finish")]
    profile_template: Option<PathBuf>,
    #[arg(value_name = "SCRIPT", conflicts_with_all = ["script_crate", "dev_script"])]
    script: Option<PathBuf>,
    #[arg(long, value_name = "PATH", conflicts_with = "dev_script")]
    script_crate: Option<PathBuf>,
    #[arg(long, value_name = "PATH", conflicts_with = "script_crate")]
    dev_script: Option<PathBuf>,
    #[arg(long)]
    script_name: Option<String>,
    #[arg(long)]
    script_package: Option<String>,
    #[arg(long = "script-feature")]
    script_features: Vec<String>,
    #[arg(long, value_name = "PATH")]
    portable_root: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = PortableToolchain::default())]
    portable_toolchain: PortableToolchain,
    #[arg(long, conflicts_with = "release")]
    debug: bool,
    #[arg(long, conflicts_with = "debug")]
    release: bool,
    #[arg(long = "cmake-arg")]
    cmake_args: Vec<OsString>,
    #[arg(long)]
    build_only: bool,
    #[arg(long, value_name = "PATH")]
    report: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_secs: u64,
    #[arg(long)]
    no_wait: bool,
    #[arg(long, value_name = "PATH")]
    workspace_root: Option<PathBuf>,
    #[arg(last = true, allow_hyphen_values = true)]
    game_args: Vec<OsString>,
}

impl RunPortableArgs {
    pub fn take_bare_script(&mut self) -> Option<(PathBuf, Option<String>)> {
        self.script.take().map(|script| (script, self.script_name.clone()))
    }

    #[must_use]
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace_root.as_deref()
    }

    #[must_use]
    const fn profile(&self) -> BuildProfile {
        if self.debug {
            BuildProfile::Debug
        } else {
            BuildProfile::Release
        }
    }

    pub fn use_prepared_script(&mut self, manifest: PathBuf, default_name: String) -> Result<()> {
        if self.script_package.is_some() || !self.script_features.is_empty() {
            bail!("裸 .rs 脚本不能使用 package 或 script-feature 选项");
        }
        self.script_crate = Some(manifest);
        self.script_name.get_or_insert(default_name);
        Ok(())
    }
}

pub fn run(args: RunPortableArgs, root: &Path, current_dir: &Path) -> Result<()> {
    args.diagnostic.validate(args.no_wait, false, true)?;
    if args.close_game_on_finish
        && args
            .game_args
            .iter()
            .any(|arg| arg.to_string_lossy().starts_with("-savedir"))
    {
        bail!("managed Portable run owns -savedir; supply --profile-template instead");
    }
    if args.script.is_some() {
        bail!("裸 .rs 脚本必须由 rsvz CLI 先准备");
    }
    let script_crate = resolve_script_source(root, &args)?;
    let portable_root = args
        .portable_root
        .as_deref()
        .map(|path| current_dir.join(path))
        .unwrap_or_else(|| root.parent().unwrap_or(root).join("PvZ-Portable"));
    if !portable_root.join("CMakeLists.txt").is_file() {
        bail!("PvZ-Portable 源码目录不存在: {}", portable_root.display());
    }
    let profile = args.profile();
    let native_dir = root
        .join("target/rsvz/pvz-portable/game-sdk-v4")
        .join(args.portable_toolchain.name())
        .join(profile.dir_name())
        .join("native");
    configure_and_build(
        root,
        &portable_root,
        &native_dir,
        args.portable_toolchain,
        profile,
        &args.cmake_args,
    )?;
    let executable = PathBuf::from(
        fs::read_to_string(
            native_dir
                .join("sdk")
                .join(profile_name(profile))
                .join("executable.txt"),
        )?
        .trim(),
    );
    if !executable.is_file() {
        bail!("CMake 未生成预期的 PvZ-Portable: {}", executable.display());
    }
    let mut plugin = crate::prepare_plugin(
        root,
        current_dir,
        &script_crate,
        args.script_name.as_deref(),
        args.script_package.as_deref(),
        &args.script_features,
        args.portable_toolchain,
        profile,
    )?;
    let import_dir = executable.parent().context("PvZ-Portable executable 缺少父目录")?;
    crate::build_plugin(&mut plugin, &portable_root, &native_dir)?;
    if args.build_only {
        crate::diagnostics::build_message(
            args.diagnostic.stream_json,
            format_args!("PvZ-Portable: {}", executable.display()),
        );
        crate::diagnostics::build_message(
            args.diagnostic.stream_json,
            format_args!("plugin DLL: {}", plugin.plugin_path.display()),
        );
        crate::diagnostics::build_message(
            args.diagnostic.stream_json,
            format_args!("plugin manifest: {}", plugin.manifest_path.display()),
        );
        return Ok(());
    }

    let deadline = Instant::now() + Duration::from_secs(args.timeout_secs);
    let runtime_dir = native_dir.join("rsvz-runtime");
    fs::create_dir_all(&runtime_dir)?;
    let run_id = new_run_id();
    let report_path = args
        .report
        .as_deref()
        .map(|path| current_dir.join(path))
        .unwrap_or_else(|| runtime_dir.join(format!("{run_id}.report.json")));
    let output_path = args.output.as_deref().map(|path| current_dir.join(path));
    let stop_event = (!args.no_wait).then(|| StopEvent::create(&run_id)).transpose()?;
    let config = wire::PortableRunConfig {
        diagnostics: args
            .diagnostic
            .launch(&run_id, &runtime_dir.join("diagnostics"), current_dir, args.no_wait),
        schema_version: wire::SCHEMA_VERSION,
        run_id,
        backend: wire::BackendKind::PvzPortable {},
        output_path,
        report_path: report_path.clone(),
        stop_event_name: stop_event.as_ref().map(|event| event.name.clone()),
    };
    config.validate().map_err(anyhow::Error::msg)?;
    if report_path.exists() {
        bail!("Portable report path already exists: {}", report_path.display());
    }
    if let Some(path) = config.output_path.as_deref()
        && path.exists()
    {
        bail!("Portable output path already exists: {}", path.display());
    }
    let config_json = serde_json::to_string(&config)?;
    // The game outlives the script. Inherited pipes would make shells wait for
    // game exit even after this CLI returned; keep native diagnostics in a file.
    let game_log_path = runtime_dir.join(format!("{}.game.log", config.run_id));
    let game_log = fs::File::create(&game_log_path).context("创建 Portable 游戏日志失败")?;
    let mut game = Command::new(&executable);
    let sandbox = if let Some(template) = &args.profile_template {
        let template = current_dir.join(template);
        if !template.join("userdata/users.dat").is_file() {
            bail!("profile template must contain userdata/users.dat");
        }
        let sandbox = live_process::Sandbox::new("portable")?;
        live_process::copy_tree(&template, &sandbox.0, deadline)?;
        Some(sandbox)
    } else {
        None
    };
    #[cfg(windows)]
    game.creation_flags(DETACHED_PROCESS);
    game.current_dir(import_dir)
        .stdin(Stdio::null())
        .stdout(game_log.try_clone()?)
        .stderr(game_log)
        .env("PVZP_PLUGIN", &plugin.plugin_path)
        .env("RSVZ_PORTABLE_CONFIG", config_json)
        .args(&args.game_args);
    if let Some(sandbox) = &sandbox {
        game.arg("-savedir").arg(&sandbox.0);
    }
    if args.portable_toolchain == PortableToolchain::MingwUcrt64 {
        prepend_path(&mut game, Path::new("C:/msys64/ucrt64/bin"))?;
    }
    let mut child = live_process::ChildGuard {
        child: game.spawn().context("启动 PvZ-Portable 失败")?,
        terminate_on_drop: args.close_game_on_finish,
    };
    if args.close_game_on_finish {
        eprintln!("owned Portable PID: {}", child.id());
    }
    eprintln!("game log: {}", game_log_path.display());
    if args.no_wait {
        println!("PID: {}", child.id());
        println!("report: {}", report_path.display());
        return Ok(());
    }

    let mut follower = crate::diagnostics::Follower::new(&config.diagnostics, args.diagnostic.stream_json);
    let mut requested_exit = false;
    let run_result = (|| {
        loop {
            follower.poll()?;
            if report_path.is_file() {
                let result: wire::PortableRunResult = serde_json::from_slice(&fs::read(&report_path)?)?;
                requested_exit = matches!(result.terminal, wire::Terminal::Cancelled { exit_game: true, .. });
                return consume_report(&report_path, &config.run_id, &mut follower, args.diagnostic.stream_json);
            }
            if let Some(status) = child.try_wait()? {
                bail!("PvZ-Portable 在写出脚本结果前退出：{status}");
            }
            if Instant::now() >= deadline {
                stop_event.as_ref().expect("waiting run owns a stop event").signal()?;
                if args.close_game_on_finish {
                    bail!("live run timed out");
                }
                let grace = Instant::now() + Duration::from_secs(2);
                while Instant::now() < grace && !report_path.is_file() {
                    thread::sleep(Duration::from_millis(50));
                }
                bail!("等待脚本结果超时；已请求 runtime detach，未终止游戏进程");
            }
            thread::sleep(Duration::from_millis(50));
        }
    })();
    if !args.close_game_on_finish {
        return run_result;
    }
    if requested_exit {
        while child.try_wait()?.is_none() {
            live_process::check_deadline(deadline)?;
            live_process::poll_delay();
        }
        child.terminate_on_drop = false;
        eprintln!("Portable normal game exit observed after cleanup");
        return run_result;
    }
    let unload_deadline = if run_result.is_ok() {
        deadline
    } else {
        stop_event.as_ref().expect("managed run owns stop event").signal()?;
        Instant::now() + Duration::from_secs(7)
    };
    let unload_result = (|| {
        #[cfg(windows)]
        let module = plugin.plugin_path.file_name().context("plugin filename missing")?;
        loop {
            child.ensure_alive()?;
            live_process::check_deadline(unload_deadline)?;
            #[cfg(windows)]
            let loaded = live_process::module_loaded(child.id(), module)?;
            #[cfg(unix)]
            let loaded = unix_plugin_active(child.id())?;
            if !loaded {
                break;
            }
            live_process::poll_delay();
        }
        let observe_until = Instant::now() + Duration::from_millis(500);
        while Instant::now() < observe_until {
            child.ensure_alive()?;
            live_process::check_deadline(unload_deadline)?;
            live_process::poll_delay();
        }
        eprintln!("Portable plugin cleanup complete; game alive after stop");
        Ok(())
    })();
    if let Err(error) = &unload_result {
        eprintln!("Portable cleanup could not verify unload; terminating owned process: {error:#}");
    }
    child.terminate()?;
    eprintln!("owned Portable process reaped: {}", child.id());
    run_result.and(unload_result)
}

#[cfg(windows)]
struct StopEvent {
    handle: OwnedHandle,
    name: String,
}

#[cfg(windows)]
impl StopEvent {
    fn create(run_id: &str) -> Result<Self> {
        let name = format!("Local\\RustVsZombies.PvZPortable.{run_id}.Stop");
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: default security is requested and `wide` is NUL-terminated for this call.
        let handle = unsafe { CreateEventW(std::ptr::null(), TRUE, FALSE, wide.as_ptr()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error()).context("创建 Portable stop event 失败");
        }
        Ok(Self {
            // SAFETY: `CreateEventW` returned a new owned handle.
            handle: unsafe { OwnedHandle::from_raw_handle(handle) },
            name,
        })
    }

    fn signal(&self) -> Result<()> {
        // SAFETY: `self.handle` owns a live event handle.
        if unsafe { SetEvent(self.handle.as_raw_handle()) } == 0 {
            return Err(std::io::Error::last_os_error()).context("触发 Portable stop event 失败");
        }
        Ok(())
    }
}

#[cfg(unix)]
struct StopEvent {
    listener: std::os::unix::net::UnixListener,
    name: String,
}

#[cfg(unix)]
impl StopEvent {
    fn create(run_id: &str) -> Result<Self> {
        let name = std::env::temp_dir().join(format!("rsvz-stop-{run_id}.sock"));
        let listener = std::os::unix::net::UnixListener::bind(&name)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            name: name.to_string_lossy().into_owned(),
        })
    }
    fn signal(&self) -> Result<()> {
        use std::io::Write;
        match self.listener.accept() {
            Ok((mut stream, _)) => {
                stream.write_all(&[1])?;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(unix)]
impl Drop for StopEvent {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.name);
    }
}

#[cfg(unix)]
fn unix_plugin_active(pid: u32) -> Result<bool> {
    use std::io::{Read, Write};
    let directory = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("rsvz-{}", unsafe { libc::getuid() })));
    let mut stream = std::os::unix::net::UnixStream::connect(directory.join(format!("pvzp-plugin-load-{pid}")))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(&[1, 0, 0, 0, 3])?;
    let mut response = [0; 8];
    stream.read_exact(&mut response)?;
    if response[..4] != 4u32.to_le_bytes() {
        bail!("invalid plugin cleanup response");
    }
    Ok(u32::from_le_bytes(response[4..].try_into().unwrap()) != 0)
}

fn consume_report(
    path: &Path, expected_run_id: &str, follower: &mut crate::diagnostics::Follower, stream: bool,
) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("读取 Portable 结果失败: {}", path.display()))?;
    let result: wire::PortableRunResult = serde_json::from_str(&raw).context("解析 Portable 结果 JSON 失败")?;
    result
        .validate(expected_run_id)
        .map_err(anyhow::Error::msg)
        .context("校验 Portable 结果失败")?;
    follower.finish(&result.diagnostics, &result)?;
    if !stream {
        println!("{}", raw.trim_end());
    }
    match result.terminal {
        wire::Terminal::Success {} => Ok(()),
        wire::Terminal::Failure { message } | wire::Terminal::Cancelled { message, .. } => {
            bail!("PvZ-Portable runtime 失败: {message}")
        }
    }
}

fn configure_and_build(
    root: &Path, portable_root: &Path, native_dir: &Path, toolchain: PortableToolchain, profile: BuildProfile,
    extra_args: &[OsString],
) -> Result<()> {
    fs::create_dir_all(native_dir)?;
    let mut configure = Command::new("cmake");
    configure
        .arg("-S")
        .arg(portable_root)
        .arg("-B")
        .arg(native_dir)
        .arg(format!("-DCMAKE_BUILD_TYPE={}", profile_name(profile)))
        .args(extra_args);
    if toolchain == PortableToolchain::MingwUcrt64 {
        configure
            .arg("-G")
            .arg("Ninja")
            .arg("-DCMAKE_C_COMPILER=C:/msys64/ucrt64/bin/gcc.exe")
            .arg("-DCMAKE_CXX_COMPILER=C:/msys64/ucrt64/bin/g++.exe")
            .arg("-DCMAKE_PREFIX_PATH=C:/msys64/ucrt64");
        prepend_path(&mut configure, Path::new("C:/msys64/ucrt64/bin"))?;
    } else if toolchain == PortableToolchain::Native {
        configure.args([
            "-G",
            "Ninja",
            "-DCMAKE_C_COMPILER=clang",
            "-DCMAKE_CXX_COMPILER=clang++",
        ]);
    } else if !extra_args
        .iter()
        .any(|arg| arg.to_string_lossy().starts_with("-DCMAKE_TOOLCHAIN_FILE="))
    {
        let vcpkg = discover_vcpkg_toolchain()?;
        configure
            .arg(format!("-DCMAKE_TOOLCHAIN_FILE={}", vcpkg.display()))
            .arg(format!("-DVCPKG_MANIFEST_DIR={}", portable_root.display()))
            // vcpkg nests long build-tree paths below this directory; keep the shared cache short
            // enough for MSVC's object-file path limit.
            .arg(format!(
                "-DVCPKG_INSTALLED_DIR={}",
                root.join("target/rsvz/vcpkg/msvc").display()
            ));
    }
    if !configure.stdout(Stdio::from(std::io::stderr())).status()?.success() {
        bail!("配置 PvZ-Portable CMake 失败");
    }
    let mut build = Command::new("cmake");
    build
        .arg("--build")
        .arg(native_dir)
        .arg("--config")
        .arg(profile_name(profile));
    if toolchain == PortableToolchain::MingwUcrt64 {
        prepend_path(&mut build, Path::new("C:/msys64/ucrt64/bin"))?;
    }
    if !build.stdout(Stdio::from(std::io::stderr())).status()?.success() {
        bail!("构建 PvZ-Portable 失败");
    }
    Ok(())
}

fn discover_vcpkg_toolchain() -> Result<PathBuf> {
    if let Some(root) = env::var_os("VCPKG_ROOT") {
        let path = PathBuf::from(root).join("scripts/buildsystems/vcpkg.cmake");
        if path.is_file() {
            return Ok(path);
        }
    }

    let program_files_x86 = env::var_os("ProgramFiles(x86)").context("找不到 ProgramFiles(x86)")?;
    let vswhere = PathBuf::from(program_files_x86).join("Microsoft Visual Studio/Installer/vswhere.exe");
    let output = Command::new(&vswhere)
        .args(["-latest", "-products", "*", "-property", "installationPath"])
        .output()
        .with_context(|| format!("运行 {} 失败", vswhere.display()))?;
    let installation = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let path = PathBuf::from(installation).join("VC/vcpkg/scripts/buildsystems/vcpkg.cmake");
    if output.status.success() && path.is_file() {
        Ok(path)
    } else {
        bail!("MSVC 构建需要 vcpkg；请设置 VCPKG_ROOT 或通过 --cmake-arg 指定 CMAKE_TOOLCHAIN_FILE")
    }
}

fn resolve_script_source(root: &Path, args: &RunPortableArgs) -> Result<PathBuf> {
    if let Some(path) = args.script_crate.as_deref() {
        return Ok(path.to_path_buf());
    }
    let path = args
        .dev_script
        .as_deref()
        .context("run-portable 需要 SCRIPT、--script-crate 或 --dev-script")?;
    Ok(root.join("dev-scripts").join(path))
}

fn prepend_path(command: &mut Command, path: &Path) -> Result<()> {
    let mut paths = vec![path.to_path_buf()];
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    command.env("PATH", env::join_paths(paths)?);
    Ok(())
}

fn profile_name(profile: BuildProfile) -> &'static str {
    match profile {
        BuildProfile::Debug => "Debug",
        BuildProfile::Release => "Release",
    }
}

fn new_run_id() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{:x}-{:x}", std::process::id(), now.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_result_validation_rejects_stale_invocations() {
        let result = wire::PortableRunResult {
            cleanup_error: None,
            cleanup_complete: true,
            diagnostics: Vec::new(),
            schema_version: wire::SCHEMA_VERSION,
            run_id: "current".to_owned(),
            backend: wire::BackendKind::PvzPortable {},
            terminal: wire::Terminal::Success {},
            wall_ns: 0,
            frames: 0,
            completed_rounds: 0,
        };
        assert!(result.validate("current").is_ok());
        assert!(result.validate("stale").is_err());
        let mut wrong_version = result;
        wrong_version.schema_version += 1;
        assert!(wrong_version.validate("current").is_err());
    }
}
