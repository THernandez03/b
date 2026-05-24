/// Binary-level integration tests for the `b` CLI.
///
/// These tests invoke the compiled binary as a user would, exercising the full
/// path from CLI parsing → command dispatch → output.  All tests are offline:
/// they pre-populate a temporary cache directory with a fake binary so that
/// `fetch` and `which` return immediately without hitting the network.
use std::fs;
use std::path::Path;
use std::process::Command;

fn b() -> Command {
    Command::new(env!("CARGO_BIN_EXE_b"))
}

/// Create a fake cached Bun binary at the path `b` expects.
/// Bun binary lives at `{cache}/{version}/bun`.
fn fake_cache(dir: &Path, version: &str) {
    let vdir = dir.join(version);
    fs::create_dir_all(&vdir).unwrap();
    fs::write(vdir.join("bun"), b"#!/bin/sh\necho fake\n").unwrap();
}

// ── --help / --version ────────────────────────────────────────────────────────

#[test]
fn help_exits_zero() {
    assert!(b().arg("--help").status().unwrap().success());
}

#[test]
fn version_prints_semver() {
    let out = b().arg("--version").output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.split_whitespace()
            .any(|w| w.contains('.') && w.chars().next().is_some_and(|c| c.is_ascii_digit())),
        "expected semver in version output, got: {s:?}"
    );
}

#[test]
fn unknown_flag_exits_nonzero() {
    assert!(!b().arg("--not-a-real-flag").status().unwrap().success());
}

// ── ls ────────────────────────────────────────────────────────────────────────

#[test]
fn ls_empty_cache_reports_none() {
    let cache = tempfile::tempdir().unwrap();
    let prefix = tempfile::tempdir().unwrap();
    let out = b()
        .arg("ls")
        .env("B_CACHE_DIR", cache.path())
        .env("B_PREFIX", prefix.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("No cached Bun versions found."),
        "unexpected output: {stdout}"
    );
}

#[test]
fn ls_shows_cached_version() {
    let cache = tempfile::tempdir().unwrap();
    let prefix = tempfile::tempdir().unwrap();
    fake_cache(cache.path(), "1.1.0");
    let out = b()
        .arg("ls")
        .env("B_CACHE_DIR", cache.path())
        .env("B_PREFIX", prefix.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("1.1.0"),
        "expected version in output: {stdout}"
    );
}

#[test]
fn ls_marks_active_version() {
    let cache = tempfile::tempdir().unwrap();
    let prefix = tempfile::tempdir().unwrap();
    fake_cache(cache.path(), "1.1.0");
    fs::write(prefix.path().join(".active"), "1.1.0").unwrap();
    let out = b()
        .arg("ls")
        .env("B_CACHE_DIR", cache.path())
        .env("B_PREFIX", prefix.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("(active)"),
        "expected (active) marker in output: {stdout}"
    );
}

// ── fetch ─────────────────────────────────────────────────────────────────────

#[test]
fn fetch_skips_download_when_already_cached() {
    let cache = tempfile::tempdir().unwrap();
    let prefix = tempfile::tempdir().unwrap();
    fake_cache(cache.path(), "1.1.0");
    let status = b()
        .args(["fetch", "1.1.0"])
        .env("B_CACHE_DIR", cache.path())
        .env("B_PREFIX", prefix.path())
        .status()
        .unwrap();
    assert!(
        status.success(),
        "fetch should succeed without network when version is already cached"
    );
}

// ── which ─────────────────────────────────────────────────────────────────────

#[test]
fn which_prints_binary_path() {
    let cache = tempfile::tempdir().unwrap();
    let prefix = tempfile::tempdir().unwrap();
    fake_cache(cache.path(), "1.1.0");
    let out = b()
        .args(["which", "1.1.0"])
        .env("B_CACHE_DIR", cache.path())
        .env("B_PREFIX", prefix.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("1.1.0") && stdout.contains("bun"),
        "expected path containing version and 'bun': {stdout}"
    );
}

#[test]
fn which_fails_when_not_cached() {
    let cache = tempfile::tempdir().unwrap();
    let prefix = tempfile::tempdir().unwrap();
    let status = b()
        .args(["which", "1.1.0"])
        .env("B_CACHE_DIR", cache.path())
        .env("B_PREFIX", prefix.path())
        .status()
        .unwrap();
    assert!(
        !status.success(),
        "which should fail when the version is not cached"
    );
}

// ── prune ─────────────────────────────────────────────────────────────────────

#[test]
fn prune_removes_inactive_keeps_active() {
    let cache = tempfile::tempdir().unwrap();
    let prefix = tempfile::tempdir().unwrap();
    fake_cache(cache.path(), "1.0.0");
    fake_cache(cache.path(), "1.1.0");
    fs::write(prefix.path().join(".active"), "1.1.0").unwrap();
    b().arg("prune")
        .env("B_CACHE_DIR", cache.path())
        .env("B_PREFIX", prefix.path())
        .status()
        .unwrap();
    assert!(
        !cache.path().join("1.0.0").exists(),
        "inactive should be removed"
    );
    assert!(cache.path().join("1.1.0").exists(), "active should be kept");
}

#[test]
fn prune_force_removes_all_including_active() {
    let cache = tempfile::tempdir().unwrap();
    let prefix = tempfile::tempdir().unwrap();
    fake_cache(cache.path(), "1.0.0");
    fake_cache(cache.path(), "1.1.0");
    fs::write(prefix.path().join(".active"), "1.1.0").unwrap();
    b().args(["prune", "--force"])
        .env("B_CACHE_DIR", cache.path())
        .env("B_PREFIX", prefix.path())
        .status()
        .unwrap();
    assert!(
        !cache.path().join("1.0.0").exists(),
        "inactive should be removed by --force"
    );
    assert!(
        !cache.path().join("1.1.0").exists(),
        "active should be removed by --force"
    );
}
