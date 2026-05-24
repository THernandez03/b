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
        let marker = if Some(v) == active.as_ref() {
            " (active)"
        } else {
            ""
        };
        println!("  {v}{marker}");
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
                format!("{v} *")
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_env<F: FnOnce(&std::path::Path, &std::path::Path)>(f: F) {
        let _guard = ENV_LOCK.lock().unwrap();
        let cache = tempfile::tempdir().expect("tempdir");
        let prefix = tempfile::tempdir().expect("tempdir");
        std::env::set_var("B_CACHE_DIR", cache.path());
        std::env::set_var("B_PREFIX", prefix.path());
        f(cache.path(), prefix.path());
        std::env::remove_var("B_CACHE_DIR");
        std::env::remove_var("B_PREFIX");
    }

    /// Create a fake cached Bun binary so `cache::is_cached` returns true.
    fn fake_bun(cache: &std::path::Path, version: &str) {
        let vdir = cache.join(version);
        fs::create_dir_all(&vdir).unwrap();
        fs::write(vdir.join("bun"), b"fake").unwrap();
    }

    #[test]
    fn list_local_empty_cache_succeeds() {
        with_temp_env(|_cache, _prefix| {
            assert!(super::list_local().is_ok());
        });
    }

    #[test]
    fn list_local_shows_cached_version() {
        with_temp_env(|cache, _prefix| {
            fake_bun(cache, "1.1.0");
            // list_local() prints to stdout; we only verify it succeeds and
            // that the version appears in cache::cached_versions().
            let versions = crate::cache::cached_versions().unwrap();
            assert!(
                versions.iter().any(|v| v == "1.1.0"),
                "1.1.0 should be listed"
            );
            assert!(super::list_local().is_ok());
        });
    }

    #[test]
    fn list_local_marks_active_version() {
        with_temp_env(|cache, prefix| {
            fake_bun(cache, "1.1.0");
            // active_version() reads {prefix}/.active
            fs::write(prefix.join(".active"), "1.1.0").unwrap();
            let active = crate::symlink::active_version();
            assert_eq!(active.as_deref(), Some("1.1.0"));
            assert!(super::list_local().is_ok());
        });
    }

    #[test]
    fn list_local_multiple_versions_sorted() {
        with_temp_env(|cache, _prefix| {
            fake_bun(cache, "1.0.0");
            fake_bun(cache, "1.1.0");
            fake_bun(cache, "1.2.0");
            let versions = crate::cache::cached_versions().unwrap();
            assert_eq!(versions.len(), 3);
            assert!(super::list_local().is_ok());
        });
    }
}
