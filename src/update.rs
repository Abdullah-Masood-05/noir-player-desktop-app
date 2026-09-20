//! Update checking, downloading and installing.
//!
//! The app asks GitHub Releases for the latest tag, picks the asset that
//! matches the running platform, downloads it with a progress callback,
//! verifies it against the release checksums, and hands it to the platform
//! installer. Installation always runs after the app exits, so the installer
//! can replace files that are in use.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const REPO: &str = "Abdullah-Masood-05/noir-player-desktop-app";
const USER_AGENT: &str = "NoirPlayer-Desktop";
/// Largest installer the updater will accept, well above any current asset.
const MAX_ASSET_BYTES: u64 = 300 * 1024 * 1024;
/// A checksum list is a few kilobytes at most.
const MAX_CHECKSUM_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub name: String,
    pub html_url: String,
    pub body: String,
    pub published_at: String,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

impl ReleaseInfo {
    /// The installer for the running platform, if the release published one.
    pub fn platform_asset(&self) -> Option<&ReleaseAsset> {
        platform_asset(&self.assets)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    /// `total` is zero while the server has not declared a content length.
    Downloading {
        received: u64,
        total: u64,
    },
    Verifying,
    /// The installer is downloaded and verified, waiting for a restart.
    Ready(PathBuf),
    Installing,
    Error(String),
}

pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

pub fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_version(remote), parse_version(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(5)
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(10)))
        .build()
        .into()
}

pub fn check_latest_release(timeout: Duration) -> Result<Option<ReleaseInfo>> {
    let current_version = env!("CARGO_PKG_VERSION");
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let mut response = agent(timeout)
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| anyhow!("Could not reach GitHub Releases: {e}"))?;

    let status = response.status().as_u16();
    if status == 404 {
        return Ok(None);
    }
    if status != 200 {
        bail!("GitHub returned HTTP {status} while checking for updates.");
    }

    let body = response
        .body_mut()
        .with_config()
        .limit(1_048_576)
        .read_to_string()
        .map_err(|e| anyhow!("Failed to read update response: {e}"))?;

    let json: Value =
        serde_json::from_str(&body).map_err(|e| anyhow!("Invalid JSON in update response: {e}"))?;
    let release = parse_release(&json)?;

    Ok(is_newer(&release.version, current_version).then_some(release))
}

fn parse_release(json: &Value) -> Result<ReleaseInfo> {
    let tag_name = json
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing tag_name in release response"))?
        .to_string();

    let version = tag_name.trim_start_matches(['v', 'V']).to_string();

    let assets = json
        .get("assets")
        .and_then(Value::as_array)
        .map(|assets| {
            assets
                .iter()
                .filter_map(|asset| {
                    Some(ReleaseAsset {
                        name: asset.get("name").and_then(Value::as_str)?.to_string(),
                        url: asset
                            .get("browser_download_url")
                            .and_then(Value::as_str)?
                            .to_string(),
                        size: asset.get("size").and_then(Value::as_u64).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(ReleaseInfo {
        name: json
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(&tag_name)
            .to_string(),
        html_url: json
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or(&format!("https://github.com/{REPO}/releases"))
            .to_string(),
        body: json
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        published_at: json
            .get("published_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        version,
        tag_name,
        assets,
    })
}

/// Extension of the package this Linux system can install, read from
/// `/etc/os-release`. Debian packages are the fallback because the project
/// builds on Ubuntu.
#[cfg(target_os = "linux")]
fn linux_package_extension() -> &'static str {
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let family = release.to_ascii_lowercase();
    if family.contains("arch") || family.contains("manjaro") {
        ".pkg.tar.zst"
    } else if family.contains("fedora")
        || family.contains("rhel")
        || family.contains("centos")
        || family.contains("suse")
    {
        ".rpm"
    } else {
        ".deb"
    }
}

/// Picks the release asset that this platform can install.
pub fn platform_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    #[cfg(target_os = "windows")]
    {
        windows_asset(assets, install_scope())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let find = |suffix: &str| {
            assets
                .iter()
                .find(|asset| asset.name.to_ascii_lowercase().ends_with(suffix))
        };
        #[cfg(target_os = "macos")]
        {
            find(".dmg")
        }
        #[cfg(target_os = "linux")]
        {
            find(linux_package_extension())
                .or_else(|| find(".deb"))
                .or_else(|| find(".rpm"))
        }
    }
}

/// How the running copy of the app was installed on Windows. The two
/// installers land in different places, so updating the copy the user
/// actually launches means matching the one it came from.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallScope {
    /// A per user NSIS install, under a directory this account can write.
    PerUser,
    /// A per machine MSI install, typically under Program Files.
    PerMachine,
}

/// Probes the directory the app runs from. A directory this account cannot
/// write to means a per machine install, which the MSI upgrades in place.
#[cfg(target_os = "windows")]
pub fn install_scope() -> InstallScope {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    else {
        return InstallScope::PerUser;
    };
    let probe = dir.join(".noir-update-probe");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            InstallScope::PerUser
        }
        Err(_) => InstallScope::PerMachine,
    }
}

#[cfg(target_os = "windows")]
pub fn windows_asset(assets: &[ReleaseAsset], scope: InstallScope) -> Option<&ReleaseAsset> {
    let find = |suffix: &str| {
        assets
            .iter()
            .find(|asset| asset.name.to_ascii_lowercase().ends_with(suffix))
    };
    match scope {
        // The NSIS setup installs per user and runs unattended with /S.
        InstallScope::PerUser => find("-setup.exe").or_else(|| find(".msi")),
        // Windows Installer upgrades the existing per machine install in
        // place, asking for elevation itself.
        InstallScope::PerMachine => find(".msi").or_else(|| find("-setup.exe")),
    }
}

/// Directory that holds downloaded installers between runs.
pub fn update_dir() -> Result<PathBuf> {
    let base = dirs::cache_dir()
        .or_else(dirs::data_local_dir)
        .context("No cache directory is available for downloaded updates")?;
    let dir = base.join("noir-player").join("updates");
    std::fs::create_dir_all(&dir).with_context(|| format!("Could not create {}", dir.display()))?;
    Ok(dir)
}

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Update cancelled.");
    }
    Ok(())
}

