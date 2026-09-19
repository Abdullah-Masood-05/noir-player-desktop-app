use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::media::Track;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub playlists: Vec<SavedPlaylist>,
    #[serde(default)]
    pub favourites: Vec<PathBuf>,
    #[serde(default = "default_seek_interval")]
    pub seek_interval_seconds: u32,
    #[serde(default)]
    pub equalizer_enabled: bool,
    #[serde(default = "default_equalizer_gains")]
    pub equalizer_gains: [f32; 5],
    #[serde(default = "default_equalizer_preset")]
    pub equalizer_preset: String,
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,
    #[serde(default)]
    pub music_folder: Option<PathBuf>,
    #[serde(default)]
    pub music_folders: Vec<PathBuf>,
    #[serde(default)]
    pub download_folder: Option<PathBuf>,
    #[serde(default = "default_resume_last_song")]
    pub resume_last_song: bool,
    #[serde(default = "default_volume")]
    pub volume: f32,
}

fn default_seek_interval() -> u32 {
    10
}

fn default_equalizer_gains() -> [f32; 5] {
    [0.0; 5]
}

fn default_equalizer_preset() -> String {
    "Flat".to_string()
}

fn default_theme_mode() -> String {
    "dark".to_string()
}

fn default_resume_last_song() -> bool {
    true
}

fn default_volume() -> f32 {
    0.9
}

impl Default for Store {
    fn default() -> Self {
        Self {
            playlists: Vec::new(),
            favourites: Vec::new(),
            seek_interval_seconds: 10,
            equalizer_enabled: false,
            equalizer_gains: [0.0; 5],
            equalizer_preset: "Flat".to_string(),
            theme_mode: "dark".to_string(),
            music_folder: None,
            music_folders: Vec::new(),
            download_folder: None,
            resume_last_song: true,
            volume: 0.9,
        }
    }
}

