# ⚓ harbor

![Build](https://img.shields.io/badge/build-passing-brightgreen)
![Rust](https://img.shields.io/badge/rust-1.70%2B-blue)
![Dependencies](https://img.shields.io/badge/dependencies-minimal-orange)
![License](https://img.shields.io/badge/license-MIT-green)

Files land. Harbor moves. Done.

Fast file organizer daemon. Watches folders. Moves or copies files based on rules. Zero async runtime. Minimal dependencies.

## Why Harbor?

* **Simple:** One binary. One config.
* **Light:** Native threads and channels. No Tokio. No bloat.
* **Smart:** Waits for downloads to finish before moving.
* **Safe:** Deduplicates files on name collisions. Handles cross-device moves.
* **Flexible:** Matches by extension glob, MIME magic bytes, size limits, and date patterns.

## Install

**Download binary:**
Grab the binary for Linux, macOS, or Windows from the [Releases](../../releases) page. Put it in your PATH.

**Build from source:**
```sh
cargo build --release
sudo cp target/release/harbor /usr/local/bin/
```

## Commands

```sh
# Start daemon with default config (~/.config/harbor/config.toml)
harbor

# Start daemon with custom config
harbor -c /path/to/custom-config.toml

# Validate configuration syntax, rules, and paths
harbor check

# Simulate moves without touching files (dry-run)
harbor --dry-run
harbor -n -c /path/to/custom-config.toml

# View or clear background organization history
harbor log
harbor log -n 50
harbor log --clear

# Inspect file MIME types directly
harbor mime photo.png document.pdf archive.zip

# Print help or version
harbor --help
harbor --version
```

## Configure

On first run without a config, Harbor automatically creates a safe template at `~/.config/harbor/config.toml`.

See [`config.toml.example`](config.toml.example) for a full setup.

```toml
[defaults]
to = "$HOME/Downloads/organized"
wait = 5
overwrite = false
include_dirs = false

[ignore]
match = ["*.part", "*.crdownload", "*.tmp", ".*"]

[[folder]]
path = "$HOME/Downloads"

  [[folder.rule]]
  name = "Photos by Date"
  match = "*.{jpg,jpeg,png,webp}"
  to = "$HOME/Pictures/{year}/{month}"

  [[folder.rule]]
  name = "Large Archives"
  match = "*.{zip,tar.gz,7z,iso}"
  min_size = "500MB"
  to = "$HOME/Archives/Large"

  [[folder.rule]]
  name = "Images (MIME fallback)"
  mime = "image/*"
  to = "$HOME/Pictures/{year}/{month}"

  [[folder.rule]]
  name = "Documents"
  match = "*.{pdf,doc,docx,txt,xlsx,epub,md}"
  mime = ["application/pdf", "text/*"]
  to = "$HOME/Documents"

  [[folder.rule]]
  name = "Other"
  match = "*"
  to = "$HOME/Downloads/Other"
```

## Core Behavior

* **Rule Precedence:** Rules evaluate top-to-bottom. Inside each rule:
  1. `min_size` / `max_size` (file size filter)
  2. `match` (fast in-memory glob match)
  3. `mime` (file magic bytes check)
* **Date Variables:** Dynamic tokens in destination paths expand based on file modification date:
  * `{year}` or `{YYYY}`: e.g. `2026`
  * `{month}` or `{MM}`: e.g. `08`
  * `{day}` or `{DD}`: e.g. `22`
  * `{date}`: e.g. `2026-08-22`
* **Size Filters:** Rules accept `min_size` and `max_size` (units: `B`, `KB`, `MB`, `GB`).
* **Startup Sweep:** Scans existing items on launch and organizes non-conforming files before watching.
* **Cooldown:** Tracks file size and mtime each tick to avoid touching active downloads.
* **Dedup:** On filename conflict, renames to `name_1.ext`, `name_2.ext` (or set `overwrite = true`).
* **Directories:** Set `include_dirs = true` to organize whole folders.
* **Cross-device:** Uses atomic `rename`; falls back to copy+delete across disk partitions.
* **Ignore:** Skips temporary files (`*.part`, `*.tmp`, hidden files).

## Contribute

Keep it small. Standard library first. No unnecessary abstractions.

```sh
cargo test
cargo clippy
```

## License

MIT. Use it. Fork it. Break it. Fix it.
