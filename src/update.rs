use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub name: String,
    pub html_url: String,
    pub body: String,
    pub published_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    Error(String),
}

pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches(|c| c == 'v' || c == 'V');
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

pub fn check_latest_release(timeout: Duration) -> Result<Option<ReleaseInfo>> {
    let current_version = env!("CARGO_PKG_VERSION");
    let url =
        "https://api.github.com/repos/Abdullah-Masood-05/noir-player-desktop-app/releases/latest";

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(3)
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(5)))
        .build()
        .into();

    let mut response = agent
        .get(url)
        .header("User-Agent", "NoirPlayer-Desktop")
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

    let tag_name = json
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing tag_name in release response"))?
        .to_string();

    let version = tag_name
        .trim_start_matches(|c| c == 'v' || c == 'V')
        .to_string();

    let name = json
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&tag_name)
        .to_string();

    let html_url = json
        .get("html_url")
        .and_then(Value::as_str)
        .unwrap_or("https://github.com/Abdullah-Masood-05/noir-player-desktop-app/releases")
        .to_string();

    let release_notes = json
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let published_at = json
        .get("published_at")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    if is_newer(&version, current_version) {
        Ok(Some(ReleaseInfo {
            version,
            tag_name,
            name,
            html_url,
            body: release_notes,
            published_at,
        }))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
