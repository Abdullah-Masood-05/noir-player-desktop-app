use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::button::Button;
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::media::{self, MediaPlayer, Track};
use crate::views::{discover, library, player, playlists};
use crate::widgets::{player_bar, queue_panel, sidebar};

#[path = "store.rs"]
pub mod store;

use store::Store;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActiveTab {
    Library,
    Player,
    Playlists,
    Discover,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LibraryTab {
    Music,
    /// The song list on its own, without the greeting and recent shelves.
    AllSongs,
    Favourites,
    Albums,
    Artists,
    Folders,
    RecentlyPlayed,
}

/// Ordering applied to every song list in the library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortMode {
    TitleAsc,
    TitleDesc,
    Artist,
    Album,
    Duration,
    RecentlyAdded,
}

impl SortMode {
    pub const ALL: [SortMode; 6] = [
        SortMode::TitleAsc,
        SortMode::TitleDesc,
        SortMode::Artist,
        SortMode::Album,
        SortMode::Duration,
        SortMode::RecentlyAdded,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortMode::TitleAsc => "A to Z",
            SortMode::TitleDesc => "Z to A",
            SortMode::Artist => "Artist",
            SortMode::Album => "Album",
            SortMode::Duration => "Duration",
            SortMode::RecentlyAdded => "Recently Added",
        }
    }

    /// Only the alphabetical orderings are broken into letter sections.
    pub fn has_letter_sections(self) -> bool {
        matches!(self, SortMode::TitleAsc | SortMode::TitleDesc)
    }
}

/// The letter a song is filed under. Titles that do not start with a Latin
/// letter (numbers, symbols, other scripts) share the leading `#` section.
pub fn section_letter(title: &str) -> char {
    match title.trim_start().chars().next() {
        Some(letter) if letter.is_ascii_alphabetic() => letter.to_ascii_uppercase(),
        _ => '#',
    }
}

/// A destination in the left navigation rail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDestination {
    Library,
    Favourites,
    Albums,
    Artists,
    Folders,
    Playlists,
    Discover,
    RecentlyPlayed,
}

