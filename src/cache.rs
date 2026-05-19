use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Root cache directory: $`B_CACHE_DIR` or $`B_PREFIX/versions`, defaulting to ~/.b/versions
pub fn cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("B_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let prefix = crate::symlink::prefix();
    prefix.join("versions")
}

/// Path to a specific cached version directory.
pub fn version_dir(tag: &str) -> PathBuf {
    cache_dir().join(tag)
}

/// Path to the bun binary inside a cached version.
pub fn bun_binary(tag: &str) -> PathBuf {
    let dir = version_dir(tag);
    #[cfg(target_os = "windows")]
    return dir.join("bun.exe");
    #[cfg(not(target_os = "windows"))]
    return dir.join("bun");
}

/// Check whether a version is already cached.
pub fn is_cached(tag: &str) -> bool {
    bun_binary(tag).exists()
}

/// Return the path to the bun binary, error if not cached.
pub fn which(tag: &str) -> Result<PathBuf> {
    let path = bun_binary(tag);
    if path.exists() {
        Ok(path)
    } else {
        anyhow::bail!("Version '{tag}' is not cached. Run `b {tag}` to install it.")
    }
}

/// Remove a cached version.
pub fn remove(tag: &str) -> Result<()> {
    let dir = version_dir(tag);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .with_context(|| format!("Failed to remove cached version '{tag}'"))?;
        println!("Removed {tag}");
    } else {
        println!("Version '{tag}' is not cached.");
    }
    Ok(())
}

/// Remove all cached versions except the currently active one.
pub fn prune() -> Result<()> {
    let active = crate::symlink::active_version();
    let dir = cache_dir();

    if !dir.exists() {
        println!("Cache directory does not exist.");
        return Ok(());
    }

    for entry in fs::read_dir(&dir).context("Failed to read cache directory")? {
        let entry = entry?;
        let name = entry.file_name().into_string().unwrap_or_default();
        if Some(&name) == active.as_ref() {
            continue;
        }
        if entry.path().is_dir() {
            fs::remove_dir_all(entry.path())
                .with_context(|| format!("Failed to remove '{name}'"))?;
            println!("Removed {name}");
        }
    }
    Ok(())
}

