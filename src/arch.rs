/// Returns the Bun target string for the current platform,
/// e.g. "linux-x64", "darwin-aarch64", "windows-x64".
pub fn target() -> &'static str {
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
        all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")),
        all(target_os = "macos", any(target_arch = "x86_64", target_arch = "aarch64")),
        all(target_os = "windows", any(target_arch = "x86_64", target_arch = "aarch64")),
    )))]
    return "linux-x64"; // fallback
}

/// Download URL for a specific Bun release tag and target.
/// If tag is "latest" or empty, uses the latest release endpoint.
pub fn download_url(tag: &str, tgt: &str) -> String {
    let base = "https://github.com/oven-sh/bun/releases";
    if tag == "latest" || tag.is_empty() {
        format!("{}/latest/download/bun-{}.zip", base, tgt)
    } else {
        // Normalize: allow "1.1.0" or "bun-v1.1.0"
        let normalized = normalize_tag(tag);
        format!("{}/download/{}/bun-{}.zip", base, normalized, tgt)
    }
}

/// Normalize a version string to a Bun release tag like "bun-v1.1.0".
pub fn normalize_tag(tag: &str) -> String {
    if tag.starts_with("bun-v") {
        tag.to_string()
    } else if tag.starts_with('v') {
        format!("bun-{}", tag)
    } else if tag == "canary" || tag == "latest" {
        tag.to_string()
    } else {
        format!("bun-v{}", tag)
    }
}
