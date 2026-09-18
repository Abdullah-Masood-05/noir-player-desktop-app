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
use crate::widgets::{mini_player, nav};

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
    Favourites,
    Albums,
    Artists,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Collection {
    Album(String),
    Artist(String),
    Playlist(String),
}

impl Collection {
    pub fn name(&self) -> &str {
        match self {
            Self::Album(name) | Self::Artist(name) | Self::Playlist(name) => name,
        }
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
    pub settings_open: bool,
    pub equalizer_open: bool,
    pub settings_category: crate::views::settings::SettingsCategory,
    pub settings_search: Entity<InputState>,
    pub settings_selected_index: usize,
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
        if let Some(player) = player.as_ref() {
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
            volume: 0.9,
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
            settings_open: false,
            equalizer_open: false,
            settings_category: crate::views::settings::SettingsCategory::All,
            settings_search,
            settings_selected_index: 0,
            _subscriptions: subscriptions,
            dialog_subscription: None,
        };
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
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    media::prepare_discover_audio(
                        &source,
                        download,
                        cached.as_deref(),
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
        let Some(folder) = media::default_music_folder() else {
            self.status = "Music folder unavailable".into();
            self.error = Some("Could not locate the system Music folder".into());
            cx.notify();
            return;
        };
        self.scanning = true;
        self.status = "Scanning Music folder...".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { media::scan_folder(&folder) })
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

    pub fn now_playing(&self) -> Option<&Track> {
        self.current.and_then(|index| self.tracks.get(index))
    }
}

impl Render for NoirPlayerModel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;
        let has_track = self.now_playing().is_some();
        let dialog_layer = Root::render_dialog_layer(window, cx);
        v_flex()
            .id("app-root")
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .relative()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .when(self.settings_open || self.equalizer_open, |d| {
                d.on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" || event.keystroke.key == "Escape" {
                        if this.equalizer_open {
                            this.close_equalizer(cx);
                        }
                        if this.settings_open {
                            this.close_settings(cx);
                        }
                    }
                }))
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(crate::views::ui::tab_transition(
                        format!("main-tab-{active_tab:?}"),
                        match active_tab {
                            ActiveTab::Library => library::render_library(self, cx),
                            ActiveTab::Player => player::render_player(self, cx),
                            ActiveTab::Playlists => playlists::render_playlists(self, cx),
                            ActiveTab::Discover => discover::render_discover(self, cx),
                        },
                    )),
            )
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
            .when(has_track && active_tab != ActiveTab::Player, |d| {
                d.child(mini_player::mini_player(self, cx))
            })
            .child(nav::bottom_nav(active_tab, self.settings_open, cx))
            .children(dialog_layer)
            .when(self.equalizer_open, |d| {
                d.child(crate::views::equalizer::render_equalizer_modal(self, cx))
            })
            .when(self.settings_open, |d| {
                d.child(crate::views::settings::render_settings_modal(self, cx))
            })
    }
}
