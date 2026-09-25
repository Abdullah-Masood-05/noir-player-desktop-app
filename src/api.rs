use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::config::ApiKeys;

#[derive(Debug, Clone)]
pub struct Track {
    pub name: String,
    pub artist: String,
    pub url: Option<String>,
}

pub fn fetch_trending(keys: &ApiKeys) -> Result<Vec<Track>> {
    request(keys, None)
}

pub fn search_tracks(keys: &ApiKeys, query: &str) -> Result<Vec<Track>> {
    if query.trim().is_empty() {
        return fetch_trending(keys);
    }
    request(keys, Some(query.trim()))
}

fn request(keys: &ApiKeys, query: Option<&str>) -> Result<Vec<Track>> {
    let root = if keys.lastfm.is_empty() {
        backend_json(
            &match query {
                Some(query) => format!("/api/tracks?q={}", urlencoding::encode(query)),
                None => "/api/tracks".to_owned(),
            },
            Duration::from_secs(15),
        )?
    } else {
        lastfm_direct(&keys.lastfm, query)?
    };
    parse_tracks(&root, query.is_some())
}

fn lastfm_direct(key: &str, query: Option<&str>) -> Result<Value> {
    let method = if query.is_some() {
        "track.search"
    } else {
        "chart.gettoptracks"
    };
    let mut url = format!(
        "https://ws.audioscrobbler.com/2.0/?method={method}&api_key={}&format=json&limit=30",
        urlencoding::encode(key)
    );
    if let Some(query) = query {
        url.push_str(&format!("&track={}", urlencoding::encode(query)));
    }
    let agent = network_agent(Duration::from_secs(15));
    let mut response = agent.get(&url).call().map_err(|_| {
        anyhow!("Could not reach Last.fm. Check your connection and API key, then retry.")
    })?;
    if response.status().as_u16() != 200 {
        bail!("Last.fm returned an unsuccessful response.");
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(1_048_576)
        .read_to_string()
        .map_err(|_| anyhow!("Could not read the Last.fm response."))?;
    serde_json::from_str(&body).map_err(|_| anyhow!("Last.fm returned an invalid response."))
}

fn parse_tracks(root: &Value, search: bool) -> Result<Vec<Track>> {
    if root.get("error").is_some() {
        bail!("Last.fm rejected the request. Check your API key or retry later.");
    }
    let entries = root
        .pointer(if search {
            "/results/trackmatches/track"
        } else {
            "/tracks/track"
        })
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Last.fm returned an unexpected track list."))?;
    entries
        .iter()
        .map(|entry| {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("Last.fm returned a track without a title."))?;
            let artist = entry
                .get("artist")
                .and_then(|artist| {
                    artist
                        .as_str()
                        .or_else(|| artist.get("name").and_then(Value::as_str))
                })
                .ok_or_else(|| anyhow!("Last.fm returned a track without an artist."))?;
            Ok(Track {
                name: name.to_owned(),
                artist: artist.to_owned(),
                url: entry
                    .get("url")
                    .and_then(Value::as_str)
                    .filter(|url| safe_track_url(url))
                    .map(str::to_owned),
            })
        })
        .collect()
}

pub fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Audio request cancelled.");
    }
    Ok(())
}

fn network_agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_body(Some(Duration::from_secs(20)))
        .build()
        .into()
}

/// GETs one of Noir Player's backend endpoints. Its errors are written for
/// the person using the app, so they are shown as they come.
fn backend_json(path: &str, timeout: Duration) -> Result<Value> {
    let url = format!("{}{path}", crate::config::backend());
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        // The backend explains a failure in the body; read it instead of
        // replacing it with a generic transport error.
        .http_status_as_error(false)
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(10)))
        .build()
        .into();
    let mut response = agent.get(&url).call().map_err(|_| {
        anyhow!("Couldn't reach Noir Player's server. Check your connection and try again.")
    })?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .with_config()
        .limit(1_048_576)
        .read_to_string()
        .map_err(|_| anyhow!("Couldn't read the answer from Noir Player's server."))?;
    let root: Value = serde_json::from_str(&body)
        .map_err(|_| anyhow!("Noir Player's server sent back something unexpected."))?;
    if status != 200 {
        let message = root
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Noir Player's server couldn't handle that. Try again later.");
        bail!("{message}");
    }
    Ok(root)
}

