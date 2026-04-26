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

    /// Version to install (e.g. 1.1.0, latest, canary)
    version: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a Bun version
    Install {
        /// Version tag (e.g. bun-v1.1.0, latest, canary)
        version: String,
    },
    /// List locally cached versions
    Ls,
    /// List recent remote releases
    LsRemote,
    /// Remove one or more cached versions
    Rm {
        /// Versions to remove
        versions: Vec<String>,
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
    /// Download a version into cache without activating it
    Download {
        /// Version tag
        version: String,
    },
    /// Show diagnostic information
    Doctor,
    /// Uninstall the active Bun (does not remove cache)
    Uninstall,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            if let Some(version) = cli.version {
                install::install(&version)?;
            } else {
                list::interactive_picker()?;
            }
        }
        Some(Commands::Install { version }) => install::install(&version)?,
        Some(Commands::Ls) => list::list_local()?,
        Some(Commands::LsRemote) => releases::list_remote()?,
        Some(Commands::Rm { versions }) => {
            for v in &versions {
                cache::remove(v)?;
            }
        }
        Some(Commands::Prune) => cache::prune()?,
        Some(Commands::Which { version }) => {
            let path = cache::which(&version)?;
            println!("{}", path.display());
        }
        Some(Commands::Run { version, args }) => install::run(&version, &args)?,
        Some(Commands::Download { version }) => install::download_only(&version)?,
        Some(Commands::Doctor) => diagnostics::doctor(),
        Some(Commands::Uninstall) => symlink::uninstall(),
    }

    Ok(())
}

mod diagnostics {
    use crate::{cache, symlink};

    pub fn doctor() {
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
