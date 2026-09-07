use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Track {
    pub name: String,
    pub artist: String,
    pub url: Option<String>,
}

pub fn fetch_trending() -> Result<Vec<Track>> {
    request("chart.gettoptracks", None)
}

pub fn search_tracks(query: &str) -> Result<Vec<Track>> {
    if query.trim().is_empty() {
        return fetch_trending();
    }
    request("track.search", Some(query.trim()))
}

fn request(method: &str, query: Option<&str>) -> Result<Vec<Track>> {
    let key = &crate::config::CONFIG.lastfm_api_key;
    if key.trim().is_empty() {
        bail!("Set LASTFM_API_KEY in your environment or .env to use Discover.");
    }
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
    let root: Value = serde_json::from_str(&body)
        .map_err(|_| anyhow!("Last.fm returned an invalid response."))?;
    parse_tracks(&root, query.is_some())
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

fn reject_credentials_in_url(url: &str) -> Result<()> {
    let decoded = urlencoding::decode(url).map_err(|_| anyhow!("Invalid audio URL encoding."))?;
    let config = &crate::config::CONFIG;
    if [
        &config.youtube_api_key,
        &config.rapidapi_key,
        &config.lastfm_api_key,
    ]
    .iter()
    .any(|key| !key.is_empty() && (url.contains(key.as_str()) || decoded.contains(key.as_str())))
    {
        bail!("Audio service returned a URL containing API credentials.");
    }
    Ok(())
}

pub fn resolve_audio(
    track: &Track,
    cancel: &AtomicBool,
    progress: &impl Fn(&str),
) -> Result<String> {
    check_cancel(cancel)?;
    let config = &crate::config::CONFIG;
    if config.youtube_api_key.trim().is_empty() || config.rapidapi_key.trim().is_empty() {
        bail!("Configure YOUTUBE_API_KEY and RAPIDAPI_KEY to play or download Discover tracks.");
    }
    progress("Finding audio on YouTube...");
    let query = format!("{} {}", track.name, track.artist);
    let url = format!(
        "https://www.googleapis.com/youtube/v3/search?part=snippet&q={}&type=video&maxResults=1&key={}",
        urlencoding::encode(&query), urlencoding::encode(&config.youtube_api_key)
    );
    let agent = network_agent(Duration::from_secs(20));
    let response = agent
        .get(&url)
        .call()
        .map_err(|_| anyhow!("YouTube search failed. Check your connection, API key or quota."))?;
    let id = parse_video_id(&read_json(response)?)?;
    let url = format!("https://youtube-mp36.p.rapidapi.com/dl?id={id}");
    for attempt in 0..4 {
        check_cancel(cancel)?;
        progress(&format!("Resolving audio ({}/4)...", attempt + 1));
        let response = agent
            .get(&url)
            .header("X-RapidAPI-Key", &config.rapidapi_key)
            .header("X-RapidAPI-Host", "youtube-mp36.p.rapidapi.com")
            .call()
            .map_err(|_| {
                anyhow!("Audio resolution failed. Check your connection, RapidAPI key or quota.")
            })?;
        match parse_resolution(&read_json(response)?)? {
            Resolution::Ready(url) => {
                reject_credentials_in_url(&url)?;
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
        reject_credentials_in_url(&url)?;
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
