use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

const RELEASES_URL: &str =
    "https://api.github.com/repos/oven-sh/bun/releases";

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    prerelease: bool,
}

/// Print recent Bun releases.
pub fn list_remote() -> Result<()> {
    let releases = fetch_releases(20)?;
    println!("Available Bun versions (recent 20):");
    for r in &releases {
        let pre = if r.prerelease { " (pre-release)" } else { "" };
        println!("  {}{}", r.tag_name, pre);
    }
    Ok(())
}

fn fetch_releases(per_page: u32) -> Result<Vec<GhRelease>> {
    let client = Client::new();
    let url = format!("{}?per_page={}", RELEASES_URL, per_page);
    let releases: Vec<GhRelease> = client
        .get(&url)
        .header("User-Agent", "b-bun-version-manager")
        .send()
        .context("Failed to fetch Bun releases from GitHub")?  
        .json()
        .context("Failed to parse GitHub releases JSON")?;
    Ok(releases)
}

/// Resolve a version string to a normalized release tag.
/// Returns the tag (e.g. "bun-v1.1.0") or "latest" if none provided.
pub fn resolve_tag(version_str: &str) -> Result<String> {
    let v = version_str.trim();
    if v.is_empty() || v == "latest" {
        return Ok("latest".to_string());
    }
    if v == "canary" {
        return Ok("canary".to_string());
    }
    Ok(crate::arch::normalize_tag(v))
}
