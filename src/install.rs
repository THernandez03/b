use anyhow::{Context, Result};
use std::fs;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{arch, cache, releases, symlink};

/// Install a Bun version and activate it.
pub fn install(version_str: &str) -> Result<()> {
    let tag = releases::resolve_tag(version_str)?;
    let display_tag = if tag == "latest" {
        // Treat "latest" as a special always-fresh slot
        "latest".to_string()
    } else {
        tag.clone()
    };

    // For "latest", always re-download to ensure freshness
    if display_tag != "latest" && cache::is_cached(&display_tag) {
        println!("Version {} is already cached, activating...", display_tag);
    } else {
        println!("Downloading Bun {}...", display_tag);
        let tgt = arch::target();
        let url = arch::download_url(&tag, tgt);
        download_version(&url, &display_tag)?;
    }

    println!("Activating Bun {}...", display_tag);
    symlink::activate(&display_tag)?;
    println!("Installed Bun {} successfully.", display_tag);
    Ok(())
}

/// Download a version into cache without activating.
pub fn download_only(version_str: &str) -> Result<()> {
    let tag = releases::resolve_tag(version_str)?;
    if cache::is_cached(&tag) {
        println!("Version {} is already cached.", tag);
        return Ok(());
    }
    println!("Downloading Bun {}...", tag);
    let tgt = arch::target();
    let url = arch::download_url(&tag, tgt);
    download_version(&url, &tag)
}

/// Run a cached Bun version with given arguments.
pub fn run(version_str: &str, args: &[String]) -> Result<()> {
    let tag = releases::resolve_tag(version_str)?;

    if !cache::is_cached(&tag) {
        println!("Version {} is not cached. Downloading...", tag);
        let tgt = arch::target();
        let url = arch::download_url(&tag, tgt);
        download_version(&url, &tag)?;
    }

    let binary = cache::bun_binary(&tag);
    let status = Command::new(&binary)
        .args(args)
        .status()
        .with_context(|| format!("Failed to run bun {}", tag))?;

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
            if total > 0 {
                let pct = downloaded * 100 / total;
                print!("\r  {}/{} bytes ({}%)", downloaded, total, pct);
                io::stdout().flush()?;
            }
        }
        println!();
    }

    extract_zip(&tmp_path, &dest_dir)?;
    fs::remove_file(&tmp_path).ok();

    // Bun zips contain a single directory bun-{target}/bun — flatten it
    flatten_bun_dir(&dest_dir)?;

    // Make binary executable on Unix
    #[cfg(unix)]
    {
        let binary = cache::bun_binary(tag);
        if binary.exists() {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary, perms)?;
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

/// Bun zip layout: bun-{target}/bun   => we want just `bun` in dest_dir.
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
