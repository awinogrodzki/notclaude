use serde::Deserialize;
use std::io::{self, Read};

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

/// Bundle ID used when we cannot attribute the notification to a real app.
/// Clicking the notification does nothing because no installed app matches.
const UNFOCUSABLE_BUNDLE_ID: &str = "com.notclaude.notification";

/// Send a macOS notification.
///
/// When `bundle_id` is provided *and* the target app has notification
/// permissions, the notification is attributed to that app so clicking
/// it focuses the window. Otherwise a synthetic bundle ID is used —
/// clicking the notification does nothing.
pub fn send_notification(title: &str, message: &str, bundle_id: Option<&str>) -> bool {
    // If the parent app has notification permissions, attribute the
    // notification to it so clicking focuses that app.
    if let Some(bid) = bundle_id {
        if crate::permissions::ensure_authorized(bid) {
            let _ = mac_notification_sys::set_application(bid);
            if send_mac_notification(title, message) {
                return true;
            }
        }
    }

    // No focusable source. Use a synthetic bundle ID — clicking the
    // notification does nothing because no installed app matches it.
    let _ = mac_notification_sys::set_application(UNFOCUSABLE_BUNDLE_ID);
    send_mac_notification(title, message)
}

fn send_mac_notification(title: &str, message: &str) -> bool {
    mac_notification_sys::Notification::new()
        .title(title)
        .message(message)
        .sound("Ping")
        .send()
        .is_ok()
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
    fn send_notification_unfocusable_fallback() {
        // With bundle_id = None, sends with synthetic bundle ID.
        if cfg!(target_os = "macos") {
            let result = send_notification("Test", "Hello from notclaude tests", None);
            assert!(result);
        }
    }

    #[test]
    fn send_mac_notification_directly() {
        if cfg!(target_os = "macos") {
            let _ = mac_notification_sys::set_application(UNFOCUSABLE_BUNDLE_ID);
            assert!(send_mac_notification("Test", "unfocusable notification test"));
        }
    }
}
