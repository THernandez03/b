use anyhow::{Context, Result};
use std::fs;
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;
use std::process::Command;

use console::style;

use crate::{arch, cache, releases, symlink};

/// Returns `true` if the input is a symbolic alias resolved entirely via network.
fn is_alias(s: &str) -> bool {
    matches!(
        s,
        "lts" | "stable" | "current" | "latest" | "canary" | "nightly" | "next" | "edge" | "beta"
    )
}

/// Returns `true` if the input looks like a bare version number.
fn looks_like_version(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'x' | 'X'))
}

/// Returns `true` if the input looks like a git commit SHA (7-40 hex chars
/// with at least one letter `a`-`f`).
fn is_sha_input(s: &str) -> bool {
    let n = s.len();
    (7..=40).contains(&n)
        && s.chars().all(|c| c.is_ascii_hexdigit())
        && s.chars().any(|c| matches!(c, 'a'..='f' | 'A'..='F'))
}

/// Extract the base version (before any `+sha`) and the optional SHA from a
/// resolved tag. Also strips channel suffixes like `-canary.1` by splitting
/// at the first `-`.
fn extract_ver_sha(tag: &str) -> (String, Option<&str>) {
    let (ver_part, sha) = tag
        .split_once('+')
        .map_or((tag, None), |(v, s)| (v, Some(s)));
    let clean_ver = ver_part.split('-').next().unwrap_or(ver_part).to_string();
    (clean_ver, sha)
}

/// Query the installed bun binary to determine the canonical cache key.
/// Returns `(base_version, Some(sha9))` where `sha9` is the first 9 chars of
/// the commit hash reported by `bun --revision`.
fn query_binary_version(binary_path: &Path) -> Result<(String, Option<String>)> {
    let ver_out = Command::new(binary_path)
        .arg("--version")
        .output()
        .context("Failed to run bun --version")?;
    let ver_raw = String::from_utf8_lossy(&ver_out.stdout);
    let ver_str = ver_raw.trim();
    // Strip channel suffix: "1.4.0-canary.1" → "1.4.0"
    let base_version = ver_str.split('-').next().unwrap_or(ver_str);

    let rev_out = Command::new(binary_path)
        .arg("--revision")
        .output()
        .context("Failed to run bun --revision")?;
    let sha_raw = String::from_utf8_lossy(&rev_out.stdout);
    let rev_str = sha_raw.trim();
    // `bun --revision` may output "1.4.0-canary.1+f161e0311"; take only the hash after `+`.
    let hash_str = rev_str.split_once('+').map_or(rev_str, |(_, h)| h);
    let short_sha = hash_str[..hash_str.len().min(9)].to_string();

    Ok((base_version.to_string(), Some(short_sha)))
}

/// Activate an already-cached version (update the symlink).
fn activate_cached(tag: &str) -> Result<()> {
    if symlink::active_version().as_deref() == Some(tag) {
        println!(
            "{} Bun {} is already the active version.",
            style("\u{2713}").green().bold(),
            style(tag).cyan().bold(),
        );
        return Ok(());
    }
    let from = symlink::active_version();
    match &from {
        Some(f) => println!(
            "{} Activating Bun {} \u{2192} {}...",
            style("\u{25c6}").magenta(),
            style(f).cyan().bold(),
            style(tag).cyan().bold(),
        ),
        None => println!(
            "{} Activating Bun {}...",
            style("\u{25c6}").magenta(),
            style(tag).cyan().bold(),
        ),
    }
    symlink::activate(tag)?;
    println!(
        "{} Installed Bun {} successfully.",
        style("\u{2713}").green().bold(),
        style(tag).cyan().bold(),
    );
    Ok(())
}