fn read_json(mut response: ureq::http::Response<ureq::Body>) -> Result<Value> {
    if response.status().as_u16() != 200 {
        bail!("Audio service rejected the request. Check API configuration or retry later.");
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(1_048_576)
        .read_to_string()
        .map_err(|_| anyhow!("Could not read the audio service response."))?;
    serde_json::from_str(&body).map_err(|_| anyhow!("Audio service returned invalid JSON."))
}

fn parse_video_id(root: &Value) -> Result<String> {
    if root.get("error").is_some() {
        bail!("YouTube rejected the search. Check API configuration or quota.");
    }
    let id = root
        .pointer("/items/0/id/videoId")
        .and_then(Value::as_str)
        .filter(|id| {
            id.len() == 11
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        })
        .ok_or_else(|| anyhow!("No matching YouTube video was found."))?;
    Ok(id.to_owned())
}

#[derive(Debug, PartialEq)]
enum Resolution {
    Ready(String),
    Processing,
}

fn parse_resolution(root: &Value) -> Result<Resolution> {
    if let Some(link) = root
        .get("link")
        .and_then(Value::as_str)
        .filter(|link| !link.is_empty())
    {
        validate_audio_url(link)?;
        return Ok(Resolution::Ready(link.to_owned()));
    }
    if root.get("status").and_then(Value::as_str) == Some("processing") {
        return Ok(Resolution::Processing);
    }
    bail!("No downloadable audio source was returned.")
}

fn validate_audio_url(url: &str) -> Result<ureq::http::Uri> {
    let invalid = || anyhow!("Audio service returned an unsafe or unsupported HTTPS URL.");
    if url.len() > 8192
        || url
            .bytes()
            .any(|c| c <= 32 || c >= 127 || c == b'\\' || c == b'#')
    {
        return Err(invalid());
    }
    let uri: ureq::http::Uri = url.parse().map_err(|_| invalid())?;
    let authority = uri.authority().ok_or_else(invalid)?;
    let host = uri.host().ok_or_else(invalid)?.to_ascii_lowercase();
    if uri.scheme_str() != Some("https")
        || authority.as_str().contains('@')
        || uri.port_u16().is_some_and(|port| port != 443)
        || host.parse::<std::net::IpAddr>().is_ok()
        || !host.contains('.')
        || host.len() > 253
        || [
            ".localhost",
            ".local",
            ".internal",
            ".lan",
            ".home",
            ".localdomain",
        ]
        .iter()
        .any(|suffix| host.ends_with(suffix))
        || !host
            .rsplit('.')
            .next()
            .is_some_and(|label| label.len() >= 2 && label.bytes().all(|c| c.is_ascii_alphabetic()))
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err(invalid());
    }
    Ok(uri)
}

fn reject_credentials_in_url(url: &str, keys: &ApiKeys) -> Result<()> {
    let decoded = urlencoding::decode(url).map_err(|_| anyhow!("Invalid audio URL encoding."))?;
    if keys
        .all()
        .iter()
        .any(|key| !key.is_empty() && (url.contains(key) || decoded.contains(key)))
    {
        bail!("Audio service returned a URL containing API credentials.");
    }
    Ok(())
}