/// Downloads `asset` into `dir`, reporting `(received, total)` as it goes.
/// The file is written beside its final name and renamed once complete, so a
/// half-finished download is never mistaken for an installer.
pub fn download_asset(
    asset: &ReleaseAsset,
    dir: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(u64, u64),
) -> Result<PathBuf> {
    if asset.size > MAX_ASSET_BYTES {
        bail!("The update is larger than the 300 MiB limit.");
    }
    let destination = dir.join(&asset.name);
    let partial = dir.join(format!("{}.part", asset.name));

    let mut response = agent(Duration::from_secs(1800))
        .get(&asset.url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| anyhow!("Could not download the update: {e}"))?;

    if response.status().as_u16() != 200 {
        bail!("The download returned HTTP {}.", response.status().as_u16());
    }

    let total = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(asset.size);
    if total > MAX_ASSET_BYTES {
        bail!("The update is larger than the 300 MiB limit.");
    }

    let mut file = std::fs::File::create(&partial)
        .with_context(|| format!("Could not write to {}", partial.display()))?;
    let mut reader = response.body_mut().as_reader();
    let mut buffer = [0_u8; 128 * 1024];
    let mut received = 0_u64;

    let result = (|| -> Result<()> {
        loop {
            cancelled(cancel)?;
            let count = reader
                .read(&mut buffer)
                .map_err(|_| anyhow!("The download was interrupted."))?;
            if count == 0 {
                break;
            }
            received += count as u64;
            if received > MAX_ASSET_BYTES {
                bail!("The update is larger than the 300 MiB limit.");
            }
            file.write_all(&buffer[..count])
                .map_err(|_| anyhow!("Could not save the update; check free space."))?;
            progress(received, total);
        }
        file.sync_all()
            .map_err(|_| anyhow!("Could not flush the downloaded update."))?;
        if received == 0 || (total > 0 && received != total) {
            bail!("The download finished early and is incomplete.");
        }
        Ok(())
    })();

    drop(file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&partial);
        return Err(error);
    }

    let _ = std::fs::remove_file(&destination);
    std::fs::rename(&partial, &destination)
        .with_context(|| format!("Could not move the update into {}", destination.display()))?;
    prune_old_downloads(dir, &destination);
    Ok(destination)
}

