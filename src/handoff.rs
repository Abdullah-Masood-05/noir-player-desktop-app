//! Hands an all-users Windows copy over to a newer per-user copy.
//!
//! In-app updates never ask for administrator approval by default, so an
//! all-users (MSI, Program Files) copy updates by installing the new version
//! for the current user with the per-user `setup.exe`. The Start menu entry
//! the MSI created still opens the Program Files copy, so at startup that
//! copy looks for a newer per-user install and, if there is one, starts it
//! with the same arguments and exits.
//!
//! The per-user install is found through the uninstall entry the NSIS setup
//! writes in its `currentUser` mode, `HKCU\Software\Microsoft\Windows\
//! CurrentVersion\Uninstall\Noir Player`, which holds `DisplayVersion`,
//! `InstallLocation` (quoted) and `MainBinaryName`.
//!
//! Every failure here means "keep running this copy": a bug in this file
//! must never be the reason the app does not start.

// Only Windows gathers the facts; the decision itself is tested everywhere.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::path::{Path, PathBuf};

/// Set on the copy that was handed over to, so it never hands off again.
const HANDED_OFF_ENV: &str = "NOIR_PLAYER_HANDED_OFF";

/// A per-user install, as its uninstall entry describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerUserInstall {
    pub version: String,
    pub exe: PathBuf,
}

