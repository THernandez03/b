# b — Interactively Manage Your Bun Versions

> Inspired by [tj/n](https://github.com/tj/n). Written in Rust.

`b` is a simple, no-fuss Bun version manager. Download, cache, and switch between Bun versions with a single command.

## Features

- Install any released Bun version or `canary` builds
- Interactive version picker (arrow keys)
- Version caching — no re-downloading
- Symlink-based activation (no subshells, no profile magic)
- List local and remote versions
- Run a specific version without activating it

## Supported Platforms

| OS      | Architectures          |
|---------|------------------------|
| Linux   | x86_64, aarch64        |
| macOS   | x86_64, aarch64        |
| Windows | x86_64, aarch64        |

## Installation

### Pre-built binary (no Rust required)

```bash
curl -fsSL https://raw.githubusercontent.com/THernandez03/b/main/install.sh | sh
```

This installs `b` to `~/.local/bin/b`. You can override the destination:

```bash
INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/THernandez03/b/main/install.sh | sh
```

### From source (requires Rust)

```bash
cargo install --git https://github.com/THernandez03/b
```

### Manual

Download the latest binary from [Releases](https://github.com/THernandez03/b/releases) and place it in your `PATH`.

## Setup

Add `~/.b/bin` to your `PATH`:

```bash
# bash / zsh
export B_PREFIX="$HOME/.b"
export PATH="$HOME/.local/bin:$PATH"  # for the b binary
export PATH="$B_PREFIX/bin:$PATH"     # for managed Bun binaries
```

Optional environment variables:

| Variable      | Default        | Description                          |
|---------------|----------------|--------------------------------------|
| `B_PREFIX`    | `~/.b`         | Root installation prefix             |
| `B_CACHE_DIR` | `~/.b/versions`| Where downloaded versions are stored |

## Usage

```
# Install a specific version
b 1.1.0
b install 1.1.0
b install bun-v1.1.0
b install latest
b install canary

# Interactive picker from cached versions
b

# List cached versions
b ls

# List recent remote releases
b ls-remote

# Download without activating
b download 1.0.0

# Show path to a cached bun binary
b which bun-v1.1.0

# Run a specific version
b run 1.1.0 -- --version

# Remove a cached version
b rm bun-v1.0.0

# Remove all cached versions except the active one
b prune

# Uninstall active Bun
b uninstall

# Diagnostics
b doctor
```

## How It Works

`b` downloads prebuilt Bun binaries from the [oven-sh/bun GitHub Releases](https://github.com/oven-sh/bun/releases), caches them under `~/.b/versions/<tag>/`, and creates a symlink at `~/.b/bin/bun` pointing to the selected version.

No subshells. No profile setup. Just a symlink.

## License

MIT