/// Removes installers left behind by earlier updates, so the cache holds at
/// most the installer that is about to run.
fn prune_old_downloads(dir: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path != keep && path.is_file() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// SHA-256 of a file as lowercase hex.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        std::fs::File::open(path).with_context(|| format!("Could not read {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .with_context(|| format!("Could not read {}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Reads the expected checksum for `asset_name` from the release. Releases
/// publish a `<asset>.sha256` sidecar and a combined `SHA256SUMS`; either one
/// is accepted. Returns `None` when the release publishes neither.
pub fn expected_checksum(
    release: &ReleaseInfo,
    asset_name: &str,
    timeout: Duration,
) -> Result<Option<String>> {
    let sidecar = format!("{asset_name}.sha256");
    let source = release
        .assets
        .iter()
        .find(|asset| asset.name == sidecar)
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|asset| asset.name.eq_ignore_ascii_case("SHA256SUMS"))
        });
    let Some(source) = source else {
        return Ok(None);
    };

    let mut response = agent(timeout)
        .get(&source.url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| anyhow!("Could not download the update checksum: {e}"))?;
    if response.status().as_u16() != 200 {
        bail!("The checksum file returned HTTP {}.", response.status());
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_CHECKSUM_BYTES)
        .read_to_string()
        .map_err(|e| anyhow!("Could not read the update checksum: {e}"))?;

    Ok(find_checksum(&body, asset_name))
}

/// Pulls the digest for `asset_name` out of `sha256sum` style output. Lines
/// are `<hex>  <name>`; a sidecar for a single file may omit the name.
pub fn find_checksum(text: &str, asset_name: &str) -> Option<String> {
    let is_digest = |value: &str| value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit());

    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(digest) = parts.next() else {
            continue;
        };
        if !is_digest(digest) {
            continue;
        }
        match parts.next() {
            Some(name) if name.trim_start_matches('*').ends_with(asset_name) => {
                return Some(digest.to_ascii_lowercase())
            }
            // A sidecar holding nothing but the digest belongs to its asset.
            None => return Some(digest.to_ascii_lowercase()),
            _ => {}
        }
    }
    None
}

/// Starts the installer and returns once it is running detached from this
/// process. The caller is expected to quit immediately afterwards: the helper
/// waits for this process to exit before touching any installed files.
pub fn launch_installer(installer: &Path) -> Result<()> {
    let installer = plain_absolute(installer);
    if !installer.is_file() {
        bail!("The downloaded installer is missing.");
    }
    spawn_installer(&installer)
}

