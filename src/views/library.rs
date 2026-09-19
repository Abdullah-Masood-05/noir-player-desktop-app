use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::Input;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::{store, Collection, LibraryTab, NoirPlayerModel};
use crate::media::Track;
use crate::views::ui::{
    dynamic_hover, dynamic_muted, dynamic_subtitle, dynamic_text, format_duration, icon_text,
    img_from_bytes, red, red_a, selected_highlight, smooth_scroll, tab_transition, top_fade, white,
};

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub fn app_bar(title: &str, is_light: bool) -> Div {
    h_flex()
        .w_full()
        .h(px(56.0))
        .px(px(16.0))
        .items_center()
        .flex_shrink_0()
        .child(div().w(px(30.0)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_center()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(dynamic_text(is_light))
                .child(title.to_string()),
        )
        .child(div().w(px(30.0)))
}

pub fn song_row(
    model: &NoirPlayerModel,
    index: usize,
    queue: Vec<usize>,
    playlist: Option<String>,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let Some(track) = model.tracks.get(index) else {
        return div().into_any_element();
    };
    let is_current = model.current == Some(index);
    let favourite = model.is_favourite(index);
    let remove_path = track.path.clone();
    h_flex()
        .id(format!("song-{index}"))
        .items_center()
        .gap(px(10.0))
        .px(px(10.0))
        .py(px(7.0))
        .rounded_lg()
        .hover(move |s| s.bg(dynamic_hover(is_light)))
        .when(is_current, |d| d.bg(red_a(0.10)))
        .child(
            h_flex()
                .id(format!("play-song-{index}"))
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(14.0))
                .cursor_pointer()
                .on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.play_collection(queue.clone(), index, cx)
                    }),
                )
                .child(artwork_thumb(track, 48.0))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if is_current {
                                    red()
                                } else {
                                    dynamic_text(is_light)
                                })
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(track.title.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(dynamic_subtitle(is_light))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(format!("{} · {}", track.artist, track.album)),
                        ),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(dynamic_muted(is_light))
                        .child(format_duration(track.duration)),
                ),
        )
        .child(
            div()
                .id(format!("favourite-{index}"))
                .flex_shrink_0()
                .p(px(6.0))
                .rounded_md()
                .cursor_pointer()
                .bg(if favourite {
                    red_a(0.18)
                } else {
                    dynamic_hover(is_light)
                })
                .text_color(if favourite {
                    red()
                } else {
                    dynamic_muted(is_light)
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_favourite(index, cx);
                }))
                .child(icon_text(MusicIcon::Heart, 28.0)),
        )
        .child(
            Button::new(format!("add-song-{index}"))
                .label("Add")
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.add_to_playlist_dialog(index, window, cx);
                })),
        )
        .when_some(playlist, |row, name| {
            row.child(
                Button::new(format!("remove-song-{index}"))
                    .label("Remove")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.remove_from_playlist(&name, &remove_path, cx);
                    })),
            )
        })
        .into_any_element()
}

pub fn artwork_thumb(track: &Track, size: f32) -> AnyElement {
    match &track.artwork {
        Some(bytes) => img_from_bytes(bytes.clone())
            .size(px(size))
            .rounded_lg()
            .flex_shrink_0()
            .into_any_element(),
        None => div()
            .size(px(size))
            .rounded_lg()
            .flex_shrink_0()
            .bg(red_a(0.12))
            .flex()
            .items_center()
            .justify_center()
            .text_color(red())
            .child(icon_text(MusicIcon::Music4, size))
            .into_any_element(),
    }
}

pub fn empty_state(msg: &str, sub: &str, is_light: bool) -> Div {
    v_flex()
        .flex_1()
        .min_h_0()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .p(px(16.0))
        .child(
            div()
                .text_color(if is_light {
                    hsla(0.0, 0.0, 0.1, 0.22)
                } else {
                    white(0.2)
                })
                .child(icon_text(MusicIcon::Music4, 64.0)),
        )
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(dynamic_text(is_light))
                .child(msg.to_string()),
        )
        .child(
            div()
                .text_sm()
                .text_color(dynamic_subtitle(is_light))
                .child(sub.to_string()),
        )
}

pub fn track_list(
    model: &NoirPlayerModel,
    indices: Vec<usize>,
    playlist: Option<String>,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    if indices.is_empty() {
        return empty_state(
            "No matching songs",
            "Search another title, artist or album, or add songs to this collection.",
            is_light,
        )
        .into_any_element();
    }
    smooth_scroll(
        format!(
            "songs-scroll-{:?}-{:?}",
            model.library_tab, model.library_detail
        ),
        div().px(px(8.0)).pb(px(12.0)).children(
            indices
                .iter()
                .map(|&index| song_row(model, index, indices.clone(), playlist.clone(), cx)),
        ),
    )
    .into_any_element()
}