impl Store {
    pub fn all_music_folders(&self) -> Vec<PathBuf> {
        let mut list = self.music_folders.clone();
        if let Some(f) = self.music_folder.as_ref() {
            if !list.contains(f) {
                list.insert(0, f.clone());
            }
        }
        list
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPlaylist {
    pub name: String,
    pub paths: Vec<PathBuf>,
}

impl Store {
    pub fn default_path() -> Result<PathBuf> {
        Ok(dirs::data_local_dir()
            .context("Local application data directory unavailable")?
            .join("noir-player")
            .join("playlists.json"))
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(error).context("Could not read saved playlists"),
        };
        let mut store: Self =
            serde_json::from_slice(&bytes).context("Invalid saved playlist JSON")?;
        let mut names = Vec::new();
        for playlist in &mut store.playlists {
            playlist.name = validate_name(&playlist.name, names.iter().map(String::as_str))?;
            names.push(playlist.name.clone());
            deduplicate(&mut playlist.paths);
        }
        deduplicate(&mut store.favourites);
        Ok(store)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("Invalid playlist storage path")?;
        let bytes = serde_json::to_vec_pretty(self).context("Could not encode playlists")?;
        fs::create_dir_all(parent).context("Could not create playlist storage directory")?;
        let temporary = path.with_extension("json.tmp");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .context("Could not create playlist save file; check for a stale playlists.json.tmp")?;
        let result = (|| -> Result<()> {
            file.write_all(&bytes)
                .context("Could not write playlists")?;
            file.sync_all().context("Could not flush playlists")?;
            drop(file);
            fs::rename(&temporary, path).context("Could not replace saved playlists")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub fn create_playlist(&mut self, name: &str) -> Result<String> {
        let name = validate_name(
            name,
            self.playlists.iter().map(|playlist| playlist.name.as_str()),
        )?;
        self.playlists.push(SavedPlaylist {
            name: name.clone(),
            paths: Vec::new(),
        });
        Ok(name)
    }
}

fn deduplicate(paths: &mut Vec<PathBuf>) {
    let mut seen = std::collections::HashSet::new();
    paths.retain(|path| seen.insert(path.clone()));
}

pub fn validate_name<'a>(
    name: &str,
    existing: impl IntoIterator<Item = &'a str>,
) -> Result<String> {
    if name.chars().any(char::is_control) {
        bail!("Playlist names cannot contain control characters");
    }
    let name = name.trim();
    if name.is_empty() {
        bail!("Enter a playlist name");
    }
    if name.chars().count() > 80 {
        bail!("Playlist names must be 80 characters or fewer");
    }
    if existing
        .into_iter()
        .any(|other| other.to_lowercase() == name.to_lowercase())
    {
        bail!("A playlist with that name already exists");
    }
    Ok(name.to_owned())
}

pub fn matches_query(title: &str, artist: &str, album: &str, query: &str) -> bool {
    let fields = [
        title.to_lowercase(),
        artist.to_lowercase(),
        album.to_lowercase(),
    ];
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| fields.iter().any(|field| field.contains(word)))
}

pub fn resolve_paths(paths: &[PathBuf], tracks: &[Track]) -> Vec<usize> {
    paths
        .iter()
        .filter_map(|path| tracks.iter().position(|track| &track.path == path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "noir-store-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn validates_and_normalizes_names() {
        assert_eq!(validate_name("  Evening Mix  ", []).unwrap(), "Evening Mix");
        assert!(validate_name("   ", []).is_err());
        assert!(validate_name("bad\nname", []).is_err());
        assert!(validate_name(&"a".repeat(81), []).is_err());
        assert!(validate_name(" MIX ", ["Mix"]).is_err());
        assert!(validate_name("été", ["ÉTÉ"]).is_err());
        assert!(validate_name("Road / Trip", []).is_ok());
    }

    #[test]
    fn persists_paths_and_replaces_existing_save() {
        let temp = TempDir::new();
        let path = temp.0.join("data").join("playlists.json");
        assert_eq!(Store::load(&path).unwrap(), Store::default());
        let mut store = Store::default();
        store.create_playlist(" Mix ").unwrap();
        store.playlists[0].paths = vec![PathBuf::from("missing.flac"), PathBuf::from("歌曲.mp3")];
        store.favourites.push(PathBuf::from("missing.flac"));
        store.save(&path).unwrap();
        assert_eq!(Store::load(&path).unwrap(), store);
        store.create_playlist("Second").unwrap();
        store.save(&path).unwrap();
        assert_eq!(Store::load(&path).unwrap(), store);
    }

    #[test]
    fn invalid_store_is_not_silently_discarded() {
        let temp = TempDir::new();
        let path = temp.0.join("playlists.json");
        fs::write(&path, b"not json").unwrap();
        assert!(Store::load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"not json");
        fs::write(
            &path,
            br#"{"playlists":[{"name":"Mix","paths":[]},{"name":"mix","paths":[]}]}"#,
        )
        .unwrap();
        assert!(Store::load(&path).is_err());
    }

    #[test]
    fn failed_save_preserves_existing_store() {
        let temp = TempDir::new();
        let path = temp.0.join("playlists.json");
        Store::default().save(&path).unwrap();
        fs::write(path.with_extension("json.tmp"), b"occupied").unwrap();
        let mut changed = Store::default();
        changed.create_playlist("Unsaved").unwrap();
        assert!(changed.save(&path).is_err());
        assert_eq!(Store::load(&path).unwrap(), Store::default());
    }

    #[test]
    fn filters_all_metadata_case_insensitively() {
        assert!(matches_query(
            "Night Drive",
            "The Artist",
            "Blue Album",
            "  "
        ));
        for query in ["NIGHT", "artist", "blue", "drive artist blue"] {
            assert!(matches_query(
                "Night Drive",
                "The Artist",
                "Blue Album",
                query
            ));
        }
        assert!(!matches_query(
            "Night Drive",
            "The Artist",
            "Blue Album",
            "night missing"
        ));
    }

    #[test]
    fn rescan_resolves_paths_in_collection_order_without_losing_missing_paths() {
        let make_track = |path: &str| Track {
            path: path.into(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            duration: Default::default(),
            artwork: None,
        };
        let paths = vec![
            PathBuf::from("b.mp3"),
            PathBuf::from("missing.mp3"),
            PathBuf::from("a.mp3"),
        ];
        assert_eq!(
            resolve_paths(&paths, &[make_track("a.mp3"), make_track("b.mp3")]),
            vec![1, 0]
        );
        assert_eq!(
            resolve_paths(
                &paths,
                &[
                    make_track("b.mp3"),
                    make_track("missing.mp3"),
                    make_track("a.mp3")
                ]
            ),
            vec![0, 1, 2]
        );
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn settings_persistence_and_defaults() {
        let store = Store::default();
        assert_eq!(store.seek_interval_seconds, 10);
        assert!(!store.equalizer_enabled);
        assert_eq!(store.equalizer_gains, [0.0; 5]);
        assert_eq!(store.equalizer_preset, "Flat");

        // Test backward compatibility: legacy JSON without settings fields
        let legacy_json = r#"{"playlists":[],"favourites":[]}"#;
        let loaded: Store = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(loaded.seek_interval_seconds, 10);
        assert!(!loaded.equalizer_enabled);
        assert_eq!(loaded.equalizer_gains, [0.0; 5]);
        assert_eq!(loaded.equalizer_preset, "Flat");

        // Test saving customized settings and reloading
        let temp = TempDir::new();
        let path = temp.0.join("settings_test.json");
        let mut custom = Store {
            seek_interval_seconds: 5,
            equalizer_enabled: true,
            equalizer_gains: [6.0, 4.0, 1.0, 0.0, 0.0],
            equalizer_preset: "Bass Boost".to_string(),
            ..Default::default()
        };
        custom.save(&path).unwrap();

        let reloaded = Store::load(&path).unwrap();
        assert_eq!(reloaded.seek_interval_seconds, 5);
        assert!(reloaded.equalizer_enabled);
        assert_eq!(reloaded.equalizer_gains, [6.0, 4.0, 1.0, 0.0, 0.0]);
        assert_eq!(reloaded.equalizer_preset, "Bass Boost");

        // Test music_folders and download_folder persistence
        custom.music_folders = vec![
            PathBuf::from("/music/folder1"),
            PathBuf::from("/music/folder2"),
        ];
        custom.download_folder = Some(PathBuf::from("/music/downloads"));
        custom.save(&path).unwrap();

        let reloaded_folders = Store::load(&path).unwrap();
        assert_eq!(reloaded_folders.music_folders.len(), 2);
        assert_eq!(
            reloaded_folders.download_folder,
            Some(PathBuf::from("/music/downloads"))
        );
        assert_eq!(reloaded_folders.all_music_folders().len(), 2);
    }

    #[test]
    fn unknown_fields_in_json_are_gracefully_ignored() {
        let json = r#"{
            "playlists": [],
            "favourites": [],
            "unknown_future_field": "some_value",
            "another_field": 12345
        }"#;
        let loaded: Result<Store, _> = serde_json::from_str(json);
        assert!(
            loaded.is_ok(),
            "Unknown fields should be tolerated for schema evolution"
        );
    }
}