/// An absolute path without the `\\?\` prefix that `canonicalize` adds on
/// Windows: `cmd` and its `start` command cannot parse extended-length paths
/// and fail to find the file.
fn plain_absolute(path: &Path) -> PathBuf {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    match resolved
        .to_str()
        .and_then(|text| text.strip_prefix(r"\\?\"))
    {
        Some(plain) => PathBuf::from(plain),
        None => resolved,
    }
}

#[cfg(target_os = "windows")]
fn spawn_installer(installer: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;

    // A hidden console, not a detached process: `start` needs a console to
    // hand the installer to, and detaching leaves it without one.
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let exe = std::env::current_exe().context("Could not locate the running application")?;
    let log = installer.with_extension("install.log");
    let script = windows_install_script(std::process::id(), installer, &exe, &log);
    let script_path = installer.with_extension("install.cmd");
    std::fs::write(&script_path, script)
        .with_context(|| format!("Could not write {}", script_path.display()))?;

    std::process::Command::new("cmd")
        .arg("/C")
        .arg(&script_path)
        .creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
        .spawn()
        .context("Could not start the installer")?;
    Ok(())
}

/// Batch helper that waits for the app to exit, installs it, starts the new
/// build and deletes itself. The NSIS setup installs per user and runs
/// unattended under `/S`; Windows Installer upgrades a per machine install
/// and asks for elevation on its own. The exit code is logged either way, so
/// a failed install leaves a trace instead of silently reopening the old
/// build.
#[cfg(target_os = "windows")]
fn windows_install_script(pid: u32, installer: &Path, exe: &Path, log: &Path) -> String {
    let is_msi = installer
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("msi"));
    let run = if is_msi {
        format!(
            r#"start "" /wait %SystemRoot%\System32\msiexec.exe /i "{}" /passive /norestart"#,
            installer.display()
        )
    } else {
        // `/D` sets the target directory, pinning the update to the directory
        // the app runs from. NSIS requires it last and unquoted.
        match exe.parent() {
            Some(dir) => format!(
                r#"start "" /wait "{}" /S /D={}"#,
                installer.display(),
                dir.display()
            ),
            None => format!(r#"start "" /wait "{}" /S"#, installer.display()),
        }
    };

    // Absolute paths: a developer's PATH often puts a Unix `find` (Git for
    // Windows, MSYS, Cygwin) ahead of the Windows one, and that build reports
    // failure here, which would let the installer run while the app is alive.
    let system32 = r"%SystemRoot%\System32";
    let lines = [
        "@echo off".to_string(),
        ":waitloop".to_string(),
        format!(
            r#"{system32}\tasklist.exe /FI "PID eq {pid}" /NH 2>nul | {system32}\find.exe "{pid}" >nul"#
        ),
        "if not errorlevel 1 (".to_string(),
        format!(r"{system32}\ping.exe -n 2 127.0.0.1 >nul"),
        "goto waitloop".to_string(),
        ")".to_string(),
        run,
        format!(
            r#"echo [%date% %time%] installer exit=%errorlevel% >> "{}""#,
            log.display()
        ),
        format!(r#"if exist "{0}" start "" "{0}""#, exe.display()),
        // Leaving the batch context first lets the script delete itself.
        r#"(goto) 2>nul & del "%~f0""#.to_string(),
        String::new(),
    ];
    lines.join("\r\n")
}

#[cfg(target_os = "macos")]
fn spawn_installer(installer: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let pid = std::process::id();
    let exe = std::env::current_exe().context("Could not locate the running application")?;
    // .../Noir Player.app/Contents/MacOS/noir_player -> .../Noir Player.app
    let bundle = exe
        .ancestors()
        .find(|path| path.extension().is_some_and(|ext| ext == "app"));

    // Replacing the bundle in place is only safe when the app runs from a
    // directory this user can write to. Otherwise open the disk image and let
    // the user drag it across.
    let script = match bundle.filter(|bundle| {
        bundle.parent().is_some_and(|parent| {
            !parent
                .metadata()
                .is_ok_and(|meta| meta.permissions().readonly())
        })
    }) {
        Some(bundle) => {
            let parent = bundle.parent().unwrap_or(Path::new("/Applications"));
            // The new bundle is staged beside the old one and only swapped in
            // once the copy succeeded, so a failed copy cannot leave the user
            // with no app at all. The old bundle moves aside rather than being
            // deleted outright, and is restored if the swap fails.
            format!(
                "#!/bin/sh\n\
                 LOG='{log}'\n\
                 while kill -0 {pid} 2>/dev/null; do sleep 1; done\n\
                 MOUNT=\"$(mktemp -d)\"\n\
                 STAGE='{parent}/.noir-player-update.app'\n\
                 OLD='{parent}/.noir-player-previous.app'\n\
                 if hdiutil attach -nobrowse -quiet -mountpoint \"$MOUNT\" '{dmg}'; then\n\
                 NEW=\"$(ls -d \"$MOUNT\"/*.app 2>/dev/null | head -n 1)\"\n\
                 if [ -n \"$NEW\" ]; then\n\
                 rm -rf \"$STAGE\" \"$OLD\"\n\
                 if cp -R \"$NEW\" \"$STAGE\"; then\n\
                 if mv '{bundle}' \"$OLD\" && mv \"$STAGE\" '{bundle}'; then\n\
                 rm -rf \"$OLD\"\n\
                 echo \"installed {version}\" >> \"$LOG\"\n\
                 else\n\
                 mv \"$OLD\" '{bundle}' 2>/dev/null\n\
                 rm -rf \"$STAGE\"\n\
                 echo 'swap failed, kept the existing app' >> \"$LOG\"\n\
                 fi\n\
                 else\n\
                 echo 'copy from the disk image failed' >> \"$LOG\"\n\
                 fi\n\
                 else\n\
                 echo 'no app bundle inside the disk image' >> \"$LOG\"\n\
                 fi\n\
                 hdiutil detach -quiet \"$MOUNT\"\n\
                 else\n\
                 echo 'could not mount the disk image' >> \"$LOG\"\n\
                 fi\n\
                 rmdir \"$MOUNT\" 2>/dev/null\n\
                 open '{bundle}' 2>/dev/null || open '{dmg}'\n\
                 rm -f \"$0\"\n",
                dmg = installer.display(),
                bundle = bundle.display(),
                parent = parent.display(),
                log = installer.with_extension("install.log").display(),
                version = installer
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
        }
        None => format!(
            "#!/bin/sh\n\
             while kill -0 {pid} 2>/dev/null; do sleep 1; done\n\
             open '{dmg}'\n\
             rm -f \"$0\"\n",
            dmg = installer.display(),
        ),
    };

    let script_path = installer.with_extension("install.sh");
    std::fs::write(&script_path, script)
        .with_context(|| format!("Could not write {}", script_path.display()))?;
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
        .context("Could not make the installer helper executable")?;

    std::process::Command::new("/bin/sh")
        .arg(&script_path)
        .spawn()
        .context("Could not start the installer")?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn spawn_installer(installer: &Path) -> Result<()> {
    // Distribution packages need root, so the desktop's package installer
    // takes over from here.
    std::process::Command::new("xdg-open")
        .arg(installer)
        .spawn()
        .context("Could not open the downloaded package. Install it with your package manager.")?;
    Ok(())
}

/// Whether this platform installs the update itself, or hands the file to the
/// system and leaves the app running.
pub const fn installs_in_place() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_string(),
            url: format!("https://example.invalid/{name}"),
            size: 1024,
        }
    }

    #[test]
    fn test_version_parsing() {
        assert_eq!(parse_version("1.2.0"), Some((1, 2, 0)));
        assert_eq!(parse_version("v1.2.0"), Some((1, 2, 0)));
        assert_eq!(parse_version("V1.2.0"), Some((1, 2, 0)));
        assert_eq!(parse_version("1.2"), Some((1, 2, 0)));
        assert_eq!(parse_version("1.2.1-rc1"), Some((1, 2, 1)));
        assert_eq!(parse_version("v2.0.0+build.1"), Some((2, 0, 0)));
        assert_eq!(parse_version("invalid"), None);
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("1.2.1", "1.2.0"));
        assert!(is_newer("v1.2.1", "1.2.0"));
        assert!(is_newer("v1.3.0", "1.2.0"));
        assert!(is_newer("v2.0.0", "1.2.0"));
        assert!(!is_newer("1.2.0", "1.2.0"));
        assert!(!is_newer("v1.2.0", "1.2.0"));
        assert!(!is_newer("1.1.3", "1.2.0"));
        assert!(!is_newer("1.0.0", "1.2.0"));
    }

    #[test]
    fn release_json_keeps_assets_and_strips_the_tag_prefix() {
        let json = serde_json::json!({
            "tag_name": "v1.3.0",
            "name": "Noir Player 1.3.0",
            "html_url": "https://example.invalid/release",
            "body": "notes",
            "published_at": "2026-01-01T00:00:00Z",
            "assets": [
                {
                    "name": "noir-player-1.3.0-windows-x64-setup.exe",
                    "browser_download_url": "https://example.invalid/setup.exe",
                    "size": 42
                },
                { "name": "incomplete" }
            ]
        });
        let release = parse_release(&json).unwrap();
        assert_eq!(release.version, "1.3.0");
        assert_eq!(release.tag_name, "v1.3.0");
        assert_eq!(release.assets.len(), 1);
        assert_eq!(release.assets[0].size, 42);
    }

    #[test]
    fn platform_asset_prefers_this_platforms_installer() {
        let assets = vec![
            asset("noir-player-1.3.0-linux-x64.deb"),
            asset("noir-player-1.3.0-linux-x64.rpm"),
            asset("noir-player-1.3.0-macos-arm64.dmg"),
            asset("noir-player-1.3.0-windows-x64.msi"),
            asset("noir-player-1.3.0-windows-x64-setup.exe"),
            asset("SHA256SUMS"),
        ];
        let picked = platform_asset(&assets).expect("an asset for this platform");
        let expected_suffix = if cfg!(target_os = "windows") {
            "-setup.exe"
        } else if cfg!(target_os = "macos") {
            ".dmg"
        } else {
            "."
        };
        assert!(
            picked.name.contains(expected_suffix),
            "picked {} on this platform",
            picked.name
        );
    }

    #[test]
    fn platform_asset_is_none_without_a_matching_package() {
        let assets = vec![asset("SHA256SUMS"), asset("source.tar.gz")];
        assert!(platform_asset(&assets).is_none());
    }

    #[test]
    fn checksums_are_read_from_lists_and_sidecars() {
        let list = "\
0000000000000000000000000000000000000000000000000000000000000000  noir-player-1.3.0-linux-x64.deb
1111111111111111111111111111111111111111111111111111111111111111 *noir-player-1.3.0-windows-x64-setup.exe
";
        assert_eq!(
            find_checksum(list, "noir-player-1.3.0-windows-x64-setup.exe").as_deref(),
            Some("1111111111111111111111111111111111111111111111111111111111111111")
        );
        assert_eq!(
            find_checksum(list, "noir-player-1.3.0-macos-arm64.dmg"),
            None
        );

        let sidecar = "2222222222222222222222222222222222222222222222222222222222222222\n";
        assert_eq!(
            find_checksum(sidecar, "anything.exe").as_deref(),
            Some("2222222222222222222222222222222222222222222222222222222222222222")
        );
        assert_eq!(find_checksum("not a checksum\n", "anything.exe"), None);
    }

    /// End to end check against the live release: find the update, download
    /// the installer for this platform and verify its published checksum.
    /// Opt in with `cargo test -- --ignored live_release`.
    #[test]
    #[ignore = "downloads the current release from GitHub"]
    fn live_release_downloads_and_verifies() {
        let Some(release) = check_latest_release(Duration::from_secs(30)).unwrap() else {
            eprintln!("already on the latest release; nothing to download");
            return;
        };
        let asset = release
            .platform_asset()
            .expect("the release publishes an installer for this platform")
            .clone();

        let dir = std::env::temp_dir().join(format!("noir-update-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cancel = AtomicBool::new(false);
        let path = download_asset(&asset, &dir, &cancel, &|_, _| {}).unwrap();

        let expected = expected_checksum(&release, &asset.name, Duration::from_secs(30))
            .unwrap()
            .expect("the release publishes a checksum");
        assert_eq!(sha256_file(&path).unwrap(), expected);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_helper_waits_then_installs_and_relaunches() {
        let exe = Path::new(r"C:\Program Files\Noir Player\noir_player.exe");
        let log = Path::new(r"C:\cache\update.install.log");
        let setup = windows_install_script(
            4242,
            Path::new(r"C:\cache\noir-player-2.0.0-windows-x64-setup.exe"),
            exe,
            log,
        );
        assert!(setup.contains(r#"\tasklist.exe /FI "PID eq 4242""#));
        // The Windows tools are addressed absolutely so a Unix `find` earlier
        // on PATH cannot break the wait.
        assert!(setup.contains(r"%SystemRoot%\System32\find.exe"));
        assert!(!setup.contains("| find \""));
        assert!(setup.contains("goto waitloop"));
        // `start` cannot parse extended-length paths, so none may reach it.
        assert!(!setup.contains(r"\\?\"));
        assert!(setup.contains(
            r#"start "" /wait "C:\cache\noir-player-2.0.0-windows-x64-setup.exe" /S /D=C:\Program Files\Noir Player"#
        ));
        assert!(setup.contains(r#"start "" "C:\Program Files\Noir Player\noir_player.exe""#));
        assert!(setup.contains(r#"exit=%errorlevel% >> "C:\cache\update.install.log""#));
        assert!(setup.contains(r#"del "%~f0""#));

        let msi = windows_install_script(1, Path::new(r"C:\cache\update.msi"), exe, log);
        assert!(msi.contains(
            r#"start "" /wait %SystemRoot%\System32\msiexec.exe /i "C:\cache\update.msi" /passive /norestart"#
        ));
    }

    /// A per machine install has to be upgraded by Windows Installer. The
    /// NSIS setup would drop a second per user copy elsewhere and leave the
    /// shortcut opening the old build.
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_asset_follows_the_install_scope() {
        let assets = vec![
            asset("noir-player-2.0.0-windows-x64.msi"),
            asset("noir-player-2.0.0-windows-x64-setup.exe"),
        ];
        assert_eq!(
            windows_asset(&assets, InstallScope::PerMachine).map(|a| a.name.as_str()),
            Some("noir-player-2.0.0-windows-x64.msi")
        );
        assert_eq!(
            windows_asset(&assets, InstallScope::PerUser).map(|a| a.name.as_str()),
            Some("noir-player-2.0.0-windows-x64-setup.exe")
        );

        let only_setup = vec![asset("noir-player-2.0.0-windows-x64-setup.exe")];
        assert!(windows_asset(&only_setup, InstallScope::PerMachine).is_some());
    }

    #[test]
    fn plain_absolute_strips_the_extended_length_prefix() {
        let plain = plain_absolute(&std::env::temp_dir());
        assert!(
            !plain.to_string_lossy().starts_with(r"\\?\"),
            "{} keeps the prefix cmd cannot read",
            plain.display()
        );
    }

    #[test]
    fn sha256_matches_a_known_digest() {
        let dir = std::env::temp_dir().join(format!("noir-update-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("abc.txt");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