pub fn render_library(model: &mut NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let tab = model.library_tab;
    let detail = model.library_detail.clone();
    let body = if let Some(collection) = detail.as_ref() {
        let indices = model.filtered_indices(model.collection_indices(collection), cx);
        track_list(model, indices, None, cx)
    } else {
        match tab {
            LibraryTab::Music | LibraryTab::Favourites => {
                let indices = if tab == LibraryTab::Favourites {
                    store::resolve_paths(&model.store.favourites, &model.tracks)
                } else if let Some(folder) = model.selected_folder_filter.as_ref() {
                    (0..model.tracks.len())
                        .filter(|&i| model.tracks[i].path.starts_with(folder))
                        .collect()
                } else {
                    (0..model.tracks.len()).collect()
                };
                track_list(model, model.filtered_indices(indices, cx), None, cx)
            }
            LibraryTab::Albums | LibraryTab::Artists => {
                let groups = if tab == LibraryTab::Albums {
                    &model.albums
                } else {
                    &model.artists
                };
                let visible: Vec<_> = groups
                    .iter()
                    .filter_map(|(name, indices)| {
                        let filtered = model.filtered_indices(indices.clone(), cx);
                        (!filtered.is_empty()).then_some((name, filtered.len()))
                    })
                    .collect();
                if visible.is_empty() {
                    empty_state(
                        "No matching collections",
                        "Album and artist information comes from file tags.",
                        is_light,
                    )
                    .into_any_element()
                } else {
                    smooth_scroll(
                        "collections-scroll",
                        div().child(h_flex().flex_wrap().gap(px(18.0)).p(px(14.0)).children(
                            visible.into_iter().map(|(name, count)| {
                                let collection = if tab == LibraryTab::Albums {
                                    Collection::Album(name.clone())
                                } else {
                                    Collection::Artist(name.clone())
                                };
                                v_flex()
                                    .id(format!("collection-{name}"))
                                    .w(px(150.0))
                                    .items_center()
                                    .gap(px(6.0))
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.85))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.library_detail = Some(collection.clone());
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .size(px(120.0))
                                            .rounded_2xl()
                                            .bg(red_a(0.12))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(red())
                                            .child(icon_text(
                                                if tab == LibraryTab::Albums {
                                                    MusicIcon::Disc3
                                                } else {
                                                    MusicIcon::Mic
                                                },
                                                80.0,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .text_sm()
                                            .text_center()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(dynamic_text(is_light))
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(dynamic_subtitle(is_light))
                                            .child(format!("{count} matching songs")),
                                    )
                            }),
                        )),
                    )
                    .into_any_element()
                }
            }
            LibraryTab::Folders => {
                let folders = model.effective_music_folders();
                let visible: Vec<_> = folders
                    .into_iter()
                    .map(|f| {
                        let count = model
                            .tracks
                            .iter()
                            .filter(|t| t.path.starts_with(&f))
                            .count();
                        (f, count)
                    })
                    .collect();
                if visible.is_empty() {
                    empty_state(
                        "No folders configured",
                        "Add music folders in Settings (Ctrl+,).",
                        is_light,
                    )
                    .into_any_element()
                } else {
                    smooth_scroll(
                        "folders-scroll",
                        div().child(h_flex().flex_wrap().gap(px(18.0)).p(px(14.0)).children(
                            visible.into_iter().map(|(folder, count)| {
                                let folder_clone = folder.clone();
                                let name = folder
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("Folder")
                                    .to_string();
                                let path_str = folder.display().to_string();
                                v_flex()
                                    .id(format!("folder-card-{path_str}"))
                                    .w(px(150.0))
                                    .items_center()
                                    .gap(px(6.0))
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.85))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.library_detail =
                                            Some(Collection::Folder(folder_clone.clone()));
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .size(px(120.0))
                                            .rounded_2xl()
                                            .bg(red_a(0.12))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(red())
                                            .child(icon_text(MusicIcon::Folder, 60.0)),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .text_sm()
                                            .text_center()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(dynamic_text(is_light))
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(name),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(dynamic_subtitle(is_light))
                                            .child(format!("{count} songs")),
                                    )
                            }),
                        )),
                    )
                    .into_any_element()
                }
            }
        }
    };
    v_flex()
        .size_full()
        .min_h_0()
        .relative()
        .child(top_fade(is_light))
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .relative()
                .child(app_bar(
                    detail.as_ref().map(Collection::name).unwrap_or("Library"),
                    is_light,
                ))
                .when(detail.is_none(), |d| d.child(tab_bar(tab, is_light, cx)))
                .when(detail.is_some(), |d| {
                    d.child(
                        h_flex().flex_shrink_0().px(px(12.0)).child(
                            Button::new("library-back")
                                .label("Back")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.library_detail = None;
                                    cx.notify();
                                })),
                        ),
                    )
                })
                .child(search_bar(model))
                .when(
                    tab == LibraryTab::Music
                        && detail.is_none()
                        && model.effective_music_folders().len() > 1,
                    |d| d.child(folder_filter_bar(model, is_light, cx)),
                )
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .px(px(12.0))
                        .pb(px(8.0))
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(dynamic_subtitle(is_light))
                                .child(model.status.clone()),
                        )
                        .child(
                            Button::new("rescan-library")
                                .label(if model.scanning {
                                    "Scanning..."
                                } else {
                                    "Rescan"
                                })
                                .disabled(model.scanning)
                                .on_click(cx.listener(|this, _, _, cx| this.rescan(cx))),
                        ),
                )
                .child(tab_transition(
                    format!("library-body-{tab:?}-{detail:?}"),
                    body,
                )),
        )
}

