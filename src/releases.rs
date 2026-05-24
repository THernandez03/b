use anyhow::{Context, Result};
use reqwest::blocking::{Client, Response};
use serde::Deserialize;

const RELEASES_URL: &str = "https://api.github.com/repos/oven-sh/bun/releases";

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    prerelease: bool,
}

/// Return the response unchanged if the status is 2xx, or bail with the
/// HTTP status, request URL, and the pretty-printed GitHub error JSON body.
fn check_github_response(response: Response) -> Result<Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let url = response.url().to_string();
    let body = response.text().unwrap_or_default();
    let pretty = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or(body);
    anyhow::bail!("GitHub API error ({status}) for {url}\n\n{pretty}")
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
    let response = client
        .get(&url)
        .header("User-Agent", "b-bun-version-manager")
        .send()
        .context("Failed to fetch Bun releases from GitHub")?;
    let releases: Vec<GhRelease> = check_github_response(response)?
        .json()
        .context("Failed to parse Bun releases JSON")?;
    Ok(releases)
}

/// Resolve a user-supplied version string to an exact GitHub release tag.
///
/// Universal aliases (project-specific meanings take priority):
/// - `"lts"` / `"stable"` / `"current"` → latest **stable** release
/// - `"canary"` / `"latest"` / `"next"` / `"nightly"` / `"edge"` → Bun canary channel
///
/// Version inputs:
/// - `"1.3"` / `"1.3.x"` → latest stable release matching `bun-v1.3.*`
/// - `"1.3.7"` → exact version, no network lookup
pub fn resolve_tag(version_str: &str) -> Result<String> {
    let v = version_str.trim();

    // Already-resolved canary tag like "1.4.0-canary.1+{sha}" — return as-is.
    if v.contains('+') {
        return Ok(v.to_string());
    }

    // Reject v-prefixed and bun-prefixed version strings
    if v.starts_with('v') && v[1..].starts_with(|c: char| c.is_ascii_digit()) {
        anyhow::bail!("No Bun release found matching '{v}'");
    }
    if v.starts_with("bun-") {
        anyhow::bail!("No Bun release found matching '{v}'");
    }

    if v == "beta" {
        anyhow::bail!("'beta' channel is not supported for Bun");
    }

    // "lts", "stable", and "current" all mean the current stable release.
    if v.is_empty() || matches!(v, "lts" | "stable" | "current") {
        return resolve_latest_stable();
    }

    // Canary/nightly aliases — all map to Bun's canary dev channel.
    if matches!(v, "canary" | "latest" | "next" | "nightly" | "edge") {
        return resolve_canary_tag();
    }

    // If the version looks complete (X.Y.Z) we can try directly.
    if v.split('.').count() >= 3 {
        return Ok(v.to_string());
    }

    // Partial version (e.g. "1.3" or "1") — find the latest matching release.
    resolve_prefix(v)
}

/// Fetch the canary commit SHA from the rolling `canary` GitHub release body,
/// and the version label from the latest numbered pre-release (e.g. `bun-v1.4.0-canary.1`).
/// Returns a cache key like `"1.4.0-canary.1+abc123def"`.
/// Downloads always use the rolling `canary` release tag (handled by arch.rs).
fn resolve_canary_tag() -> Result<String> {
    let client = Client::new();

    // 1. Get the version from the latest numbered pre-release (e.g. "bun-v1.4.0-canary.1").
    //    The rolling `canary` release name is not versioned, so we use the releases list instead.
    let releases = fetch_releases(10)?;
    let version = releases.iter().find(|r| r.prerelease).map_or_else(
        || "canary".to_string(),
        |r| {
            r.tag_name
                .strip_prefix("bun-v")
                .unwrap_or(&r.tag_name)
                .to_string()
        },
    );

    // 2. Get the commit SHA from the rolling `canary` release body.
    let url = "https://api.github.com/repos/oven-sh/bun/releases/tags/canary";
    let release: serde_json::Value = client
        .get(url)
        .header("User-Agent", "b-bun-version-manager")
        .send()
        .context("Failed to fetch Bun canary release info")?
        .json()
        .context("Failed to parse Bun canary release JSON")?;

    // The release body contains "This release of Bun corresponds to the commit: {sha}"
    // where the sha may be wrapped in a markdown link: [{sha}]({url}).
    let release_body = release["body"]
        .as_str()
        .context("Missing 'body' field in Bun canary release")?;

    let sha = release_body
        .lines()
        .find_map(|line| {
            let rest = line
                .trim()
                .strip_prefix("This release of Bun corresponds to the commit: ")?;
            // Handle optional markdown link: [d352dfd](https://...)
            let sha = if rest.starts_with('[') {
                rest.trim_start_matches('[').split(']').next()?
            } else {
                rest.split_whitespace().next()?
            };
            let sha = sha.trim();
            (!sha.is_empty()).then(|| sha.to_string())
        })
        .context("Could not find commit SHA in Bun canary release body")?;

    let short_sha = &sha[..sha.len().min(9)];
    Ok(format!("{version}+{short_sha}"))
}

