/// Returns the Bun target string for the current platform,
/// e.g. "linux-x64", "darwin-aarch64", "windows-x64".
pub const fn target() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "linux-x64";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "linux-aarch64";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "darwin-x64";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "darwin-aarch64";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "windows-x64";
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    return "windows-aarch64";
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(
            target_os = "macos",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(
            target_os = "windows",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
    )))]
    return "linux-x64"; // fallback
}

/// Download URL for a specific Bun release tag and target.
/// `tag` must be a resolved GitHub release tag like `bun-v1.2.3` or `canary`.
pub fn download_url(tag: &str, tgt: &str) -> String {
    let base = "https://github.com/oven-sh/bun/releases";
    if tag == "canary" {
        // Canary builds live under a special "canary" release on GitHub.
        format!("{base}/download/canary/bun-{tgt}.zip")
    } else {
        format!("{base}/download/{tag}/bun-{tgt}.zip")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── target() ────────────────────────────────────────────────────────────

    #[test]
    fn target_is_non_empty() {
        assert!(!target().is_empty());
    }

    #[test]
    fn target_has_valid_format() {
        let t = target();
        let valid = [
            "linux-x64",
            "linux-aarch64",
            "darwin-x64",
            "darwin-aarch64",
            "windows-x64",
            "windows-aarch64",
        ];
        assert!(valid.contains(&t), "unexpected target: {t}");
    }

    // ── download_url() ───────────────────────────────────────────────────────

    #[test]
    fn download_url_canary_tag() {
        let url = download_url("canary", "linux-x64");
        assert!(
            url.contains("/download/canary/bun-linux-x64.zip"),
            "unexpected url: {url}"
        );
    }

    #[test]
    fn download_url_versioned_tag() {
        let url = download_url("bun-v1.1.0", "darwin-aarch64");
        assert!(
            url.contains("/download/bun-v1.1.0/bun-darwin-aarch64.zip"),
            "unexpected url: {url}"
        );
    }

    #[test]
    fn download_url_starts_with_github() {
        let url = download_url("bun-v1.0.0", "linux-x64");
        assert!(url.starts_with("https://github.com/oven-sh/bun/releases"));
    }

    #[test]
    fn download_url_ends_with_zip() {
        let url = download_url("bun-v1.0.0", "linux-x64");
        assert!(url.to_ascii_lowercase().ends_with(".zip"));
    }
}