fn folder_filter_bar(
    model: &NoirPlayerModel,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let folders = model.effective_music_folders();
    let current_filter = model.selected_folder_filter.clone();
    h_flex()
        .w_full()
        .flex_shrink_0()
        .px(px(12.0))
        .pb(px(8.0))
        .gap(px(6.0))
        .items_center()
        .overflow_x_hidden()
        .child(
            div()
                .id("folder-filter-all")
                .px(px(10.0))
                .py(px(4.0))
                .rounded_full()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(current_filter.is_none(), |d| {
                    d.bg(red()).text_color(white(1.0))
                })
                .when(current_filter.is_some(), |d| {
                    d.bg(if is_light { c(0xEDEAEF) } else { c(0x1F2228) })
                        .text_color(dynamic_subtitle(is_light))
                        .hover(move |s| s.bg(dynamic_hover(is_light)))
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.selected_folder_filter = None;
                    cx.notify();
                }))
                .child(format!("All ({})", model.tracks.len())),
        )
        .children(folders.into_iter().map(|f| {
            let count = model
                .tracks
                .iter()
                .filter(|t| t.path.starts_with(&f))
                .count();
            let name = f
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Folder")
                .to_string();
            let is_selected = current_filter.as_ref() == Some(&f);
            let f_clone = f.clone();
            div()
                .id(format!("filter-folder-{}", f.display()))
                .px(px(10.0))
                .py(px(4.0))
                .rounded_full()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(is_selected, |d| d.bg(red()).text_color(white(1.0)))
                .when(!is_selected, |d| {
                    d.bg(if is_light { c(0xEDEAEF) } else { c(0x1F2228) })
                        .text_color(dynamic_subtitle(is_light))
                        .hover(move |s| s.bg(dynamic_hover(is_light)))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected_folder_filter = Some(f_clone.clone());
                    cx.notify();
                }))
                .child(format!("{name} ({count})"))
        }))
}

pub fn tab_bar(active: LibraryTab, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    let tabs = [
        (LibraryTab::Music, "Music"),
        (LibraryTab::Favourites, "Favourites"),
        (LibraryTab::Albums, "Albums"),
        (LibraryTab::Artists, "Artists"),
        (LibraryTab::Folders, "Folders"),
    ];
    let unselected_color = dynamic_subtitle(is_light);
    h_flex()
        .w_full()
        .flex_shrink_0()
        .children(tabs.map(|(tab, label)| {
            let selected = active == tab;
            h_flex()
                .id(format!("libtab-{tab:?}"))
                .flex_1()
                .h(px(44.0))
                .items_center()
                .justify_center()
                .cursor_pointer()
                .relative()
                .child(
                    div()
                        .text_sm()
                        .when(selected, |d| {
                            d.text_color(red()).font_weight(FontWeight::BOLD)
                        })
                        .when(!selected, |d| d.text_color(unselected_color))
                        .child(label),
                )
                .child(selected_highlight(
                    format!("library-highlight-{tab:?}"),
                    selected,
                    div()
                        .absolute()
                        .bottom_0()
                        .left(px(24.0))
                        .right(px(24.0))
                        .h(px(2.5))
                        .rounded_full(),
                ))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.library_tab = tab;
                    this.library_detail = None;
                    cx.notify();
                }))
        }))
}

pub fn search_bar(model: &NoirPlayerModel) -> Div {
    div()
        .m(px(12.0))
        .flex_shrink_0()
        .child(Input::new(&model.library_search))
}
