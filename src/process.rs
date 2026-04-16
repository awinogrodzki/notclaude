use std::mem;
use std::path::Path;

/// Get the parent PID of a given process using `proc_pidinfo`.
pub fn get_ppid(pid: i32) -> Option<i32> {
    let mut info: libc::proc_bsdinfo = unsafe { mem::zeroed() };
    let size = mem::size_of::<libc::proc_bsdinfo>() as i32;
    let result = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size,
        )
    };
    if result <= 0 {
        None
    } else {
        Some(info.pbi_ppid as i32)
    }
}

/// Get the executable path of a process using `proc_pidpath`.
pub fn get_process_path(pid: i32) -> Option<String> {
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let result = unsafe {
        libc::proc_pidpath(pid, buf.as_mut_ptr() as *mut libc::c_void, buf.len() as u32)
    };
    if result <= 0 {
        return None;
    }
    Some(String::from_utf8_lossy(&buf[..result as usize]).to_string())
}

/// Extract the `.app` bundle path from an executable path.
///
/// Given `/Applications/iTerm.app/Contents/MacOS/iTerm2`, returns
/// `/Applications/iTerm.app`. Returns the *first* `.app` component
/// so that nested helper apps (e.g. `Code Helper.app` inside
/// `Visual Studio Code.app`) resolve to the outermost bundle.
pub fn extract_app_bundle_path(exe_path: &str) -> Option<String> {
    let marker = ".app/";
    if let Some(idx) = exe_path.find(marker) {
        return Some(exe_path[..idx + 4].to_string()); // include ".app"
    }
    // Path ends with .app (no trailing slash)
    if exe_path.ends_with(".app") {
        return Some(exe_path.to_string());
    }
    None
}

/// Read `CFBundleIdentifier` from an app bundle's `Info.plist`.
pub fn read_bundle_id(app_bundle_path: &str) -> Option<String> {
    let plist_path = Path::new(app_bundle_path)
        .join("Contents")
        .join("Info.plist");
    let value = plist::Value::from_file(&plist_path).ok()?;
    let dict = value.as_dictionary()?;
    dict.get("CFBundleIdentifier")?
        .as_string()
        .map(|s| s.to_string())
}

/// Return the bundle identifier of the terminal/IDE hosting this session.
///
/// Primary method: read the `__CFBundleIdentifier` environment variable
/// that macOS sets for processes launched from a `.app` bundle (inherited
/// by all children).
///
/// Fallback: walk the process tree via `proc_pidinfo`.  This can fail on
/// macOS 15+ when a privileged process (e.g. `login`) sits between the
/// shell and the terminal app, because `proc_pidinfo` requires entitlements
/// to inspect processes owned by other users.
pub fn find_parent_app_bundle_id() -> Option<String> {
    // Fast path: env var set by LaunchServices, inherited by children.
    if let Ok(bundle_id) = std::env::var("__CFBundleIdentifier") {
        if !bundle_id.is_empty() {
            return Some(bundle_id);
        }
    }

    // Fallback: walk the process tree.
    find_parent_app_bundle_id_via_proctree()
}

