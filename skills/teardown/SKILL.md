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

If none exist, check if `.claude/settings.local.json` has any notclaude hook entries. If it does, tell the user the binary is missing but offer to manually remove the hook entries from `.claude/settings.local.json` (remove entries in the `hooks.Notification` array where a hook command contains "notclaude").

## Step 2: Uninstall project hooks

Run: `<NOTCLAUDE_BIN> uninstall --project`

## Step 3: Remove binary if installed in ~/.notclaude

If the binary was found at `~/.notclaude/bin/notclaude`, remove the `~/.notclaude` directory:

```
rm -rf ~/.notclaude
```

## Step 4: Verify

Run: `<NOTCLAUDE_BIN> status` (skip if the binary was removed in Step 3)

Report that notifications have been removed for this project. Mention that:
- Global hooks (if any) are not affected — use `notclaude uninstall --global` to remove those
- If the binary was installed via `cargo install`, it is still present at `~/.cargo/bin/notclaude` — to fully remove it, run `cargo uninstall notclaude`
