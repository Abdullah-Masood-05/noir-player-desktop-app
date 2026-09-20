use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::input::Input;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::{Collection, LibraryTab, NoirPlayerModel, SortMode};
use crate::media::Track;
use crate::views::ui::{
    dynamic_border, dynamic_card, dynamic_divider, dynamic_hover, dynamic_muted, dynamic_panel,
    dynamic_row_hover, dynamic_subtitle, dynamic_text, format_duration, icon_text, img_from_bytes,
    red, red_a, smooth_scroll, tab_transition, top_fade, waveform, white,
};

const CONTENT_PADDING: f32 = 24.0;
const SORT_BUTTON_WIDTH: f32 = 170.0;
const MEDIA_BUTTON_WIDTH: f32 = 160.0;

pub fn render_library(model: &mut NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let tab = model.library_tab;
    let detail = model.library_detail.clone();
    let menus_open = model.sort_menu_open || model.media_menu_open;

    let body = match detail.as_ref() {
        Some(collection) => {
            let indices = model
                .sorted_indices(model.filtered_indices(model.collection_indices(collection), cx));
            collection_detail(model, collection.clone(), indices, is_light, cx)
        }
        None => match tab {
            LibraryTab::Music
            | LibraryTab::AllSongs
            | LibraryTab::Favourites
            | LibraryTab::RecentlyPlayed => songs_page(model, tab, is_light, cx),
            LibraryTab::Albums | LibraryTab::Artists => collection_grid(model, tab, is_light, cx),
            LibraryTab::Folders => folder_grid(model, is_light, cx),
        },
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
                .child(header(model, is_light, cx))
                .child(tab_transition(
                    format!("library-body-{tab:?}-{detail:?}"),
                    body,
                )),
        )
        .when(menus_open, |d| {
            d.child(
                div()
                    .id("library-menu-backdrop")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, _, cx| this.close_menus(cx))),
            )
        })
        .when(model.media_menu_open, |d| {
            d.child(media_menu(model, is_light, cx))
        })
        .when(model.sort_menu_open, |d| {
            d.child(sort_menu(model, is_light, cx))
        })
}

/// Page title, library search, and the media/sort pickers.
fn header(model: &NoirPlayerModel, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    let title = match model.library_detail.as_ref() {
        Some(collection) => collection.name().to_string(),
        None => match model.library_tab {
            LibraryTab::Music => "Your Library".to_string(),
            LibraryTab::AllSongs => "All Songs".to_string(),
            LibraryTab::Favourites => "Favorites".to_string(),
            LibraryTab::Albums => "Albums".to_string(),
            LibraryTab::Artists => "Artists".to_string(),
            LibraryTab::Folders => "Folders".to_string(),
            LibraryTab::RecentlyPlayed => "Recently Played".to_string(),
        },
    };
    let in_detail = model.library_detail.is_some() || model.library_tab == LibraryTab::AllSongs;

    h_flex()
        .w_full()
        .flex_shrink_0()
        .h(px(78.0))
        .px(px(CONTENT_PADDING))
        .gap(px(16.0))
        .items_center()
        .child(
            h_flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(10.0))
                .when(in_detail, |d| {
                    d.child(
                        div()
                            .id("library-back")
                            .size(px(32.0))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(dynamic_subtitle(is_light))
                            .hover(move |s| {
                                s.bg(dynamic_row_hover(is_light))
                                    .text_color(dynamic_text(is_light))
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.library_detail.is_some() {
                                    this.library_detail = None;
                                } else {
                                    this.library_tab = LibraryTab::Music;
                                }
                                cx.notify();
                            }))
                            .child(icon_text(MusicIcon::ChevronLeft, 20.0)),
                    )
                })
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(dynamic_text(is_light))
                        .child(title),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .justify_center()
                .child(div().w_full().max_w(px(470.0)).child(search_bar(model))),
        )
        .child(
            h_flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(10.0))
                .child(picker_button(
                    "library-media-picker",
                    &media_filter_label(model),
                    MEDIA_BUTTON_WIDTH,
                    model.media_menu_open,
                    is_light,
                    cx.listener(|this, _, _, cx| {
                        this.media_menu_open = !this.media_menu_open;
                        this.sort_menu_open = false;
                        cx.notify();
                    }),
                ))
                .child(picker_button(
                    "library-sort-picker",
                    model.sort_mode.label(),
                    SORT_BUTTON_WIDTH,
                    model.sort_menu_open,
                    is_light,
                    cx.listener(|this, _, _, cx| {
                        this.sort_menu_open = !this.sort_menu_open;
                        this.media_menu_open = false;
                        cx.notify();
                    }),
                ))
                .child(
                    div()
                        .id("library-rescan")
                        .size(px(36.0))
                        .rounded_lg()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(if model.scanning {
                            red()
                        } else {
                            dynamic_subtitle(is_light)
                        })
                        .hover(move |s| {
                            s.bg(dynamic_row_hover(is_light))
                                .text_color(dynamic_text(is_light))
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.rescan(cx)))
                        .child(icon_text(MusicIcon::RefreshCw, 18.0)),
                ),
        )
}