/// Everything the handoff decision depends on, gathered at startup.
#[derive(Clone, Debug)]
pub struct Facts<'a> {
    /// The version of the copy that is running now.
    pub running_version: &'a str,
    pub running_exe: &'a Path,
    /// The running copy sits in a folder this account cannot write, which is
    /// how an all-users install looks.
    pub per_machine: bool,
    pub elevated: bool,
    /// This process was itself started by a handoff.
    pub handed_off: bool,
    pub per_user: Option<PerUserInstall>,
    /// Whether `per_user.exe` exists as a file.
    pub per_user_exe_exists: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Start this executable with the same arguments, then exit.
    HandOff(PathBuf),
    Stay(StayReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StayReason {
    AlreadyHandedOff,
    /// A per-user copy never hands off; only an all-users copy does.
    NotPerMachine,
    /// An elevated process would pass its administrator token on.
    Elevated,
    NoPerUserInstall,
    /// The per-user copy is the same version or older, for example after a
    /// newer MSI was installed over the all-users copy.
    NotNewer,
    ExeMissing,
    /// The registered per-user copy is this very executable.
    SameExe,
}

/// Whether to start the per-user copy instead of this one. Only a strictly
/// newer per-user copy, whose executable exists and is not this one, is
/// started, and only from an all-users copy that is not elevated and was not
/// itself started by a handoff.
pub fn decide(facts: &Facts) -> Decision {
    if facts.handed_off {
        return Decision::Stay(StayReason::AlreadyHandedOff);
    }
    if !facts.per_machine {
        return Decision::Stay(StayReason::NotPerMachine);
    }
    if facts.elevated {
        return Decision::Stay(StayReason::Elevated);
    }
    let Some(per_user) = &facts.per_user else {
        return Decision::Stay(StayReason::NoPerUserInstall);
    };
    if !crate::update::is_newer(&per_user.version, facts.running_version) {
        return Decision::Stay(StayReason::NotNewer);
    }
    if !facts.per_user_exe_exists {
        return Decision::Stay(StayReason::ExeMissing);
    }
    if same_path(&per_user.exe, facts.running_exe) {
        return Decision::Stay(StayReason::SameExe);
    }
    Decision::HandOff(per_user.exe.clone())
}

/// Windows paths compare without regard to case or a `\\?\` prefix.
fn same_path(a: &Path, b: &Path) -> bool {
    let normal = |path: &Path| {
        let text = path.to_string_lossy().replace('/', "\\");
        text.strip_prefix(r"\\?\")
            .unwrap_or(&text)
            .trim_end_matches('\\')
            .to_ascii_lowercase()
    };
    normal(a) == normal(b)
}

/// The per-user install from the values of its uninstall entry. NSIS writes
/// `InstallLocation` wrapped in quotes.
pub fn per_user_install(
    version: Option<String>,
    location: Option<String>,
    main_binary: Option<String>,
) -> Option<PerUserInstall> {
    let version = version?.trim().to_string();
    let location = location?;
    let location = location.trim().trim_matches('"').trim();
    if version.is_empty() || location.is_empty() {
        return None;
    }
    let binary = main_binary
        .map(|name| name.trim().to_string())
        .filter(|name| {
            !name.is_empty()
                && !name.contains(['\\', '/'])
                && name.to_ascii_lowercase().ends_with(".exe")
        })
        .unwrap_or_else(|| "noir_player.exe".to_string());
    Some(PerUserInstall {
        version,
        exe: Path::new(location).join(binary),
    })
}

/// Starts a newer per-user copy and returns `true` when this process should
/// exit straight away. Runs before anything else at startup.
#[cfg(target_os = "windows")]
pub fn hand_off_to_newer_copy() -> bool {
    let handed_off = std::env::var_os(HANDED_OFF_ENV).is_some();
    // Cleared so nothing this copy starts later, such as the update helper
    // reopening the app, inherits it. No other threads exist yet.
    std::env::remove_var(HANDED_OFF_ENV);
    if handed_off {
        return false;
    }
    let Some(per_user) = windows::registered_per_user_install() else {
        return false;
    };
    let Ok(running_exe) = std::env::current_exe() else {
        return false;
    };
    let facts = Facts {
        running_version: env!("CARGO_PKG_VERSION"),
        running_exe: &running_exe,
        per_machine: windows::per_machine(),
        elevated: windows::is_elevated(),
        handed_off,
        per_user_exe_exists: per_user.exe.is_file(),
        per_user: Some(per_user),
    };
    match decide(&facts) {
        Decision::HandOff(exe) => windows::start(&exe),
        Decision::Stay(_) => false,
    }
}

/// The per-user install this account has registered, if any.
#[cfg(target_os = "windows")]
pub fn registered_per_user_install() -> Option<PerUserInstall> {
    windows::registered_per_user_install()
}

#[cfg(not(target_os = "windows"))]
pub fn hand_off_to_newer_copy() -> bool {
    false
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{per_user_install, PerUserInstall, HANDED_OFF_ENV};
    use std::path::Path;
    use std::time::{Duration, Instant};

    /// Written by the NSIS setup in `currentUser` mode, from `UNINSTKEY` in
    /// tauri-bundler's `installer.nsi`.
    const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Noir Player";

    pub fn registered_per_user_install() -> Option<PerUserInstall> {
        let key = uninstall_key();
        per_user_install(
            read_hkcu_string(&key, "DisplayVersion"),
            read_hkcu_string(&key, "InstallLocation"),
            read_hkcu_string(&key, "MainBinaryName"),
        )
    }

    /// Debug builds can point the lookup at a scratch key, so the handoff can
    /// be tried without installing anything. Release builds always read the
    /// real entry.
    fn uninstall_key() -> String {
        #[cfg(debug_assertions)]
        if let Some(key) = std::env::var("NOIR_HANDOFF_TEST_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
        {
            return key;
        }
        UNINSTALL_KEY.to_string()
    }

    pub fn per_machine() -> bool {
        #[cfg(debug_assertions)]
        if std::env::var_os("NOIR_HANDOFF_TEST_PER_MACHINE").is_some() {
            return true;
        }
        crate::update::install_scope() == crate::update::InstallScope::PerMachine
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn read_hkcu_string(subkey: &str, value: &str) -> Option<String> {
        use windows_sys::Win32::Foundation::ERROR_SUCCESS;
        use windows_sys::Win32::System::Registry::{
            RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ,
        };

        let subkey = wide(subkey);
        let value = wide(value);
        let mut size: u32 = 0;
        // SAFETY: both names are NUL terminated; a null buffer asks for the
        // size only.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS || size == 0 || size > 64 * 1024 {
            return None;
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
        let mut size = (buffer.len() * 2) as u32;
        // SAFETY: `buffer` holds `size` bytes.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16(&buffer[..len]).ok()
    }

    /// Whether this process runs with an elevated (administrator) token.
    /// Unknown counts as elevated, so a failed check never hands off.
    pub fn is_elevated() -> bool {
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
        use windows_sys::Win32::Security::{
            GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: the pseudo handle from GetCurrentProcess needs no closing;
        // the token handle is closed below.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return true;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut returned = 0u32;
        // SAFETY: `elevation` is the size passed in.
        let ok = unsafe {
            GetTokenInformation(
                token,
                TokenElevation,
                (&mut elevation as *mut TOKEN_ELEVATION).cast(),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut returned,
            )
        };
        // SAFETY: `token` came from OpenProcessToken.
        unsafe { CloseHandle(token) };
        ok == 0 || elevation.TokenIsElevated != 0
    }

    /// Starts `exe` with this process's arguments. Returns `true` once it is
    /// running, or exited cleanly within the first moment (as it does when
    /// it hands a launch to a window that is already open). A copy that
    /// cannot start, or fails right away, leaves this copy running instead.
    pub fn start(exe: &Path) -> bool {
        let mut child = match std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .env(HANDED_OFF_ENV, "1")
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                eprintln!("Could not start {}: {error}", exe.display());
                return false;
            }
        };
        let deadline = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => return true,
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>(running_exe: &'a Path, per_user_version: Option<&str>) -> Facts<'a> {
        Facts {
            running_version: "2.4.1",
            running_exe,
            per_machine: true,
            elevated: false,
            handed_off: false,
            per_user: per_user_version.map(|version| PerUserInstall {
                version: version.to_string(),
                exe: PathBuf::from(r"C:\Users\me\AppData\Local\Noir Player\noir_player.exe"),
            }),
            per_user_exe_exists: true,
        }
    }

    const MSI_EXE: &str = r"C:\Program Files\Noir Player\noir_player.exe";

    #[test]
    fn a_newer_per_user_copy_takes_over() {
        let exe = Path::new(MSI_EXE);
        assert_eq!(
            decide(&facts(exe, Some("2.5.0"))),
            Decision::HandOff(PathBuf::from(
                r"C:\Users\me\AppData\Local\Noir Player\noir_player.exe"
            ))
        );
    }

    #[test]
    fn an_older_or_equal_per_user_copy_is_ignored() {
        let exe = Path::new(MSI_EXE);
        // A newer MSI installed over the all-users copy wins over an older
        // per-user copy.
        for version in ["2.4.0", "2.4.1", "2.4.1-rc.1", "1.0.0", "not a version", ""] {
            assert_eq!(
                decide(&facts(exe, Some(version))),
                Decision::Stay(StayReason::NotNewer),
                "{version:?}"
            );
        }
    }

    #[test]
    fn a_missing_per_user_copy_is_ignored() {
        let exe = Path::new(MSI_EXE);
        assert_eq!(
            decide(&facts(exe, None)),
            Decision::Stay(StayReason::NoPerUserInstall)
        );
        let gone = Facts {
            per_user_exe_exists: false,
            ..facts(exe, Some("2.5.0"))
        };
        assert_eq!(decide(&gone), Decision::Stay(StayReason::ExeMissing));
    }

    #[test]
    fn an_elevated_copy_never_hands_off() {
        let exe = Path::new(MSI_EXE);
        let elevated = Facts {
            elevated: true,
            ..facts(exe, Some("9.0.0"))
        };
        assert_eq!(decide(&elevated), Decision::Stay(StayReason::Elevated));
    }

    #[test]
    fn a_copy_started_by_a_handoff_never_hands_off_again() {
        let exe = Path::new(MSI_EXE);
        let again = Facts {
            handed_off: true,
            ..facts(exe, Some("9.0.0"))
        };
        assert_eq!(decide(&again), Decision::Stay(StayReason::AlreadyHandedOff));
    }

    #[test]
    fn only_an_all_users_copy_hands_off() {
        let exe = Path::new(r"C:\Users\me\AppData\Local\Noir Player\noir_player.exe");
        let per_user = Facts {
            per_machine: false,
            ..facts(exe, Some("9.0.0"))
        };
        assert_eq!(decide(&per_user), Decision::Stay(StayReason::NotPerMachine));

        // Registered at the running executable itself, in any spelling.
        let itself = Facts {
            running_exe: Path::new(r"\\?\c:\users\ME\appdata\local\noir player\NOIR_PLAYER.EXE"),
            ..facts(exe, Some("9.0.0"))
        };
        assert_eq!(decide(&itself), Decision::Stay(StayReason::SameExe));
    }

    #[test]
    fn the_uninstall_entry_reads_as_an_install() {
        let some = |s: &str| Some(s.to_string());
        assert_eq!(
            per_user_install(
                some("2.5.0"),
                some(r#""C:\Users\me\AppData\Local\Noir Player""#),
                some("noir_player.exe"),
            ),
            Some(PerUserInstall {
                version: "2.5.0".to_string(),
                exe: PathBuf::from(r"C:\Users\me\AppData\Local\Noir Player\noir_player.exe"),
            })
        );
        // A missing or odd binary name falls back to the one the app ships.
        assert_eq!(
            per_user_install(some("2.5.0"), some(r"D:\Apps\Noir"), some(r"..\evil.exe"))
                .unwrap()
                .exe,
            PathBuf::from(r"D:\Apps\Noir\noir_player.exe")
        );
        assert_eq!(
            per_user_install(some("2.5.0"), some(r"D:\Apps\Noir"), None)
                .unwrap()
                .exe,
            PathBuf::from(r"D:\Apps\Noir\noir_player.exe")
        );
        assert_eq!(per_user_install(None, some(r"D:\Apps"), None), None);
        assert_eq!(per_user_install(some("2.5.0"), None, None), None);
        assert_eq!(per_user_install(some("2.5.0"), some(r#""""#), None), None);
    }
}
