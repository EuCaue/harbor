# harbor

Download organizer daemon: watch dirs, move/copy files into place per rules.
Written in Rust.

## Commands

- `cargo run` — build + run daemon (foreground)
- `cargo build --release` — release binary
- `cargo test` — run tests
- `cargo clippy` — lint before committing

## Layout

- `src/main.rs` — CLI entry, daemon bootstrap
- `src/config.rs` — TOML config parse (serde)
- `src/rules.rs` — pattern matching + dest resolution
- `src/daemon.rs` — watch loop, queue, cooldown
- `src/files.rs` — move/copy/dedup operations

## Architecture rules

- No tokio. Threads + `std::sync::mpsc` channel for the event queue.
- Logging = `println!` to stdout (systemd/journald owns rotation). Moves
  buffer and print as one grouped data table (flush every 5s or 50 rows).
- Table renderer is char-width based (multibyte filenames align). No unicode-width dep.
- Zero new dependencies unless approved in a plan. stdlib first.
- Move = `fs::rename`; on `EXDEV` (cross-device) fallback copy+remove.
- Dedup: dest exists → `name_1.ext`, `name_2.ext`... or overwrite when `dedup = false`.
- Cooldown: re-check file size/mtime each tick; reset timer if changed.
- First matching rule wins; `pattern = "*"` is the dir fallback.

## Config semantics

- Global `[defaults]` / `[ignore]` inherited by all `[[watch]]` dirs.
- `[watch.defaults]` overrides field-by-field; `[watch.ignore]` patterns
  append to global (does not replace).
- Always `[[watch]]` form, even for a single dir.
- `[watch.defaults] cooldown_secs = 0` = no cooldown for that dir.
- Patterns use globset syntax: `*.{jpg,png}`, `invoice-*`, `*`.
- `[[watch.rules]]` accepts optional `name` label (defaults to pattern); used in logs.
- `dir`/`dest` expand `$VAR` and `${VAR}` env vars; unknown vars stay literal.

## Conventions

- Terse code, no comments unless they explain a non-obvious decision.
- Mark deliberate shortcuts with a `// ponytail:` comment naming the ceiling.
- One runnable check (assert-based `#[test]` or small test fn) for any
  non-trivial logic: config parse, rule matching, dedup naming.
- CLI stays dumb: `harbor <config-path>` with plain args, no clap unless
  multi-command need appears.

## Install layout (deploy-time, not code)

- Binary → `/usr/local/bin/harbor`
- Config → `~/.config/harbor/config.toml`
- systemd unit → `~/.config/systemd/user/harbor.service` (`Restart=always`,
  logs via journald, `SIGHUP` reloads config)
