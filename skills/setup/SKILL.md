---
name: setup
description: Install notclaude macOS notifications for this project
---

Set up notclaude to receive macOS desktop notifications when Claude Code needs attention (permission prompts, idle prompts). Follow these steps in order:

## Step 1: Check if notclaude is already installed

Check these locations for an existing binary:
1. Run `which notclaude` to check PATH
2. Check if `~/.notclaude/bin/notclaude` exists
3. Check if `~/.cargo/bin/notclaude` exists

If any returns a valid path, store it as NOTCLAUDE_BIN and skip to Step 3.

## Step 2: Install the binary

This tool only works on macOS. Verify with `uname -s`.

### Option A: Download pre-built binary (preferred — no Rust required)

1. Detect architecture: `uname -m` (expect `arm64` or `x86_64`)
2. Create install directory: `mkdir -p ~/.notclaude/bin`
3. Download:
   - arm64: `curl -fsSL "https://github.com/awinogrodzki/notclaude/releases/latest/download/notclaude-darwin-arm64" -o ~/.notclaude/bin/notclaude`
   - x86_64: `curl -fsSL "https://github.com/awinogrodzki/notclaude/releases/latest/download/notclaude-darwin-x86_64" -o ~/.notclaude/bin/notclaude`
4. `chmod +x ~/.notclaude/bin/notclaude`
5. Verify it runs: `~/.notclaude/bin/notclaude status`
6. Set NOTCLAUDE_BIN=`~/.notclaude/bin/notclaude`

If the download or verification fails, continue to Option B.

### Option B: Build from source (requires Rust)

1. Check if cargo is available: `which cargo`
2. If available: `cargo install --git https://github.com/awinogrodzki/notclaude`
3. Set NOTCLAUDE_BIN to the cargo bin path (usually `~/.cargo/bin/notclaude`)

If cargo is not available, tell the user:
- "notclaude requires a pre-built binary or Rust toolchain to install."
- "Either create a GitHub release (see the repo's release workflow) or install Rust via https://rustup.rs"
- Stop here.

## Step 3: Install hooks for this project

Run: `<NOTCLAUDE_BIN> install --project`

This adds notification hooks to `.claude/settings.json` in the current project.

## Step 4: Verify and report

Run: `<NOTCLAUDE_BIN> status`

Report the result. On success, tell the user:
- They will now receive macOS notifications when Claude Code needs permission or is idle
- Clicking a notification will focus the terminal/IDE window
- They can check status anytime with `/notclaude:status`
- To remove: use the `/notclaude:teardown` skill
