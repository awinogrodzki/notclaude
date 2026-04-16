---
description: Check notclaude notification hook status
---

Check the current status of notclaude notifications. Follow these steps:

## Step 1: Find the notclaude binary

Check these locations:
1. `which notclaude`
2. `~/.notclaude/bin/notclaude`
3. `~/.cargo/bin/notclaude`

If none exist, report that notclaude is not installed and suggest running `/notclaude:setup` to install it.

## Step 2: Show status

Run: `<NOTCLAUDE_BIN> status`

Report the output, which shows installation state for both global and project scopes.
