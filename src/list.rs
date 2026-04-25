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
    use std::env;
    use std::fs;

    struct TempCache {
        dir: std::path::PathBuf,
        prev_cache: Option<String>,
        prev_prefix: Option<String>,
    }

    impl TempCache {
        fn new(suffix: &str) -> Self {
            let dir = env::temp_dir().join(format!("b_test_list_{suffix}_{}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            let prev_cache = env::var("B_CACHE_DIR").ok();
            let prev_prefix = env::var("B_PREFIX").ok();
            unsafe { env::set_var("B_CACHE_DIR", &dir) };
            // Set B_PREFIX to something that won't find an .active marker
            let prefix_dir =
                env::temp_dir().join(format!("b_test_list_pfx_{suffix}_{}", std::process::id()));
            fs::create_dir_all(&prefix_dir).unwrap();
            unsafe { env::set_var("B_PREFIX", &prefix_dir) };
            Self {
                dir,
                prev_cache,
                prev_prefix,
            }
        }
    }

    impl Drop for TempCache {
        fn drop(&mut self) {
            match &self.prev_cache {
                Some(v) => unsafe { env::set_var("B_CACHE_DIR", v) },
                None => unsafe { env::remove_var("B_CACHE_DIR") },
            }
            match &self.prev_prefix {
                Some(v) => unsafe { env::set_var("B_PREFIX", v) },
                None => unsafe { env::remove_var("B_PREFIX") },
            }
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    // list_local() calls cache::cached_versions() and symlink::active_version()
    // — both respect environment variables, so we can exercise the real logic.

    #[test]
    fn list_local_succeeds_with_empty_cache() {
        let _tc = TempCache::new("empty");
        // No versions dir → should succeed and print a message
        assert!(super::list_local().is_ok());
    }

    #[test]
    fn list_local_succeeds_with_cached_versions() {
        let tc = TempCache::new("with_versions");
        fs::create_dir_all(tc.dir.join("bun-v1.0.0")).unwrap();
        fs::create_dir_all(tc.dir.join("bun-v1.1.0")).unwrap();
        assert!(super::list_local().is_ok());
    }
}