impl NavDestination {
    pub fn label(self) -> &'static str {
        match self {
            NavDestination::Library => "Library",
            NavDestination::Favourites => "Favorites",
            NavDestination::Albums => "Albums",
            NavDestination::Artists => "Artists",
            NavDestination::Folders => "Folders",
            NavDestination::Playlists => "Playlists",
            NavDestination::Discover => "Discover",
            NavDestination::RecentlyPlayed => "Recently Played",
        }
    }

    /// The library tab a destination selects, if it is a library view.
    fn library_tab(self) -> Option<LibraryTab> {
        match self {
            NavDestination::Library => Some(LibraryTab::Music),
            NavDestination::Favourites => Some(LibraryTab::Favourites),
            NavDestination::Albums => Some(LibraryTab::Albums),
            NavDestination::Artists => Some(LibraryTab::Artists),
            NavDestination::Folders => Some(LibraryTab::Folders),
            NavDestination::RecentlyPlayed => Some(LibraryTab::RecentlyPlayed),
            NavDestination::Playlists | NavDestination::Discover => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Collection {
    Album(String),
    Artist(String),
    Playlist(String),
    Folder(PathBuf),
}

impl Collection {
    pub fn name(&self) -> &str {
        match self {
            Self::Album(name) | Self::Artist(name) | Self::Playlist(name) => name,
            Self::Folder(path) => path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Folder"),
        }
    }
}

/// The song an action sheet is open for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongMenu {
    pub index: usize,
    /// Set when the sheet was opened from a playlist, which adds a remove action.
    pub playlist: Option<String>,
}

/// A running update download. Dropping it cancels the transfer.
struct UpdateJob {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    received: std::sync::Arc<std::sync::atomic::AtomicU64>,
    total: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl Drop for UpdateJob {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

struct DiscoverAudioJob {
    request: u64,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    progress: std::sync::Arc<std::sync::Mutex<String>>,
    preparing: bool,
}

impl Drop for DiscoverAudioJob {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

pub struct NoirPlayerModel {
    pub active_tab: ActiveTab,
    pub library_tab: LibraryTab,
    pub library_search: Entity<InputState>,
    pub library_detail: Option<Collection>,
    pub selected_folder_filter: Option<PathBuf>,
    pub playlist_detail: Option<String>,
    pub tracks: Vec<Track>,
    pub albums: Vec<(String, Vec<usize>)>,
    pub artists: Vec<(String, Vec<usize>)>,
    pub store: Store,
    store_path: Option<PathBuf>,
    pub storage_error: Option<String>,
    pub error: Option<String>,
    pub current: Option<usize>,
    pub queue: Vec<usize>,
    pub is_playing: bool,
    pub player: Option<MediaPlayer>,
    pub status: SharedString,
    pub scanning: bool,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat_all: bool,
    pub discover_search: Entity<InputState>,
    pub discover_tracks: Vec<crate::api::Track>,
    pub discover_loading: bool,
    pub discover_error: Option<String>,
    pub discover_request: u64,
    pub discover_audio_status: Option<String>,
    pub discover_audio_error: Option<String>,
    discover_audio_request: u64,
    discover_audio_job: Option<DiscoverAudioJob>,
    discover_cached: Vec<media::PreparedAudio>,
    discover_downloads: Vec<Track>,
    pub song_menu: Option<SongMenu>,
    /// Resuming happens once, after the first scan finds the library.
    resume_attempted: bool,
    pub sort_mode: SortMode,
    pub sort_menu_open: bool,
    pub media_menu_open: bool,
    pub queue_open: bool,
    pub lyrics_open: bool,
    pub lyrics: Option<String>,
    lyrics_path: Option<PathBuf>,
    /// Laid-out bounds of the player bar scrubber, used to map clicks to a time.
    pub seek_bounds: Option<Bounds<Pixels>>,
    /// Laid-out bounds of the volume slider track.
    pub volume_bounds: Option<Bounds<Pixels>>,
    pub settings_open: bool,
    pub equalizer_open: bool,
    pub root_focus_handle: FocusHandle,
    pub settings_focus_handle: FocusHandle,
    pub equalizer_focus_handle: FocusHandle,
    pub settings_category: crate::views::settings::SettingsCategory,
    pub settings_search: Entity<InputState>,
    pub settings_selected_index: usize,
    pub volume_hud_until: Option<std::time::Instant>,
    pub update_status: crate::update::UpdateStatus,
    pub update_dialog_open: bool,
    pub latest_release: Option<crate::update::ReleaseInfo>,
    update_job: Option<UpdateJob>,
    _subscriptions: Vec<Subscription>,
    dialog_subscription: Option<Subscription>,
}

impl NoirPlayerModel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (player, error) = match MediaPlayer::new() {
            Ok(player) => (Some(player), None),
            Err(error) => (None, Some(format!("Audio device unavailable: {error:#}"))),
        };
        let (store, store_path, storage_error) = match Store::default_path() {
            Ok(path) => match Store::load(&path) {
                Ok(store) => (store, Some(path), None),
                Err(error) => (
                    Store::default(),
                    None,
                    Some(format!("{error:#}. Saved data was not changed.")),
                ),
            },
            Err(error) => (Store::default(), None, Some(format!("{error:#}"))),
        };
        let library_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search title, artist or album"));
        let discover_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search tracks (Enter)"));
        let settings_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Select an option..."));
        let subscriptions = vec![
            cx.subscribe(&library_search, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe(&discover_search, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.load_discover(input.read(cx).value().to_string(), cx);
                }
            }),
            cx.subscribe(&settings_search, |_, _, _: &InputEvent, cx| cx.notify()),
        ];
        let volume = store.volume;
        if let Some(player) = player.as_ref() {
            player.set_volume(volume);
            player.set_equalizer_enabled(store.equalizer_enabled);
            player.set_equalizer_gains(store.equalizer_gains);
        }
        cx.spawn(async move |this, cx| loop {
            smol::Timer::after(Duration::from_millis(250)).await;
            if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                break;
            }
        })
        .detach();
        let mut model = Self {
            active_tab: ActiveTab::Library,
            library_tab: LibraryTab::Music,
            library_search,
            library_detail: None,
            selected_folder_filter: None,
            playlist_detail: None,
            tracks: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
            store,
            store_path,
            storage_error,
            error,
            current: None,
            queue: Vec::new(),
            is_playing: false,
            player,
            status: "Loading library...".into(),
            scanning: false,
            volume,
            volume_hud_until: None,
            shuffle: false,
            repeat_all: true,
            discover_search,
            discover_tracks: Vec::new(),
            discover_loading: false,
            discover_error: None,
            discover_request: 0,
            discover_audio_status: None,
            discover_audio_error: None,
            discover_audio_request: 0,
            discover_audio_job: None,
            discover_cached: Vec::new(),
            discover_downloads: Vec::new(),
            song_menu: None,
            resume_attempted: false,
            sort_mode: SortMode::TitleAsc,
            sort_menu_open: false,
            media_menu_open: false,
            queue_open: true,
            lyrics_open: false,
            lyrics: None,
            lyrics_path: None,
            seek_bounds: None,
            volume_bounds: None,
            settings_open: false,
            equalizer_open: false,
            root_focus_handle: cx.focus_handle(),
            settings_focus_handle: cx.focus_handle(),
            equalizer_focus_handle: cx.focus_handle(),
            settings_category: crate::views::settings::SettingsCategory::All,
            settings_search,
            settings_selected_index: 0,
            update_status: crate::update::UpdateStatus::Idle,
            update_dialog_open: false,
            latest_release: None,
            update_job: None,
            _subscriptions: subscriptions,
            dialog_subscription: None,
        };
        if model.store.auto_check_updates {
            cx.spawn(async move |this, cx| {
                smol::Timer::after(Duration::from_secs(3)).await;
                let _ = this.update(cx, |this, cx| {
                    this.check_for_updates(false, cx);
                });
            })
            .detach();
        }
        model.rescan(cx);
        model
    }

    pub fn load_discover(&mut self, query: String, cx: &mut Context<Self>) {
        self.discover_request = self.discover_request.wrapping_add(1);
        let request = self.discover_request;
        self.discover_loading = true;
        self.discover_error = None;
        self.discover_tracks.clear();
        let query = query.trim().to_owned();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if query.is_empty() {
                        crate::api::fetch_trending()
                    } else {
                        crate::api::search_tracks(&query)
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.discover_request != request {
                    return;
                }
                this.discover_loading = false;
                match result {
                    Ok(tracks) => this.discover_tracks = tracks,
                    Err(error) => this.discover_error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn discover_audio_busy(&self) -> bool {
        self.discover_audio_job.is_some()
    }

    pub fn check_for_updates(&mut self, _manual: bool, cx: &mut Context<Self>) {
        if self.update_status == crate::update::UpdateStatus::Checking {
            return;
        }
        self.update_status = crate::update::UpdateStatus::Checking;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::update::check_latest_release(Duration::from_secs(10)) })
                .await;

            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Some(release)) => {
                        this.latest_release = Some(release.clone());
                        this.update_status = crate::update::UpdateStatus::Available(release);
                        this.update_dialog_open = true;
                    }
                    Ok(None) => {
                        this.update_status = crate::update::UpdateStatus::UpToDate;
                    }
                    Err(error) => {
                        this.update_status = crate::update::UpdateStatus::Error(error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Downloads the installer for the available release and verifies it
    /// against the checksum published with the release.
    pub fn start_update_download(&mut self, cx: &mut Context<Self>) {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        use std::sync::Arc;

        if self.update_job.is_some() {
            return;
        }
        let Some(release) = self.latest_release.clone() else {
            return;
        };
        let Some(asset) = release.platform_asset().cloned() else {
            self.update_status = crate::update::UpdateStatus::Error(format!(
                "Release {} has no installer for this platform.",
                release.tag_name
            ));
            cx.notify();
            return;
        };

        let cancel = Arc::new(AtomicBool::new(false));
        let received = Arc::new(AtomicU64::new(0));
        let total = Arc::new(AtomicU64::new(asset.size));
        self.update_job = Some(UpdateJob {
            cancel: cancel.clone(),
            received: received.clone(),
            total: total.clone(),
        });
        self.update_status = crate::update::UpdateStatus::Downloading {
            received: 0,
            total: asset.size,
        };
        self.update_dialog_open = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let asset_name = asset.name.clone();
            let download = cx
                .background_executor()
                .spawn(async move {
                    let dir = crate::update::update_dir()?;
                    crate::update::download_asset(&asset, &dir, &cancel, &|got, size| {
                        received.store(got, Ordering::Relaxed);
                        total.store(size, Ordering::Relaxed);
                    })
                })
                .await;

            let path = match download {
                Ok(path) => path,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.finish_update_job(Err(format!("{error:#}")), cx);
                    });
                    return;
                }
            };

            let _ = this.update(cx, |this, cx| {
                this.update_status = crate::update::UpdateStatus::Verifying;
                cx.notify();
            });

            let verify_path = path.clone();
            let verified = cx
                .background_executor()
                .spawn(async move {
                    let expected = crate::update::expected_checksum(
                        &release,
                        &asset_name,
                        Duration::from_secs(30),
                    )?;
                    let Some(expected) = expected else {
                        anyhow::bail!(
                            "This release publishes no checksum, so the download cannot be verified."
                        );
                    };
                    let actual = crate::update::sha256_file(&verify_path)?;
                    if !actual.eq_ignore_ascii_case(&expected) {
                        let _ = std::fs::remove_file(&verify_path);
                        anyhow::bail!("The download did not match its checksum and was deleted.");
                    }
                    Ok(())
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.finish_update_job(
                    verified.map(|()| path).map_err(|error| format!("{error:#}")),
                    cx,
                );
            });
        })
        .detach();
    }

    fn finish_update_job(&mut self, result: Result<PathBuf, String>, cx: &mut Context<Self>) {
        self.update_job = None;
        self.update_status = match result {
            Ok(path) => crate::update::UpdateStatus::Ready(path),
            Err(error) => crate::update::UpdateStatus::Error(error),
        };
        cx.notify();
    }

    pub fn cancel_update_download(&mut self, cx: &mut Context<Self>) {
        if self.update_job.take().is_none() {
            return;
        }
        self.update_status = match self.latest_release.clone() {
            Some(release) => crate::update::UpdateStatus::Available(release),
            None => crate::update::UpdateStatus::Idle,
        };
        cx.notify();
    }

    /// Hands the verified installer to the platform and quits, so it can
    /// replace files this process is holding open.
    pub fn install_update(&mut self, cx: &mut Context<Self>) {
        let crate::update::UpdateStatus::Ready(path) = self.update_status.clone() else {
            return;
        };
        self.update_status = crate::update::UpdateStatus::Installing;
        cx.notify();

        match crate::update::launch_installer(&path) {
            Ok(()) if crate::update::installs_in_place() => {
                self.is_playing = false;
                if let Some(player) = self.player.as_mut() {
                    player.stop();
                }
                cx.quit();
            }
            Ok(()) => {
                // The system package installer took over; stay running.
                self.update_status = crate::update::UpdateStatus::Ready(path);
                self.update_dialog_open = false;
                cx.notify();
            }
            Err(error) => {
                self.update_status = crate::update::UpdateStatus::Error(format!("{error:#}"));
                cx.notify();
            }
        }
    }

    pub fn cancel_discover_audio(&mut self, cx: &mut Context<Self>) {
        if self.discover_audio_job.take().is_some() {
            self.discover_audio_request = self.discover_audio_request.wrapping_add(1);
            self.discover_audio_status = Some("Audio request cancelled.".into());
        }
        cx.notify();
    }

    fn cancel_preparing_audio(&mut self, cx: &mut Context<Self>) {
        if self
            .discover_audio_job
            .as_ref()
            .is_some_and(|job| job.preparing)
        {
            self.cancel_discover_audio(cx);
        }
    }

    pub fn discover_audio(
        &mut self,
        source: crate::api::Track,
        download: bool,
        cx: &mut Context<Self>,
    ) {
        if !download {
            if let Some(index) = self.tracks.iter().position(|track| {
                track.title.trim().eq_ignore_ascii_case(source.name.trim())
                    && track
                        .artist
                        .trim()
                        .eq_ignore_ascii_case(source.artist.trim())
            }) {
                self.play_index(index, cx);
                return;
            }
            if self.player.is_none() {
                self.discover_audio_error =
                    Some("Audio device unavailable. Download is still available.".into());
                cx.notify();
                return;
            }
        }
        self.cancel_discover_audio(cx);
        self.discover_audio_request = self.discover_audio_request.wrapping_add(1);
        let request = self.discover_audio_request;
        let progress = std::sync::Arc::new(std::sync::Mutex::new("Starting...".to_owned()));
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.discover_audio_job = Some(DiscoverAudioJob {
            request,
            cancel: cancel.clone(),
            progress: progress.clone(),
            preparing: !download,
        });
        self.discover_audio_error = None;
        self.discover_audio_status = Some(if download {
            format!("Downloading {} — {}...", source.name, source.artist)
        } else {
            format!(
                "Preparing audio: {} — {} (cached download, not streaming)...",
                source.name, source.artist
            )
        });
        let cached = self
            .discover_cached
            .iter()
            .find(|cached| {
                cached.track.title == source.name && cached.track.artist == source.artist
            })
            .map(|cached| cached.track.path.clone());
        let download_folder = if download {
            Some(self.effective_download_folder())
        } else {
            None
        };
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    media::prepare_discover_audio(
                        &source,
                        download,
                        cached.as_deref(),
                        download_folder.as_deref(),
                        &cancel,
                        &|message| {
                            if let Ok(mut value) = progress.lock() {
                                *value = message.to_owned();
                            }
                        },
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.discover_audio_request != request
                    || !this
                        .discover_audio_job
                        .as_ref()
                        .is_some_and(|job| job.request == request)
                {
                    return;
                }
                this.discover_audio_job = None;
                match result {
                    Ok(mut prepared) => {
                        let path = prepared.track.path.clone();
                        let mut tracks = this.tracks.clone();
                        if !tracks.iter().any(|track| track.path == path) {
                            tracks.push(prepared.track.clone());
                        }
                        if download {
                            prepared.keep();
                            this.discover_downloads.push(prepared.track.clone());
                            this.discover_audio_status =
                                Some(format!("Downloaded to {}", path.display()));
                        } else {
                            this.discover_audio_status =
                                Some("Audio prepared. Playing local cached audio.".into());
                            this.discover_cached.push(prepared);
                        }
                        this.set_library(tracks, &[], cx);
                        if !download {
                            if let Some(index) =
                                this.tracks.iter().position(|track| track.path == path)
                            {
                                if this.load_index(index, cx) {
                                    this.queue = vec![index];
                                } else {
                                    this.discover_audio_status = None;
                                    this.discover_audio_error = this.error.clone();
                                }
                            }
                        }
                    }
                    Err(error) => {
                        this.discover_audio_status = None;
                        this.discover_audio_error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn rescan(&mut self, cx: &mut Context<Self>) {
        if self.scanning {
            return;
        }
        let folders = self.effective_music_folders();
        if folders.is_empty() {
            self.status = "Music folder unavailable".into();
            self.error = Some(
                "Could not locate any configured music folders or the system Music folder".into(),
            );
            cx.notify();
            return;
        }
        self.scanning = true;
        self.status = "Scanning music folders...".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { media::scan_folders(&folders) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.scanning = false;
                this.set_library(result.tracks, &result.errors, cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub fn set_library(
        &mut self,
        mut tracks: Vec<Track>,
        errors: &[String],
        cx: &mut Context<Self>,
    ) {
        for track in self
            .discover_cached
            .iter()
            .map(|cached| &cached.track)
            .chain(self.discover_downloads.iter())
        {
            if track.path.is_file() && !tracks.iter().any(|entry| entry.path == track.path) {
                tracks.push(track.clone());
            }
        }
        let current_path = self.now_playing().map(|track| track.path.clone());
        let queue_paths: Vec<_> = self
            .queue
            .iter()
            .filter_map(|&index| self.tracks.get(index).map(|track| track.path.clone()))
            .collect();
        self.status = format!("{} songs", tracks.len()).into();
        if !errors.is_empty() {
            self.error = Some(format!(
                "{} scan error(s): {}",
                errors.len(),
                errors.join("; ")
            ));
        }
        let mut album_map: Vec<(String, Vec<usize>)> = Vec::new();
        let mut artist_map: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, track) in tracks.iter().enumerate() {
            match album_map.iter_mut().find(|(name, _)| *name == track.album) {
                Some((_, list)) => list.push(i),
                None => album_map.push((track.album.clone(), vec![i])),
            }
            match artist_map
                .iter_mut()
                .find(|(name, _)| *name == track.artist)
            {
                Some((_, list)) => list.push(i),
                None => artist_map.push((track.artist.clone(), vec![i])),
            }
        }
        album_map.sort_by_key(|a| a.0.to_lowercase());
        artist_map.sort_by_key(|a| a.0.to_lowercase());
        self.tracks = tracks;
        self.albums = album_map;
        self.artists = artist_map;
        self.queue = store::resolve_paths(&queue_paths, &self.tracks);
        self.current =
            current_path.and_then(|path| self.tracks.iter().position(|track| track.path == path));
        if self.current.is_none() {
            self.is_playing = false;
            if let Some(player) = self.player.as_mut() {
                player.stop();
            }
        }
        self.resume_last_song(cx);
        cx.notify();
    }

    pub fn collection_indices(&self, collection: &Collection) -> Vec<usize> {
        match collection {
            Collection::Album(name) => self
                .albums
                .iter()
                .find(|(album, _)| album == name)
                .map(|(_, indices)| indices.clone())
                .unwrap_or_default(),
            Collection::Artist(name) => self
                .artists
                .iter()
                .find(|(artist, _)| artist == name)
                .map(|(_, indices)| indices.clone())
                .unwrap_or_default(),
            Collection::Playlist(name) => self
                .store
                .playlists
                .iter()
                .find(|playlist| &playlist.name == name)
                .map(|playlist| store::resolve_paths(&playlist.paths, &self.tracks))
                .unwrap_or_default(),
            Collection::Folder(path) => self
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, track)| track.path.starts_with(path))
                .map(|(index, _)| index)
                .collect(),
        }
    }

    pub fn filtered_indices(&self, indices: Vec<usize>, cx: &App) -> Vec<usize> {
        let query = self.library_search.read(cx).value();
        indices
            .into_iter()
            .filter(|&index| {
                self.tracks.get(index).is_some_and(|track| {
                    store::matches_query(&track.title, &track.artist, &track.album, &query)
                })
            })
            .collect()
    }

    /// Applies the active sort order to `indices`.
    pub fn sorted_indices(&self, mut indices: Vec<usize>) -> Vec<usize> {
        let title_key = |index: &usize| -> (u8, char, String, String) {
            let track = &self.tracks[*index];
            let letter = section_letter(&track.title);
            (
                u8::from(letter != '#'),
                letter,
                track.title.to_lowercase(),
                track.artist.to_lowercase(),
            )
        };
        indices.retain(|&index| index < self.tracks.len());
        match self.sort_mode {
            SortMode::TitleAsc => indices.sort_by_cached_key(title_key),
            SortMode::TitleDesc => {
                indices.sort_by_cached_key(title_key);
                indices.reverse();
            }
            SortMode::Artist => indices.sort_by_cached_key(|&index| {
                let track = &self.tracks[index];
                (
                    track.artist.to_lowercase(),
                    track.album.to_lowercase(),
                    track.title.to_lowercase(),
                )
            }),
            SortMode::Album => indices.sort_by_cached_key(|&index| {
                let track = &self.tracks[index];
                (
                    track.album.to_lowercase(),
                    track.title.to_lowercase(),
                    track.artist.to_lowercase(),
                )
            }),
            SortMode::Duration => indices.sort_by_cached_key(|&index| {
                let track = &self.tracks[index];
                (track.duration, track.title.to_lowercase())
            }),
            SortMode::RecentlyAdded => indices.sort_by_cached_key(|&index| {
                let track = &self.tracks[index];
                // Newest first; files without a timestamp sort last.
                (std::cmp::Reverse(track.added), track.title.to_lowercase())
            }),
        }
        indices
    }

    /// Splits sorted `indices` into the sections drawn in the song list. In an
    /// alphabetical order every section carries its letter heading; other
    /// orders return a single unlabelled section.
    pub fn letter_sections(&self, indices: Vec<usize>) -> Vec<(Option<char>, Vec<usize>)> {
        if !self.sort_mode.has_letter_sections() {
            return if indices.is_empty() {
                Vec::new()
            } else {
                vec![(None, indices)]
            };
        }
        let mut sections: Vec<(Option<char>, Vec<usize>)> = Vec::new();
        for index in indices {
            let letter = section_letter(&self.tracks[index].title);
            match sections.last_mut() {
                Some((Some(current), songs)) if *current == letter => songs.push(index),
                _ => sections.push((Some(letter), vec![index])),
            }
        }
        sections
    }

    /// Forgets the play history. Only the list is dropped; the songs and
    /// everything else saved alongside them are untouched.
    pub fn clear_recently_played(&mut self, cx: &mut Context<Self>) {
        if self.store.recently_played.is_empty() {
            return;
        }
        let mut next = self.store.clone();
        next.recently_played.clear();
        self.save_store(next, cx);
        cx.notify();
    }

    /// Library indices of the songs played most recently, newest first.
    pub fn recently_played_indices(&self) -> Vec<usize> {
        store::resolve_paths(&self.store.recently_played, &self.tracks)
    }

    /// The indices backing the selected sidebar destination, before the search
    /// and the sort order are applied.
    pub fn tab_indices(&self) -> Vec<usize> {
        match self.library_tab {
            LibraryTab::Favourites => store::resolve_paths(&self.store.favourites, &self.tracks),
            LibraryTab::RecentlyPlayed => self.recently_played_indices(),
            _ => match self.selected_folder_filter.as_ref() {
                Some(folder) => (0..self.tracks.len())
                    .filter(|&index| self.tracks[index].path.starts_with(folder))
                    .collect(),
                None => (0..self.tracks.len()).collect(),
            },
        }
    }

    /// Whether a navigation rail destination is the one on screen.
    pub fn is_current_destination(&self, destination: NavDestination) -> bool {
        if self.settings_open {
            return false;
        }
        match destination {
            NavDestination::Playlists => self.active_tab == ActiveTab::Playlists,
            NavDestination::Discover => self.active_tab == ActiveTab::Discover,
            NavDestination::Library => {
                self.active_tab == ActiveTab::Library
                    && matches!(self.library_tab, LibraryTab::Music | LibraryTab::AllSongs)
            }
            other => {
                self.active_tab == ActiveTab::Library
                    && other.library_tab() == Some(self.library_tab)
            }
        }
    }

    pub fn navigate(&mut self, destination: NavDestination, cx: &mut Context<Self>) {
        self.settings_open = false;
        self.sort_menu_open = false;
        self.media_menu_open = false;
        match destination {
            NavDestination::Playlists => {
                self.active_tab = ActiveTab::Playlists;
                self.playlist_detail = None;
            }
            NavDestination::Discover => {
                self.active_tab = ActiveTab::Discover;
                if self.discover_request == 0 {
                    self.load_discover(String::new(), cx);
                }
            }
            other => {
                self.active_tab = ActiveTab::Library;
                self.library_detail = None;
                if let Some(tab) = other.library_tab() {
                    self.library_tab = tab;
                }
            }
        }
        cx.notify();
    }

    pub fn set_sort_mode(&mut self, mode: SortMode, cx: &mut Context<Self>) {
        self.sort_mode = mode;
        self.sort_menu_open = false;
        cx.notify();
    }

    pub fn set_folder_filter(&mut self, folder: Option<PathBuf>, cx: &mut Context<Self>) {
        self.selected_folder_filter = folder;
        self.media_menu_open = false;
        cx.notify();
    }

    pub fn close_menus(&mut self, cx: &mut Context<Self>) {
        if self.sort_menu_open || self.media_menu_open {
            self.sort_menu_open = false;
            self.media_menu_open = false;
            cx.notify();
        }
    }

    pub fn toggle_queue_panel(&mut self, cx: &mut Context<Self>) {
        self.queue_open = !self.queue_open;
        cx.notify();
    }

    /// Position of the playing song inside the queue.
    pub fn queue_position(&self) -> Option<usize> {
        let current = self.current?;
        self.queue.iter().position(|&index| index == current)
    }

    /// The playing song followed by everything still to come.
    pub fn upcoming_queue(&self) -> Vec<usize> {
        match self.queue_position() {
            Some(position) => self.queue[position..].to_vec(),
            None => self.queue.clone(),
        }
    }

    pub fn clear_queue(&mut self, cx: &mut Context<Self>) {
        self.queue = self.current.into_iter().collect();
        cx.notify();
    }

    pub fn add_to_queue(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tracks.len() && !self.queue.contains(&index) {
            self.queue.push(index);
        }
        cx.notify();
    }

    pub fn play_next(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tracks.len() {
            return;
        }
        self.queue.retain(|&queued| queued != index);
        match self.queue_position() {
            Some(position) => self.queue.insert(position + 1, index),
            None => self.queue.insert(0, index),
        }
        cx.notify();
    }

    pub fn remove_from_queue(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.current == Some(index) {
            return;
        }
        self.queue.retain(|&queued| queued != index);
        cx.notify();
    }

    /// Seeks the playing song to `fraction` of its duration.
    pub fn seek_fraction(&mut self, fraction: f32, cx: &mut Context<Self>) {
        let Some(player) = self.player.as_ref() else {
            return;
        };
        let Some(total) = player.duration().filter(|total| !total.is_zero()) else {
            return;
        };
        let target = total.mul_f32(fraction.clamp(0.0, 1.0));
        if let Err(error) = player.seek(target) {
            self.error = Some(format!("{error:#}"));
        }
        cx.notify();
    }

    pub fn toggle_lyrics(&mut self, cx: &mut Context<Self>) {
        self.lyrics_open = !self.lyrics_open;
        if self.lyrics_open {
            self.refresh_lyrics(cx);
        }
        cx.notify();
    }

    /// Loads the lyrics tag of the playing song in the background, once per file.
    pub fn refresh_lyrics(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.now_playing().map(|track| track.path.clone()) else {
            self.lyrics = None;
            self.lyrics_path = None;
            return;
        };
        if self.lyrics_path.as_ref() == Some(&path) {
            return;
        }
        self.lyrics_path = Some(path.clone());
        self.lyrics = None;
        cx.spawn(async move |this, cx| {
            let lookup = path.clone();
            let lyrics = cx
                .background_executor()
                .spawn(async move { crate::media::read_lyrics(&lookup) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.lyrics_path.as_ref() == Some(&path) {
                    this.lyrics = lyrics;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Opens the per-song action sheet behind a row's "more" button.
    pub fn song_actions_dialog(
        &mut self,
        index: usize,
        playlist: Option<String>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tracks.len() {
            return;
        }
        self.song_menu = Some(SongMenu { index, playlist });
        cx.notify();
    }

    pub fn close_song_menu(&mut self, cx: &mut Context<Self>) {
        if self.song_menu.take().is_some() {
            cx.notify();
        }
    }

    pub fn is_favourite(&self, index: usize) -> bool {
        self.tracks
            .get(index)
            .is_some_and(|track| self.store.favourites.contains(&track.path))
    }

    fn save_store(&mut self, next: Store, cx: &mut Context<Self>) -> bool {
        let result = self
            .store_path
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!("Playlist storage unavailable; saved data has not been overwritten")
            })
            .and_then(|path| next.save(path));
        match result {
            Ok(()) => {
                self.store = next;
                self.error = None;
                cx.notify();
                true
            }
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                cx.notify();
                false
            }
        }
    }

    pub fn save_current_store(&mut self, cx: &mut Context<Self>) -> bool {
        let store = self.store.clone();
        self.save_store(store, cx)
    }

    #[allow(dead_code)]
    pub fn open_settings(
        &mut self,
        category: crate::views::settings::SettingsCategory,
        cx: &mut Context<Self>,
    ) {
        self.settings_open = true;
        self.equalizer_open = false;
        self.settings_category = category;
        self.settings_selected_index = 0;
        cx.notify();
    }

    pub fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = false;
        cx.notify();
    }

    pub fn toggle_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        if self.settings_open {
            self.equalizer_open = false;
        }
        cx.notify();
    }

    pub fn effective_music_folders(&self) -> Vec<PathBuf> {
        let mut folders = self.store.all_music_folders();
        if folders.is_empty() {
            if let Some(default_folder) = media::default_music_folder() {
                folders.push(default_folder);
            }
        }
        folders
    }

    pub fn effective_download_folder(&self) -> PathBuf {
        if let Some(folder) = self.store.download_folder.as_ref() {
            if folder.is_dir() {
                return folder.clone();
            }
        }
        media::default_music_folder().unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn add_music_folder(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.into_iter().next() {
                    let _ = this.update(cx, |this, cx| {
                        if !this.store.music_folders.contains(&path) {
                            let mut next = this.store.clone();
                            if next.music_folders.is_empty() {
                                if let Some(default_folder) = media::default_music_folder() {
                                    if default_folder != path {
                                        next.music_folders.push(default_folder);
                                    }
                                }
                            }
                            next.music_folders.push(path);
                            this.save_store(next, cx);
                            this.rescan(cx);
                        }
                    });
                }
            }
        })
        .detach();
    }

    pub fn remove_music_folder(&mut self, folder: &std::path::Path, cx: &mut Context<Self>) {
        let mut next = self.store.clone();
        next.music_folders.retain(|p| p != folder);
        if next.music_folder.as_deref() == Some(folder) {
            next.music_folder = None;
        }
        self.save_store(next, cx);
        self.rescan(cx);
    }

    pub fn change_download_folder(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.into_iter().next() {
                    let _ = this.update(cx, |this, cx| {
                        let mut next = this.store.clone();
                        next.download_folder = Some(path);
                        this.save_store(next, cx);
                    });
                }
            }
        })
        .detach();
    }

    pub fn reset_download_folder(&mut self, cx: &mut Context<Self>) {
        let mut next = self.store.clone();
        next.download_folder = None;
        self.save_store(next, cx);
    }

    pub fn set_theme_mode(&mut self, mode: &str, cx: &mut Context<Self>) {
        let mut next = self.store.clone();
        next.theme_mode = mode.to_string();
        crate::views::ui::apply_theme(mode, cx);
        self.save_store(next, cx);
    }

    pub fn open_equalizer(&mut self, cx: &mut Context<Self>) {
        self.equalizer_open = true;
        self.settings_open = false;
        cx.notify();
    }

    pub fn close_equalizer(&mut self, cx: &mut Context<Self>) {
        self.equalizer_open = false;
        cx.notify();
    }

    pub fn toggle_equalizer(&mut self, cx: &mut Context<Self>) {
        self.equalizer_open = !self.equalizer_open;
        if self.equalizer_open {
            self.settings_open = false;
        }
        cx.notify();
    }

    pub fn skip_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(player) = self.player.as_ref() {
            let offset = Duration::from_secs(self.store.seek_interval_seconds as u64);
            let _ = player.skip_forward(offset);
            cx.notify();
        }
    }

    pub fn skip_backward(&mut self, cx: &mut Context<Self>) {
        if let Some(player) = self.player.as_ref() {
            let offset = Duration::from_secs(self.store.seek_interval_seconds as u64);
            let _ = player.skip_backward(offset);
            cx.notify();
        }
    }

    pub fn set_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        let volume = volume.clamp(0.0, 1.0);
        self.volume = volume;
        self.store.volume = volume;
        if let Some(player) = self.player.as_ref() {
            player.set_volume(volume);
        }
        self.volume_hud_until = Some(std::time::Instant::now() + Duration::from_millis(1500));
        self.save_current_store(cx);
        cx.spawn(async move |this, cx| {
            smol::Timer::after(Duration::from_millis(1550)).await;
            let _ = this.update(cx, |_, cx| {
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn adjust_volume(&mut self, delta: f32, cx: &mut Context<Self>) {
        let next = (self.volume + delta).clamp(0.0, 1.0);
        let rounded = (next * 20.0).round() / 20.0;
        self.set_volume(rounded, cx);
    }

    pub fn toggle_favourite(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(track) = self.tracks.get(index) else {
            return;
        };
        let mut next = self.store.clone();
        if next.favourites.contains(&track.path) {
            next.favourites.retain(|path| path != &track.path);
        } else {
            next.favourites.push(track.path.clone());
        }
        self.save_store(next, cx);
    }

    pub fn remove_from_playlist(
        &mut self,
        name: &str,
        path: &std::path::Path,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut next = self.store.clone();
        let Some(playlist) = next
            .playlists
            .iter_mut()
            .find(|playlist| playlist.name == name)
        else {
            return false;
        };
        playlist.paths.retain(|saved| saved != path);
        self.save_store(next, cx)
    }

    pub fn create_playlist_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Playlist name"));
        let error = cx.new(|_| None::<String>);
        let weak = cx.entity().downgrade();
        let enter_error = error.clone();
        self.dialog_subscription = Some(cx.subscribe_in(
            &input,
            window,
            move |this, input, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    if this.create_named_playlist(&input.read(cx).value(), &enter_error, cx) {
                        window.close_dialog(cx);
                    }
                }
                InputEvent::Change => {
                    enter_error.update(cx, |error, cx| {
                        *error = None;
                        cx.notify();
                    });
                    cx.notify();
                }
                _ => {}
            },
        ));
        input.update(cx, |input, cx| input.focus(window, cx));
        window.open_dialog(cx, move |dialog, _, cx| {
            let weak = weak.clone();
            let submit_input = input.clone();
            let submit_error = error.clone();
            dialog
                .title("New playlist")
                .child(Input::new(&input))
                .children(error.read(cx).clone().map(|message| {
                    div()
                        .text_sm()
                        .text_color(crate::views::ui::red())
                        .child(message)
                }))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Create")
                        .show_cancel(true),
                )
                .on_ok(move |_, _, cx| {
                    let name = submit_input.read(cx).value();
                    weak.update(cx, |this, cx| {
                        this.create_named_playlist(&name, &submit_error, cx)
                    })
                    .unwrap_or(false)
                })
        });
    }

    fn create_named_playlist(
        &mut self,
        name: &str,
        dialog_error: &Entity<Option<String>>,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut next = self.store.clone();
        let result = next.create_playlist(name);
        let success = match result {
            Ok(name) => {
                if self.save_store(next, cx) {
                    self.playlist_detail = Some(name);
                    self.active_tab = ActiveTab::Playlists;
                    true
                } else {
                    dialog_error.update(cx, |error, cx| {
                        *error = self.error.clone();
                        cx.notify();
                    });
                    false
                }
            }
            Err(error) => {
                dialog_error.update(cx, |message, cx| {
                    *message = Some(error.to_string());
                    cx.notify();
                });
                false
            }
        };
        cx.notify();
        success
    }

    pub fn add_to_playlist_dialog(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(track) = self.tracks.get(index) else {
            return;
        };
        let path = track.path.clone();
        let title = track.title.clone();
        let choices: Vec<_> = self
            .store
            .playlists
            .iter()
            .map(|playlist| (playlist.name.clone(), playlist.paths.contains(&path)))
            .collect();
        let weak = cx.entity().downgrade();
        let error = cx.new(|_| None::<String>);
        window.open_dialog(cx, move |dialog, _, cx| {
            let mut list = v_flex()
                .id("playlist-choices")
                .max_h(px(300.0))
                .overflow_y_scrollbar()
                .gap(px(8.0));
            if choices.is_empty() {
                list = list.child("No playlists yet. Create one in Playlists first.");
            }
            for (name, contains) in &choices {
                let name = name.clone();
                let path = path.clone();
                let weak = weak.clone();
                let error = error.clone();
                list = list.child(
                    Button::new(format!("choose-{name}"))
                        .label(if *contains {
                            format!("{name} (already added)")
                        } else {
                            name.clone()
                        })
                        .disabled(*contains)
                        .on_click(move |_, window, cx| {
                            let success = weak
                                .update(cx, |this, cx| {
                                    let mut next = this.store.clone();
                                    let Some(playlist) = next
                                        .playlists
                                        .iter_mut()
                                        .find(|playlist| playlist.name == name)
                                    else {
                                        return false;
                                    };
                                    if !playlist.paths.contains(&path) {
                                        playlist.paths.push(path.clone());
                                    }
                                    let saved = this.save_store(next, cx);
                                    if !saved {
                                        error.update(cx, |error, cx| {
                                            *error = this.error.clone();
                                            cx.notify();
                                        });
                                    }
                                    saved
                                })
                                .unwrap_or(false);
                            if success {
                                window.close_dialog(cx);
                            }
                        }),
                );
            }
            dialog
                .title(format!("Add {title} to playlist"))
                .child(list)
                .children(error.read(cx).clone().map(|message| {
                    div()
                        .text_sm()
                        .text_color(crate::views::ui::red())
                        .child(message)
                }))
                .button_props(DialogButtonProps::default().ok_text("Done"))
        });
    }

    pub fn tick(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.discover_audio_job {
            if let Ok(progress) = job.progress.lock() {
                self.discover_audio_status = Some(format!(
                    "{}: {}",
                    if job.preparing {
                        "Preparing audio"
                    } else {
                        "Download"
                    },
                    progress
                ));
            }
        }
        if let Some(job) = self.update_job.as_ref() {
            if matches!(
                self.update_status,
                crate::update::UpdateStatus::Downloading { .. }
            ) {
                use std::sync::atomic::Ordering;
                self.update_status = crate::update::UpdateStatus::Downloading {
                    received: job.received.load(Ordering::Relaxed),
                    total: job.total.load(Ordering::Relaxed),
                };
            }
        }
        if self.is_playing && self.player.as_ref().is_some_and(MediaPlayer::is_finished) {
            self.next(cx);
        }
        cx.notify();
    }

    pub fn play_collection(&mut self, indices: Vec<usize>, index: usize, cx: &mut Context<Self>) {
        let queue: Vec<_> = indices
            .into_iter()
            .filter(|&index| index < self.tracks.len())
            .collect();
        if queue.contains(&index) && self.load_index(index, cx) {
            self.queue = queue;
        }
    }

    pub fn play_index(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.load_index(index, cx) {
            self.queue = (0..self.tracks.len()).collect();
        }
    }

    /// Loads a song and leaves it paused at the start, for "Resume on
    /// startup". Nothing is recorded as played, because nothing played.
    fn cue_index(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        let loaded = match (self.tracks.get(index), self.player.as_mut()) {
            (Some(track), Some(player)) => player.load(&track.path).is_ok(),
            _ => false,
        };
        if !loaded {
            return false;
        }
        if let Some(player) = self.player.as_ref() {
            player.set_volume(self.volume);
        }
        self.current = Some(index);
        self.is_playing = false;
        cx.notify();
        true
    }

    /// Re-opens the song played most recently, paused and queued, when the
    /// setting asks for it. Runs once per launch, after the first scan, and
    /// only when nothing is already playing.
    fn resume_last_song(&mut self, cx: &mut Context<Self>) {
        if self.resume_attempted {
            return;
        }
        self.resume_attempted = true;
        if !self.store.resume_last_song || self.current.is_some() {
            return;
        }
        let Some(index) = self.recently_played_indices().first().copied() else {
            return;
        };
        if self.cue_index(index, cx) {
            self.queue = (0..self.tracks.len()).collect();
        }
    }

    fn load_index(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        self.cancel_preparing_audio(cx);
        let result = match (self.tracks.get(index), self.player.as_mut()) {
            (Some(track), Some(player)) => player.load(&track.path),
            (None, _) => Err(anyhow::anyhow!(
                "This song is no longer in the library; rescan the Music folder"
            )),
            (_, None) => Err(anyhow::anyhow!("Audio device unavailable")),
        };
        match result {
            Ok(()) => {
                if let Some(player) = self.player.as_ref() {
                    player.set_volume(self.volume);
                    player.play();
                }
                self.current = Some(index);
                self.is_playing = true;
                self.error = None;
                if let Some(path) = self.tracks.get(index).map(|track| track.path.clone()) {
                    let mut next = self.store.clone();
                    next.remember_played(&path);
                    self.save_store(next, cx);
                }
                if self.lyrics_open {
                    self.refresh_lyrics(cx);
                }
                cx.notify();
                true
            }
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                cx.notify();
                false
            }
        }
    }

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        self.cancel_preparing_audio(cx);
        if let (Some(index), Some(player)) = (self.current, self.player.as_ref()) {
            if player.is_finished() {
                self.load_index(index, cx);
            } else {
                if self.is_playing {
                    player.pause();
                } else {
                    player.play();
                }
                self.is_playing = !self.is_playing;
                cx.notify();
            }
        }
    }

    pub fn next(&mut self, cx: &mut Context<Self>) {
        let Some(position) = self
            .current
            .and_then(|index| self.queue.iter().position(|&queued| queued == index))
        else {
            return;
        };
        let next = if self.shuffle && self.queue.len() > 1 {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize;
            (position + 1 + seed % (self.queue.len() - 1)) % self.queue.len()
        } else if position + 1 < self.queue.len() {
            position + 1
        } else if self.repeat_all {
            0
        } else {
            self.is_playing = false;
            if let Some(player) = self.player.as_ref() {
                player.pause();
            }
            cx.notify();
            return;
        };
        if let Some(&index) = self.queue.get(next) {
            if !self.load_index(index, cx) {
                self.is_playing = false;
                if let Some(player) = self.player.as_ref() {
                    player.pause();
                }
            }
        }
    }

    pub fn prev(&mut self, cx: &mut Context<Self>) {
        let Some(position) = self
            .current
            .and_then(|index| self.queue.iter().position(|&queued| queued == index))
        else {
            return;
        };
        let previous = if position == 0 && self.repeat_all {
            self.queue.len().saturating_sub(1)
        } else {
            position.saturating_sub(1)
        };
        if let Some(&index) = self.queue.get(previous) {
            self.load_index(index, cx);
        }
    }

    pub fn progress(&self) -> f32 {
        match (self.current, self.player.as_ref()) {
            (Some(_), Some(player)) => {
                let total = player.duration().unwrap_or(Duration::from_secs(1));
                if total.is_zero() {
                    0.0
                } else {
                    (player.position().as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0)
                }
            }
            _ => 0.0,
        }
    }

    pub fn times(&self) -> (String, String) {
        match self.player.as_ref() {
            Some(player) => {
                let fmt = |d: Duration| format!("{}:{:02}", d.as_secs() / 60, d.as_secs() % 60);
                (
                    fmt(player.position()),
                    fmt(player.duration().unwrap_or_default()),
                )
            }
            None => ("0:00".to_string(), "0:00".to_string()),
        }
    }

    /// Peak meter of the audio being played, for the waveform strips.
    pub fn audio_meter(&self) -> Option<crate::media::LevelMeter> {
        self.player.as_ref().map(MediaPlayer::meter)
    }

    pub fn now_playing(&self) -> Option<&Track> {
        self.current.and_then(|index| self.tracks.get(index))
    }
}

