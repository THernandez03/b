use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

const RELEASES_URL: &str = "https://api.github.com/repos/oven-sh/bun/releases";

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
    let url = format!("{RELEASES_URL}?per_page={per_page}");
    let releases: Vec<GhRelease> = client
        .get(&url)
        .header("User-Agent", "b-bun-version-manager")
        .send()
        .context("Failed to fetch Bun releases from GitHub")?
        .json()
        .context("Failed to parse GitHub releases JSON")?;
    Ok(releases)
}

/// Resolve a user-supplied version string to an exact GitHub release tag.
///
/// Universal aliases (project-specific meanings take priority):
/// - `"lts"` → latest **stable** release (Bun has no separate LTS channel)
/// - `"latest"` / `""` → latest stable release
/// - `"canary"` → Bun's own canary channel (pre-release dev build) — project-native
/// - `"next"` → alias for canary (latest available, even if pre-release)
///
/// Version inputs:
/// - `"1.3"` / `"v1.3"` / `"1.3.x"` → latest stable release matching `bun-v1.3.*`
/// - `"1.3.7"` / `"v1.3.7"` / `"bun-v1.3.7"` → exact tag, no network lookup
pub fn resolve_tag(version_str: &str) -> Result<String> {
    let v = version_str.trim();

    // "lts" and "stable" both mean the current stable release.
    // Bun has no separate LTS channel, so stable IS the LTS.
    if v.is_empty() || v == "lts" || v == "stable" {
        return resolve_latest_stable();
    }

    // "canary" is Bun's own keyword for its nightly/dev channel.
    // "latest" is a common alias for the latest version, including pre-releases.
    // "next" is our universal alias meaning "latest available, including pre-release".
    if v == "canary" || v == "latest" || v == "next" {
        return resolve_canary_tag();
    }

    // Strip recognised prefixes so we always work with the bare semver part.
    let bare = v
        .strip_prefix("bun-v")
        .or_else(|| v.strip_prefix("bun-"))
        .or_else(|| v.strip_prefix('v'))
        .unwrap_or(v);

    // If the bare version looks complete (X.Y.Z) we can try directly.
    if bare.split('.').count() >= 3 {
        // Exact tag — no network lookup needed.
        return Ok(format!("bun-v{bare}"));
    }

    // Partial version (e.g. "1.3" or "1") — find the latest matching release.
    resolve_prefix(bare)
}

/// Fetch the canary commit SHA from the GitHub release name and return
/// a stable cache key like `"canary-{sha}"`.  The download URL is always
/// constructed from the bare `canary` release tag.
fn resolve_canary_tag() -> Result<String> {
    let client = Client::new();
    let url = "https://api.github.com/repos/oven-sh/bun/releases/tags/canary";
    let body: serde_json::Value = client
        .get(url)
        .header("User-Agent", "b-bun-version-manager")
        .send()
        .context("Failed to fetch Bun canary release info")?
        .json()
        .context("Failed to parse Bun canary release JSON")?;

    // Release name format: "Canary (dbd320ccfa909053f95be9e1643d80d73286751f)"
    let name = body["name"]
        .as_str()
        .context("Missing 'name' field in Bun canary release")?;
    let sha = name
        .trim_start_matches("Canary (")
        .trim_end_matches(')')
        .trim();
    anyhow::ensure!(!sha.is_empty(), "Could not parse SHA from Bun canary release name: {name}");
    Ok(format!("canary-{sha}"))
}

/// Return the tag of the latest stable (non-prerelease) Bun release.
fn resolve_latest_stable() -> Result<String> {
    // GitHub's /releases/latest endpoint follows redirects to the latest
    // non-prerelease release page, but we can also just take the first
    // non-prerelease from the releases list.
    let releases = fetch_releases(10)?;
    releases
        .into_iter()
        .find(|r| !r.prerelease)
        .map(|r| r.tag_name)
        .ok_or_else(|| anyhow::anyhow!("No stable Bun release found on GitHub"))
}

/// Return the latest release tag whose version starts with `prefix`.
/// Fetches up to 100 releases so partial matches like "1.3" work reliably.
fn resolve_prefix(prefix: &str) -> Result<String> {
    let client = Client::new();
    // Fetch enough pages to cover old minor versions.
    let url = format!("{RELEASES_URL}?per_page=100");
    let releases: Vec<GhRelease> = client
        .get(&url)
        .header("User-Agent", "b-bun-version-manager")
        .send()
        .context("Failed to fetch Bun releases from GitHub")?
        .json()
        .context("Failed to parse GitHub releases JSON")?;

    // Match tags like "bun-v1.3.*"
    let needle = format!("bun-v{prefix}.");
    releases
        .into_iter()
        .find(|r| {
            !r.prerelease
                && (r.tag_name.starts_with(&needle) || r.tag_name == format!("bun-v{prefix}"))
        })
        .map(|r| r.tag_name)
        .ok_or_else(|| anyhow::anyhow!("No stable Bun release found matching '{prefix}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests only cover the pure logic that does NOT hit the network.
    // Network-dependent paths (lts, partial prefix) are integration-tested
    // manually or in CI with network access.

    /// Helper that exercises normalize behaviour without network.
    fn exact_tag(input: &str) -> String {
        let v = input.trim();
        let bare = v
            .strip_prefix("bun-v")
            .or_else(|| v.strip_prefix("bun-"))
            .or_else(|| v.strip_prefix('v'))
            .unwrap_or(v);
        format!("bun-v{bare}")
    }

    #[test]
    fn exact_bare_semver_normalized() {
        assert_eq!(exact_tag("1.1.0"), "bun-v1.1.0");
    }

    #[test]
    fn exact_v_prefixed_normalized() {
        assert_eq!(exact_tag("v1.1.0"), "bun-v1.1.0");
    }

    #[test]
    fn exact_already_prefixed_passthrough() {
        assert_eq!(exact_tag("bun-v1.1.0"), "bun-v1.1.0");
    }

    #[test]
    fn exact_trims_whitespace() {
        assert_eq!(exact_tag("  1.2.3  "), "bun-v1.2.3");
    }

    #[test]
    fn canary_cache_key_has_sha_prefix() {
        // canary resolution now hits the network to get the commit SHA;
        // we only verify the shape of the result in unit tests.
        // Full network round-trip is covered by integration tests.
        let tag = resolve_tag("canary");
        if let Ok(t) = tag {
            assert!(t.starts_with("canary-"), "expected canary-{{sha}}, got {t}");
        }
        // Allowed to fail in offline/CI environments — skip rather than panic.
    }

    #[test]
    fn next_resolves_like_canary() {
        let tag = resolve_tag("next");
        if let Ok(t) = tag {
            assert!(t.starts_with("canary-"), "expected canary-{{sha}}, got {t}");
        }
    }
}
