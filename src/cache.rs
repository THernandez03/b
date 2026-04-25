use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Root cache directory: $B_CACHE_DIR or $B_PREFIX/versions, defaulting to ~/.b/versions
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
        anyhow::bail!("Version '{}' is not cached. Run `b install {}` first.", tag, tag)
    }
}

/// Remove a cached version.
pub fn remove(tag: &str) -> Result<()> {
    let dir = version_dir(tag);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .with_context(|| format!("Failed to remove cached version '{}'", tag))?;
        println!("Removed {}", tag);
    } else {
        println!("Version '{}' is not cached.", tag);
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
                .with_context(|| format!("Failed to remove '{}'", name))?;
            println!("Removed {}", name);
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
