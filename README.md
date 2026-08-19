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

Give it a config.
```sh
harbor ~/.config/harbor/config.toml
```
No config path? Reads `~/.config/harbor/config.toml` by default.
It runs in the foreground. Use `systemd` to keep it alive in the background.

## Configure

See [`config.toml.example`](config.toml.example) for a full setup.

Rules match top to bottom. First match wins.

```toml
[defaults]
dest = "$HOME/Downloads/organized"
cooldown_secs = 5

[[watch]]
dir = "$HOME/Downloads"

  [[watch.rules]]
  pattern = "*.{jpg,png}"
  dest = "$HOME/Pictures"

  [[watch.rules]]
  pattern = "*"
  dest = "$HOME/Misc"
```

## Core Behavior

* **Cooldown:** Waits for file size to stop changing. No moving half-downloaded files.
* **Dedup:** Conflict? Harbor renames to `name_1.ext`. Or set `dedup = false` to overwrite.
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
