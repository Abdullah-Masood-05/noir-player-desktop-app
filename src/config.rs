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
        lastfm_api_key: std::env::var("LASTFM_API_KEY").unwrap_or_default(),
        youtube_api_key: std::env::var("YOUTUBE_API_KEY").unwrap_or_default(),
        rapidapi_key: std::env::var("RAPIDAPI_KEY").unwrap_or_default(),
    }
});