/// Install a Bun version and activate it.
pub fn install(version_str: &str) -> Result<()> {
    let v = version_str.trim();

    // 1. Pre-resolve cache check — skip network for version/SHA inputs
    if !is_alias(v) {
        // Passthrough for already-resolved canary tags like "1.4.0-canary.1+sha"
        if v.contains('+') {
            if cache::is_cached(v) {
                return activate_cached(v);
            }
            let (ver_prefix, tag_sha) = extract_ver_sha(v);
            if let Some(cached) = cache::find_by_version_prefix(&ver_prefix) {
                let sha_ok = match (tag_sha, cache::cache_key_sha(&cached)) {
                    (Some(ts), Some(cs)) => cache::sha_matches(cs, ts),
                    (None, _) => true,
                    (Some(_), None) => false,
                };
                if sha_ok {
                    return activate_cached(&cached);
                }
            }
        } else if is_sha_input(v) {
            if let Some(cached) = cache::find_by_sha(v) {
                return activate_cached(&cached);
            }
        } else if looks_like_version(v) {
            let prefix = v.trim_end_matches(".x").trim_end_matches(".X");
            if let Some(cached) = cache::find_by_version_prefix(prefix) {
                return activate_cached(&cached);
            }
        }
    }

    // 2. Resolve via network
    let tag = releases::resolve_tag(v)?;

    // 3. Post-resolve cache check
    {
        let (ver_prefix, tag_sha) = extract_ver_sha(&tag);
        if let Some(cached) = cache::find_by_version_prefix(&ver_prefix) {
            let sha_ok = match (tag_sha, cache::cache_key_sha(&cached)) {
                (Some(ts), Some(cs)) => cache::sha_matches(cs, ts),
                (None, _) => true,
                (Some(_), None) => false,
            };
            if sha_ok {
                return activate_cached(&cached);
            }
        }
        // Alias tags (e.g. "canary+sha") resolve with a known SHA but an
        // unpredictable version prefix — search by SHA directly.
        if let Some(ts) = tag_sha {
            if let Some(cached) = cache::find_by_sha(ts) {
                return activate_cached(&cached);
            }
        }
    }

    // 4. Download if not already cached
    if !cache::is_cached(&tag) {
        println!(
            "{} Downloading Bun {}...",
            style("\u{2b07}").cyan(),
            style(&tag).cyan().bold(),
        );
        let tgt = arch::target();
        let url = arch::download_url(&tag, tgt);
        download_version(&url, &tag)?;
    }

    // 5. Query the installed binary to get the canonical cache key
    let binary = cache::bun_binary(&tag);
    let canonical = match query_binary_version(&binary) {
        Ok((ver, sha_opt)) => sha_opt.map_or_else(|| ver.clone(), |s| format!("{ver}+{s}")),
        Err(_) => tag.clone(),
    };
    if canonical != tag {
        cache::rename_version(&tag, &canonical)?;
    }

    activate_cached(&canonical)
}

/// Download a version into cache without activating.
pub fn download_only(version_str: &str) -> Result<()> {
    let v = version_str.trim();

    // Pre-resolve cache check
    if !is_alias(v) {
        if is_sha_input(v) {
            if let Some(cached) = cache::find_by_sha(v) {
                println!("Version {cached} is already cached.");
                return Ok(());
            }
        } else if looks_like_version(v) {
            let prefix = v.trim_end_matches(".x").trim_end_matches(".X");
            if let Some(cached) = cache::find_by_version_prefix(prefix) {
                println!("Version {cached} is already cached.");
                return Ok(());
            }
        }
    }

    let tag = releases::resolve_tag(v)?;

    // Post-resolve cache check
    {
        let (ver_prefix, tag_sha) = extract_ver_sha(&tag);
        if let Some(cached) = cache::find_by_version_prefix(&ver_prefix) {
            let sha_ok = match (tag_sha, cache::cache_key_sha(&cached)) {
                (Some(ts), Some(cs)) => cache::sha_matches(cs, ts),
                (None, _) => true,
                (Some(_), None) => false,
            };
            if sha_ok {
                println!("Version {cached} is already cached.");
                return Ok(());
            }
        }
        if let Some(ts) = tag_sha {
            if let Some(cached) = cache::find_by_sha(ts) {
                println!("Version {cached} is already cached.");
                return Ok(());
            }
        }
    }

    if cache::is_cached(&tag) {
        println!("Version {tag} is already cached.");
        return Ok(());
    }
    println!("Downloading Bun {tag}...");
    let tgt = arch::target();
    let url = arch::download_url(&tag, tgt);
    download_version(&url, &tag)?;
    let binary = cache::bun_binary(&tag);
    if let Ok((ver, sha_opt)) = query_binary_version(&binary) {
        let canonical = sha_opt.map_or_else(|| ver.clone(), |s| format!("{ver}+{s}"));
        if canonical != tag {
            cache::rename_version(&tag, &canonical)?;
        }
    }
    Ok(())
}

