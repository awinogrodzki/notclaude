use clap::{Parser, Subcommand};

use notclaude::config;
use notclaude::notification;
use notclaude::process;

#[derive(Parser)]
#[command(name = "notclaude", version, about = "macOS desktop notifications for Claude Code hooks")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as a Claude Code hook (reads JSON from stdin, sends notification)
    Hook,
    /// Install the notification hook into Claude Code settings
    Install {
        /// Install globally (~/.claude/settings.json)
        #[arg(long, conflicts_with = "project")]
        global: bool,
        /// Install for the current project (.claude/settings.json)
        #[arg(long, conflicts_with = "global")]
        project: bool,
    },
    /// Remove the notification hook from Claude Code settings
    Uninstall {
        /// Uninstall globally (~/.claude/settings.json)
        #[arg(long, conflicts_with = "project")]
        global: bool,
        /// Uninstall from the current project (.claude/settings.json)
        #[arg(long, conflicts_with = "global")]
        project: bool,
    },
    /// Show current installation status
    Status,
    /// Internal: send notification and activate app on click (spawned by hook)
    #[command(hide = true)]
    Activate {
        #[arg(long)]
        title: String,
        #[arg(long)]
        message: String,
        #[arg(long)]
        bundle_id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hook => run_hook(),
        Commands::Install { global, project } => run_install(global, project),
        Commands::Uninstall { global, project } => run_uninstall(global, project),
        Commands::Status => run_status(),
        Commands::Activate {
            title,
            message,
            bundle_id,
        } => run_activate(&title, &message, &bundle_id),
    }
}

fn run_hook() {
    let Some(input) = notification::read_hook_input() else {
        std::process::exit(0);
    };

    if let Some((title, message)) = notification::handle_hook(&input) {
        let bundle_id = process::find_parent_app_bundle_id();

        if let Some(bid) = &bundle_id {
            // Spawn a detached child process to send the notification
            // and wait for a click.  We use `exec` (via Command::new)
            // instead of `fork` because the ObjC runtime — specifically
            // NSUserNotificationCenter's delegate callbacks — is not
            // fork-safe and silently breaks in a forked child.
            spawn_activate(title, message, bid);
        } else {
            notification::send_notification(title, message, None, false);
        }
    }
}

/// Spawn a detached `notclaude activate` process.  The current process
/// returns immediately so Claude Code's hook isn't blocked.
fn spawn_activate(title: &str, message: &str, bundle_id: &str) {
    use std::os::unix::process::CommandExt;

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => {
            // Can't find our own binary — fall back to inline send.
            notification::send_notification(title, message, Some(bundle_id), false);
            return;
        }
    };

    let mut cmd = std::process::Command::new(exe);
    cmd.args([
        "activate",
        "--title",
        title,
        "--message",
        message,
        "--bundle-id",
        bundle_id,
    ]);

    // Detach all IO so the child doesn't hold the hook's pipes open.
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    // Create a new session so the child survives after the hook exits.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    if cmd.spawn().is_err() {
        // Spawn failed — fall back to inline (non-activating) send.
        notification::send_notification(title, message, Some(bundle_id), false);
    }
}

/// Send notification and activate the target app on click.
/// Runs as a standalone process spawned by the hook.
fn run_activate(title: &str, message: &str, bundle_id: &str) {
    // Safety net: kill after 5 minutes so a notification that is
    // never clicked doesn't leak a process.
    unsafe {
        libc::alarm(300);
    }

    notification::send_notification(title, message, Some(bundle_id), true);
}

fn run_install(global: bool, project: bool) {
    if !global && !project {
        eprintln!("Error: specify --global or --project");
        std::process::exit(1);
    }

    let path = if global {
        config::global_settings_path().unwrap_or_else(|| {
            eprintln!("Error: could not determine home directory");
            std::process::exit(1);
        })
    } else {
        config::project_settings_path()
    };

    match config::install(&path) {
        Ok(()) => {
            let scope = if global { "global" } else { "project" };
            println!("Installed notclaude hook ({scope}): {}", path.display());
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn run_uninstall(global: bool, project: bool) {
    if !global && !project {
        eprintln!("Error: specify --global or --project");
        std::process::exit(1);
    }

    let path = if global {
        config::global_settings_path().unwrap_or_else(|| {
            eprintln!("Error: could not determine home directory");
            std::process::exit(1);
        })
    } else {
        config::project_settings_path()
    };

    match config::uninstall(&path) {
        Ok(()) => {
            let scope = if global { "global" } else { "project" };
            println!("Uninstalled notclaude hook ({scope}): {}", path.display());
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn run_status() {
    if let Some(global_path) = config::global_settings_path() {
        println!("Global:  {}", config::status(&global_path));
    } else {
        println!("Global:  Could not determine home directory");
    }
    let project_path = config::project_settings_path();
    println!("Project: {}", config::status(&project_path));
}
