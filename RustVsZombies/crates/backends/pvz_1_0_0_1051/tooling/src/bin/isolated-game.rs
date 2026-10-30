//! External process owner for multi-command tests. Reuses the production
//! hash-locked startup-isolation path; no second injected runner is introduced.

#[path = "../../../../../tools/live_process.rs"]
#[allow(
    dead_code,
    reason = "shared external-tool helpers include other validation operations"
)]
mod live_process;
#[path = "../owned_game.rs"]
#[allow(dead_code, reason = "shared owned-game helpers include other validation operations")]
mod owned_game;

use anyhow::{Result, bail};
use clap::Parser;
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    game: PathBuf,
    #[arg(long)]
    profile_template: PathBuf,
    #[arg(long)]
    directory: PathBuf,
    #[arg(long, default_value_t = 120)]
    timeout_secs: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let source = args.game.canonicalize()?;
    let template = args.profile_template.canonicalize()?;
    owned_game::verify_exe(&source)?;
    owned_game::ensure_no_game()?;
    if args.directory.exists() || !template.join("users.dat").is_file() || args.timeout_secs == 0 {
        bail!("requires a fresh test directory, a userdata template and a positive timeout");
    }
    let deadline = Instant::now() + Duration::from_secs(args.timeout_secs);
    live_process::copy_tree(source.parent().unwrap(), &args.directory, deadline)?;
    let directory = args.directory.canonicalize()?;
    let userdata = directory.join("userdata");
    if userdata.exists() {
        if !userdata.canonicalize()?.starts_with(&directory) {
            bail!("userdata leaves the owned directory");
        }
        std::fs::remove_dir_all(&userdata)?;
    }
    live_process::copy_tree(&template, &userdata, deadline)?;
    let game = owned_game::OwnedGame::launch(&directory.join(source.file_name().unwrap()), &directory)?;
    game.wait_ready(deadline)?;
    println!("{{\"pid\":{}}}", game.pid());
    io::stdout().flush()?;
    let (sender, receiver) = mpsc::channel();
    // This thread belongs to external test tooling, not the injected runtime.
    std::thread::spawn(move || {
        let mut line = String::new();
        let _read = io::stdin().read_line(&mut line);
        let _sent = sender.send(());
    });
    while Instant::now() < deadline {
        game.alive()?;
        if receiver.recv_timeout(Duration::from_millis(25)).is_ok() {
            break;
        }
    }
    game.terminate()?;
    Ok(())
}