pub fn search_bar(model: &NoirPlayerModel) -> Div {
    div().w_full().child(
        Input::new(&model.library_search)
            .prefix(icon_text(MusicIcon::Search, 16.0))
            .cleanable(true)
            .h(px(38.0))
            .rounded_lg(),
    )
}

fn media_filter_label(model: &NoirPlayerModel) -> String {
    match model.selected_folder_filter.as_ref() {
        Some(folder) => folder
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Folder")
            .to_string(),
        None => "All Media".to_string(),
    }
}

fn picker_button(
    id: &'static str,
    label: &str,
    width: f32,
    open: bool,
    is_light: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .w(px(width))
        .h(px(38.0))
        .px(px(12.0))
        .items_center()
        .justify_between()
        .rounded_lg()
        .cursor_pointer()
        .border_1()
        .border_color(if open {
            red_a(0.6)
        } else {
            dynamic_border(is_light)
        })
        .bg(dynamic_card(is_light))
        .text_color(dynamic_text(is_light))
        .hover(move |s| s.border_color(red_a(0.45)))
        .on_click(handler)
        .child(
            div()
                .text_sm()
                .overflow_hidden()
                .text_ellipsis()
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_shrink_0()
                .text_color(dynamic_muted(is_light))
                .child(icon_text(MusicIcon::ChevronDown, 16.0)),
        )
}

fn menu_panel(right: f32, is_light: bool) -> Div {
    v_flex()
        .absolute()
        .top(px(70.0))
        .right(px(right))
        .w(px(220.0))
        .p(px(6.0))
        .gap(px(2.0))
        .rounded_xl()
        .bg(dynamic_panel(is_light))
        .border_1()
        .border_color(dynamic_border(is_light))
        .shadow(vec![BoxShadow::new(
            px(0.),
            px(12.),
            hsla(0.0, 0.0, 0.0, 0.45),
        )
        .blur_radius(px(28.0))])
}

fn menu_item(
    id: impl Into<ElementId>,
    label: String,
    selected: bool,
    is_light: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(px(34.0))
        .px(px(10.0))
        .items_center()
        .justify_between()
        .rounded_lg()
        .cursor_pointer()
        .text_color(if selected {
            red()
        } else {
            dynamic_text(is_light)
        })
        .hover(move |s| s.bg(dynamic_row_hover(is_light)))
        .on_click(handler)
        .child(
            div()
                .text_sm()
                .overflow_hidden()
                .text_ellipsis()
                .child(label),
        )
        .when(selected, |d| d.child(icon_text(MusicIcon::Check, 14.0)))
}

