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
/// When `force` is `true`, all versions including the active one are removed.
pub fn prune(force: bool) -> Result<()> {
    let active = crate::symlink::active_version();
    let dir = cache_dir();

    if !dir.exists() {
        println!("Cache directory does not exist.");
        return Ok(());
    }

    for entry in fs::read_dir(&dir).context("Failed to read cache directory")? {
        let entry = entry?;
        let name = entry.file_name().into_string().unwrap_or_default();
        if !force && Some(&name) == active.as_ref() {
            println!("Skipped {name} (active — use --force to remove)");
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

/// Returns `true` if `a` is a prefix of `b` or `b` is a prefix of `a`.
/// Used for fuzzy SHA matching between stored (short) and user-provided (long) SHAs.
pub fn sha_matches(a: &str, b: &str) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// Returns the SHA portion of a cache key (the part after `+`), if any.
pub fn cache_key_sha(key: &str) -> Option<&str> {
    key.split_once('+').map(|(_, sha)| sha)
}

/// Find a cached version matching the given version prefix.
///
/// A match occurs when the cache-directory name equals `prefix` exactly, starts
/// with `"{prefix}+"` (exact version with SHA), or starts with `"{prefix}."`
/// (partial version, e.g. `"1.3"` matches `"1.3.14+abc"`).
/// If multiple entries match, the most recently modified is returned.
pub fn find_by_version_prefix(prefix: &str) -> Option<String> {
    let dir = cache_dir();
    if !dir.exists() {
        return None;
    }
    let prefix_plus = format!("{prefix}+");
    let prefix_dot = format!("{prefix}.");
    let mut best: Option<(std::time::SystemTime, String)> = None;
    let Ok(entries) = fs::read_dir(&dir) else {
        return None;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if (name == prefix || name.starts_with(&prefix_plus) || name.starts_with(&prefix_dot))
            && bun_binary(&name).exists()
        {
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let is_newer = best.as_ref().map_or(true, |(t, _)| mtime > *t);
            if is_newer {
                best = Some((mtime, name));
            }
        }
    }
    best.map(|(_, name)| name)
}

/// Find a cached version whose SHA component fuzzy-matches the given SHA.
///
/// The SHA component is the part after `+` in the cache key.
/// If multiple entries match, the most recently modified is returned.
pub fn find_by_sha(sha: &str) -> Option<String> {
    let dir = cache_dir();
    if !dir.exists() {
        return None;
    }
    let mut best: Option<(std::time::SystemTime, String)> = None;
    let Ok(entries) = fs::read_dir(&dir) else {
        return None;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if let Some(cached_sha) = cache_key_sha(&name) {
            if sha_matches(cached_sha, sha) && bun_binary(&name).exists() {
                let mtime = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let is_newer = best.as_ref().map_or(true, |(t, _)| mtime > *t);
                if is_newer {
                    best = Some((mtime, name));
                }
            }
        }
    }
    best.map(|(_, name)| name)
}

/// Rename a cached version directory from `old_key` to `new_key`.
/// No-op if `old_key` does not exist or `new_key` already exists.
pub fn rename_version(old_key: &str, new_key: &str) -> Result<()> {
    let old_dir = version_dir(old_key);
    let new_dir = version_dir(new_key);
    if old_dir.exists() && !new_dir.exists() {
        fs::rename(&old_dir, &new_dir).with_context(|| {
            format!("Failed to rename cache entry '{old_key}' \u{2192} '{new_key}'")
        })?;
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

    fn make_cached_bun(_tc: &TempCache, tag: &str) {
        let path = bun_binary(tag);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"fake").unwrap();
    }

    // ── prune ─────────────────────────────────────────────────────────

    #[test]
    fn prune_skips_active_version() {
        let tc = TempCache::new("prune_skips");
        let prefix = tempfile::tempdir().expect("tempdir");
        unsafe { env::set_var("B_PREFIX", prefix.path()) };

        fs::create_dir_all(tc.dir.join("1.0.0")).unwrap();
        fs::create_dir_all(tc.dir.join("1.1.0")).unwrap();
        fs::write(prefix.path().join(".active"), "1.1.0").unwrap();

        prune(false).unwrap();

        assert!(!tc.dir.join("1.0.0").exists(), "inactive should be removed");
        assert!(tc.dir.join("1.1.0").exists(), "active should be kept");

        unsafe { env::remove_var("B_PREFIX") };
    }

    #[test]
    fn prune_force_removes_active_version() {
        let tc = TempCache::new("prune_force");
        let prefix = tempfile::tempdir().expect("tempdir");
        unsafe { env::set_var("B_PREFIX", prefix.path()) };

        fs::create_dir_all(tc.dir.join("1.0.0")).unwrap();
        fs::create_dir_all(tc.dir.join("1.1.0")).unwrap();
        fs::write(prefix.path().join(".active"), "1.1.0").unwrap();

        prune(true).unwrap();

        assert!(
            !tc.dir.join("1.0.0").exists(),
            "--force should remove inactive"
        );
        assert!(
            !tc.dir.join("1.1.0").exists(),
            "--force should remove active"
        );

        unsafe { env::remove_var("B_PREFIX") };
    }

    // ── sha_matches ───────────────────────────────────────────────────

    #[test]
    fn sha_matches_identical() {
        assert!(sha_matches("abc1234def", "abc1234def"));
    }

    #[test]
    fn sha_matches_a_prefix_of_b() {
        assert!(sha_matches("abc1234", "abc1234def5678"));
    }

    #[test]
    fn sha_matches_b_prefix_of_a() {
        assert!(sha_matches("abc1234def5678", "abc1234"));
    }

    #[test]
    fn sha_matches_unrelated_returns_false() {
        assert!(!sha_matches("abc1234", "def5678"));
    }

    // ── cache_key_sha ─────────────────────────────────────────────────

    #[test]
    fn cache_key_sha_present() {
        assert_eq!(cache_key_sha("1.1.0+abc1234def"), Some("abc1234def"));
    }

    #[test]
    fn cache_key_sha_absent() {
        assert!(cache_key_sha("1.1.0").is_none());
    }

    // ── find_by_version_prefix ────────────────────────────────────────

    #[test]
    fn find_by_version_prefix_exact() {
        let tc = TempCache::new("fbvp_exact");
        make_cached_bun(&tc, "1.1.0");
        assert_eq!(find_by_version_prefix("1.1.0"), Some("1.1.0".to_string()));
    }

    #[test]
    fn find_by_version_prefix_with_sha_suffix() {
        let tc = TempCache::new("fbvp_sha");
        make_cached_bun(&tc, "1.1.0+abc1234def");
        assert_eq!(
            find_by_version_prefix("1.1.0"),
            Some("1.1.0+abc1234def".to_string())
        );
    }

    #[test]
    fn find_by_version_prefix_dot_match() {
        let tc = TempCache::new("fbvp_dot");
        make_cached_bun(&tc, "1.1.0");
        assert_eq!(find_by_version_prefix("1.1"), Some("1.1.0".to_string()));
    }

    #[test]
    fn find_by_version_prefix_no_match() {
        let tc = TempCache::new("fbvp_nomatch");
        make_cached_bun(&tc, "1.1.0");
        assert!(find_by_version_prefix("2.0.0").is_none());
    }

    #[test]
    fn find_by_version_prefix_requires_binary() {
        let tc = TempCache::new("fbvp_nobin");
        fs::create_dir_all(tc.dir.join("1.1.0")).unwrap();
        assert!(find_by_version_prefix("1.1.0").is_none());
    }

    // ── find_by_sha ───────────────────────────────────────────────────

    #[test]
    fn find_by_sha_exact() {
        let tc = TempCache::new("fbs_exact");
        make_cached_bun(&tc, "1.1.0+abc1234def");
        assert_eq!(
            find_by_sha("abc1234def"),
            Some("1.1.0+abc1234def".to_string())
        );
    }

    #[test]
    fn find_by_sha_input_prefix_of_stored() {
        let tc = TempCache::new("fbs_prefix_in");
        make_cached_bun(&tc, "1.1.0+abc1234def5678");
        assert_eq!(
            find_by_sha("abc1234d"),
            Some("1.1.0+abc1234def5678".to_string())
        );
    }

    #[test]
    fn find_by_sha_stored_prefix_of_input() {
        let tc = TempCache::new("fbs_prefix_st");
        make_cached_bun(&tc, "1.1.0+abc1234d");
        assert_eq!(
            find_by_sha("abc1234def5678"),
            Some("1.1.0+abc1234d".to_string())
        );
    }

    #[test]
    fn find_by_sha_no_match() {
        let tc = TempCache::new("fbs_nomatch");
        make_cached_bun(&tc, "1.1.0+abc1234def");
        assert!(find_by_sha("xyz99999").is_none());
    }

    #[test]
    fn find_by_sha_ignores_entry_without_sha() {
        let tc = TempCache::new("fbs_nosha");
        make_cached_bun(&tc, "1.1.0");
        assert!(find_by_sha("11100").is_none());
    }

    // ── rename_version ────────────────────────────────────────────────

    #[test]
    fn rename_version_moves_dir() {
        let tc = TempCache::new("rename_moves");
        fs::create_dir_all(tc.dir.join("1.1.0")).unwrap();
        rename_version("1.1.0", "1.1.0+abc1234def").unwrap();
        assert!(!tc.dir.join("1.1.0").exists());
        assert!(tc.dir.join("1.1.0+abc1234def").exists());
    }

    #[test]
    fn rename_version_noop_when_old_missing() {
        let _tc = TempCache::new("rename_noop_old");
        assert!(rename_version("nonexistent", "also-nonexistent").is_ok());
    }

    #[test]
    fn rename_version_noop_when_new_exists() {
        let tc = TempCache::new("rename_noop_new");
        fs::create_dir_all(tc.dir.join("1.1.0")).unwrap();
        fs::create_dir_all(tc.dir.join("1.1.0+abc1234def")).unwrap();
        rename_version("1.1.0", "1.1.0+abc1234def").unwrap();
        assert!(tc.dir.join("1.1.0").exists());
        assert!(tc.dir.join("1.1.0+abc1234def").exists());
    }
}
