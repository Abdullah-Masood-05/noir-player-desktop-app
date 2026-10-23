//! Update checking, downloading and installing.
//!
//! The app asks GitHub Releases for the latest tag, picks the asset that
//! matches the running platform, downloads it with a progress callback,
//! verifies it against the release checksums, and hands it to the platform
//! installer. On Windows and macOS installation runs after the app exits, so
//! the installer can replace files that are in use. Linux can replace files
//! that are in use, so the package manager installs the update while the app
//! is still open, and the app then reopens itself.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub use crate::app::store::InstallPreference;

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
    pub fn platform_asset(&self, preference: Option<InstallPreference>) -> Option<&ReleaseAsset> {
        platform_asset(&self.assets, preference)
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
    /// The updater stopped short of installing for a reason that is not a
    /// failure, such as a sandboxed install. The text tells the user why.
    Notice(String),
    Error(String),
}

impl UpdateStatus {
    /// An update is being downloaded, checked or handed to the installer.
    /// The update dialog stays open through these; only Cancel stops one.
    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            Self::Downloading { .. } | Self::Verifying | Self::Installing
        )
    }
}

/// What the app should do once the installer has been started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    /// Quit now. A helper either installs the update once this process has
    /// exited, or reopens the app after an install that already finished.
    Quit,
    /// Keep running. The text says what happened instead of an install.
    Stay(String),
}

/// A release version, ordered by semver precedence. Build metadata after `+`
/// is dropped when parsing, since it takes no part in the ordering.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    core: (u64, u64, u64),
    /// Dot separated prerelease identifiers; empty for a full release.
    pre: Vec<PreIdentifier>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PreIdentifier {
    /// Digits with leading zeros removed, kept as text so a long run of
    /// digits cannot overflow.
    Numeric(String),
    Alphanumeric(String),
}

impl Version {
    /// Reads `1.2.3`, `v1.2`, `2.3.0-rc.1` or `1.0.0+build.5`. Missing minor
    /// and patch numbers count as zero. Anything else is `None`.
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text
            .strip_prefix(['v', 'V'])
            .unwrap_or(text)
            .split('+')
            .next()?;
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };

        let mut numbers = [0_u64; 3];
        let mut parts = core.split('.');
        for (index, part) in parts.by_ref().take(3).enumerate() {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            numbers[index] = part.parse().ok()?;
        }
        // Four or more numbers is not a version this project publishes.
        if parts.next().is_some() {
            return None;
        }

        let pre = match pre {
            None => Vec::new(),
            Some(pre) => pre
                .split('.')
                .map(|identifier| {
                    if identifier.is_empty()
                        || !identifier
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        None
                    } else if identifier.bytes().all(|b| b.is_ascii_digit()) {
                        let trimmed = identifier.trim_start_matches('0');
                        Some(PreIdentifier::Numeric(
                            if trimmed.is_empty() { "0" } else { trimmed }.to_string(),
                        ))
                    } else {
                        Some(PreIdentifier::Alphanumeric(identifier.to_string()))
                    }
                })
                .collect::<Option<Vec<_>>>()?,
        };

        Some(Self {
            core: (numbers[0], numbers[1], numbers[2]),
            pre,
        })
    }
}