fn sort_menu(model: &NoirPlayerModel, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    let active = model.sort_mode;
    menu_panel(CONTENT_PADDING, is_light).children(SortMode::ALL.map(|mode| {
        menu_item(
            SharedString::from(format!("sort-{mode:?}")),
            mode.label().to_string(),
            mode == active,
            is_light,
            cx.listener(move |this, _, _, cx| this.set_sort_mode(mode, cx)),
        )
    }))
}

fn media_menu(model: &NoirPlayerModel, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    let selected = model.selected_folder_filter.clone();
    let folders = model.effective_music_folders();
    menu_panel(CONTENT_PADDING + SORT_BUTTON_WIDTH + 10.0, is_light)
        .child(menu_item(
            "media-all",
            format!("All Media ({})", model.tracks.len()),
            selected.is_none(),
            is_light,
            cx.listener(|this, _, _, cx| this.set_folder_filter(None, cx)),
        ))
        .children(folders.into_iter().map(|folder| {
            let count = model
                .tracks
                .iter()
                .filter(|track| track.path.starts_with(&folder))
                .count();
            let name = folder
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Folder")
                .to_string();
            let is_selected = selected.as_ref() == Some(&folder);
            let target = folder.clone();
            menu_item(
                SharedString::from(format!("media-folder-{}", folder.display())),
                format!("{name} ({count})"),
                is_selected,
                is_light,
                cx.listener(move |this, _, _, cx| this.set_folder_filter(Some(target.clone()), cx)),
            )
        }))
}

/// Library, Favorites and Recently played all render the same page: an
/// optional hero, the recently played shelf, and the sorted song list.
fn songs_page(
    model: &NoirPlayerModel,
    tab: LibraryTab,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let indices = model.sorted_indices(model.filtered_indices(model.tab_indices(), cx));
    let recent = model.recently_played_indices();
    let searching = !model.library_search.read(cx).value().is_empty();
    let show_hero = tab == LibraryTab::Music && !searching;
    let show_recent = tab == LibraryTab::Music && !searching && !recent.is_empty();
    let list_title = match tab {
        LibraryTab::Favourites => "Favorite Songs",
        LibraryTab::RecentlyPlayed => "Recently Played",
        LibraryTab::AllSongs => "All Songs",
        _ => "Your Songs",
    };
    // The home page previews the list; "See all" opens it on its own page.
    let see_all = (tab == LibraryTab::Music && !searching).then(|| {
        see_all_link(
            "songs-see-all",
            cx.listener(|this, _, _, cx| {
                this.library_tab = LibraryTab::AllSongs;
                this.library_detail = None;
                cx.notify();
            }),
        )
        .into_any_element()
    });
    let empty = match tab {
        LibraryTab::Favourites => (
            "No favorites yet",
            "Tap the heart beside a song to keep it here.",
        ),
        LibraryTab::RecentlyPlayed => (
            "Nothing played yet",
            "Songs you play show up here, newest first.",
        ),
        _ => (
            "No matching songs",
            "Search another title, artist or album, or add a folder in Settings.",
        ),
    };

    smooth_scroll(
        format!("library-scroll-{tab:?}"),
        v_flex()
            .w_full()
            .px(px(CONTENT_PADDING))
            .pb(px(20.0))
            .gap(px(24.0))
            .when(show_hero, |d| d.child(hero(model, is_light)))
            .when(show_recent, |d| {
                d.child(recently_played_shelf(model, recent, is_light, cx))
            })
            .child(if indices.is_empty() {
                empty_state(empty.0, empty.1, is_light).into_any_element()
            } else {
                song_table(model, list_title, see_all, indices, None, is_light, cx)
            }),
    )
    .into_any_element()
}

