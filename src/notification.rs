use serde::Deserialize;
use std::io::{self, Read};
use std::process::Command;

#[derive(Debug, Deserialize)]
pub struct HookInput {
    pub notification_type: Option<String>,
    pub message: Option<String>,
}

pub fn read_hook_input() -> Option<HookInput> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).ok()?;
    serde_json::from_str(&buf).ok()
}

/// Send a macOS notification. When `bundle_id` is provided, the notification
/// is attributed to that application so that clicking it causes macOS to
/// activate (focus) the app — even if it's minimised or on another desktop.
///
/// Falls back to plain `osascript` when the native path is unavailable.
pub fn send_notification(title: &str, message: &str, bundle_id: Option<&str>) -> bool {
    if let Some(bid) = bundle_id {
        if send_native(title, message, bid) {
            return true;
        }
    }
    send_osascript(title, message)
}

/// Send via `mac-notification-sys` which swizzles `NSBundle.bundleIdentifier`
/// so macOS attributes the notification to the target app.
fn send_native(title: &str, message: &str, bundle_id: &str) -> bool {
    // set_application is internally call_once — safe to call repeatedly but
    // only the first invocation takes effect.
    if mac_notification_sys::set_application(bundle_id).is_err() {
        return false;
    }

    mac_notification_sys::Notification::new()
        .title(title)
        .message(message)
        .sound("Ping")
        .send()
        .is_ok()
}

/// Fallback: fire-and-forget via osascript (no click-to-focus).
fn send_osascript(title: &str, message: &str) -> bool {
    let escaped_title = title.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_message = message.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        "display notification \"{}\" with title \"{}\" sound name \"Ping\"",
        escaped_message, escaped_title
    );

    Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .is_ok_and(|o| o.status.success())
}

pub fn handle_hook(input: &HookInput) -> Option<(&str, &str)> {
    let notification_type = input.notification_type.as_deref()?;
    match notification_type {
        "permission_prompt" => Some((
            "Claude Code - Permission Required",
            input
                .message
                .as_deref()
                .unwrap_or("Claude needs your permission to continue"),
        )),
        "idle_prompt" => Some((
            "Claude Code - Waiting",
            input
                .message
                .as_deref()
                .unwrap_or("Claude is waiting for your input"),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- JSON parsing ------------------------------------------------------

    #[test]
    fn parse_permission_prompt() {
        let json = r#"{"notification_type": "permission_prompt", "message": "Allow file write?"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.notification_type.as_deref(), Some("permission_prompt"));
        assert_eq!(input.message.as_deref(), Some("Allow file write?"));
    }

    #[test]
    fn parse_idle_prompt() {
        let json = r#"{"notification_type": "idle_prompt", "message": "Waiting for input"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.notification_type.as_deref(), Some("idle_prompt"));
        assert_eq!(input.message.as_deref(), Some("Waiting for input"));
    }

    #[test]
    fn parse_missing_fields() {
        let json = r#"{}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert!(input.notification_type.is_none());
        assert!(input.message.is_none());
    }

    #[test]
    fn parse_extra_fields_ignored() {
        let json =
            r#"{"notification_type": "idle_prompt", "message": "hi", "extra": "ignored"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.notification_type.as_deref(), Some("idle_prompt"));
    }

    #[test]
    fn parse_invalid_json_fails() {
        let result: Result<HookInput, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    // -- handle_hook routing -----------------------------------------------

    #[test]
    fn handle_permission_prompt_with_message() {
        let input = HookInput {
            notification_type: Some("permission_prompt".into()),
            message: Some("Allow file write?".into()),
        };
        let (title, msg) = handle_hook(&input).unwrap();
        assert_eq!(title, "Claude Code - Permission Required");
        assert_eq!(msg, "Allow file write?");
    }

    #[test]
    fn handle_permission_prompt_default_message() {
        let input = HookInput {
            notification_type: Some("permission_prompt".into()),
            message: None,
        };
        let (title, msg) = handle_hook(&input).unwrap();
        assert_eq!(title, "Claude Code - Permission Required");
        assert_eq!(msg, "Claude needs your permission to continue");
    }

    #[test]
    fn handle_idle_prompt_with_message() {
        let input = HookInput {
            notification_type: Some("idle_prompt".into()),
            message: Some("Done, need input".into()),
        };
        let (title, msg) = handle_hook(&input).unwrap();
        assert_eq!(title, "Claude Code - Waiting");
        assert_eq!(msg, "Done, need input");
    }

    #[test]
    fn handle_idle_prompt_default_message() {
        let input = HookInput {
            notification_type: Some("idle_prompt".into()),
            message: None,
        };
        let (title, msg) = handle_hook(&input).unwrap();
        assert_eq!(title, "Claude Code - Waiting");
        assert_eq!(msg, "Claude is waiting for your input");
    }

    #[test]
    fn handle_unknown_type_returns_none() {
        let input = HookInput {
            notification_type: Some("something_else".into()),
            message: Some("hi".into()),
        };
        assert!(handle_hook(&input).is_none());
    }

    #[test]
    fn handle_missing_type_returns_none() {
        let input = HookInput {
            notification_type: None,
            message: Some("hi".into()),
        };
        assert!(handle_hook(&input).is_none());
    }

    // -- send_notification -------------------------------------------------

    #[test]
    fn send_notification_osascript_fallback() {
        // With bundle_id = None, falls back to osascript
        if cfg!(target_os = "macos") {
            let result = send_notification("Test", "Hello from notclaude tests", None);
            assert!(result);
        }
    }

    #[test]
    fn send_osascript_directly() {
        if cfg!(target_os = "macos") {
            assert!(send_osascript("Test", "osascript fallback test"));
        }
    }
}
