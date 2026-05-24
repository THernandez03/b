#![deny(clippy::all, clippy::pedantic, clippy::nursery)]

mod arch;
mod cache;
mod install;
mod list;
mod releases;
mod symlink;

use anyhow::Result;
use clap::{
    builder::styling::{AnsiColor, Effects, Styles},
    Parser, Subcommand,
};

const fn cli_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
        .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
        .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
        .placeholder(AnsiColor::Cyan.on_default())
}

const AFTER_HELP: &str = "\x1b[1;32mInstall a version:\x1b[0m
  \x1b[36mb <version>\x1b[0m    Install and activate (e.g. \x1b[36m1.1.0\x1b[0m, \x1b[36mlatest\x1b[0m, \x1b[36mcanary\x1b[0m)

\x1b[1;32mVersion aliases:\x1b[0m
  \x1b[36mlatest\x1b[0m   Latest stable release
  \x1b[36mcanary\x1b[0m   Latest canary build
  \x1b[36m1.1\x1b[0m      Latest patch in 1.1.x
  \x1b[36m1\x1b[0m        Latest release in major 1";

/// b — Interactively manage your Bun versions
#[derive(Parser)]
#[command(
    name = "b",
    version,
    about,
    styles = cli_styles(),
    disable_help_subcommand = true,
    disable_help_flag = true,
    disable_version_flag = true,
    after_help = AFTER_HELP,
)]
struct Cli {
    /// Print help
    #[arg(short = 'h', long = "help", visible_short_alias = 'H', action = clap::ArgAction::Help)]
    help: Option<bool>,
    /// Print version
    #[arg(short = 'V', long = "version", visible_short_alias = 'v', action = clap::ArgAction::Version)]
    version: Option<bool>,
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
    /// Remove all cached versions except the currently active one.
    /// Use `--force` to also remove the active version.
    Prune {
        /// Also remove the currently active version.
        #[arg(long)]
        force: bool,
    },
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
    /// Uninstall b completely (removes cached versions, prefix, and the b binary).
    /// Use `--yes`/`-y` to skip the confirmation prompt.
    Uninstall {
        /// Skip the confirmation prompt.
        #[arg(short, long)]
        yes: bool,
    },
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
        Some(Commands::Prune { force }) => cache::prune(force)?,
        Some(Commands::Which { version }) => {
            let path = cache::which(&version)?;
            println!("{}", path.display());
        }
        Some(Commands::Run { version, args }) => install::run(&version, &args)?,
        Some(Commands::Fetch { version }) => install::download_only(&version)?,
        Some(Commands::Info) => diagnostics::info(),
        Some(Commands::Update) => install::update_self()?,
        Some(Commands::Uninstall { yes }) => install::uninstall_self(yes)?,
        Some(Commands::Version(args)) => install::install(&args[0])?,
    }

    Ok(())
}

mod diagnostics {
    use crate::{cache, symlink};
    use console::style;

    pub fn info() {
        let prefix = symlink::prefix();
        println!("  {} {}", style("install prefix :").dim(), prefix.display());

        let cache_dir = cache::cache_dir();
        println!(
            "  {} {}",
            style("cache dir      :").dim(),
            cache_dir.display()
        );

        let active = symlink::active_version();
        match active {
            Some(v) => println!(
                "  {} {}",
                style("active version :").dim(),
                style(v).cyan().bold()
            ),
            None => println!(
                "  {} {}",
                style("active version :").dim(),
                style("(none)").dim()
            ),
        }
    }
}