fn collection_detail(
    model: &NoirPlayerModel,
    collection: Collection,
    indices: Vec<usize>,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let name = collection.name().to_string();
    smooth_scroll(
        format!("library-detail-{name}"),
        v_flex()
            .w_full()
            .px(px(CONTENT_PADDING))
            .pb(px(20.0))
            .child(if indices.is_empty() {
                empty_state(
                    "Nothing to play here",
                    "Album and artist details come from file tags.",
                    is_light,
                )
                .into_any_element()
            } else {
                song_table(model, "Songs", None, indices, None, is_light, cx)
            }),
    )
    .into_any_element()
}

/// Greeting banner with the library totals.
fn hero(model: &NoirPlayerModel, is_light: bool) -> Div {
    let songs = model.tracks.len();
    let albums = model.albums.len();
    let artists = model.artists.len();
    let phase = model.progress();
    let intensity = if model.is_playing { 1.0 } else { 0.55 };

    div()
        .w_full()
        .h(px(126.0))
        .rounded_2xl()
        .relative()
        .overflow_hidden()
        .bg(if is_light {
            red_a(0.08)
        } else {
            rgb(0x160709).into()
        })
        .border_1()
        .border_color(if is_light {
            red_a(0.18)
        } else {
            red_a(0.22)
        })
        .child(
            // Corner radius has to be repeated: the clip mask is rectangular,
            // so a square overlay would paint over the rounded corners.
            div()
                .absolute()
                .inset_0()
                .rounded_2xl()
                .bg(linear_gradient(
                    90.0,
                    linear_color_stop(red_a(if is_light { 0.16 } else { 0.30 }), 0.0),
                    linear_color_stop(red_a(0.0), 1.0),
                )),
        )
        .child(
            div()
                .absolute()
                .right(px(28.0))
                .top(px(30.0))
                .child(waveform(44, 2.5, 3.0, 62.0, phase, intensity)),
        )
        .child(
            v_flex()
                .relative()
                .size_full()
                .justify_center()
                .px(px(26.0))
                .gap(px(6.0))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(red())
                        .child(greeting()),
                )
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(dynamic_text(is_light))
                        .child("Your music, your story."),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(dynamic_subtitle(is_light))
                        .child(if model.scanning {
                            "Scanning music folders...".to_string()
                        } else {
                            format!(
                                "{songs} songs  \u{2022}  {albums} albums  \u{2022}  {artists} artists"
                            )
                        }),
                ),
        )
}

fn greeting() -> &'static str {
    use chrono::Timelike;
    match chrono::Local::now().hour() {
        5..=11 => "GOOD MORNING",
        12..=16 => "GOOD AFTERNOON",
        17..=21 => "GOOD EVENING",
        _ => "GOOD NIGHT",
    }
}

fn recently_played_shelf(
    model: &NoirPlayerModel,
    recent: Vec<usize>,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let shown: Vec<usize> = recent.iter().copied().take(6).collect();
    let queue = shown.clone();

    v_flex()
        .w_full()
        .gap(px(12.0))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(dynamic_text(is_light))
                        .child("Recently Played"),
                )
                .child(see_all_link(
                    "recent-see-all",
                    cx.listener(|this, _, _, cx| {
                        this.library_tab = LibraryTab::RecentlyPlayed;
                        this.library_detail = None;
                        cx.notify();
                    }),
                )),
        )
        .child(
            h_flex()
                .w_full()
                .gap(px(16.0))
                .children(shown.iter().map(|&index| {
                    let queue = queue.clone();
                    let Some(track) = model.tracks.get(index) else {
                        return div().id(SharedString::from(format!("recent-missing-{index}")));
                    };
                    let playing = model.current == Some(index);
                    v_flex()
                        .id(SharedString::from(format!("recent-card-{index}")))
                        .w(px(158.0))
                        .gap(px(8.0))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.88))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.play_collection(queue.clone(), index, cx);
                        }))
                        .child(
                            div()
                                .size(px(158.0))
                                .rounded_xl()
                                .overflow_hidden()
                                .relative()
                                .child(artwork_thumb(track, 158.0))
                                .when(playing, |d| {
                                    d.child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .border_2()
                                            .border_color(red())
                                            .rounded_xl(),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .w_full()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if playing {
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
                                .w_full()
                                .text_xs()
                                .text_color(dynamic_subtitle(is_light))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(track.artist.clone()),
                        )
                })),
        )
}

