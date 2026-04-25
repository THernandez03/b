use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Installation prefix: $B_PREFIX or ~/.b
pub fn prefix() -> PathBuf {
    if let Ok(p) = std::env::var("B_PREFIX") {
        return PathBuf::from(p);
    }
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".b")
}

/// The bin directory where the active `bun` symlink lives.
pub fn bin_dir() -> PathBuf {
    prefix().join("bin")
}

/// Activate a cached version by creating/updating a symlink.
pub fn activate(tag: &str) -> Result<()> {
    let bin = bin_dir();
    fs::create_dir_all(&bin).context("Failed to create bin directory")?;

    let bun_src = crate::cache::bun_binary(tag);

    #[cfg(target_os = "windows")]
    let link_path = bin.join("bun.exe");
    #[cfg(not(target_os = "windows"))]
    let link_path = bin.join("bun");

    if link_path.exists() || link_path.symlink_metadata().is_ok() {
        fs::remove_file(&link_path).ok();
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(&bun_src, &link_path)
        .with_context(|| format!("Failed to create symlink {:?} -> {:?}", link_path, bun_src))?;

    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&bun_src, &link_path)
        .with_context(|| format!("Failed to create symlink {:?} -> {:?}", link_path, bun_src))?;

    let marker = prefix().join(".active");
    fs::write(&marker, tag).context("Failed to write active version marker")?;

    Ok(())
}

/// Read the currently active version from the marker file.
pub fn active_version() -> Option<String> {
    let marker = prefix().join(".active");
    fs::read_to_string(marker).ok().map(|s| s.trim().to_string())
}

/// Remove the active bun symlink.
pub fn uninstall() -> Result<()> {
    let bin = bin_dir();

    #[cfg(target_os = "windows")]
    let link_path = bin.join("bun.exe");
    #[cfg(not(target_os = "windows"))]
    let link_path = bin.join("bun");

    if link_path.exists() || link_path.symlink_metadata().is_ok() {
        fs::remove_file(&link_path).context("Failed to remove bun symlink")?;
        println!("Removed active Bun installation.");
    } else {
        println!("No active Bun installation found.");
    }

    let marker = prefix().join(".active");
    fs::remove_file(marker).ok();

    Ok(())
}