impl Render for NoirPlayerModel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;
        let dialog_layer = Root::render_dialog_layer(window, cx);

        if self.equalizer_open {
            if !self.equalizer_focus_handle.is_focused(window) {
                window.focus(&self.equalizer_focus_handle, cx);
            }
        } else if self.settings_open {
            let search_focused = self
                .settings_search
                .read(cx)
                .focus_handle(cx)
                .is_focused(window);
            if !search_focused && !self.settings_focus_handle.is_focused(window) {
                window.focus(&self.settings_focus_handle, cx);
            }
        } else {
            let lib_search_focused = self
                .library_search
                .read(cx)
                .focus_handle(cx)
                .is_focused(window);
            let disc_search_focused = self
                .discover_search
                .read(cx)
                .focus_handle(cx)
                .is_focused(window);
            if !lib_search_focused
                && !disc_search_focused
                && !self.root_focus_handle.is_focused(window)
            {
                window.focus(&self.root_focus_handle, cx);
            }
        }

        let show_volume_hud = self
            .volume_hud_until
            .is_some_and(|until| std::time::Instant::now() < until);
        let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);

        v_flex()
            .id("app-root")
            .track_focus(&self.root_focus_handle)
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .relative()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let is_typing = this
                    .library_search
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
                    || this
                        .discover_search
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                    || this
                        .settings_search
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window);

                let k = event.keystroke.key.trim();
                let ctrl_or_cmd =
                    event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
                let shift = event.keystroke.modifiers.shift;
                let alt = event.keystroke.modifiers.alt;

                if k.eq_ignore_ascii_case("escape") || k.eq_ignore_ascii_case("esc") {
                    if this.song_menu.is_some() {
                        this.close_song_menu(cx);
                        cx.stop_propagation();
                    } else if this.update_dialog_open {
                        this.update_dialog_open = false;
                        cx.notify();
                        cx.stop_propagation();
                    } else if this.equalizer_open {
                        this.close_equalizer(cx);
                        cx.stop_propagation();
                    } else if this.settings_open {
                        this.close_settings(cx);
                        cx.stop_propagation();
                    }
                } else if ctrl_or_cmd && (k == "," || k.eq_ignore_ascii_case("comma")) {
                    this.toggle_settings(cx);
                    cx.stop_propagation();
                } else if ctrl_or_cmd && k.eq_ignore_ascii_case("e") {
                    this.toggle_equalizer(cx);
                    cx.stop_propagation();
                } else if !is_typing
                    && !ctrl_or_cmd
                    && !alt
                    && !this.settings_open
                    && !this.equalizer_open
                    && !this.update_dialog_open
                {
                    if k.eq_ignore_ascii_case("p") || k == " " || k.eq_ignore_ascii_case("space") {
                        this.toggle_play(cx);
                        cx.stop_propagation();
                    } else if k.eq_ignore_ascii_case("arrowright")
                        || k.eq_ignore_ascii_case("right")
                    {
                        if shift {
                            this.next(cx);
                        } else {
                            this.skip_forward(cx);
                        }
                        cx.stop_propagation();
                    } else if k.eq_ignore_ascii_case("arrowleft") || k.eq_ignore_ascii_case("left")
                    {
                        if shift {
                            this.prev(cx);
                        } else {
                            this.skip_backward(cx);
                        }
                        cx.stop_propagation();
                    } else if k.eq_ignore_ascii_case("arrowup") || k.eq_ignore_ascii_case("up") {
                        this.adjust_volume(0.05, cx);
                        cx.stop_propagation();
                    } else if k.eq_ignore_ascii_case("arrowdown") || k.eq_ignore_ascii_case("down")
                    {
                        this.adjust_volume(-0.05, cx);
                        cx.stop_propagation();
                    }
                }
            }))
            .child(if active_tab == ActiveTab::Player {
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(player::render_player(self, cx))
                    .into_any_element()
            } else {
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .items_stretch()
                    .overflow_hidden()
                    .child(sidebar::sidebar(self, cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .child(crate::views::ui::tab_transition(
                                format!("main-tab-{active_tab:?}"),
                                match active_tab {
                                    ActiveTab::Playlists => playlists::render_playlists(self, cx),
                                    ActiveTab::Discover => discover::render_discover(self, cx),
                                    _ => library::render_library(self, cx),
                                },
                            )),
                    )
                    .when(self.queue_open, |d| {
                        d.child(queue_panel::queue_panel(self, cx))
                    })
                    .when(!self.queue_open, |d| {
                        d.child(queue_panel::queue_handle(self, cx))
                    })
                    .into_any_element()
            })
            .children(self.storage_error.clone().map(|error| {
                div()
                    .flex_shrink_0()
                    .px(px(12.0))
                    .py(px(6.0))
                    .text_sm()
                    .text_color(crate::views::ui::red())
                    .child(error)
            }))
            .children(self.error.clone().map(|error| {
                h_flex()
                    .flex_shrink_0()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(6.0))
                    .child(
                        div()
                            .id("app-error")
                            .flex_1()
                            .min_w_0()
                            .max_h(px(80.0))
                            .overflow_y_scrollbar()
                            .text_sm()
                            .text_color(crate::views::ui::red())
                            .child(error),
                    )
                    .child(
                        Button::new("dismiss-error")
                            .label("Dismiss")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.error = None;
                                cx.notify();
                            })),
                    )
            }))
            .when(active_tab != ActiveTab::Player, |d| {
                d.child(player_bar::player_bar(self, cx))
            })
            .children(dialog_layer)
            .when(self.song_menu.is_some(), |d| {
                d.child(crate::views::song_menu::render_song_menu(self, cx))
            })
            .when(self.equalizer_open, |d| {
                d.child(crate::views::equalizer::render_equalizer_modal(self, cx))
            })
            .when(self.settings_open, |d| {
                d.child(crate::views::settings::render_settings_modal(self, cx))
            })
            .when(self.update_dialog_open, |d| {
                d.child(crate::views::update::render_update_modal(self, cx))
            })
            .when(show_volume_hud, |d| {
                d.child(volume_hud(self.volume, is_light))
            })
    }
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn volume_hud(volume: f32, is_light: bool) -> Div {
    let pct = (volume * 100.0).round() as u32;
    let icon = if volume == 0.0 {
        gpui_kit::assets::IconName::VolumeX
    } else if volume < 0.5 {
        gpui_kit::assets::IconName::Volume1
    } else {
        gpui_kit::assets::IconName::Volume2
    };

    div()
        .absolute()
        .top(px(24.0))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            h_flex()
                .items_center()
                .gap(px(12.0))
                .px(px(16.0))
                .py(px(8.0))
                .rounded_full()
                .bg(if is_light { c(0xFFFFFF) } else { c(0x131317) })
                .border(px(1.0))
                .border_color(if is_light { c(0xE4E4E7) } else { c(0x2E2E33) })
                .shadow(vec![BoxShadow::new(
                    px(0.0),
                    px(8.0),
                    hsla(0.0, 0.0, 0.0, 0.35),
                )
                .blur_radius(px(20.0))])
                .child(
                    div()
                        .text_color(if is_light {
                            c(0x18181B)
                        } else {
                            crate::views::ui::white(0.9)
                        })
                        .child(crate::views::ui::icon_text(icon, 18.0)),
                )
                .child(
                    div()
                        .w(px(110.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(if is_light { c(0xE4E4E7) } else { c(0x222227) })
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .bg(crate::views::ui::red())
                                .rounded_full()
                                .w(relative(volume)),
                        ),
                )
                .child(
                    div()
                        .w(px(38.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_light {
                            c(0x18181B)
                        } else {
                            crate::views::ui::white(1.0)
                        })
                        .child(format!("{pct}%")),
                ),
        )
}