/// The song list: a column header, then one block per alphabetical section.
/// A "See all" link, matching the one above the recently played shelf.
pub fn see_all_link(
    id: &'static str,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .text_sm()
        .text_color(red())
        .hover(|s| s.opacity(0.8))
        .on_click(handler)
        .child("See all")
        .child(icon_text(MusicIcon::ArrowRight, 16.0))
}

#[allow(clippy::too_many_arguments)]
pub fn song_table(
    model: &NoirPlayerModel,
    title: &str,
    see_all: Option<AnyElement>,
    indices: Vec<usize>,
    playlist: Option<String>,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let sections = model.letter_sections(indices.clone());
    let mut position = 0usize;
    let mut list = v_flex().w_full().gap(px(2.0));
    for (letter, songs) in sections {
        if let Some(letter) = letter {
            list = list.child(letter_heading(letter, songs.len(), is_light));
        }
        for &index in &songs {
            position += 1;
            list = list.child(song_row(
                model,
                index,
                position,
                indices.clone(),
                playlist.clone(),
                is_light,
                cx,
            ));
        }
    }

    v_flex()
        .w_full()
        .gap(px(10.0))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(dynamic_text(is_light))
                        .child(title.to_string()),
                )
                .children(see_all),
        )
        .child(column_header(is_light))
        .child(list)
        .into_any_element()
}

fn column_header(is_light: bool) -> Div {
    h_flex()
        .w_full()
        .h(px(30.0))
        .px(px(10.0))
        .items_center()
        .gap(px(12.0))
        .text_xs()
        .text_color(dynamic_muted(is_light))
        .border_b_1()
        .border_color(dynamic_divider(is_light))
        .child(div().w(px(26.0)).child("#"))
        .child(div().w(px(36.0)))
        .child(div().flex_1().min_w_0().child("Title"))
        .child(div().w(px(330.0)).child("Artist / Album"))
        .child(div().w(px(70.0)).child("Duration"))
        .child(div().w(px(96.0)))
}

