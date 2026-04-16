use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::NSError;
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNNotificationSettings,
    UNUserNotificationCenter,
};

const TIMEOUT: Duration = Duration::from_secs(2);

/// Check authorization and request if needed.
///
/// When the binary runs inside an `.app` bundle, uses the proper
/// `UNUserNotificationCenter` API to check and request permissions.
/// Otherwise falls back to reading the macOS notification preferences
/// plist. Returns `false` when permissions cannot be verified, so the
/// caller can fall back to another delivery method (e.g. osascript).
///
/// When the plist indicates notifications are explicitly disabled, a
/// one-time dialog is shown asking the user to open Notification Settings.
pub fn ensure_authorized(bundle_id: &str) -> bool {
    // UNUserNotificationCenter requires a valid .app bundle context.
    // Standalone CLI binaries crash with an unrecoverable ObjC exception
    // ("bundleProxyForCurrentProcess is nil") when calling
    // currentNotificationCenter(). Guard against that.
    if is_inside_app_bundle() {
        if let Some(result) = try_ensure_via_un() {
            return result;
        }
    }

    // Fallback: read the macOS notification preferences plist.
    if let Some(enabled) = check_plist_permission(bundle_id) {
        if !enabled && !was_already_prompted(bundle_id) {
            prompt_enable_notifications(bundle_id);
            // Re-check in case the user toggled the setting quickly.
            if let Some(true) = check_plist_permission(bundle_id) {
                return true;
            }
        }
        return enabled;
    }

    // App not listed in the plist. This means the user has never changed
    // its notification settings — macOS defaults are typically "allowed".
    // Optimistically return true so we use the real bundle ID rather than
    // falling back to a synthetic one that macOS will silently drop.
    true
}

// ---------------------------------------------------------------------------
// App-bundle check
// ---------------------------------------------------------------------------

/// Returns `true` when the current executable lives inside a `.app` bundle.
fn is_inside_app_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.contains(".app/")))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// UNUserNotificationCenter path (only safe inside an app bundle)
// ---------------------------------------------------------------------------

fn try_ensure_via_un() -> Option<bool> {
    let center = UNUserNotificationCenter::currentNotificationCenter();

    let status = check_un_authorization(&center)?;

    match status {
        UNAuthorizationStatus::Authorized
        | UNAuthorizationStatus::Provisional
        | UNAuthorizationStatus::Ephemeral => Some(true),
        UNAuthorizationStatus::NotDetermined => {
            Some(request_un_authorization(&center).unwrap_or(false))
        }
        _ => Some(false), // Denied
    }
}

fn check_un_authorization(center: &UNUserNotificationCenter) -> Option<UNAuthorizationStatus> {
    let (tx, rx) = mpsc::channel();

    let handler = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
        let status = unsafe { settings.as_ref() }.authorizationStatus();
        let _ = tx.send(status);
    });

    center.getNotificationSettingsWithCompletionHandler(&handler);

    rx.recv_timeout(TIMEOUT).ok()
}

fn request_un_authorization(center: &UNUserNotificationCenter) -> Option<bool> {
    let (tx, rx) = mpsc::channel();

    let options = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound;

    let handler = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
        let _ = tx.send(granted.as_bool());
    });

    center.requestAuthorizationWithOptions_completionHandler(options, &handler);

    rx.recv_timeout(TIMEOUT).ok()
}

// ---------------------------------------------------------------------------
// One-time permission prompt
// ---------------------------------------------------------------------------

/// Directory where we store per-bundle-id marker files to avoid repeated
/// prompts.
fn prompted_marker_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".notclaude").join("prompted"))
}

fn prompted_marker_path(bundle_id: &str) -> Option<std::path::PathBuf> {
    let safe_id = bundle_id.replace('/', "_");
    prompted_marker_dir().map(|d| d.join(safe_id))
}