/// Run a cached Bun version with given arguments.
pub fn run(version_str: &str, args: &[String]) -> Result<()> {
    let v = version_str.trim();

    // Pre-resolve cache check
    if !is_alias(v) {
        if is_sha_input(v) {
            if let Some(cached) = cache::find_by_sha(v) {
                return run_cached(&cached, args);
            }
        } else if looks_like_version(v) {
            let prefix = v.trim_end_matches(".x").trim_end_matches(".X");
            if let Some(cached) = cache::find_by_version_prefix(prefix) {
                return run_cached(&cached, args);
            }
        }
    }

    let tag = releases::resolve_tag(v)?;

    // Post-resolve cache check
    let resolved_tag = {
        let (ver_prefix, tag_sha) = extract_ver_sha(&tag);
        if let Some(cached) = cache::find_by_version_prefix(&ver_prefix) {
            let sha_ok = match (tag_sha, cache::cache_key_sha(&cached)) {
                (Some(ts), Some(cs)) => cache::sha_matches(cs, ts),
                (None, _) => true,
                (Some(_), None) => false,
            };
            if sha_ok {
                return run_cached(&cached, args);
            }
        }
        if let Some(ts) = tag_sha {
            if let Some(cached) = cache::find_by_sha(ts) {
                return run_cached(&cached, args);
            }
        }
        tag
    };

    if !cache::is_cached(&resolved_tag) {
        println!("Version {resolved_tag} is not cached. Downloading...");
        let tgt = arch::target();
        let url = arch::download_url(&resolved_tag, tgt);
        download_version(&url, &resolved_tag)?;
    }

    let binary = cache::bun_binary(&resolved_tag);
    let canonical = match query_binary_version(&binary) {
        Ok((ver, sha_opt)) => sha_opt.map_or_else(|| ver.clone(), |s| format!("{ver}+{s}")),
        Err(_) => resolved_tag.clone(),
    };
    if canonical != resolved_tag {
        cache::rename_version(&resolved_tag, &canonical)?;
    }
    run_cached(&canonical, args)
}

fn run_cached(tag: &str, args: &[String]) -> Result<()> {
    let binary = cache::bun_binary(tag);
    let status = Command::new(&binary)
        .args(args)
        .status()
        .with_context(|| format!("Failed to run bun {tag}"))?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn download_version(url: &str, tag: &str) -> Result<()> {
    let dest_dir = cache::version_dir(tag);
    fs::create_dir_all(&dest_dir).context("Failed to create cache directory")?;

    let tmp_path = dest_dir.with_extension("zip");

    {
        let client = reqwest::blocking::Client::new();
        let mut resp = client
            .get(url)
            .header("User-Agent", "b-bun-version-manager")
            .send()
            .context("HTTP request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            // Clean up the empty dest dir we just created.
            fs::remove_dir_all(&dest_dir).ok();
            anyhow::bail!("Download failed: server returned HTTP {status} for {url}");
        }

        let total = resp.content_length().unwrap_or(0);
        let file = fs::File::create(&tmp_path).context("Failed to create temp file")?;
        let mut writer = BufWriter::new(file);

        let mut downloaded = 0u64;
        let mut buf = vec![0u8; 65536];
        loop {
            let n = resp.read(&mut buf)?;
            if n == 0 {
                break;
            }
            writer.write_all(&buf[..n])?;
            downloaded += n as u64;
            if let Some(pct) = downloaded.saturating_mul(100).checked_div(total) {
                print!("\r  {downloaded}/{total} bytes ({pct}%)");
                io::stdout().flush()?;
            }
        }
        println!();
    }

    extract_zip(&tmp_path, &dest_dir)?;
    fs::remove_file(&tmp_path).ok();

    // Bun zips contain a single directory bun-{target}/bun — flatten it
    flatten_bun_dir(&dest_dir)?;

    // Make binary executable on Unix and create bunx symlink
    #[cfg(unix)]
    {
        let binary = cache::bun_binary(tag);
        if binary.exists() {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary, perms)?;
        }

        // bunx is just bun under a different name
        let bunx = dest_dir.join("bunx");
        if !bunx.exists() {
            std::os::unix::fs::symlink("bun", &bunx).context("Failed to create bunx symlink")?;
        }
    }
    #[cfg(windows)]
    {
        // On Windows, create bunx.exe as a copy of bun.exe
        let binary = dest_dir.join("bun.exe");
        let bunx = dest_dir.join("bunx.exe");
        if binary.exists() && !bunx.exists() {
            fs::copy(&binary, &bunx).context("Failed to create bunx.exe")?;
        }
    }

    Ok(())
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
    let file = fs::File::open(archive).context("Failed to open zip")?;
    let mut zip = zip::ZipArchive::new(file).context("Failed to read zip")?;
    zip.extract(dest).context("Failed to extract zip")?;
    Ok(())
}

