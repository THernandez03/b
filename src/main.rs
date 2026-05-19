#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::module_name_repetitions)]

mod arch;
mod cache;
mod install;
mod list;
mod releases;
mod symlink;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// b — Interactively manage your Bun versions
#[derive(Parser)]
#[command(name = "b", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// List locally cached versions
    Ls,
    /// List recent remote releases
    LsRemote,
    /// Remove a cached Bun version (interactive if no version given)
    #[command(alias = "rm")]
    Remove {
        /// Version to remove (omit for interactive selection)
        version: Option<String>,
    },
    /// Remove all cached versions except the active one
    Prune,
    /// Show path to a cached Bun binary
    Which {
        /// Version to look up
        version: String,
    },
    /// Run a specific cached Bun version
    Run {
        /// Version tag
        version: String,
        /// Arguments to pass to bun
        args: Vec<String>,
    },
    /// Fetch a Bun version into cache without activating it
    Fetch {
        /// Version tag
        version: String,
    },
    /// Show version manager and runtime information
    Info,
    /// Update b to the latest available version
    Update,
    /// Uninstall b completely (removes cached versions, prefix, and the b binary)
    Uninstall,
    /// Install a Bun version (e.g. 1.1.0, latest, canary)
    #[command(external_subcommand)]
    Version(Vec<String>),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => list::interactive_picker()?,
        Some(Commands::Ls) => list::list_local()?,
        Some(Commands::LsRemote) => releases::list_remote()?,
        Some(Commands::Remove { version }) => install::remove_version(version)?,
        Some(Commands::Prune) => cache::prune()?,
        Some(Commands::Which { version }) => {
            let path = cache::which(&version)?;
            println!("{}", path.display());
        }
        Some(Commands::Run { version, args }) => install::run(&version, &args)?,
        Some(Commands::Fetch { version }) => install::download_only(&version)?,
        Some(Commands::Info) => diagnostics::info(),
        Some(Commands::Update) => install::update_self()?,
        Some(Commands::Uninstall) => install::uninstall_self()?,
        Some(Commands::Version(args)) => install::install(&args[0])?,
    }

    Ok(())
}

mod diagnostics {
    use crate::{cache, symlink};

    pub fn info() {
        println!("b — Bun version manager diagnostics");
        println!();

        let prefix = symlink::prefix();
        println!("  install prefix : {}", prefix.display());

        let cache_dir = cache::cache_dir();
        println!("  cache dir      : {}", cache_dir.display());

        let active = symlink::active_version();
        match active {
            Some(v) => println!("  active version : {v}"),
            None => println!("  active version : (none)"),
        }
    }
}