pub fn resolve_audio(
    track: &Track,
    keys: &ApiKeys,
    cancel: &AtomicBool,
    progress: &impl Fn(&str),
) -> Result<String> {
    check_cancel(cancel)?;
    progress("Finding audio on YouTube...");
    let query = format!("{} {}", track.name, track.artist);
    let agent = network_agent(Duration::from_secs(20));
    let found = if keys.youtube.is_empty() {
        backend_json(
            &format!("/api/video?q={}", urlencoding::encode(&query)),
            Duration::from_secs(25),
        )?
    } else {
        let url = format!(
            "https://www.googleapis.com/youtube/v3/search?part=snippet&q={}&type=video&maxResults=1&key={}",
            urlencoding::encode(&query), urlencoding::encode(&keys.youtube)
        );
        let response = agent.get(&url).call().map_err(|_| {
            anyhow!("YouTube search failed. Check your connection, API key or quota.")
        })?;
        read_json(response)?
    };
    let id = parse_video_id(&found)?;
    for attempt in 0..4 {
        check_cancel(cancel)?;
        progress(&format!("Resolving audio ({}/4)...", attempt + 1));
        let answer = if keys.rapidapi.is_empty() {
            backend_json(&format!("/api/resolve?id={id}"), Duration::from_secs(30))?
        } else {
            let response = agent
                .get(&format!("https://youtube-mp36.p.rapidapi.com/dl?id={id}"))
                .header("X-RapidAPI-Key", &keys.rapidapi)
                .header("X-RapidAPI-Host", "youtube-mp36.p.rapidapi.com")
                .call()
                .map_err(|_| {
                    anyhow!(
                        "Audio resolution failed. Check your connection, RapidAPI key or quota."
                    )
                })?;
            read_json(response)?
        };
        match parse_resolution(&answer)? {
            Resolution::Ready(url) => {
                reject_credentials_in_url(&url, keys)?;
                check_cancel(cancel)?;
                return Ok(url);
            }
            Resolution::Processing if attempt < 3 => {
                progress("Audio service processing; retrying in 2 seconds...");
                for _ in 0..20 {
                    check_cancel(cancel)?;
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            Resolution::Processing => {
                bail!("Audio is still processing after four attempts. Retry later.")
            }
        }
    }
    bail!("No downloadable audio source was returned.")
}

pub fn download_audio(
    url: &str,
    keys: &ApiKeys,
    file: &mut std::fs::File,
    cancel: &AtomicBool,
    progress: &impl Fn(&str),
) -> Result<()> {
    const MAX_BYTES: u64 = 100 * 1024 * 1024;
    let started = Instant::now();
    let mut url = url.to_owned();
    for redirect in 0..=5 {
        check_cancel(cancel)?;
        let uri = validate_audio_url(&url)?;
        reject_credentials_in_url(&url, keys)?;
        let remaining = Duration::from_secs(180)
            .checked_sub(started.elapsed())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| anyhow!("Audio download timed out."))?;
        let agent = network_agent(remaining);
        let mut response = agent.get(&url).call().map_err(|_| {
            anyhow!("Audio download failed. Check your connection or retry resolution.")
        })?;
        let status = response.status().as_u16();
        if matches!(status, 301 | 302 | 303 | 307 | 308) && redirect < 5 {
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| anyhow!("Audio download returned an invalid redirect."))?;
            url = if location.starts_with('/') && !location.starts_with("//") {
                format!(
                    "https://{}{}",
                    uri.authority()
                        .ok_or_else(|| anyhow!("Invalid audio host."))?,
                    location
                )
            } else {
                location.to_owned()
            };
            continue;
        }
        if status != 200 {
            bail!("Audio download returned an unsuccessful response or too many redirects.");
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        if content_type.starts_with("text/")
            || content_type.contains("json")
            || content_type.contains("html")
        {
            bail!("Audio source returned a document instead of audio.");
        }
        let total = response
            .headers()
            .get("content-length")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        if total.is_some_and(|total| total == 0 || total > MAX_BYTES) {
            bail!("Audio download is empty or exceeds the 100 MiB limit.");
        }
        let mut reader = response.body_mut().as_reader();
        let mut buffer = [0_u8; 64 * 1024];
        let mut received = 0_u64;
        loop {
            check_cancel(cancel)?;
            if started.elapsed() >= Duration::from_secs(180) {
                bail!("Audio download timed out.");
            }
            let count = reader
                .read(&mut buffer)
                .map_err(|_| anyhow!("Audio download was interrupted."))?;
            if count == 0 {
                break;
            }
            received += count as u64;
            if received > MAX_BYTES {
                bail!("Audio download exceeds the 100 MiB limit.");
            }
            file.write_all(&buffer[..count]).map_err(|_| {
                anyhow!("Could not write audio; check free space and folder permissions.")
            })?;
            progress(&match total {
                Some(total) => format!(
                    "Downloading audio: {}% ({:.1} MiB)",
                    (received * 100 / total).min(100),
                    received as f64 / 1_048_576.0
                ),
                None => format!(
                    "Downloading audio: {:.1} MiB",
                    received as f64 / 1_048_576.0
                ),
            });
        }
        if received == 0 || total.is_some_and(|total| total != received) {
            bail!("Audio download was empty or incomplete.");
        }
        return check_cancel(cancel);
    }
    bail!("Audio download exceeded the redirect limit.")
}

pub fn safe_track_url(url: &str) -> bool {
    url.starts_with("https://www.last.fm/music/") || url.starts_with("https://last.fm/music/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_youtube_video_id_without_accepting_service_errors() {
        assert_eq!(
            parse_video_id(&json!({"items":[{"id":{"videoId":"abcDEF_12-3"}}]})).unwrap(),
            "abcDEF_12-3"
        );
        for root in [
            json!({"items":[]}),
            json!({"error":{"message":"secret"}}),
            json!({"items":[{"id":{"videoId":"../bad"}}]}),
        ] {
            let error = parse_video_id(&root).unwrap_err().to_string();
            assert!(!error.contains("secret"));
        }
    }

    #[test]
    fn parses_ready_processing_and_failed_resolution() {
        assert_eq!(
            parse_resolution(&json!({"status":"processing"})).unwrap(),
            Resolution::Processing
        );
        assert_eq!(
            parse_resolution(
                &json!({"link":"https://audio.example.com/song.mp3", "status":"processing"})
            )
            .unwrap(),
            Resolution::Ready("https://audio.example.com/song.mp3".into())
        );
        for root in [
            json!({}),
            json!({"status":"failed", "msg":"secret"}),
            json!({"link":42}),
            json!({"link":"http://audio.example.com/song.mp3"}),
        ] {
            assert!(parse_resolution(&root).is_err());
        }
    }

    #[test]
    fn audio_urls_require_https_public_dns_names_without_credentials() {
        for url in [
            "http://audio.example.com/song",
            "https://localhost/song",
            "https://a.localhost/song",
            "https://127.0.0.1/song",
            "https://10.0.0.1/song",
            "https://172.16.0.1/song",
            "https://192.168.1.2/song",
            "https://169.254.169.254/song",
            "https://[::1]/song",
            "https://[fc00::1]/song",
            "https://[::ffff:127.0.0.1]/song",
            "https://2130706433/song",
            "https://0x7f000001/song",
            "https://0177.0.0.1/song",
            "https://user@audio.example.com/song",
            "https://audio.example.com:80/song",
            "https://audio.example.com\\@localhost/song",
            "https://audio.local/song",
            "https://audio.example.com./song",
            "https://audio.example.com/song#fragment",
            "https://audio.example.com/song\n",
        ] {
            assert!(validate_audio_url(url).is_err(), "{url}");
        }
        assert!(validate_audio_url("https://audio.example.com/song.mp3?token=abc%20def").is_ok());
        assert!(validate_audio_url("https://audio.example.com:443/song").is_ok());
    }

    #[test]
    fn cancellation_is_checked_without_network() {
        assert!(check_cancel(&AtomicBool::new(true)).is_err());
        assert!(check_cancel(&AtomicBool::new(false)).is_ok());
    }

    #[test]
    fn parses_search_and_chart_artist_shapes() {
        let search = json!({"results":{"trackmatches":{"track":[{"name":"Song", "artist":"Artist", "url":"https://www.last.fm/music/Artist/_/Song"}]}}});
        let chart = json!({"tracks":{"track":[{"name":"Song","artist":{"name":"Artist"}}]}});
        assert_eq!(parse_tracks(&search, true).unwrap()[0].artist, "Artist");
        assert_eq!(parse_tracks(&chart, false).unwrap()[0].name, "Song");
    }

    #[test]
    fn empty_results_stay_empty_and_errors_are_not_samples() {
        assert!(
            parse_tracks(&json!({"results":{"trackmatches":{"track":[]}}}), true)
                .unwrap()
                .is_empty()
        );
        assert!(parse_tracks(&json!({"error":6}), true).is_err());
        assert!(parse_tracks(&json!({}), false).is_err());
    }

    /// Drives the app's own request code against the deployed backend with
    /// no keys of its own, as it runs for anyone who hasn't entered any. Makes one Last.fm request and
    /// one YouTube search, so it is opt-in:
    /// `cargo test -- --ignored live_backend`.
    #[test]
    #[ignore = "calls the deployed backend and spends service quota"]
    fn live_backend_serves_discover_without_any_keys() {
        let keys = ApiKeys::default();
        let trending = fetch_trending(&keys).expect("trending through the backend");
        assert!(!trending.is_empty());
        let found = search_tracks(&keys, "Numb Linkin Park").expect("search through the backend");
        assert!(found
            .iter()
            .any(|track| track.name.eq_ignore_ascii_case("numb")));
        let video = backend_json("/api/video?q=Linkin%20Park%20Numb", Duration::from_secs(25))
            .expect("video through the backend");
        parse_video_id(&video).expect("a video id");
        let refused = backend_json("/api/resolve?id=nope", Duration::from_secs(15))
            .expect_err("a malformed id is refused");
        assert!(!refused.to_string().is_empty());
    }

    #[test]
    fn only_lastfm_https_track_links_are_opened() {
        for url in [
            "file:///song",
            "javascript:alert(1)",
            "https://www.last.fm.evil.test/music/a",
            "https://www.last.fm@evil.test/music/a",
        ] {
            assert!(!safe_track_url(url));
        }
        assert!(safe_track_url("https://www.last.fm/music/Artist/_/Song"));
    }
}
