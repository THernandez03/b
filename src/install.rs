use anyhow::{Context, Result};
use std::fs;
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;
use std::process::Command;

use console::style;

use crate::{arch, cache, releases, symlink};

/// Install a Bun version and activate it.
pub fn install(version_str: &str) -> Result<()> {
    let tag = releases::resolve_tag(version_str)?;

    if symlink::active_version().as_deref() == Some(&tag) {
        println!(
            "{} Bun {} is already the active version.",
            style("✓").green().bold(),
            style(&tag).cyan().bold(),
        );
        return Ok(());
    }

    if cache::is_cached(&tag) {
        println!(
            "{} Bun {} is already cached.",
            style("◆").dim(),
            style(&tag).cyan(),
        );
    } else {
        println!(
            "{} Downloading Bun {}...",
            style("⬇").cyan(),
            style(&tag).cyan().bold(),
        );
        let tgt = arch::target();
        let url = arch::download_url(&tag, tgt);
        download_version(&url, &tag)?;
    }

    let from = symlink::active_version();
    match &from {
        Some(f) => println!(
            "{} Activating Bun {} → {}...",
            style("◆").magenta(),
            style(f).cyan().bold(),
            style(&tag).cyan().bold(),
        ),
        None => println!(
            "{} Activating Bun {}...",
            style("◆").magenta(),
            style(&tag).cyan().bold(),
        ),
    }
    symlink::activate(&tag)?;
    println!(
        "{} Installed Bun {} successfully.",
        style("✓").green().bold(),
        style(&tag).cyan().bold(),
    );
    Ok(())
}

/// Download a version into cache without activating.
pub fn download_only(version_str: &str) -> Result<()> {
    let tag = releases::resolve_tag(version_str)?;
    if cache::is_cached(&tag) {
        println!("Version {tag} is already cached.");
        return Ok(());
    }
    println!("Downloading Bun {tag}...");
    let tgt = arch::target();
    let url = arch::download_url(&tag, tgt);
    download_version(&url, &tag)
}

/// Run a cached Bun version with given arguments.
pub fn run(version_str: &str, args: &[String]) -> Result<()> {
    let tag = releases::resolve_tag(version_str)?;

    if !cache::is_cached(&tag) {
        println!("Version {tag} is not cached. Downloading...");
        let tgt = arch::target();
        let url = arch::download_url(&tag, tgt);
        download_version(&url, &tag)?;
    }

    let binary = cache::bun_binary(&tag);
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
        cache::remove(&v)?;
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
    let remote = tag.trim_start_matches('v');
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
pub fn uninstall_self() -> Result<()> {
    let name = env!("CARGO_PKG_NAME");
    let confirmed = dialoguer::Confirm::new()
        .with_prompt(format!(
            "This will remove all cached versions and the {name} binary. Continue?"
        ))
        .default(false)
        .interact()?;
    if !confirmed {
        println!("Aborted.");
        return Ok(());
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
