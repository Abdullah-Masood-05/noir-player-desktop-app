use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::button::Button;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::{Collection, NoirPlayerModel};
use crate::views::library::{empty_state, search_bar, song_table};
use crate::views::ui::{
    dynamic_border, dynamic_muted, dynamic_row_hover, dynamic_subtitle, dynamic_text, icon_text,
    red, red_a, smooth_scroll, tab_transition, top_fade,
};

const CONTENT_PADDING: f32 = 24.0;

pub fn render_playlists(model: &mut NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let detail = model.playlist_detail.clone();

    let body = match detail.as_ref() {
        Some(name) => playlist_detail(model, name.clone(), is_light, cx),
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

fn playlist_detail(
    model: &NoirPlayerModel,
    name: String,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let collection = Collection::Playlist(name.clone());
    let indices =
        model.sorted_indices(model.filtered_indices(model.collection_indices(&collection), cx));
    let saved = model
        .store
        .playlists
        .iter()
        .find(|playlist| playlist.name == name);
    let missing: Vec<_> = saved
        .into_iter()
        .flat_map(|playlist| &playlist.paths)
        .filter(|path| !model.tracks.iter().any(|track| &track.path == *path))
        .cloned()
        .collect();
    let total = saved.map_or(0, |playlist| playlist.paths.len());

    smooth_scroll(
        format!("playlist-detail-scroll-{name}"),
        v_flex()
            .w_full()
            .px(px(CONTENT_PADDING))
            .pb(px(20.0))
            .gap(px(14.0))
            .child(
                div()
                    .text_sm()
                    .text_color(dynamic_subtitle(is_light))
                    .child(format!(
                        "{total} saved songs \u{2022} {} matching \u{2022} {} unavailable",
                        indices.len(),
                        missing.len()
                    )),
            )
            .child(if total == 0 {
                empty_state(
                    "This playlist is empty",
                    "Use the song menu in your library to add songs here.",
                    is_light,
                )
                .into_any_element()
            } else if indices.is_empty() {
                div()
                    .text_sm()
                    .text_color(dynamic_subtitle(is_light))
                    .child("No available songs match the search.")
                    .into_any_element()
            } else {
                song_table(
                    model,
                    "Songs",
                    None,
                    indices,
                    Some(name.clone()),
                    is_light,
                    cx,
                )
            })
            .children(missing.into_iter().enumerate().map(|(position, path)| {
                let name = name.clone();
                h_flex()
                    .w_full()
                    .gap(px(12.0))
                    .items_center()
                    .px(px(10.0))
                    .py(px(8.0))
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
                        Button::new(SharedString::from(format!("remove-missing-{position}")))
                            .label("Remove")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.remove_from_playlist(&name, &path, cx);
                            })),
                    )
            })),
    )
    .into_any_element()
}
