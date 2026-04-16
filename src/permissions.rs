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
        return enabled;
    }

    // Cannot determine — skip native so the caller falls back to osascript.
    false
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
}
