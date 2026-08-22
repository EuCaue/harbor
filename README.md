# ⚓ harbor

![Build](https://img.shields.io/badge/build-passing-brightgreen)
![Rust](https://img.shields.io/badge/rust-1.70%2B-blue)
![Dependencies](https://img.shields.io/badge/dependencies-minimal-orange)
![License](https://img.shields.io/badge/license-MIT-green)

Files land. Harbor moves. Done.

Fast file organizer. Watches folders. Moves or copies files based on rules. Zero async runtime. No fluff.

## Why Harbor?

* **Simple.** One binary. One config.
* **Light.** Threads + channels. No Tokio. No bloat.
* **Smart.** Waits for downloads to finish before moving.
* **Safe.** Dedups files. Handles cross-device moves.

## Install

**Easy way:**
Grab the binary for Linux, Mac, or Windows from the [Releases](../../releases) page. Put it in your PATH.

**Build way:**
```sh
cargo build --release
sudo cp target/release/harbor /usr/local/bin/
```

## Run

Start Harbor with default config (`~/.config/harbor/config.toml`):
```sh
harbor
```

Or pass a custom config path:
```sh
harbor -c /path/to/custom-config.toml
```
No config file yet? Harbor automatically creates an initial template at `~/.config/harbor/config.toml` on first run.
It runs in the foreground. To keep it alive in the background, use `systemd` (Linux), `launchd` (Mac), or the **Startup folder** / Task Scheduler (Windows).

### CLI Commands & Flags

```sh
# Validate configuration file and folder paths
harbor check

# Simulate moves without touching files (dry-run)
harbor --dry-run
harbor -n -c /path/to/custom.toml

# Inspect file MIME types
harbor mime photo.png document.pdf archive.zip

# Print help or version
harbor --help
harbor --version
```

## Configure

See [`config.toml.example`](config.toml.example) for a full setup.

Rules match top to bottom. First match wins. Within a single rule, `match` (glob) is evaluated first before `mime`.

```toml
[defaults]
to = "$HOME/Downloads/organized"
wait = 5
include_dirs = false

[[folder]]
path = "$HOME/Downloads"

  [[folder.rule]]
  name = "Photos by Date"
  match = "*.{jpg,png}"
  to = "$HOME/Pictures/{year}/{month}"

  [[folder.rule]]
  name = "Large Archives"
  match = "*.{zip,tar.gz,iso}"
  min_size = "500MB"
  to = "$HOME/Archives/Large"

  [[folder.rule]]
  name = "Images Fallback"
  mime = "image/*"
  to = "$HOME/Pictures/{year}/{month}"

  [[folder.rule]]
  name = "Documents"
  mime = ["application/pdf", "text/*"]
  to = "$HOME/Documents"

  [[folder.rule]]
  match = "*"
  to = "$HOME/Misc"
```

## Core Behavior

* **Rule Precedence:** Evaluated top-to-bottom. Within a rule: size filter -> `match` (extension glob) -> `mime` (magic bytes / MIME type).
* **Date Variables:** Destination `to` paths support `{year}`, `{month}`, `{day}`, and `{date}` (expanded based on file modification date).
* **Size Filters:** Rules accept `min_size` and `max_size` (e.g. `500MB`, `10KB`, `1GB`).
* **Startup Sweep:** On startup, Harbor scans existing files and folders, organizes non-conforming items, then starts watching.
* **Cooldown:** Waits for file size to stop changing. No moving half-downloaded files.
* **Dedup:** Conflict? Harbor renames to `name_1.ext`. Or set `overwrite = true` to overwrite.
* **Directories:** Set `include_dirs = true` to also organize folders.
* **Cross-device:** Uses atomic `rename`. Falls back to copy+delete across partitions.
* **Ignore:** Skips `*.part` or `*.tmp` out of the box.

## Contribute

Keep it small. Standard library first. No unnecessary abstractions.
```sh
cargo test
cargo clippy
```

## License

MIT. Use it. Fork it. Break it. Fix it.
