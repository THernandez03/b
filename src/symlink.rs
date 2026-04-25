use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Installation prefix: $`B_PREFIX` or ~/.b
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
        .with_context(|| format!("Failed to create symlink {} -> {}", link_path.display(), bun_src.display()))?;

    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&bun_src, &link_path)
        .with_context(|| format!("Failed to create symlink {} -> {}", link_path.display(), bun_src.display()))?;

    let marker = prefix().join(".active");
    fs::write(&marker, tag).context("Failed to write active version marker")?;

    Ok(())
}

/// Read the currently active version from the marker file.
pub fn active_version() -> Option<String> {
    let marker = prefix().join(".active");
    fs::read_to_string(marker)
        .ok()
        .map(|s| s.trim().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    struct TempPrefix {
        dir: std::path::PathBuf,
        prev: Option<String>,
    }

    impl TempPrefix {
        fn new(suffix: &str) -> Self {
            let dir =
                env::temp_dir().join(format!("b_test_prefix_{suffix}_{}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            let prev = env::var("B_PREFIX").ok();
            unsafe { env::set_var("B_PREFIX", &dir) };
            // Also point B_CACHE_DIR inside the prefix so cache helpers work.
            unsafe { env::set_var("B_CACHE_DIR", dir.join("versions")) };
            Self { dir, prev }
        }
    }

    impl Drop for TempPrefix {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { env::set_var("B_PREFIX", v) },
                None => unsafe { env::remove_var("B_PREFIX") },
            }
            unsafe { env::remove_var("B_CACHE_DIR") };
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    // ── prefix() / bin_dir() ─────────────────────────────────────────────────

    #[test]
    fn prefix_uses_env_override() {
        let tp = TempPrefix::new("prefix");
        assert_eq!(prefix(), tp.dir);
    }

    #[test]
    fn bin_dir_is_inside_prefix() {
        let tp = TempPrefix::new("bin_dir");
        assert_eq!(bin_dir(), tp.dir.join("bin"));
    }

    // ── active_version() ─────────────────────────────────────────────────────

    #[test]
    fn active_version_none_when_marker_missing() {
        let _tp = TempPrefix::new("active_none");
        assert!(active_version().is_none());
    }

    #[test]
    fn active_version_reads_marker() {
        let tp = TempPrefix::new("active_reads");
        fs::write(tp.dir.join(".active"), "bun-v1.2.3").unwrap();
        assert_eq!(active_version(), Some("bun-v1.2.3".to_string()));
    }

    #[test]
    fn active_version_trims_whitespace() {
        let tp = TempPrefix::new("active_trim");
        fs::write(tp.dir.join(".active"), "bun-v1.2.3\n").unwrap();
        assert_eq!(active_version(), Some("bun-v1.2.3".to_string()));
    }

    // ── activate() ───────────────────────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn activate_creates_symlink_and_marker() {
        let tp = TempPrefix::new("activate");
        // Create a fake bun binary in the versions dir
        let tag = "bun-v1.0.0";
        let versions = tp.dir.join("versions");
        let ver_dir = versions.join(tag);
        fs::create_dir_all(&ver_dir).unwrap();
        fs::write(ver_dir.join("bun"), b"#!/bin/sh\necho hi").unwrap();

        activate(tag).unwrap();

        let link = tp.dir.join("bin").join("bun");
        assert!(link.symlink_metadata().is_ok(), "symlink should exist");
        assert_eq!(active_version(), Some(tag.to_string()));
    }

    // ── uninstall() ──────────────────────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn uninstall_removes_symlink_and_marker() {
        let tp = TempPrefix::new("uninstall");
        let tag = "bun-v1.0.0";
        let versions = tp.dir.join("versions");
        let ver_dir = versions.join(tag);
        fs::create_dir_all(&ver_dir).unwrap();
        fs::write(ver_dir.join("bun"), b"#!/bin/sh").unwrap();
        activate(tag).unwrap();

        uninstall().unwrap();

        let link = tp.dir.join("bin").join("bun");
        assert!(!link.exists() && link.symlink_metadata().is_err());
        assert!(active_version().is_none());
    }

    #[test]
    fn uninstall_ok_when_nothing_installed() {
        let _tp = TempPrefix::new("uninstall_empty");
        assert!(uninstall().is_ok());
    }
}
