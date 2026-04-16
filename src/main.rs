use std::process;

use clap::{Parser, Subcommand};

use notclaude::config;
use notclaude::notification;

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
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hook => run_hook(),
        Commands::Install { global, project } => run_install(global, project),
        Commands::Uninstall { global, project } => run_uninstall(global, project),
        Commands::Status => run_status(),
    }
}

fn run_hook() {
    let Some(input) = notification::read_hook_input() else {
        process::exit(0);
    };

    if let Some((title, message)) = notification::handle_hook(&input) {
        notification::send_notification(title, message);
    }
}

fn run_install(global: bool, project: bool) {
    if !global && !project {
        eprintln!("Error: specify --global or --project");
        process::exit(1);
    }

    let path = if global {
        config::global_settings_path().unwrap_or_else(|| {
            eprintln!("Error: could not determine home directory");
            process::exit(1);
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
            process::exit(1);
        }
    }
}

fn run_uninstall(global: bool, project: bool) {
    if !global && !project {
        eprintln!("Error: specify --global or --project");
        process::exit(1);
    }

    let path = if global {
        config::global_settings_path().unwrap_or_else(|| {
            eprintln!("Error: could not determine home directory");
            process::exit(1);
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
            process::exit(1);
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