impl Ord for PreIdentifier {
    /// Semver §11: numeric identifiers compare numerically and sort below
    /// alphanumeric ones, which compare in ASCII order.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, other) {
            (Self::Numeric(a), Self::Numeric(b)) => a.len().cmp(&b.len()).then_with(|| a.cmp(b)),
            (Self::Numeric(_), Self::Alphanumeric(_)) => Ordering::Less,
            (Self::Alphanumeric(_), Self::Numeric(_)) => Ordering::Greater,
            (Self::Alphanumeric(a), Self::Alphanumeric(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for PreIdentifier {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    /// A prerelease sorts below the release it leads up to, so `2.3.0-rc1`
    /// is older than `2.3.0`. Between prereleases, the first differing
    /// identifier decides, and a shorter list that matches so far is older.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        self.core
            .cmp(&other.core)
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The `(major, minor, patch)` numbers of a version, ignoring any prerelease
/// or build suffix.
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    Version::parse(v).map(|version| version.core)
}

/// Whether `remote` is a later release than `current` by semver precedence.
/// A version that cannot be read is never newer, so a malformed tag cannot
/// prompt an update.
pub fn is_newer(remote: &str, current: &str) -> bool {
    match (Version::parse(remote), Version::parse(current)) {
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

/// The GitHub repository releases are read from. For testing the updater
/// end to end, `NOIR_UPDATE_REPO=owner/name` reads the latest release of
/// another repository instead, such as a scratch repository holding a test
/// release with a higher version. It changes only where releases are looked
/// up: downloads stay HTTPS only and must match the release's published
/// SHA-256 checksums, and nothing is installed without the user choosing to.
/// Only debug builds read it; a release build always uses the real feed.
fn release_repo() -> String {
    #[cfg(debug_assertions)]
    if let Some(repo) = std::env::var("NOIR_UPDATE_REPO")
        .ok()
        .filter(|repo| is_repo_name(repo))
    {
        return repo;
    }
    REPO.to_string()
}

/// `owner/name` in the characters GitHub allows, so the override can only
/// ever name a repository path on api.github.com.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
fn is_repo_name(text: &str) -> bool {
    let part = |part: &str| {
        !part.is_empty()
            && !part.starts_with('.')
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    matches!(text.split_once('/'), Some((owner, name)) if part(owner) && part(name))
}

pub fn check_latest_release(timeout: Duration) -> Result<Option<ReleaseInfo>> {
    let current_version = env!("CARGO_PKG_VERSION");
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        release_repo()
    );

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

/// Picks the release asset that this platform can install. `preference` is
/// the saved Windows install type and is ignored elsewhere.
pub fn platform_asset(
    assets: &[ReleaseAsset],
    preference: Option<InstallPreference>,
) -> Option<&ReleaseAsset> {
    #[cfg(target_os = "windows")]
    {
        windows_asset(assets, update_plan(install_scope(), preference))
    }
    #[cfg(target_os = "macos")]
    {
        let _ = preference;
        assets
            .iter()
            .find(|asset| asset.name.to_ascii_lowercase().ends_with(".dmg"))
    }
    #[cfg(target_os = "linux")]
    {
        let _ = preference;
        // The package the running copy came from, so the package manager can
        // upgrade it. Sandboxed copies fall back to the distribution's format.
        let package = linux::detected()
            .package()
            .unwrap_or_else(linux::system_package);
        linux::asset(assets, package)
    }
}

/// How the running copy of the app was installed on Windows. The two
/// installers land in different places, so updating the copy the user
/// actually launches means matching the one it came from.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallScope {
    /// A per user NSIS install, under a directory this account can write.
    PerUser,
    /// A per machine MSI install, typically under Program Files.
    PerMachine,
}

/// How the next Windows update installs.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdatePlan {
    /// The default for every install type: the NSIS setup installs the new
    /// version for this account, silently and with no administrator prompt.
    /// On an all-users copy that adds a per-user copy, which the all-users
    /// copy hands over to at startup (see `crate::handoff`).
    PerUserSetup,
    /// Only when the owner opted in: Windows Installer upgrades the
    /// all-users copy in Program Files, asking for administrator approval.
    MachineMsi,
}

/// The update plan for a copy installed with `scope`, given the saved
/// install type. Every copy updates per user, with no administrator prompt,
/// unless an all-users copy's owner explicitly opted into updating the copy
/// for all users. Nothing is asked first.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn update_plan(scope: InstallScope, preference: Option<InstallPreference>) -> UpdatePlan {
    match (scope, preference) {
        (InstallScope::PerMachine, Some(InstallPreference::AllUsers)) => UpdatePlan::MachineMsi,
        (InstallScope::PerUser, _)
        | (InstallScope::PerMachine, None | Some(InstallPreference::JustMe)) => {
            UpdatePlan::PerUserSetup
        }
    }
}

/// Probes the directory the app runs from. A directory this account cannot
/// write to means a per machine install, which the MSI upgrades in place.
/// The answer cannot change while the app runs, so the probe runs once.
#[cfg(target_os = "windows")]
pub fn install_scope() -> InstallScope {
    static SCOPE: std::sync::OnceLock<InstallScope> = std::sync::OnceLock::new();
    *SCOPE.get_or_init(|| {
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
    })
}

#[cfg(target_os = "windows")]
pub fn windows_asset(assets: &[ReleaseAsset], plan: UpdatePlan) -> Option<&ReleaseAsset> {
    let find = |suffix: &str| {
        assets
            .iter()
            .find(|asset| asset.name.to_ascii_lowercase().ends_with(suffix))
    };
    match plan {
        // The NSIS setup installs per user and runs unattended with /S. There
        // is no MSI fallback: the MSI installs for all users, which always
        // needs an administrator.
        UpdatePlan::PerUserSetup => find("-setup.exe"),
        // Windows Installer upgrades the existing per machine install in
        // place, asking for elevation itself. The per user setup is a safe
        // fallback, since it never elevates.
        UpdatePlan::MachineMsi => find(".msi").or_else(|| find("-setup.exe")),
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

/// Starts the installer. On Windows and macOS it returns once a helper runs
/// detached from this process, and the caller is expected to quit straight
/// away: the helper waits for this process to exit before touching any
/// installed files. On Linux it blocks until the package manager finishes,
/// so call it off the UI thread.
pub fn launch_installer(
    installer: &Path,
    preference: Option<InstallPreference>,
) -> Result<InstallOutcome> {
    let installer = plain_absolute(installer);
    if !installer.is_file() {
        bail!("The downloaded installer is missing.");
    }
    // An installer pointed at a build directory registers that directory as
    // the install location, which later installs then inherit. A development
    // build has nothing to update in place, so it does not install at all.
    if running_from_build_output() {
        bail!(
            "This build runs from the cargo target directory, so there is nothing to update in \
             place. Install a released build to use the updater."
        );
    }
    #[cfg(target_os = "windows")]
    {
        let plan = update_plan(install_scope(), preference);
        if plan == UpdatePlan::PerUserSetup && !is_setup_exe(&installer) {
            // Only the MSI asks for elevation, and only an explicit opt-in
            // may lead to it.
            bail!(
                "This download updates Noir Player for all users, which needs administrator \
                 approval. Download the update again to install it just for you."
            );
        }
        if plan == UpdatePlan::PerUserSetup && install_scope() == InstallScope::PerMachine {
            return spawn_per_user_install(&installer);
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = preference;
    spawn_installer(&installer)
}

/// Whether the running executable sits in a cargo build directory.
fn running_from_build_output() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(is_cargo_build_output))
        .unwrap_or(false)
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
fn spawn_installer(installer: &Path) -> Result<InstallOutcome> {
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
    Ok(InstallOutcome::Quit)
}

/// Whether a directory is a cargo build output such as `target/debug` or
/// `target/x86_64-pc-windows-msvc/release`.
fn is_cargo_build_output(dir: &Path) -> bool {
    let profile = dir.file_name().and_then(|name| name.to_str());
    matches!(profile, Some("debug") | Some("release"))
        && dir
            .ancestors()
            .any(|ancestor| ancestor.file_name().and_then(|name| name.to_str()) == Some("target"))
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
        // INSTALLDIR pins the upgrade to the directory the app runs from.
        // Without it the package reads Software\<Manufacturer>\<ProductName>
        // from the registry, which the NSIS installer also writes, so an MSI
        // can land wherever an earlier per user install happened to go.
        match exe.parent().filter(|dir| !is_cargo_build_output(dir)) {
            Some(dir) => format!(
                r#"start "" /wait %SystemRoot%\System32\msiexec.exe /i "{}" INSTALLDIR="{}" /passive /norestart"#,
                installer.display(),
                dir.display()
            ),
            None => format!(
                r#"start "" /wait %SystemRoot%\System32\msiexec.exe /i "{}" /passive /norestart"#,
                installer.display()
            ),
        }
    } else {
        // No `/D`: the setup knows where it installed itself, and forcing a
        // directory is what put an install inside a build folder and left
        // that path in the registry for the MSI to inherit.
        format!(r#"start "" /wait "{}" /S"#, installer.display())
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

/// The product name both Windows installers use, from `bundle.name` in
/// Cargo.toml. The NSIS setup installs per user into
/// `%LOCALAPPDATA%\<product name>`, and the MSI registers under it in
/// Programs and Features.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const WINDOWS_PRODUCT_NAME: &str = "Noir Player";
/// The executable both installers place, from the `noir_player` binary.
#[cfg(target_os = "windows")]
const WINDOWS_MAIN_BINARY: &str = "noir_player.exe";
/// Written by the update helper when it could not finish, and read once by
/// the next launch. It lives beside the downloads, never in the library.
const INSTALL_NOTICE_FILE: &str = "install-notice.txt";
/// Left by the 2.4 helper when the all-users copy was not removed. Still
/// read, so a notice written before this version is shown once.
const NOTICE_OLD_COPY_REMAINS: &str = "old-copy-remains";
const NOTICE_SETUP_FAILED: &str = "setup-failed";

#[cfg(target_os = "windows")]
fn is_setup_exe(installer: &Path) -> bool {
    installer
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with("-setup.exe"))
}

/// Where the per user setup installs for an all-users copy: the folder of an
/// earlier per user install if this account has one, otherwise the setup's
/// own default, `%LOCALAPPDATA%\<product name>`.
#[cfg(target_os = "windows")]
fn per_user_install_dir() -> Result<PathBuf> {
    let running_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let registered = crate::handoff::registered_per_user_install()
        .and_then(|install| install.exe.parent().map(Path::to_path_buf))
        .filter(|dir| {
            dir.is_absolute()
                && !is_cargo_build_output(dir)
                && running_dir.as_deref() != Some(dir.as_path())
        });
    match registered {
        Some(dir) => Ok(dir),
        None => Ok(dirs::data_local_dir()
            .context("No local app data folder is available for a per user install")?
            .join(WINDOWS_PRODUCT_NAME)),
    }
}

/// Updates an all-users copy without an administrator: once this process
/// exits, the helper installs the new version for this account with the per
/// user setup and opens it. The all-users copy is left alone; from the next
/// launch it hands over to the newer per user copy.
#[cfg(target_os = "windows")]
fn spawn_per_user_install(setup: &Path) -> Result<InstallOutcome> {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let old_exe = std::env::current_exe().context("Could not locate the running application")?;
    let new_dir = per_user_install_dir()?;
    let dir = setup
        .parent()
        .context("The downloaded installer has no folder")?;
    let log = setup.with_extension("install.log");
    let notice = dir.join(INSTALL_NOTICE_FILE);
    let script = windows_per_user_script(&PerUserPaths {
        pid: std::process::id(),
        setup,
        old_exe: &old_exe,
        new_dir: &new_dir,
        log: &log,
        notice: &notice,
    });
    let script_path = setup.with_extension("install.cmd");
    std::fs::write(&script_path, script)
        .with_context(|| format!("Could not write {}", script_path.display()))?;

    std::process::Command::new("cmd")
        .arg("/C")
        .arg(&script_path)
        .creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
        .spawn()
        .context("Could not start the installer")?;
    Ok(InstallOutcome::Quit)
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct PerUserPaths<'a> {
    pid: u32,
    setup: &'a Path,
    /// The all-users copy running now, reopened if the setup fails.
    old_exe: &'a Path,
    /// Where the per user setup installs.
    new_dir: &'a Path,
    log: &'a Path,
    notice: &'a Path,
}

/// Batch helper that updates an all-users copy per user. It waits for the
/// app to exit, runs the NSIS setup silently (`/S`) into `new_dir` (`/D=`,
/// which NSIS requires last and unquoted) and opens the new copy. Nothing
/// here asks for elevation: the setup runs as the user, and the all-users
/// copy is not touched. A failed setup leaves a notice and reopens the old
/// copy.
#[cfg(target_os = "windows")]
fn windows_per_user_script(paths: &PerUserPaths) -> String {
    let PerUserPaths {
        pid,
        setup,
        old_exe,
        new_dir,
        log,
        notice,
    } = *paths;
    let new_exe = new_dir.join(WINDOWS_MAIN_BINARY);
    let system32 = r"%SystemRoot%\System32";
    let log = log.display();
    let notice = notice.display();

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
        format!(
            r#"start "" /wait "{}" /S /D={}"#,
            setup.display(),
            new_dir.display()
        ),
        r#"set "SETUP_EXIT=%errorlevel%""#.to_string(),
        format!(r#"echo [%date% %time%] per user setup exit=%SETUP_EXIT% >> "{log}""#),
        r#"if not "%SETUP_EXIT%"=="0" goto setupfailed"#.to_string(),
        format!(r#"if not exist "{}" goto setupfailed"#, new_exe.display()),
        format!(r#"start "" "{}""#, new_exe.display()),
        "goto done".to_string(),
        ":setupfailed".to_string(),
        format!(r#">"{notice}" echo {NOTICE_SETUP_FAILED}"#),
        format!(r#"if exist "{0}" start "" "{0}""#, old_exe.display()),
        ":done".to_string(),
        // Leaving the batch context first lets the script delete itself.
        r#"(goto) 2>nul & del "%~f0""#.to_string(),
        String::new(),
    ];
    lines.join("\r\n")
}

/// What the update helper left for this launch to say, read once.
pub fn take_install_notice() -> Option<String> {
    let path = update_dir().ok()?.join(INSTALL_NOTICE_FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    install_notice_message(&text)
}

fn install_notice_message(marker: &str) -> Option<String> {
    match marker.trim() {
        NOTICE_OLD_COPY_REMAINS => Some(format!(
            "Noir Player now updates just for you, with no administrator prompts. The \
             all-users copy is still installed, and opens your newer copy whenever it is \
             older. You can remove it any time in Windows Settings \u{2192} Apps, where it is \
             listed as {WINDOWS_PRODUCT_NAME}. Your library and playlists are unchanged."
        )),
        NOTICE_SETUP_FAILED => Some(
            "The update could not be installed, so Noir Player kept the copy you already \
             had. Nothing was removed, and your library and playlists are unchanged. You \
             can try again from Settings."
                .to_string(),
        ),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn spawn_installer(installer: &Path) -> Result<InstallOutcome> {
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
    Ok(InstallOutcome::Quit)
}

#[cfg(target_os = "linux")]
fn spawn_installer(installer: &Path) -> Result<InstallOutcome> {
    linux::install(installer)
}

/// Whether this platform installs the update itself, or hands the file to the
/// system and leaves the app running. A sandboxed Linux copy (AppImage,
/// Flatpak, Snap) updates through its own channel, so it is handed over.
pub fn installs_in_place() -> bool {
    #[cfg(target_os = "linux")]
    {
        linux::detected().package().is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        cfg!(any(target_os = "windows", target_os = "macos"))
    }
}

/// Linux installs: how the running copy was installed, which package to
/// fetch, and the `pkexec` command that upgrades it in place.
///
/// The pure functions here build on every host so their tests run
/// everywhere. Only the parts that read the environment or start processes
/// are Linux only, which keeps the tests deterministic.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod linux {
    use std::ffi::OsString;
    use std::path::Path;

    #[cfg(target_os = "linux")]
    use super::InstallOutcome;
    use super::ReleaseAsset;
    #[cfg(target_os = "linux")]
    use anyhow::{bail, Context, Result};

    /// Package formats the updater can install.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Package {
        Deb,
        Rpm,
        Pacman,
    }

    impl Package {
        pub const fn extension(self) -> &'static str {
            match self {
                Self::Deb => ".deb",
                Self::Rpm => ".rpm",
                Self::Pacman => ".pkg.tar.zst",
            }
        }

        /// The package manager `pkexec` runs to install this format.
        pub const fn manager(self) -> &'static str {
            match self {
                Self::Deb => "apt-get",
                Self::Rpm => "dnf",
                Self::Pacman => "pacman",
            }
        }

        /// The format of a downloaded package, read from its file name.
        pub fn of_file(path: &Path) -> Option<Self> {
            let name = path.file_name()?.to_str()?.to_ascii_lowercase();
            [Self::Deb, Self::Rpm, Self::Pacman]
                .into_iter()
                .find(|package| name.ends_with(package.extension()))
        }
    }

    /// How the running copy of the app was installed.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Install {
        /// A distribution package. When no package manager claims the
        /// executable, this is the distribution's own format instead.
        Package(Package),
        /// Self-contained or sandboxed formats, which update through their
        /// own channel rather than a downloaded package.
        AppImage,
        Flatpak,
        Snap,
    }

    impl Install {
        pub const fn package(self) -> Option<Package> {
            match self {
                Self::Package(package) => Some(package),
                Self::AppImage | Self::Flatpak | Self::Snap => None,
            }
        }
    }

    /// A program and its arguments, ready to run.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct InstallCommand {
        pub program: &'static str,
        pub args: Vec<OsString>,
    }

    /// The `pkexec` command that installs `package` over the running copy,
    /// or the reason the app should hand the file to the desktop instead.
    /// The path must be absolute: `apt-get` reads a bare file name as the
    /// name of a package in its repositories.
    pub fn install_command(install: Install, package: &Path) -> Result<InstallCommand, String> {
        let kind = match install {
            Install::Package(kind) => kind,
            Install::AppImage => {
                return Err("Noir Player is running as an AppImage, which it does not \
                            update itself."
                    .to_string())
            }
            Install::Flatpak => {
                return Err("Noir Player is running as a Flatpak, which updates \
                            through Flatpak rather than a downloaded package."
                    .to_string())
            }
            Install::Snap => {
                return Err("Noir Player is running as a Snap, which updates \
                            through the Snap Store rather than a downloaded package."
                    .to_string())
            }
        };
        // `has_root` rather than `is_absolute` so the tests read the same on
        // a Windows host, where `/tmp` has no drive letter.
        if !package.has_root() {
            return Err("The downloaded package has no absolute path.".to_string());
        }
        if Package::of_file(package) != Some(kind) {
            return Err(format!(
                "The downloaded package is not a {} package, which is how Noir \
                 Player was installed here.",
                kind.extension()
            ));
        }

        let args: &[&str] = match kind {
            Package::Deb => &["apt-get", "install", "-y"],
            Package::Rpm => &["dnf", "install", "-y"],
            Package::Pacman => &["pacman", "-U", "--noconfirm"],
        };
        let mut args: Vec<OsString> = args.iter().map(OsString::from).collect();
        args.push(package.as_os_str().to_owned());
        Ok(InstallCommand {
            program: "pkexec",
            args,
        })
    }

    /// A message for a failed install. `pkexec` exits 126 when the password
    /// prompt is dismissed and 127 when this account is not authorized;
    /// anything else is the package manager's own exit code.
    pub fn install_failure(manager: &str, code: Option<i32>, stderr: &str) -> String {
        match code {
            Some(126) => "The password prompt was closed, so the update was not \
                          installed."
                .to_string(),
            Some(127) => "This account is not allowed to install software, so the \
                          update was not installed. Ask an administrator to install \
                          the downloaded package."
                .to_string(),
            code => {
                let detail = stderr
                    .lines()
                    .map(str::trim)
                    .rfind(|line| !line.is_empty())
                    .map(|line| format!(": {line}"))
                    .unwrap_or_default();
                match code {
                    Some(code) => {
                        format!("{manager} could not install the update (exit code {code}){detail}")
                    }
                    None => format!("{manager} stopped before the update was installed{detail}"),
                }
            }
        }
    }

    /// Shell script that waits for process `pid` to exit, then runs the
    /// executable passed as `$1`. Starting the new build any earlier would
    /// find this instance still holding the single instance socket, and it
    /// would hand itself over to the process that is about to quit. The wait
    /// gives up after 30 seconds rather than hang behind a process that never
    /// gets reaped.
    pub fn relaunch_script(pid: u32) -> String {
        format!(
            "i=0; while kill -0 {pid} 2>/dev/null && [ $i -lt 30 ]; do i=$((i+1)); sleep 1; done; \
             exec \"$1\""
        )
    }

    /// Picks the release asset in `preferred` format, then any Debian or RPM
    /// package.
    pub fn asset(assets: &[ReleaseAsset], preferred: Package) -> Option<&ReleaseAsset> {
        let find = |suffix: &str| {
            assets
                .iter()
                .find(|asset| asset.name.to_ascii_lowercase().ends_with(suffix))
        };
        find(preferred.extension())
            .or_else(|| find(Package::Deb.extension()))
            .or_else(|| find(Package::Rpm.extension()))
    }

    /// The package format of a distribution, read from `/etc/os-release`.
    /// Debian packages are the fallback because the project builds on Ubuntu.
    pub fn distribution_package(os_release: &str) -> Package {
        let family = os_release.to_ascii_lowercase();
        if family.contains("arch") || family.contains("manjaro") {
            Package::Pacman
        } else if family.contains("fedora")
            || family.contains("rhel")
            || family.contains("centos")
            || family.contains("suse")
        {
            Package::Rpm
        } else {
            Package::Deb
        }
    }

    /// The package format of the distribution this copy runs on.
    #[cfg(target_os = "linux")]
    pub fn system_package() -> Package {
        distribution_package(&std::fs::read_to_string("/etc/os-release").unwrap_or_default())
    }

    /// How the running copy was installed, worked out once per run since it
    /// asks the package managers.
    #[cfg(target_os = "linux")]
    pub fn detected() -> Install {
        static DETECTED: std::sync::OnceLock<Install> = std::sync::OnceLock::new();
        *DETECTED.get_or_init(detect)
    }

    #[cfg(target_os = "linux")]
    fn detect() -> Install {
        let set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
        if set("APPIMAGE") {
            return Install::AppImage;
        }
        if set("FLATPAK_ID") || Path::new("/.flatpak-info").exists() {
            return Install::Flatpak;
        }
        if set("SNAP") {
            return Install::Snap;
        }
        let owner = running_executable()
            .ok()
            .and_then(|exe| owning_package(&exe));
        Install::Package(owner.unwrap_or_else(system_package))
    }

    /// Asks each package manager whether it owns `exe`. A manager that is
    /// not installed fails to start and is skipped.
    #[cfg(target_os = "linux")]
    fn owning_package(exe: &Path) -> Option<Package> {
        use std::process::{Command, Stdio};

        [
            (Package::Deb, "dpkg", "-S"),
            (Package::Rpm, "rpm", "-qf"),
            (Package::Pacman, "pacman", "-Qo"),
        ]
        .into_iter()
        .find(|(_, program, flag)| {
            Command::new(program)
                .arg(flag)
                .arg(exe)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        })
        .map(|(package, ..)| package)
    }

    /// The path of the running executable. The kernel appends ` (deleted)`
    /// once a package upgrade has replaced the file this process started
    /// from; the path itself then holds the new build.
    #[cfg(target_os = "linux")]
    fn running_executable() -> Result<std::path::PathBuf> {
        let exe = std::env::current_exe().context("Could not locate the running application")?;
        Ok(
            match exe
                .to_str()
                .and_then(|text| text.strip_suffix(" (deleted)"))
            {
                Some(path) => std::path::PathBuf::from(path),
                None => exe,
            },
        )
    }

    #[cfg(target_os = "linux")]
    fn on_path(program: &str) -> bool {
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join(program).is_file())
        })
    }

    /// Installs `package` with the system package manager through `pkexec`,
    /// then starts a helper that reopens the app once it quits. Anything that
    /// rules out installing hands the file to the desktop instead, the way
    /// the updater always did on Linux.
    #[cfg(target_os = "linux")]
    pub fn install(package: &Path) -> Result<InstallOutcome> {
        use std::process::{Command, Stdio};

        // Read before installing: afterwards the kernel reports the old file
        // as deleted.
        let exe = running_executable()?;
        let install = detected();
        let command = match install_command(install, package) {
            Ok(command) => command,
            Err(reason) => return open_package(package, &reason),
        };
        let manager = install
            .package()
            .map_or("The package manager", Package::manager);
        if !on_path(manager) {
            return open_package(
                package,
                &format!("{manager} was not found to install the update."),
            );
        }

        let output = match Command::new(command.program)
            .args(&command.args)
            .stdin(Stdio::null())
            .output()
        {
            Ok(output) => output,
            Err(_) => {
                return open_package(
                    package,
                    "pkexec, which asks for the password to install software, could \
                     not be started.",
                )
            }
        };
        if !output.status.success() {
            bail!(install_failure(
                manager,
                output.status.code(),
                &String::from_utf8_lossy(&output.stderr),
            ));
        }

        let relaunch = Command::new("/bin/sh")
            .arg("-c")
            .arg(relaunch_script(std::process::id()))
            .arg("noir-player-relaunch")
            .arg(&exe)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        Ok(match relaunch {
            Ok(_) => InstallOutcome::Quit,
            // Installed, but nothing would reopen the app, so leave it open.
            Err(_) => InstallOutcome::Stay(
                "The update is installed. Restart Noir Player to start using it.".to_string(),
            ),
        })
    }

    /// Hands `package` to the desktop's software installer, explaining why
    /// with `reason`.
    #[cfg(target_os = "linux")]
    fn open_package(package: &Path, reason: &str) -> Result<InstallOutcome> {
        std::process::Command::new("xdg-open")
            .arg(package)
            .spawn()
            .context(
                "Could not open the downloaded package. Install it with your package manager.",
            )?;
        Ok(InstallOutcome::Stay(format!(
            "{reason} The downloaded package was opened in your software installer instead."
        )))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn args(command: &InstallCommand) -> Vec<&str> {
            command
                .args
                .iter()
                .map(|arg| arg.to_str().unwrap())
                .collect()
        }

        #[test]
        fn each_package_manager_installs_through_pkexec() {
            let deb = install_command(
                Install::Package(Package::Deb),
                Path::new("/home/me/.cache/noir-player/updates/noir-player-2.3.0-linux-x64.deb"),
            )
            .unwrap();
            assert_eq!(deb.program, "pkexec");
            assert_eq!(
                args(&deb),
                [
                    "apt-get",
                    "install",
                    "-y",
                    "/home/me/.cache/noir-player/updates/noir-player-2.3.0-linux-x64.deb"
                ]
            );

            let rpm = install_command(
                Install::Package(Package::Rpm),
                Path::new("/tmp/noir-player-2.3.0-linux-x64.rpm"),
            )
            .unwrap();
            assert_eq!(rpm.program, "pkexec");
            assert_eq!(
                args(&rpm),
                [
                    "dnf",
                    "install",
                    "-y",
                    "/tmp/noir-player-2.3.0-linux-x64.rpm"
                ]
            );

            let pacman = install_command(
                Install::Package(Package::Pacman),
                Path::new("/tmp/noir-player-2.3.0-x86_64.pkg.tar.zst"),
            )
            .unwrap();
            assert_eq!(pacman.program, "pkexec");
            assert_eq!(
                args(&pacman),
                [
                    "pacman",
                    "-U",
                    "--noconfirm",
                    "/tmp/noir-player-2.3.0-x86_64.pkg.tar.zst"
                ]
            );
        }

        #[test]
        fn a_path_with_spaces_stays_one_argument() {
            let command = install_command(
                Install::Package(Package::Deb),
                Path::new("/home/Jane Doe/.cache/noir player/update.deb"),
            )
            .unwrap();
            assert_eq!(command.args.len(), 4);
            assert_eq!(
                command.args[3],
                OsString::from("/home/Jane Doe/.cache/noir player/update.deb")
            );
        }

        #[test]
        fn sandboxed_installs_are_never_installed_over() {
            let path = Path::new("/tmp/noir-player-2.3.0-linux-x64.deb");
            for (install, name) in [
                (Install::AppImage, "AppImage"),
                (Install::Flatpak, "Flatpak"),
                (Install::Snap, "Snap"),
            ] {
                let reason = install_command(install, path).unwrap_err();
                assert!(reason.contains(name), "{reason}");
                assert_eq!(install.package(), None);
            }
        }

        #[test]
        fn a_relative_path_or_a_mismatched_package_is_not_installed() {
            assert!(
                install_command(Install::Package(Package::Deb), Path::new("update.deb")).is_err()
            );
            // `apt-get` must never be handed an RPM, nor `pacman` a Debian
            // package.
            assert!(
                install_command(Install::Package(Package::Deb), Path::new("/tmp/update.rpm"))
                    .is_err()
            );
            assert!(install_command(
                Install::Package(Package::Pacman),
                Path::new("/tmp/update.deb")
            )
            .is_err());
        }

        #[test]
        fn package_formats_are_read_from_file_names() {
            assert_eq!(
                Package::of_file(Path::new("/tmp/a.deb")),
                Some(Package::Deb)
            );
            assert_eq!(
                Package::of_file(Path::new("/tmp/A.RPM")),
                Some(Package::Rpm)
            );
            assert_eq!(
                Package::of_file(Path::new("/tmp/a-x86_64.pkg.tar.zst")),
                Some(Package::Pacman)
            );
            assert_eq!(Package::of_file(Path::new("/tmp/a.tar.zst")), None);
            assert_eq!(Package::of_file(Path::new("/tmp/a.AppImage")), None);
        }

        #[test]
        fn pkexec_exit_codes_read_as_messages() {
            let dismissed = install_failure("apt-get", Some(126), "");
            assert!(
                dismissed.contains("password prompt was closed"),
                "{dismissed}"
            );
            let refused = install_failure("apt-get", Some(127), "");
            assert!(refused.contains("not allowed"), "{refused}");

            let failed = install_failure(
                "apt-get",
                Some(100),
                "Reading package lists...\nE: Unable to locate package\n\n",
            );
            assert_eq!(
                failed,
                "apt-get could not install the update (exit code 100): E: Unable to locate package"
            );
            assert_eq!(
                install_failure("dnf", None, ""),
                "dnf stopped before the update was installed"
            );
        }

        #[test]
        fn relaunch_waits_for_this_process_then_runs_the_same_path() {
            let script = relaunch_script(4242);
            assert!(script.contains("kill -0 4242"));
            assert!(script.contains("-lt 30"));
            // The executable arrives as an argument, so no path is ever
            // spliced into the script and quoting cannot break it.
            assert!(script.ends_with("exec \"$1\""));
        }

        #[test]
        fn assets_follow_the_install_then_fall_back() {
            let release_asset = |name: &str| ReleaseAsset {
                name: name.to_string(),
                url: format!("https://example.invalid/{name}"),
                size: 1,
            };
            let assets = vec![
                release_asset("noir-player-2.3.0-linux-x64.deb"),
                release_asset("noir-player-2.3.0-linux-x64.rpm"),
                release_asset("noir-player-2.3.0-x86_64.pkg.tar.zst"),
            ];
            for package in [Package::Deb, Package::Rpm, Package::Pacman] {
                let picked = asset(&assets, package).unwrap();
                assert!(
                    picked.name.ends_with(package.extension()),
                    "{}",
                    picked.name
                );
            }

            let only_rpm = vec![release_asset("noir-player-2.3.0-linux-x64.rpm")];
            assert_eq!(
                asset(&only_rpm, Package::Pacman).map(|a| a.name.as_str()),
                Some("noir-player-2.3.0-linux-x64.rpm")
            );
            assert!(asset(&[release_asset("SHA256SUMS")], Package::Deb).is_none());
        }

        #[test]
        fn distributions_map_to_their_package_format() {
            assert_eq!(
                distribution_package("NAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\n"),
                Package::Deb
            );
            assert_eq!(
                distribution_package("NAME=\"Fedora Linux\"\nID=fedora\n"),
                Package::Rpm
            );
            assert_eq!(
                distribution_package("NAME=\"openSUSE Tumbleweed\"\nID_LIKE=\"opensuse suse\"\n"),
                Package::Rpm
            );
            assert_eq!(
                distribution_package("NAME=\"Arch Linux\"\nID=arch\n"),
                Package::Pacman
            );
            assert_eq!(distribution_package(""), Package::Deb);
        }
    }
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
    fn a_prerelease_is_older_than_its_release() {
        assert!(is_newer("2.3.0", "2.3.0-rc1"));
        assert!(!is_newer("2.3.0-rc1", "2.3.0"));
        assert!(!is_newer("2.3.0-rc1", "2.3.0-rc1"));
        assert!(is_newer("1.2.1", "1.2.1-rc1"));
        assert!(!is_newer("v1.2.1-rc1", "1.2.1"));
        // A prerelease of a later version still beats an earlier release.
        assert!(is_newer("2.3.0-rc1", "2.2.0"));
        assert!(!is_newer("2.2.0", "2.3.0-rc1"));
    }

    /// The precedence example from semver §11, in ascending order.
    #[test]
    fn prerelease_identifiers_follow_semver_precedence() {
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];
        for (index, lower) in ordered.iter().enumerate() {
            for higher in &ordered[index + 1..] {
                assert!(
                    is_newer(higher, lower),
                    "{higher} should be newer than {lower}"
                );
                assert!(
                    !is_newer(lower, higher),
                    "{lower} should be older than {higher}"
                );
            }
        }

        // Numeric identifiers compare as numbers, however long, and sort
        // below alphanumeric ones.
        assert!(is_newer("1.0.0-rc.10", "1.0.0-rc.9"));
        assert!(is_newer("1.0.0-alpha", "1.0.0-1"));
        assert!(is_newer(
            "1.0.0-99999999999999999999999",
            "1.0.0-9999999999999999999999"
        ));
        assert!(!is_newer("1.0.0-rc.01", "1.0.0-rc.1"));
        assert!(is_newer("1.0.0-rc-2", "1.0.0-rc-1"));
    }

    #[test]
    fn build_metadata_and_missing_numbers_do_not_change_the_order() {
        assert!(!is_newer("1.2.0+build.5", "1.2.0"));
        assert!(!is_newer("1.2.0", "1.2.0+build.5"));
        assert!(!is_newer("1.2.0+b", "1.2.0+a"));
        assert!(!is_newer("2.3.0-rc.1+build.9", "2.3.0-rc.1"));
        assert!(is_newer("2.3.0+build.1", "2.3.0-rc.1"));

        assert_eq!(parse_version("2"), Some((2, 0, 0)));
        assert!(!is_newer("2", "2.0.0"));
        assert!(!is_newer("1.2", "1.2.0"));
        assert!(is_newer("1.3", "1.2.9"));
        assert!(is_newer("V2.3.0-RC1", "2.2.0"));
    }

    #[test]
    fn unreadable_versions_are_never_newer() {
        for bad in [
            "",
            "v",
            "latest",
            "1.x",
            "1..2",
            "1.2.3.4",
            "-rc1",
            "1.2.3-",
            "1.2.3-rc..1",
            "1.2.3-rc!1",
            "1.2.3-rc 1",
            "99999999999999999999999.0.0",
        ] {
            assert!(!is_newer(bad, "1.0.0"), "{bad:?} should not be newer");
            assert!(!is_newer("1.0.0", bad), "nothing is newer than {bad:?}");
            assert_eq!(parse_version(bad), None, "{bad:?}");
        }
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
    fn platform_asset_picks_an_installer_for_this_os() {
        let assets = vec![
            asset("noir-player-1.3.0-linux-x64.deb"),
            asset("noir-player-1.3.0-linux-x64.rpm"),
            asset("noir-player-1.3.0-macos-arm64.dmg"),
            asset("noir-player-1.3.0-windows-x64.msi"),
            asset("noir-player-1.3.0-windows-x64-setup.exe"),
            asset("SHA256SUMS"),
        ];

        // On Windows the free function `platform_asset` delegates to
        // `install_scope()` which is filesystem-dependent and flaky in CI.
        // Test the underlying `windows_asset` with explicit scopes instead.
        #[cfg(target_os = "windows")]
        {
            let per_user =
                windows_asset(&assets, UpdatePlan::PerUserSetup).expect("per-user asset");
            assert!(
                per_user.name.ends_with("-setup.exe"),
                "PerUserSetup should prefer -setup.exe, got {}",
                per_user.name
            );

            let per_machine =
                windows_asset(&assets, UpdatePlan::MachineMsi).expect("per-machine asset");
            assert!(
                per_machine.name.ends_with(".msi"),
                "MachineMsi should prefer .msi, got {}",
                per_machine.name
            );
        }

        #[cfg(target_os = "macos")]
        {
            let picked = platform_asset(&assets, None).expect("macOS asset");
            assert!(
                picked.name.ends_with(".dmg"),
                "macOS should pick .dmg, got {}",
                picked.name
            );
        }

        // On Linux `platform_asset` asks the package managers which one owns
        // the running binary, and tests start no processes, so the pure
        // selection is tested with each format in `linux::tests` instead.
        #[cfg(target_os = "linux")]
        {
            let picked = linux::asset(&assets, linux::Package::Deb).expect("Linux asset");
            assert!(
                picked.name.ends_with(".deb"),
                "Linux should pick .deb, got {}",
                picked.name
            );
        }
    }

    #[test]
    fn platform_asset_is_none_without_a_matching_package() {
        let assets = vec![asset("SHA256SUMS"), asset("source.tar.gz")];
        #[cfg(target_os = "linux")]
        assert!(linux::asset(&assets, linux::Package::Deb).is_none());
        #[cfg(not(target_os = "linux"))]
        assert!(platform_asset(&assets, Some(InstallPreference::JustMe)).is_none());
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
            .platform_asset(Some(InstallPreference::JustMe))
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
        // No /D: the setup keeps its own install location, the way Zed leaves
        // the directory to the installer that owns it.
        assert!(setup
            .contains(r#"start "" /wait "C:\cache\noir-player-2.0.0-windows-x64-setup.exe" /S"#));
        assert!(!setup.contains("/D="));
        assert!(setup.contains(r#"start "" "C:\Program Files\Noir Player\noir_player.exe""#));
        assert!(setup.contains(r#"exit=%errorlevel% >> "C:\cache\update.install.log""#));
        assert!(setup.contains(r#"del "%~f0""#));

        // INSTALLDIR keeps an upgrade in the directory the app runs from,
        // rather than a path read out of the registry.
        let msi = windows_install_script(1, Path::new(r"C:\cache\update.msi"), exe, log);
        assert!(msi.contains(
            r#"msiexec.exe /i "C:\cache\update.msi" INSTALLDIR="C:\Program Files\Noir Player" /passive /norestart"#
        ));
    }

    /// Running from `cargo build` output is not an install: pointing an
    /// installer there registers the build directory as the app's location,
    /// which a later MSI then inherits through the registry.
    #[cfg(target_os = "windows")]
    #[test]
    fn a_build_directory_is_never_used_as_the_install_target() {
        let log = Path::new(r"C:\cache\update.install.log");

        for build in [
            r"C:\src\noir\target\debug\noir_player.exe",
            r"C:\src\noir\target\release\noir_player.exe",
            r"C:\src\noir\target\x86_64-pc-windows-msvc\release\noir_player.exe",
        ] {
            let script =
                windows_install_script(7, Path::new(r"C:\cache\update.msi"), Path::new(build), log);
            assert!(
                !script.contains("INSTALLDIR="),
                "pinned Windows Installer into the build directory: {build}"
            );
            assert!(is_cargo_build_output(Path::new(build).parent().unwrap()));
        }

        assert!(!is_cargo_build_output(Path::new(
            r"C:\Users\me\AppData\Local\Programs\Noir Player"
        )));
    }

    /// Every install type updates per user by default. Only an all-users copy
    /// whose owner explicitly chose to update for all users gets the MSI,
    /// and a per user copy has no all-users copy to upgrade.
    #[test]
    fn update_plan_defaults_to_per_user_for_every_install_type() {
        use InstallPreference::{AllUsers, JustMe};
        let cases = [
            (InstallScope::PerUser, None, UpdatePlan::PerUserSetup),
            (
                InstallScope::PerUser,
                Some(JustMe),
                UpdatePlan::PerUserSetup,
            ),
            (
                InstallScope::PerUser,
                Some(AllUsers),
                UpdatePlan::PerUserSetup,
            ),
            (InstallScope::PerMachine, None, UpdatePlan::PerUserSetup),
            (
                InstallScope::PerMachine,
                Some(JustMe),
                UpdatePlan::PerUserSetup,
            ),
            (
                InstallScope::PerMachine,
                Some(AllUsers),
                UpdatePlan::MachineMsi,
            ),
        ];
        for (scope, preference, plan) in cases {
            assert_eq!(
                update_plan(scope, preference),
                plan,
                "{scope:?} {preference:?}"
            );
        }
    }

    /// The testing override can only name a repository on api.github.com.
    #[test]
    fn the_release_repository_override_must_be_owner_and_name() {
        assert!(is_repo_name("someone/noir-player-test"));
        assert!(is_repo_name("Abdullah-Masood-05/noir-player-desktop-app"));
        assert!(is_repo_name("a_b/c.d"));
        for bad in [
            "",
            "noir",
            "/name",
            "owner/",
            "owner/name/releases",
            "../x",
            "owner/..",
            "owner/.hidden",
            "owner/name?x=1",
            "owner/name#x",
            "evil.com:443/x",
            "https://evil.example/x",
            "owner /name",
        ] {
            assert!(!is_repo_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn install_notices_read_back_as_messages() {
        let remains = install_notice_message("old-copy-remains\r\n").unwrap();
        assert!(remains.contains("Windows Settings \u{2192} Apps"));
        assert!(remains.contains("library and playlists are unchanged"));
        assert!(install_notice_message(NOTICE_SETUP_FAILED)
            .unwrap()
            .contains("kept the copy"));
        assert_eq!(install_notice_message("something else"), None);
    }

    /// The default plan only ever downloads the per user setup: the MSI
    /// installs for all users and always needs an administrator. Only the
    /// opt-in plan picks the MSI.
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_asset_follows_the_update_plan() {
        let msi = "noir-player-2.0.0-windows-x64.msi";
        let setup = "noir-player-2.0.0-windows-x64-setup.exe";
        let assets = vec![asset(msi), asset(setup)];
        let pick = |assets: &[ReleaseAsset], plan| {
            windows_asset(assets, plan).map(|a: &ReleaseAsset| a.name.clone())
        };
        assert_eq!(pick(&assets, UpdatePlan::MachineMsi).as_deref(), Some(msi));
        assert_eq!(
            pick(&assets, UpdatePlan::PerUserSetup).as_deref(),
            Some(setup)
        );

        // A release with only an MSI offers nothing that installs without
        // an administrator.
        let only_msi = vec![asset(msi)];
        assert_eq!(pick(&only_msi, UpdatePlan::PerUserSetup), None);
        // The opt-in falls back to the setup, which never elevates.
        let only_setup = vec![asset(setup)];
        assert_eq!(
            pick(&only_setup, UpdatePlan::MachineMsi).as_deref(),
            Some(setup)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_per_user_update_installs_without_elevation_and_opens_the_new_copy() {
        let setup = Path::new(r"C:\cache\noir-player-2.5.0-windows-x64-setup.exe");
        let paths = PerUserPaths {
            pid: 4242,
            setup,
            old_exe: Path::new(r"C:\Program Files\Noir Player\noir_player.exe"),
            new_dir: Path::new(r"C:\Users\me\AppData\Local\Noir Player"),
            log: Path::new(r"C:\cache\update.install.log"),
            notice: Path::new(r"C:\cache\install-notice.txt"),
        };
        let script = windows_per_user_script(&paths);
        let position = |needle: &str| {
            script
                .find(needle)
                .unwrap_or_else(|| panic!("missing {needle:?} in\n{script}"))
        };

        // Waits for the running all-users copy to exit before anything else.
        let wait = position(r#"\tasklist.exe /FI "PID eq 4242""#);
        // Silent per user setup; /D= last and unquoted, as NSIS requires.
        let install = position(
            r#"start "" /wait "C:\cache\noir-player-2.5.0-windows-x64-setup.exe" /S /D=C:\Users\me\AppData\Local\Noir Player"#,
        );
        let checked = position(r#"if not "%SETUP_EXIT%"=="0" goto setupfailed"#);
        let relaunch =
            position(r#"start "" "C:\Users\me\AppData\Local\Noir Player\noir_player.exe""#);
        assert!(wait < install && install < checked && checked < relaunch);

        // Nothing elevates, and the all-users copy is never removed.
        for forbidden in ["RunAs", "runas", "msiexec", "/x", "powershell"] {
            assert!(!script.contains(forbidden), "{forbidden} in\n{script}");
        }

        // A failed setup leaves a notice and reopens the old copy.
        let failed = position(":setupfailed");
        assert!(relaunch < failed);
        assert!(script[failed..].contains(r#">"C:\cache\install-notice.txt" echo setup-failed"#));
        assert!(script[failed..].contains(
            r#"if exist "C:\Program Files\Noir Player\noir_player.exe" start "" "C:\Program Files\Noir Player\noir_player.exe""#
        ));
        assert!(!script.contains(r"\\?\"));
        assert!(script.contains(r#"del "%~f0""#));
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
