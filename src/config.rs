#![allow(dead_code)]
use std::sync::LazyLock;

pub struct ApiConfig {
    pub lastfm_api_key: String,
    pub youtube_api_key: String,
    pub rapidapi_key: String,
}

pub static CONFIG: LazyLock<ApiConfig> = LazyLock::new(|| {
    dotenvy::dotenv().ok();
    ApiConfig {
        lastfm_api_key: key("LASTFM_API_KEY", option_env!("LASTFM_API_KEY")),
        youtube_api_key: key("YOUTUBE_API_KEY", option_env!("YOUTUBE_API_KEY")),
        rapidapi_key: key("RAPIDAPI_KEY", option_env!("RAPIDAPI_KEY")),
    }
});

/// A key from the environment, or from the one compiled in at build time.
///
/// Release builds carry keys supplied by the build, so Discover works in an
/// installed copy with no `.env` beside it. The environment still wins, so a
/// developer or a user can override the shipped key with their own. A key
/// compiled into a binary is readable by anyone who has the binary; treat a
/// published build as publishing the key.
fn key(name: &str, built_in: Option<&'static str>) -> String {
    let usable = |value: &String| !value.trim().is_empty();
    std::env::var(name)
        .ok()
        .filter(usable)
        .or_else(|| built_in.map(str::to_owned).filter(usable))
        .unwrap_or_default()
}
