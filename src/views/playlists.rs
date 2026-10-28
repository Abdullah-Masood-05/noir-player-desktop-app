use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::button::Button;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::{Collection, NoirPlayerModel};
use crate::views::library::{
    empty_state, search_bar, virtual_song_list, virtual_song_section, TrailingRows,
};
use crate::views::ui::{
    dynamic_border, dynamic_muted, dynamic_row_hover, dynamic_subtitle, dynamic_text, icon_text,
    red, red_a, smooth_scroll, tab_transition, top_fade,
};

const CONTENT_PADDING: f32 = 24.0;

pub fn render_playlists(
    model: &mut NoirPlayerModel,
    window: &mut Window,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let detail = model.playlist_detail.clone();

    let body = match detail.as_ref() {
        Some(name) => playlist_detail(model, name.clone(), window, is_light, cx),
        None => playlist_grid(model, is_light, cx),
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
                .child(header(model, detail.clone(), is_light, cx))
                .child(tab_transition(format!("playlist-body-{detail:?}"), body)),
        )
}

fn header(
    model: &NoirPlayerModel,
    detail: Option<String>,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let title = detail.clone().unwrap_or_else(|| "Playlists".to_string());

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
                .when(detail.is_some(), |d| {
                    d.child(
                        div()
                            .id("playlist-back")
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
                                this.playlist_detail = None;
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
            Button::new("new-playlist").label("New playlist").on_click(
                cx.listener(|this, _, window, cx| this.create_playlist_dialog(window, cx)),
            ),
        )
}

fn playlist_grid(
    model: &NoirPlayerModel,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    if model.store.playlists.is_empty() {
        return empty_state(
            "No playlists yet",
            "Create a named playlist, then add songs from your library.",
            is_light,
        )
        .into_any_element();
    }

    smooth_scroll(
        "playlists-scroll",
        div().w_full().px(px(CONTENT_PADDING)).pb(px(20.0)).child(
            h_flex()
                .w_full()
                .flex_wrap()
                .gap(px(18.0))
                .children(model.store.playlists.iter().map(|playlist| {
                    let name = playlist.name.clone();
                    let open = name.clone();
                    let available = model
                        .collection_indices(&Collection::Playlist(name.clone()))
                        .len();
                    v_flex()
                        .id(SharedString::from(format!("playlist-{name}")))
                        .w(px(168.0))
                        .gap(px(8.0))
                        .p(px(10.0))
                        .rounded_xl()
                        .cursor_pointer()
                        .hover(move |s| s.bg(dynamic_row_hover(is_light)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.playlist_detail = Some(open.clone());
                            cx.notify();
                        }))
                        .child(
                            div()
                                .size(px(148.0))
                                .rounded_lg()
                                .bg(red_a(0.12))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(red())
                                .child(icon_text(MusicIcon::ListMusic, 56.0)),
                        )
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
                                .child(format!(
                                    "{} songs \u{2022} {available} available",
                                    playlist.paths.len()
                                )),
                        )
                })),
        ),
    )
    .into_any_element()
}

/// One "Unavailable" row plus the gap above it. The row itself is pinned to
/// `MISSING_ROW_HEIGHT` so the virtual list's fixed sizes stay exact.
const MISSING_ROW_HEIGHT: f32 = 48.0;
const MISSING_ROW_GAP: f32 = 14.0;

fn playlist_detail(
    model: &mut NoirPlayerModel,
    name: String,
    window: &mut Window,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let collection = Collection::Playlist(name.clone());
    let indices =
        model.sorted_indices(model.filtered_indices(model.collection_indices(&collection), cx));
    // Cached on the model until the library or playlists change; this runs
    // every render, including every playback tick.
    let missing = model.playlist_missing(&name);
    let saved = model
        .store
        .playlists
        .iter()
        .find(|playlist| playlist.name == name);
    let total = saved.map_or(0, |playlist| playlist.paths.len());

    let summary = div()
        .flex_shrink_0()
        .px(px(CONTENT_PADDING))
        .text_sm()
        .text_color(dynamic_subtitle(is_light))
        .child(format!(
            "{total} saved songs \u{2022} {} matching \u{2022} {} unavailable",
            indices.len(),
            missing.len()
        ));

    if total == 0 {
        return smooth_scroll(
            format!("playlist-detail-scroll-{name}"),
            v_flex()
                .w_full()
                .pb(px(20.0))
                .gap(px(14.0))
                .child(summary)
                .child(div().px(px(CONTENT_PADDING)).child(empty_state(
                    "This playlist is empty",
                    "Use the song menu in your library to add songs here.",
                    is_light,
                ))),
        )
        .into_any_element();
    }

    // Unavailable files follow the songs inside the same virtual list, so
    // only the visible ones are built however long the playlist is.
    let trailing = (!missing.is_empty()).then(|| TrailingRows {
        height: MISSING_ROW_GAP + MISSING_ROW_HEIGHT,
        count: missing.len(),
        render: {
            let name = name.clone();
            Rc::new(move |row: usize, cx: &mut Context<NoirPlayerModel>| {
                missing_row(&name, missing[row].clone(), is_light, cx)
            })
        },
    });
    let list_id = format!("playlist-detail-{name}");
    let queue: Rc<[usize]> = Rc::from(indices.as_slice());

    let body = if indices.is_empty() {
        // Nothing playable matches: the message, then any unavailable files.
        v_flex()
            .w_full()
            .flex_1()
            .min_h_0()
            .px(px(CONTENT_PADDING))
            .pb(px(20.0))
            .child(
                div()
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(dynamic_subtitle(is_light))
                    .child("No available songs match the search."),
            )
            .when_some(trailing, |d, trailing| {
                d.child(div().w_full().flex_1().min_h_0().child(virtual_song_list(
                    model,
                    list_id,
                    &[],
                    queue,
                    Some(name.clone().into()),
                    Some(trailing),
                    window,
                    is_light,
                    cx,
                )))
            })
            .into_any_element()
    } else {
        virtual_song_section(
            model,
            list_id,
            "Songs",
            None,
            &indices,
            queue,
            Some(name.clone().into()),
            trailing,
            window,
            is_light,
            cx,
        )
    };

    v_flex()
        .w_full()
        .flex_1()
        .min_h_0()
        .gap(px(14.0))
        .child(summary)
        .child(body)
        .into_any_element()
}

fn missing_row(
    name: &str,
    path: PathBuf,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let name = name.to_string();
    div()
        .w_full()
        .pt(px(MISSING_ROW_GAP))
        .child(
            h_flex()
                .w_full()
                .h(px(MISSING_ROW_HEIGHT))
                .gap(px(12.0))
                .items_center()
                .px(px(10.0))
                .rounded_lg()
                .border_1()
                .border_color(dynamic_border(is_light))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(dynamic_muted(is_light))
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format!("Unavailable: {}", path.display())),
                )
                .child(
                    // Keyed by the path, not the row position, so the id
                    // follows the file when rows above it are removed.
                    Button::new(SharedString::from(format!(
                        "remove-missing-{}",
                        path.display()
                    )))
                    .label("Remove")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_from_playlist(&name, &path, cx);
                    })),
                ),
        )
        .into_any_element()
}