/// Return all locally cached version tags.
pub fn cached_versions() -> Result<Vec<String>> {
    let dir = cache_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut versions = vec![];
    for entry in fs::read_dir(&dir).context("Failed to read cache directory")? {
        let entry = entry?;
        if entry.path().is_dir() {
            let name = entry.file_name().into_string().unwrap_or_default();
            if !name.is_empty() {
                versions.push(name);
            }
        }
    }
    versions.sort_by(|a, b| b.cmp(a));
    Ok(versions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    /// Creates a temporary directory scoped to the test and sets `B_CACHE_DIR`
    /// so that all cache functions use it instead of the real `~/.b/versions`.
    struct TempCache {
        dir: std::path::PathBuf,
        /// Captured previous value of `B_CACHE_DIR` so we can restore it.
        prev: Option<String>,
    }

    impl TempCache {
        fn new(suffix: &str) -> Self {
            let dir = env::temp_dir().join(format!("b_test_cache_{suffix}_{}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            let prev = env::var("B_CACHE_DIR").ok();
            // SAFETY: tests run with `--test-threads=1` or via serial isolation;
            // we restore the env var on drop.
            unsafe { env::set_var("B_CACHE_DIR", &dir) };
            Self { dir, prev }
        }
    }

    impl Drop for TempCache {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { env::set_var("B_CACHE_DIR", v) },
                None => unsafe { env::remove_var("B_CACHE_DIR") },
            }
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    // ── cache_dir() ──────────────────────────────────────────────────────────

    #[test]
    fn cache_dir_uses_env_override() {
        let tc = TempCache::new("cache_dir");
        assert_eq!(cache_dir(), tc.dir);
    }

    // ── version_dir() ────────────────────────────────────────────────────────

    #[test]
    fn version_dir_is_inside_cache_dir() {
        let tc = TempCache::new("version_dir");
        let vd = version_dir("bun-v1.0.0");
        assert_eq!(vd, tc.dir.join("bun-v1.0.0"));
    }

    // ── bun_binary() ─────────────────────────────────────────────────────────

    #[test]
    fn bun_binary_path_ends_with_bun() {
        let tc = TempCache::new("bun_binary");
        let bin = bun_binary("bun-v1.0.0");
        let name = bin.file_name().unwrap().to_string_lossy();
        #[cfg(windows)]
        assert_eq!(name, "bun.exe");
        #[cfg(not(windows))]
        assert_eq!(name, "bun");
        drop(tc);
    }

    // ── is_cached() ──────────────────────────────────────────────────────────

    #[test]
    fn is_cached_returns_false_when_missing() {
        let tc = TempCache::new("is_cached_false");
        assert!(!is_cached("bun-v99.99.99"));
        drop(tc);
    }

    #[test]
    fn is_cached_returns_true_when_binary_exists() {
        let tc = TempCache::new("is_cached_true");
        let tag = "bun-v1.0.0";
        let dir = tc.dir.join(tag);
        fs::create_dir_all(&dir).unwrap();
        #[cfg(not(windows))]
        fs::write(dir.join("bun"), b"fake").unwrap();
        #[cfg(windows)]
        fs::write(dir.join("bun.exe"), b"fake").unwrap();
        assert!(is_cached(tag));
    }

    // ── which() ──────────────────────────────────────────────────────────────

    #[test]
    fn which_errors_when_not_cached() {
        let tc = TempCache::new("which_err");
        assert!(which("bun-v99.0.0").is_err());
        drop(tc);
    }

    #[test]
    fn which_returns_path_when_cached() {
        let tc = TempCache::new("which_ok");
        let tag = "bun-v1.0.0";
        let dir = tc.dir.join(tag);
        fs::create_dir_all(&dir).unwrap();
        #[cfg(not(windows))]
        fs::write(dir.join("bun"), b"fake").unwrap();
        #[cfg(windows)]
        fs::write(dir.join("bun.exe"), b"fake").unwrap();
        let path = which(tag).unwrap();
        assert!(path.exists());
    }

    // ── remove() ─────────────────────────────────────────────────────────────

    #[test]
    fn remove_deletes_version_dir() {
        let tc = TempCache::new("remove");
        let tag = "bun-v1.0.0";
        let dir = tc.dir.join(tag);
        fs::create_dir_all(&dir).unwrap();
        remove(tag).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn remove_is_ok_when_not_present() {
        let tc = TempCache::new("remove_missing");
        assert!(remove("bun-v99.0.0").is_ok());
        drop(tc);
    }

    // ── cached_versions() ────────────────────────────────────────────────────

    #[test]
    fn cached_versions_empty_when_no_cache_dir() {
        // Point B_CACHE_DIR at a path that does not exist.
        let prev = env::var("B_CACHE_DIR").ok();
        let missing = env::temp_dir().join("b_test_nonexistent_dir_xyz");
        unsafe { env::set_var("B_CACHE_DIR", &missing) };
        let result = cached_versions().unwrap();
        assert!(result.is_empty());
        match prev {
            Some(v) => unsafe { env::set_var("B_CACHE_DIR", v) },
            None => unsafe { env::remove_var("B_CACHE_DIR") },
        }
    }

    #[test]
    fn cached_versions_lists_and_sorts_dirs() {
        let tc = TempCache::new("versions_list");
        for tag in &["bun-v1.0.0", "bun-v1.1.0", "bun-v0.9.0"] {
            fs::create_dir_all(tc.dir.join(tag)).unwrap();
        }
        let versions = cached_versions().unwrap();
        // Sorted reverse-lexicographically
        assert_eq!(versions, vec!["bun-v1.1.0", "bun-v1.0.0", "bun-v0.9.0"]);
    }

    #[test]
    fn cached_versions_ignores_files() {
        let tc = TempCache::new("versions_files");
        // A plain file should NOT appear in the list
        fs::write(tc.dir.join("not-a-version"), b"hi").unwrap();
        fs::create_dir_all(tc.dir.join("bun-v1.0.0")).unwrap();
        let versions = cached_versions().unwrap();
        assert_eq!(versions, vec!["bun-v1.0.0"]);
    }
}