/// Bun zip layout: bun-{target}/bun   => we want just `bun` in `dest_dir`.
fn flatten_bun_dir(dir: &Path) -> Result<()> {
    let entries: Vec<_> = fs::read_dir(dir)?.collect::<std::io::Result<_>>()?;
    if entries.len() == 1 && entries[0].path().is_dir() {
        let inner = entries[0].path();
        for entry in fs::read_dir(&inner)? {
            let entry = entry?;
            let dest = dir.join(entry.file_name());
            fs::rename(entry.path(), dest).ok();
        }
        fs::remove_dir_all(&inner).ok();
    }
    Ok(())
}

/// Remove a cached version, or prompt for interactive selection if no version is given.
pub fn remove_version(version: Option<String>) -> Result<()> {
    if let Some(v) = version {
        if cache::is_cached(&v) {
            cache::remove(&v)?;
        } else if is_sha_input(&v) {
            if let Some(matched) = cache::find_by_sha(&v) {
                cache::remove(&matched)?;
            } else {
                println!("Version '{v}' is not cached.");
            }
        } else if let Some(matched) = cache::find_by_version_prefix(&v) {
            cache::remove(&matched)?;
        } else {
            println!("Version '{v}' is not cached.");
        }
        return Ok(());
    }
    let versions = cache::cached_versions()?;
    if versions.is_empty() {
        println!("No cached versions to remove.");
        return Ok(());
    }
    let active = symlink::active_version();
    let items: Vec<String> = versions
        .iter()
        .map(|v| {
            if Some(v.as_str()) == active.as_deref() {
                format!("{v}  (active)")
            } else {
                v.clone()
            }
        })
        .collect();
    let idx = dialoguer::Select::new()
        .with_prompt("Select a version to remove")
        .items(&items)
        .interact()?;
    cache::remove(&versions[idx])?;
    Ok(())
}

const GITHUB_REPO: &str = "THernandez03/b";

fn self_artifact() -> String {
    let name = env!("CARGO_PKG_NAME");
    let os_arch = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "linux-arm64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "darwin-x64"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "darwin-arm64"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows-x64"
    } else {
        "linux-x64"
    };
    if cfg!(target_os = "windows") {
        format!("{name}-{os_arch}.exe")
    } else {
        format!("{name}-{os_arch}")
    }
}

fn strip_version_tag<'a>(tag: &'a str, name: &str) -> &'a str {
    tag.trim_start_matches(&format!("{name}-v"))
        .trim_start_matches('v')
}