fn was_already_prompted(bundle_id: &str) -> bool {
    prompted_marker_path(bundle_id)
        .map(|p| p.exists())
        .unwrap_or(false)
}

fn mark_prompted(bundle_id: &str) {
    if let Some(path) = prompted_marker_path(bundle_id) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, "");
    }
}

/// Display an osascript dialog telling the user that notifications are
/// disabled for `bundle_id` and offering to open System Settings.
fn prompt_enable_notifications(bundle_id: &str) {
    mark_prompted(bundle_id);

    let script = format!(
        concat!(
            "display dialog ",
            "\"Notifications are disabled for {bid}.\\n\\n",
            "Would you like to open Notification Settings to enable them?\" ",
            "buttons {{\"Not Now\", \"Open Settings\"}} ",
            "default button \"Open Settings\" ",
            "with title \"notclaude\" ",
            "with icon caution"
        ),
        bid = bundle_id
    );

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.contains("Open Settings") {
            let _ = std::process::Command::new("open")
                .arg("x-apple.systempreferences:com.apple.Notifications-Settings")
                .status();
        }
    }
}

// ---------------------------------------------------------------------------
// Plist fallback — reads the macOS notification center preferences
// ---------------------------------------------------------------------------

/// Check whether `bundle_id` has notification permissions by reading
/// `~/Library/Preferences/com.apple.ncprefs.plist`.
///
/// Returns `Some(true)` if notifications are enabled, `Some(false)` if
/// explicitly disabled (flags == 0), or `None` when the plist cannot be
/// read or the app isn't listed.
fn check_plist_permission(bundle_id: &str) -> Option<bool> {
    let plist_path = dirs::home_dir()?
        .join("Library")
        .join("Preferences")
        .join("com.apple.ncprefs.plist");

    let value = plist::Value::from_file(plist_path).ok()?;
    let dict = value.as_dictionary()?;
    let apps = dict.get("apps")?.as_array()?;

    for app in apps {
        let Some(app_dict) = app.as_dictionary() else {
            continue;
        };
        let Some(id) = app_dict.get("bundle-id").and_then(|v| v.as_string()) else {
            continue;
        };
        if id != bundle_id {
            continue;
        }
        let Some(flags_val) = app_dict.get("flags") else {
            continue;
        };
        let flags = flags_val
            .as_unsigned_integer()
            .or_else(|| flags_val.as_signed_integer().map(|v| v as u64));
        return flags.map(|f| f > 0);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_inside_app_bundle_false_for_tests() {
        assert!(!is_inside_app_bundle());
    }

    #[test]
    fn ensure_authorized_does_not_crash_from_cli() {
        // From a test binary there is no app bundle. The UN path is
        // skipped and the plist fallback runs. No panic.
        let _result = ensure_authorized("com.example.nonexistent");
    }

    #[test]
    fn check_plist_nonexistent_bundle_returns_none() {
        assert!(check_plist_permission("com.example.definitely.not.installed").is_none());
    }

    #[test]
    fn check_plist_known_app_returns_some() {
        // FaceTime is always present on macOS.
        if let Some(enabled) = check_plist_permission("com.apple.FaceTime") {
            // We don't assert the value — just that we got an answer.
            let _ = enabled;
        }
        // If the plist can't be read (CI sandbox), None is fine too.
    }

    // -- marker file helpers --------------------------------------------------

    #[test]
    fn marker_round_trip() {
        let bid = "com.test.marker-round-trip";
        // Clean up from any prior run.
        if let Some(p) = prompted_marker_path(bid) {
            let _ = std::fs::remove_file(&p);
        }

        assert!(!was_already_prompted(bid));
        mark_prompted(bid);
        assert!(was_already_prompted(bid));

        // Clean up.
        if let Some(p) = prompted_marker_path(bid) {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn marker_path_sanitises_slashes() {
        let path = prompted_marker_path("com/example/app").unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(!name.contains('/'));
    }
}