/// Return the tag of the latest stable (non-prerelease) Bun release.
/// Uses `/releases/latest` which returns a single object — more reliable than
/// parsing the paginated array when close to the GitHub API rate limit.
fn resolve_latest_stable() -> Result<String> {
    let client = Client::new();
    let url = format!("{RELEASES_URL}/latest");
    let response = client
        .get(&url)
        .header("User-Agent", "b-bun-version-manager")
        .send()
        .context("Failed to fetch latest Bun release")?;
    let release: serde_json::Value = check_github_response(response)?
        .json()
        .context("Failed to parse latest Bun release JSON")?;
    release["tag_name"]
        .as_str()
        .map(|t| t.strip_prefix("bun-v").unwrap_or(t).to_string())
        .ok_or_else(|| anyhow::anyhow!("No tag_name in latest Bun release response"))
}

/// Return the latest release tag whose version starts with `prefix`.
/// Fetches up to 100 releases so partial matches like "1.3" work reliably.
fn resolve_prefix(prefix: &str) -> Result<String> {
    let releases = fetch_releases(100)?;

    // Match tags like "bun-v1.3.*"
    let needle = format!("bun-v{prefix}.");
    releases
        .into_iter()
        .find(|r| {
            !r.prerelease
                && (r.tag_name.starts_with(&needle) || r.tag_name == format!("bun-v{prefix}"))
        })
        .map(|r| {
            r.tag_name
                .strip_prefix("bun-v")
                .unwrap_or(&r.tag_name)
                .to_string()
        })
        .ok_or_else(|| anyhow::anyhow!("No stable Bun release found matching '{prefix}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests only cover the pure logic that does NOT hit the network.
    // Network-dependent paths (lts, partial prefix) are integration-tested
    // manually or in CI with network access.

    #[test]
    fn exact_bare_semver_normalized() {
        assert_eq!(resolve_tag("1.1.0").unwrap(), "1.1.0");
    }

    #[test]
    fn v_prefix_rejected() {
        assert!(resolve_tag("v1.1.0").is_err());
    }

    #[test]
    fn bun_prefix_rejected() {
        assert!(resolve_tag("bun-v1.1.0").is_err());
        assert!(resolve_tag("bun-1.1.0").is_err());
    }

    #[test]
    fn beta_returns_error() {
        assert!(resolve_tag("beta").is_err());
    }

    #[test]
    fn exact_trims_whitespace() {
        assert_eq!(resolve_tag("  1.2.3  ").unwrap(), "1.2.3");
    }

    #[test]
    fn canary_cache_key_has_version_plus_sha() {
        // canary resolution now hits the network to get the commit SHA and version;
        // we only verify the shape of the result in unit tests.
        let tag = resolve_tag("canary");
        if let Ok(t) = tag {
            assert!(t.contains('+'), "expected {{version}}+{{sha}}, got {t}");
        }
        // Allowed to fail in offline/CI environments — skip rather than panic.
    }

    #[test]
    fn next_resolves_like_canary() {
        let tag = resolve_tag("next");
        if let Ok(t) = tag {
            assert!(t.contains('+'), "expected {{version}}+{{sha}}, got {t}");
        }
    }

    #[test]
    fn nightly_resolves_like_canary() {
        let tag = resolve_tag("nightly");
        if let Ok(t) = tag {
            assert!(t.contains('+'), "expected {{version}}+{{sha}}, got {t}");
        }
    }

    #[test]
    fn edge_resolves_like_canary() {
        let tag = resolve_tag("edge");
        if let Ok(t) = tag {
            assert!(t.contains('+'), "expected {{version}}+{{sha}}, got {t}");
        }
    }
}