fn find_parent_app_bundle_id_via_proctree() -> Option<String> {
    let mut pid = std::process::id() as i32;

    // Walk up at most 20 levels to avoid infinite loops
    for _ in 0..20 {
        pid = get_ppid(pid)?;
        if pid <= 1 {
            return None; // reached launchd / kernel
        }
        if let Some(path) = get_process_path(pid) {
            if let Some(app_path) = extract_app_bundle_path(&path) {
                if let Some(bundle_id) = read_bundle_id(&app_path) {
                    return Some(bundle_id);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- extract_app_bundle_path -------------------------------------------

    #[test]
    fn extract_standard_app_path() {
        assert_eq!(
            extract_app_bundle_path("/Applications/iTerm.app/Contents/MacOS/iTerm2"),
            Some("/Applications/iTerm.app".into())
        );
    }

    #[test]
    fn extract_nested_helper_returns_outermost() {
        let path = "/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper.app/Contents/MacOS/Code Helper";
        assert_eq!(
            extract_app_bundle_path(path),
            Some("/Applications/Visual Studio Code.app".into())
        );
    }

    #[test]
    fn extract_bare_app_path() {
        assert_eq!(
            extract_app_bundle_path("/Applications/Ghostty.app"),
            Some("/Applications/Ghostty.app".into())
        );
    }

    #[test]
    fn extract_no_app_returns_none() {
        assert_eq!(extract_app_bundle_path("/usr/bin/zsh"), None);
    }

    #[test]
    fn extract_home_dir_app() {
        assert_eq!(
            extract_app_bundle_path("/Users/user/Applications/Warp.app/Contents/MacOS/stable"),
            Some("/Users/user/Applications/Warp.app".into())
        );
    }

    #[test]
    fn extract_spaces_in_path() {
        assert_eq!(
            extract_app_bundle_path("/Applications/My App.app/Contents/MacOS/myapp"),
            Some("/Applications/My App.app".into())
        );
    }

    // -- read_bundle_id ----------------------------------------------------

    #[test]
    fn read_bundle_id_from_temp_plist() {
        let dir = tempfile::tempdir().unwrap();
        let contents = dir.path().join("Test.app").join("Contents");
        std::fs::create_dir_all(&contents).unwrap();

        let mut dict = plist::Dictionary::new();
        dict.insert(
            "CFBundleIdentifier".into(),
            plist::Value::String("com.test.myapp".into()),
        );
        plist::Value::Dictionary(dict)
            .to_file_xml(contents.join("Info.plist"))
            .unwrap();

        let app_path = dir.path().join("Test.app");
        assert_eq!(
            read_bundle_id(app_path.to_str().unwrap()),
            Some("com.test.myapp".into())
        );
    }

    #[test]
    fn read_bundle_id_missing_plist() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_bundle_id(dir.path().to_str().unwrap()), None);
    }

    #[test]
    fn read_bundle_id_missing_key() {
        let dir = tempfile::tempdir().unwrap();
        let contents = dir.path().join("Test.app").join("Contents");
        std::fs::create_dir_all(&contents).unwrap();

        let dict = plist::Dictionary::new(); // empty — no CFBundleIdentifier
        plist::Value::Dictionary(dict)
            .to_file_xml(contents.join("Info.plist"))
            .unwrap();

        let app_path = dir.path().join("Test.app");
        assert_eq!(read_bundle_id(app_path.to_str().unwrap()), None);
    }

    // -- get_ppid / get_process_path ---------------------------------------

    #[test]
    fn get_ppid_of_current_process() {
        let pid = std::process::id() as i32;
        let ppid = get_ppid(pid);
        assert!(ppid.is_some(), "should resolve parent of current process");
        assert!(ppid.unwrap() > 0);
    }

    #[test]
    fn get_ppid_invalid_pid() {
        assert!(get_ppid(-1).is_none());
    }

    #[test]
    fn get_ppid_nonexistent_pid() {
        // PID 99999999 almost certainly doesn't exist
        assert!(get_ppid(99_999_999).is_none());
    }

    #[test]
    fn get_process_path_of_current_process() {
        let pid = std::process::id() as i32;
        let path = get_process_path(pid);
        assert!(path.is_some(), "should resolve path of current process");
        assert!(!path.unwrap().is_empty());
    }

    #[test]
    fn get_process_path_invalid_pid() {
        assert!(get_process_path(-1).is_none());
    }

    // -- find_parent_app_bundle_id -----------------------------------------

    #[test]
    fn find_parent_returns_something_or_none() {
        // In a GUI terminal this returns Some; in headless CI it may be None.
        // Either outcome is acceptable — we just verify it doesn't panic.
        let _result = find_parent_app_bundle_id();
    }
}