/// Self-update this version manager binary to the latest GitHub release.
pub fn update_self() -> Result<()> {
    let name = env!("CARGO_PKG_NAME");
    println!("{} Checking for {} updates...", style("◆").cyan(), name);
    let client = reqwest::blocking::Client::new();
    let release: serde_json::Value = client
        .get(format!(
            "https://api.github.com/repos/{GITHUB_REPO}/releases/latest"
        ))
        .header("User-Agent", format!("{name}-version-manager"))
        .send()
        .context("Failed to fetch latest release info")?
        .json()
        .context("Failed to parse release JSON")?;
    let tag = release["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No tag_name in GitHub release response"))?;
    let current = env!("CARGO_PKG_VERSION");
    let remote = strip_version_tag(tag, name);
    if remote == current {
        println!(
            "{} {} is already up to date ({})",
            style("✓").green().bold(),
            name,
            style(current).cyan().bold()
        );
        return Ok(());
    }
    println!(
        "{} Updating {} {} \u{2192} {}...",
        style("⬇").cyan(),
        name,
        style(current).dim(),
        style(remote).cyan().bold()
    );
    let artifact = self_artifact();
    let url = format!("https://github.com/{GITHUB_REPO}/releases/download/{tag}/{artifact}");
    let exe = std::env::current_exe().context("Failed to locate current executable")?;
    let tmp = exe.with_extension("update-tmp");
    {
        let mut resp = client
            .get(&url)
            .header("User-Agent", format!("{name}-version-manager"))
            .send()
            .context("Failed to download update")?;
        if !resp.status().is_success() {
            anyhow::bail!("Download failed: HTTP {} for {}", resp.status(), url);
        }
        let file = fs::File::create(&tmp).context("Failed to create temp file for update")?;
        let mut writer = BufWriter::new(file);
        let mut buf = vec![0u8; 65536];
        loop {
            let n = resp.read(&mut buf).context("Read error during download")?;
            if n == 0 {
                break;
            }
            writer
                .write_all(&buf[..n])
                .context("Write error during download")?;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))
            .context("Failed to set executable permission")?;
    }
    fs::rename(&tmp, &exe).context("Failed to replace current binary")?;
    println!(
        "{} {} updated to {}.",
        style("✓").green().bold(),
        name,
        style(remote).cyan().bold()
    );
    Ok(())
}