fn letter_heading(letter: char, count: usize, is_light: bool) -> Div {
    h_flex()
        .w_full()
        .items_center()
        .gap(px(10.0))
        .pt(px(14.0))
        .pb(px(4.0))
        .px(px(10.0))
        .child(
            div()
                .size(px(22.0))
                .rounded_md()
                .flex()
                .items_center()
                .justify_center()
                .bg(red_a(0.16))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(red())
                .child(letter.to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(dynamic_muted(is_light))
                .child(format!("{count}")),
        )
        .child(div().flex_1().h(px(1.0)).bg(dynamic_divider(is_light)))
}

#[allow(clippy::too_many_arguments)]
pub fn song_row(
    model: &NoirPlayerModel,
    index: usize,
    position: usize,
    queue: Vec<usize>,
    playlist: Option<String>,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let Some(track) = model.tracks.get(index) else {
        return div().into_any_element();
    };
    let is_current = model.current == Some(index);
    let favourite = model.is_favourite(index);
    let group = SharedString::from(format!("song-group-{index}"));
    let play_queue = queue.clone();
    let phase = model.progress();

    h_flex()
        .id(SharedString::from(format!("song-{index}")))
        .group(group.clone())
        .w_full()
        .h(px(46.0))
        .px(px(10.0))
        .items_center()
        .gap(px(12.0))
        .rounded_lg()
        .cursor_pointer()
        .when(is_current, |d| d.bg(red_a(0.10)))
        .when(!is_current, |d| {
            d.hover(move |s| s.bg(dynamic_row_hover(is_light)))
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.play_collection(play_queue.clone(), index, cx);
        }))
        .child(
            div()
                .w(px(26.0))
                .flex_shrink_0()
                .text_xs()
                .text_color(if is_current {
                    red()
                } else {
                    dynamic_muted(is_light)
                })
                .child(if is_current {
                    waveform(3, 2.5, 2.0, 14.0, phase, 1.0).into_any_element()
                } else {
                    div().child(format!("{position}")).into_any_element()
                }),
        )
        .child(artwork_thumb(track, 36.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
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
            h_flex()
                .w(px(330.0))
                .flex_shrink_0()
                .gap(px(6.0))
                .text_xs()
                .text_color(dynamic_subtitle(is_light))
                .child(
                    div()
                        .max_w(px(150.0))
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(track.artist.clone()),
                )
                .child(div().text_color(dynamic_muted(is_light)).child("\u{2022}"))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(dynamic_muted(is_light))
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(album_line(track)),
                ),
        )
        .child(
            div()
                .w(px(70.0))
                .flex_shrink_0()
                .text_xs()
                .text_color(dynamic_muted(is_light))
                .child(format_duration(track.duration)),
        )
        .child(
            h_flex()
                .w(px(96.0))
                .flex_shrink_0()
                .items_center()
                .justify_end()
                .gap(px(4.0))
                .child(
                    h_flex()
                        .id(SharedString::from(format!("song-play-{index}")))
                        .h(px(26.0))
                        .px(px(10.0))
                        .items_center()
                        .gap(px(4.0))
                        .rounded_full()
                        .bg(red())
                        .text_color(white(1.0))
                        .cursor_pointer()
                        .opacity(0.0)
                        .group_hover(group.clone(), |s| s.opacity(1.0))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.play_collection(queue.clone(), index, cx);
                        }))
                        .child(icon_text(MusicIcon::Play, 12.0))
                        .child(div().text_xs().child("Play")),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("song-favourite-{index}")))
                        .size(px(26.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(if favourite {
                            red()
                        } else {
                            dynamic_muted(is_light)
                        })
                        .when(!favourite, |d| {
                            d.opacity(0.0)
                                .group_hover(group.clone(), |s| s.opacity(1.0))
                        })
                        .hover(move |s| s.bg(dynamic_hover(is_light)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle_favourite(index, cx);
                        }))
                        .child(icon_text(MusicIcon::Heart, 14.0)),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("song-more-{index}")))
                        .size(px(26.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(dynamic_muted(is_light))
                        .hover(move |s| {
                            s.bg(dynamic_hover(is_light))
                                .text_color(dynamic_text(is_light))
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.song_actions_dialog(index, playlist.clone(), window, cx);
                        }))
                        .child(icon_text(MusicIcon::Ellipsis, 16.0)),
                ),
        )
        .into_any_element()
}

/// "Album - Year", unless the album tag already spells the year out.
pub fn album_line(track: &Track) -> String {
    match track.year {
        Some(year) if !track.album.contains(&year.to_string()) => {
            format!("{} - {year}", track.album)
        }
        _ => track.album.clone(),
    }
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
            .child(icon_text(MusicIcon::Music4, size * 0.6))
            .into_any_element(),
    }
}

