use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Select};

use crate::{cache, symlink};

/// Print locally cached versions.
pub fn list_local() -> Result<()> {
    let versions = cache::cached_versions()?;
    let active = symlink::active_version();

    if versions.is_empty() {
        println!("No cached Bun versions found.");
        println!("Run `b install <version>` to install one.");
        return Ok(());
    }

    println!("Cached Bun versions:");
    for v in &versions {
        let marker = if Some(v) == active.as_ref() { " (active)" } else { "" };
        println!("  {}{}", v, marker);
    }

    Ok(())
}

/// Interactive version picker using arrow keys.
pub fn interactive_picker() -> Result<()> {
    let versions = cache::cached_versions()?;

    if versions.is_empty() {
        println!("No cached Bun versions found.");
        println!("Run `b install <version>` or `b ls-remote` to get started.");
        return Ok(());
    }

    let active = symlink::active_version();
    let items: Vec<String> = versions
        .iter()
        .map(|v| {
            if Some(v) == active.as_ref() {
                format!("{} *", v)
            } else {
                v.clone()
            }
        })
        .collect();

    let default = versions
        .iter()
        .position(|v| Some(v) == active.as_ref())
        .unwrap_or(0);

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a Bun version (arrow keys, Enter to install, q to quit)")
        .default(default)
        .items(&items)
        .interact_opt()?
        .unwrap_or(usize::MAX);

    if selection == usize::MAX {
        return Ok(());
    }

    let chosen = &versions[selection];
    crate::install::install(chosen)?;

    Ok(())
}