/// Uninstall this version manager completely (removes cache, prefix directory, and the binary).
pub fn uninstall_self(yes: bool) -> Result<()> {
    let name = env!("CARGO_PKG_NAME");
    if !yes {
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!(
                "This will remove all cached versions and the {name} binary. Continue?"
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            println!("{}", style("Aborted.").yellow());
            return Ok(());
        }
    }
    println!("Uninstalling {}...", style(name).cyan().bold());
    let prefix = symlink::prefix();
    if prefix.exists() {
        fs::remove_dir_all(&prefix)
            .with_context(|| format!("Failed to remove {}", prefix.display()))?;
        println!("  {} Removed {}", style("✓").green(), prefix.display());
    }
    let exe = std::env::current_exe().context("Failed to locate current executable")?;
    fs::remove_file(&exe).with_context(|| format!("Failed to remove {}", exe.display()))?;
    println!("  {} Removed {}", style("✓").green(), exe.display());
    println!();
    println!(
        "{} {} uninstalled. Remove {} from your PATH if needed.",
        style("✓").green().bold(),
        name,
        exe.parent()
            .map_or_else(String::new, |p| p.display().to_string())
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_dirs<F: FnOnce(&std::path::Path, &std::path::Path)>(f: F) {
        let _guard = ENV_LOCK.lock().unwrap();
        let cache = tempfile::tempdir().expect("tempdir");
        let prefix = tempfile::tempdir().expect("tempdir");
        std::env::set_var("B_CACHE_DIR", cache.path());
        std::env::set_var("B_PREFIX", prefix.path());
        f(cache.path(), prefix.path());
        std::env::remove_var("B_CACHE_DIR");
        std::env::remove_var("B_PREFIX");
    }

    // ── download_only cache hit ─────────────────────────────────────

    #[test]
    fn download_only_skips_if_already_cached() {
        with_temp_dirs(|cache, _prefix| {
            // Bun binary path: {cache}/{tag}/bun (no bin/ subdir).
            // resolve_tag("1.1.0") returns "1.1.0" without network (exact 3-part semver).
            let vdir = cache.join("1.1.0");
            fs::create_dir_all(&vdir).unwrap();
            fs::write(vdir.join("bun"), b"fake").unwrap();
            let result = download_only("1.1.0");
            assert!(
                result.is_ok(),
                "should skip download when cached: {result:?}"
            );
        });
    }

    // ── is_alias ───────────────────────────────────────────────────

    #[test]
    fn is_alias_known_aliases() {
        assert!(is_alias("lts"));
        assert!(is_alias("stable"));
        assert!(is_alias("current"));
        assert!(is_alias("latest"));
        assert!(is_alias("canary"));
        assert!(is_alias("nightly"));
        assert!(is_alias("next"));
        assert!(is_alias("edge"));
        assert!(is_alias("beta"));
    }

    #[test]
    fn is_alias_version_not_alias() {
        assert!(!is_alias("1.1.0"));
        assert!(!is_alias("abc1234d"));
        assert!(!is_alias(""));
    }

    // ── looks_like_version ──────────────────────────────────────────

    #[test]
    fn looks_like_version_semver() {
        assert!(looks_like_version("1.1.0"));
        assert!(looks_like_version("2.0.0"));
    }

    #[test]
    fn looks_like_version_x_notation() {
        assert!(looks_like_version("1.x"));
        assert!(looks_like_version("1.1.X"));
    }

    #[test]
    fn looks_like_version_non_versions() {
        assert!(!looks_like_version("canary"));
        assert!(!looks_like_version("v1.2.3"));
        assert!(!looks_like_version("abc1234d"));
    }

    // ── is_sha_input ───────────────────────────────────────────────

    #[test]
    fn is_sha_input_valid() {
        assert!(is_sha_input("abc1234d"));
        assert!(is_sha_input("abc1234def5678"));
    }

    #[test]
    fn is_sha_input_too_short() {
        assert!(!is_sha_input("abc123"));
    }

    #[test]
    fn is_sha_input_all_digits_rejected() {
        assert!(!is_sha_input("12345678"));
    }

    #[test]
    fn is_sha_input_non_hex_rejected() {
        assert!(!is_sha_input("abc1234g"));
    }

    // ── extract_ver_sha ─────────────────────────────────────────────

    #[test]
    fn extract_ver_sha_with_sha() {
        let (ver, sha) = extract_ver_sha("1.1.0+abc1234def");
        assert_eq!(ver, "1.1.0");
        assert_eq!(sha, Some("abc1234def"));
    }

    #[test]
    fn extract_ver_sha_without_sha() {
        let (ver, sha) = extract_ver_sha("1.1.0");
        assert_eq!(ver, "1.1.0");
        assert!(sha.is_none());
    }

    #[test]
    fn extract_ver_sha_strips_channel_suffix() {
        let (ver, sha) = extract_ver_sha("1.1.0-canary.5+abc1234de");
        assert_eq!(ver, "1.1.0");
        assert_eq!(sha, Some("abc1234de"));
    }

    #[test]
    fn extract_ver_sha_channel_no_sha() {
        let (ver, sha) = extract_ver_sha("1.1.0-canary.5");
        assert_eq!(ver, "1.1.0");
        assert!(sha.is_none());
    }

    #[test]
    fn extract_ver_sha_alias_with_sha() {
        // resolve_tag("latest") returns "canary+{sha}" — the alias name becomes the
        // version prefix. find_by_version_prefix("canary") finds nothing, so the
        // post-resolve check falls back to find_by_sha to locate cached versions.
        let (ver, sha) = extract_ver_sha("canary+f161e0311");
        assert_eq!(ver, "canary");
        assert_eq!(sha, Some("f161e0311"));
    }

    #[test]
    fn extract_ver_sha_bun_revision_format() {
        // bun --revision outputs "1.4.0-canary.1+f161e0311"; this is the full
        // tag form stored in the cache after stripping the channel suffix.
        let (ver, sha) = extract_ver_sha("1.4.0-canary.1+f161e0311");
        assert_eq!(ver, "1.4.0");
        assert_eq!(sha, Some("f161e0311"));
    }

    // ── strip_version_tag ──────────────────────────────────────────

    #[test]
    fn strip_version_tag_strips_name_prefix() {
        assert_eq!(strip_version_tag("b-v0.5.0", "b"), "0.5.0");
    }

    #[test]
    fn strip_version_tag_strips_bare_v_prefix() {
        assert_eq!(strip_version_tag("v0.5.0", "b"), "0.5.0");
    }

    #[test]
    fn strip_version_tag_bare_version_unchanged() {
        assert_eq!(strip_version_tag("0.5.0", "b"), "0.5.0");
    }
}
