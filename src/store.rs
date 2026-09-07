use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::media::Track;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Store {
    #[serde(default)]
    pub playlists: Vec<SavedPlaylist>,
    #[serde(default)]
    pub favourites: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
}
