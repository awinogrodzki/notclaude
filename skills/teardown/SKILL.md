---
name: teardown
description: Remove notclaude notifications from this project
---

Remove notclaude notification hooks from the current project. Follow these steps:

## Step 1: Find the notclaude binary

Check these locations:
1. `which notclaude`
2. `~/.notclaude/bin/notclaude`
3. `~/.cargo/bin/notclaude`

If none exist, check if `.claude/settings.json` has any notclaude hook entries. If it does, tell the user the binary is missing but offer to manually remove the hook entries from `.claude/settings.json` (remove entries in the `hooks.Notification` array where a hook command contains "notclaude").

## Step 2: Uninstall project hooks

Run: `<NOTCLAUDE_BIN> uninstall --project`

## Step 3: Verify

Run: `<NOTCLAUDE_BIN> status`

Report that notifications have been removed for this project. Mention that:
- Global hooks (if any) are not affected — use `notclaude uninstall --global` to remove those
- The binary itself is still installed — to fully remove it, delete it from its install location