pub fn empty_state(msg: &str, sub: &str, is_light: bool) -> Div {
    v_flex()
        .w_full()
        .py(px(70.0))
        .items_center()
        .justify_center()
        .gap(px(10.0))
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

/// Album and artist cards.
fn collection_grid(
    model: &NoirPlayerModel,
    tab: LibraryTab,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let groups = if tab == LibraryTab::Albums {
        &model.albums
    } else {
        &model.artists
    };
    let visible: Vec<_> = groups
        .iter()
        .filter_map(|(name, indices)| {
            let filtered = model.filtered_indices(indices.clone(), cx);
            (!filtered.is_empty()).then(|| (name.clone(), filtered))
        })
        .collect();

    if visible.is_empty() {
        return empty_state(
            "No matching collections",
            "Album and artist information comes from file tags.",
            is_light,
        )
        .into_any_element();
    }

    smooth_scroll(
        format!("collections-scroll-{tab:?}"),
        div().w_full().px(px(CONTENT_PADDING)).pb(px(20.0)).child(
            h_flex()
                .w_full()
                .flex_wrap()
                .gap(px(18.0))
                .children(visible.into_iter().map(|(name, songs)| {
                    let collection = if tab == LibraryTab::Albums {
                        Collection::Album(name.clone())
                    } else {
                        Collection::Artist(name.clone())
                    };
                    let artwork = songs
                        .iter()
                        .find_map(|&index| model.tracks.get(index))
                        .cloned();
                    collection_card(
                        ElementId::from(SharedString::from(format!("collection-{tab:?}-{name}"))),
                        name.clone(),
                        format!("{} songs", songs.len()),
                        artwork.as_ref(),
                        if tab == LibraryTab::Albums {
                            MusicIcon::Disc3
                        } else {
                            MusicIcon::Mic
                        },
                        is_light,
                        cx.listener(move |this, _, _, cx| {
                            this.library_detail = Some(collection.clone());
                            cx.notify();
                        }),
                    )
                })),
        ),
    )
    .into_any_element()
}

fn folder_grid(
    model: &NoirPlayerModel,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let folders = model.effective_music_folders();
    if folders.is_empty() {
        return empty_state(
            "No folders configured",
            "Add music folders in Settings (Ctrl+,).",
            is_light,
        )
        .into_any_element();
    }

    smooth_scroll(
        "folders-scroll",
        div().w_full().px(px(CONTENT_PADDING)).pb(px(20.0)).child(
            h_flex()
                .w_full()
                .flex_wrap()
                .gap(px(18.0))
                .children(folders.into_iter().map(|folder| {
                    let count = model
                        .tracks
                        .iter()
                        .filter(|track| track.path.starts_with(&folder))
                        .count();
                    let name = folder
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("Folder")
                        .to_string();
                    let target = folder.clone();
                    collection_card(
                        ElementId::from(SharedString::from(format!(
                            "folder-card-{}",
                            folder.display()
                        ))),
                        name,
                        format!("{count} songs"),
                        None,
                        MusicIcon::Folder,
                        is_light,
                        cx.listener(move |this, _, _, cx| {
                            this.library_detail = Some(Collection::Folder(target.clone()));
                            cx.notify();
                        }),
                    )
                })),
        ),
    )
    .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn collection_card(
    id: ElementId,
    name: String,
    subtitle: String,
    artwork: Option<&Track>,
    fallback: MusicIcon,
    is_light: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    v_flex()
        .id(id)
        .w(px(168.0))
        .gap(px(8.0))
        .p(px(10.0))
        .rounded_xl()
        .cursor_pointer()
        .hover(move |s| s.bg(dynamic_row_hover(is_light)))
        .on_click(handler)
        .child(match artwork.and_then(|track| track.artwork.clone()) {
            Some(bytes) => img_from_bytes(bytes)
                .size(px(148.0))
                .rounded_lg()
                .into_any_element(),
            None => div()
                .size(px(148.0))
                .rounded_lg()
                .bg(red_a(0.12))
                .flex()
                .items_center()
                .justify_center()
                .text_color(red())
                .child(icon_text(fallback, 64.0))
                .into_any_element(),
        })
        .child(
            div()
                .w_full()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(dynamic_text(is_light))
                .overflow_hidden()
                .text_ellipsis()
                .child(name),
        )
        .child(
            div()
                .w_full()
                .text_xs()
                .text_color(dynamic_subtitle(is_light))
                .overflow_hidden()
                .text_ellipsis()
                .child(subtitle),
        )
}
