use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

const HOOK_GROUP_NAME: &str = "Notification";

fn hook_command() -> String {
    // Resolve to the absolute path of the installed binary so the hook
    // works regardless of the user's $PATH at hook execution time.
    let bin = env::current_exe()
        .ok()
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "notclaude".into());
    format!("{bin} hook")
}

fn hook_entry() -> Value {
    json!({
        "matcher": "permission_prompt|idle_prompt",
        "hooks": [
            {
                "type": "command",
                "command": hook_command(),
                "timeout": 5
            }
        ]
    })
}

pub fn global_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

pub fn project_settings_path() -> PathBuf {
    PathBuf::from(".claude").join("settings.json")
}

fn read_settings(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_settings(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    let content =
        serde_json::to_string_pretty(value).map_err(|e| format!("Failed to serialize: {e}"))?;
    fs::write(path, content + "\n").map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

pub fn install(path: &Path) -> Result<(), String> {
    let mut settings = read_settings(path);
    let obj = settings
        .as_object_mut()
        .ok_or("settings.json root is not an object")?;

    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("\"hooks\" field is not an object")?;

    let group = hooks
        .entry(HOOK_GROUP_NAME)
        .or_insert_with(|| json!([]));

    let arr = group.as_array_mut().ok_or(format!(
        "\"hooks.{HOOK_GROUP_NAME}\" is not an array"
    ))?;

    // Remove any existing notclaude entries to avoid duplicates
    arr.retain(|entry| !is_notclaude_entry(entry));

    arr.push(hook_entry());

    write_settings(path, &settings)?;
    Ok(())
}

pub fn uninstall(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let mut settings = read_settings(path);
    let obj = settings
        .as_object_mut()
        .ok_or("settings.json root is not an object")?;

    let Some(hooks) = obj.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Ok(());
    };

    let Some(group) = hooks.get_mut(HOOK_GROUP_NAME).and_then(Value::as_array_mut) else {
        return Ok(());
    };

    group.retain(|entry| !is_notclaude_entry(entry));

    // Clean up empty structures
    if group.is_empty() {
        hooks.remove(HOOK_GROUP_NAME);
    }
    if hooks.is_empty() {
        obj.remove("hooks");
    }

    write_settings(path, &settings)
}

fn is_notclaude_entry(entry: &Value) -> bool {
    entry["hooks"]
        .as_array()
        .is_some_and(|hooks| {
            hooks.iter().any(|h| {
                h["command"]
                    .as_str()
                    .is_some_and(|cmd| cmd.contains("notclaude"))
            })
        })
}

pub fn is_installed(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    let settings = read_settings(path);
    settings["hooks"][HOOK_GROUP_NAME]
        .as_array()
        .is_some_and(|arr| arr.iter().any(|e| is_notclaude_entry(e)))
}

pub fn status(path: &Path) -> String {
    if is_installed(path) {
        format!("Installed ({})", path.display())
    } else if path.exists() {
        format!("Not installed ({})", path.display())
    } else {
        format!("Not found ({})", path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_settings(dir: &Path) -> PathBuf {
        let path = dir.join(".claude").join("settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        path
    }

    #[test]
    fn install_creates_new_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_settings(dir.path());

        install(&path).unwrap();

        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let group = settings["hooks"][HOOK_GROUP_NAME].as_array().unwrap();
        assert_eq!(group.len(), 1);
        assert_eq!(group[0]["matcher"], "permission_prompt|idle_prompt");
        assert!(is_notclaude_entry(&group[0]));
    }

    #[test]
    fn install_preserves_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_settings(dir.path());

        let existing = json!({
            "permissions": { "allow": ["Read"] },
            "hooks": {
                "Other": [{ "matcher": "something", "hooks": [] }]
            }
        });
        fs::write(&path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        install(&path).unwrap();

        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        // Existing permission preserved
        assert_eq!(settings["permissions"]["allow"][0], "Read");
        // Existing hook group preserved
        assert!(settings["hooks"]["Other"].as_array().unwrap().len() == 1);
        // Our group added
        assert!(settings["hooks"][HOOK_GROUP_NAME].as_array().unwrap().len() == 1);
    }

    #[test]
    fn install_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_settings(dir.path());

        install(&path).unwrap();
        install(&path).unwrap();
        install(&path).unwrap();

        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let group = settings["hooks"][HOOK_GROUP_NAME].as_array().unwrap();
        assert_eq!(group.len(), 1, "should not duplicate entries");
    }

    #[test]
    fn uninstall_removes_hook() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_settings(dir.path());

        install(&path).unwrap();
        assert!(is_installed(&path));

        uninstall(&path).unwrap();
        assert!(!is_installed(&path));

        // File should still be valid JSON
        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(settings.as_object().unwrap().get("hooks").is_none());
    }

    #[test]
    fn uninstall_preserves_other_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_settings(dir.path());

        let existing = json!({
            "hooks": {
                HOOK_GROUP_NAME: [
                    { "matcher": "idle_prompt", "hooks": [{ "type": "command", "command": "other-tool" }] }
                ]
            }
        });
        fs::write(&path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        install(&path).unwrap();
        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let group = settings["hooks"][HOOK_GROUP_NAME].as_array().unwrap();
        assert_eq!(group.len(), 2); // other-tool + notclaude

        uninstall(&path).unwrap();
        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let group = settings["hooks"][HOOK_GROUP_NAME].as_array().unwrap();
        assert_eq!(group.len(), 1);
        assert_eq!(group[0]["hooks"][0]["command"], "other-tool");
    }

    #[test]
    fn uninstall_nonexistent_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        assert!(uninstall(&path).is_ok());
    }

    #[test]
    fn is_installed_false_when_no_file() {
        let path = Path::new("/tmp/notclaude-nonexistent/settings.json");
        assert!(!is_installed(path));
    }

    #[test]
    fn hook_entry_structure() {
        let entry = hook_entry();
        assert_eq!(entry["matcher"], "permission_prompt|idle_prompt");
        let hooks = entry["hooks"].as_array().unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["type"], "command");
        assert_eq!(hooks[0]["timeout"], 5);
        assert!(hooks[0]["command"].as_str().unwrap().contains("notclaude"));
    }

    #[test]
    fn status_messages() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_settings(dir.path());

        // File doesn't exist yet
        let s = status(&path);
        assert!(s.starts_with("Not found"), "expected 'Not found', got: {s}");

        // Create empty settings (no hooks)
        fs::write(&path, "{}").unwrap();
        let s = status(&path);
        assert!(s.starts_with("Not installed"), "expected 'Not installed', got: {s}");

        install(&path).unwrap();
        let s = status(&path);
        assert!(s.starts_with("Installed"), "expected 'Installed', got: {s}");
    }

    #[test]
    fn is_notclaude_entry_detection() {
        let entry = json!({
            "matcher": "permission_prompt",
            "hooks": [{ "type": "command", "command": "notclaude hook" }]
        });
        assert!(is_notclaude_entry(&entry));

        let other = json!({
            "matcher": "permission_prompt",
            "hooks": [{ "type": "command", "command": "other-tool" }]
        });
        assert!(!is_notclaude_entry(&other));
    }
}
