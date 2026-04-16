# notclaude

Rust CLI that sends macOS desktop notifications for Claude Code hooks.

## Build & test

```sh
cargo build          # debug
cargo build --release
cargo test           # all unit tests
cargo install --path . # install to ~/.cargo/bin
```

## Architecture

Single binary, three modules:

- `src/notification.rs` — Hook handler: parses JSON from stdin, sends macOS notification via `osascript`. Handles `permission_prompt` and `idle_prompt` types.
- `src/config.rs` — Reads/writes Claude Code settings files. Merges hook config without clobbering existing settings. Supports global (`~/.claude/settings.json`) and project (`.claude/settings.local.json`) scopes.
- `src/main.rs` — CLI entry point using `clap` derive. Subcommands: `hook`, `install`, `uninstall`, `status`.

## Key design decisions

- `install` is idempotent — deduplicates by detecting existing `notclaude` entries
- Hook command uses the absolute path to the binary at install time
- `uninstall` cleans up empty `hooks` objects to avoid leaving noise in settings
- All JSON manipulation preserves unknown fields (forward-compatible)

## Plugin

This repo doubles as a Claude Code plugin. The plugin structure:

- `.claude-plugin/plugin.json` — Plugin manifest (name, version, author, repo)
- `.claude-plugin/marketplace.json` — Marketplace registration
- `skills/setup/SKILL.md` — `/notclaude:setup` skill: installs binary + configures project hooks
- `skills/teardown/SKILL.md` — `/notclaude:teardown` skill: removes project hooks
- `skills/notification-status/SKILL.md` — `/notclaude:notification-status` skill: shows hook status
- `scripts/install.sh` — Standalone install script (curl-pipeable)
- `.github/workflows/release.yml` — Builds macOS arm64/x86_64 binaries on tag push

Binary distribution strategy: pre-built GitHub Release binaries (no Rust needed), with `cargo install` fallback.

## Testing

Tests live alongside their modules (`#[cfg(test)]`). Config tests use `tempfile` crate for isolated filesystem operations. Notification parsing and routing are tested exhaustively; `send_notification` has one integration test that actually fires `osascript` on macOS.
