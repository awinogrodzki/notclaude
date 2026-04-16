# notclaude

Native macOS desktop notifications for [Claude Code](https://docs.anthropic.com/en/docs/claude-code) hooks. Get alerted when Claude needs your attention — no more staring at the terminal.

Inspired by [this post](https://alexop.dev/posts/claude-code-notification-hooks/), reimplemented in Rust for instant startup and zero runtime dependencies.

## Features

- Sends native macOS notifications (with sound) when Claude Code:
  - Needs **permission** to proceed
  - Is **idle** and waiting for your input
- One-command install/uninstall for global or per-project scope
- Merges cleanly into existing `settings.json` without clobbering other config
- Idempotent — safe to run `install` multiple times

## Install

### As a Claude Code plugin (recommended)

Install the plugin in any project — no Rust toolchain required if pre-built binaries are available:

```sh
# In Claude Code, run:
/plugin marketplace add awinogrodzki/notclaude
/plugin install notclaude@notclaude
```

Then use the `/notclaude:setup` skill to install notifications for the current project. The setup will automatically download a pre-built binary or build from source as a fallback.

### Via install script

```sh
curl -fsSL https://raw.githubusercontent.com/awinogrodzki/notclaude/main/scripts/install.sh | bash
```

### From source

Requires [Rust toolchain](https://rustup.rs/).

```sh
cargo install --git https://github.com/awinogrodzki/notclaude
```

Or clone and build locally:

```sh
cargo install --path .
```

## Usage

### Plugin skills

When installed as a Claude Code plugin, the following skills are available:

| Skill | Description |
|-------|-------------|
| `/notclaude:setup` | Install notclaude and configure hooks for the current project |
| `/notclaude:teardown` | Remove notification hooks from the current project |
| `/notclaude:notification-status` | Check installation status |

### Configure the hook

Install globally (applies to all projects):

```sh
notclaude install --global
```

Or for the current project only:

```sh
notclaude install --project
```

This writes the hook configuration into the appropriate `.claude/settings.json`.

### Check status

```sh
notclaude status
```

```
Global:  Installed (/Users/you/.claude/settings.json)
Project: Not found (.claude/settings.json)
```

### Remove the hook

```sh
notclaude uninstall --global
notclaude uninstall --project
```

### Manual / direct use

The hook handler reads JSON from stdin and sends a notification:

```sh
echo '{"notification_type":"permission_prompt","message":"Allow file write?"}' | notclaude hook
```

## How it works

Claude Code [hooks](https://docs.anthropic.com/en/docs/claude-code/hooks) are shell commands that run at specific lifecycle events. `notclaude install` adds a hook entry that calls `notclaude hook` whenever a `permission_prompt` or `idle_prompt` event fires.

The hook handler:

1. Reads JSON from stdin (provided by Claude Code)
2. Matches on `notification_type`
3. Sends a native macOS notification via `osascript` with a "Ping" sound

### Generated settings.json entry

```json
{
  "hooks": {
    "Notification": [
      {
        "matcher": "permission_prompt|idle_prompt",
        "hooks": [
          {
            "type": "command",
            "command": "/Users/you/.cargo/bin/notclaude hook",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

## Development

```sh
cargo test        # run all tests
cargo build       # debug build
cargo run -- status  # run locally without installing
```

## License

MIT
