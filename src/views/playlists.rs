use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::button::Button;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::{Collection, NoirPlayerModel};
use crate::views::library::{app_bar, empty_state, search_bar, song_row};
use crate::views::ui::{
    dynamic_subtitle, dynamic_text, icon_text, red_a, smooth_scroll, tab_transition, top_fade,
};

pub fn render_playlists(model: &mut NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let detail = model.playlist_detail.clone();
    let body = if let Some(name) = detail.as_ref() {
        let collection = Collection::Playlist(name.clone());
        let indices = model.filtered_indices(model.collection_indices(&collection), cx);
        let saved = model
            .store
            .playlists
            .iter()
            .find(|playlist| &playlist.name == name);
        let missing: Vec<_> = saved
            .into_iter()
            .flat_map(|playlist| &playlist.paths)
            .filter(|path| !model.tracks.iter().any(|track| &track.path == *path))
            .cloned()
            .collect();
        let total = saved.map_or(0, |playlist| playlist.paths.len());
        v_flex()
            .flex_1()
            .min_h_0()
            .child(
                h_flex()
                    .flex_shrink_0()
                    .px(px(12.0))
                    .gap(px(12.0))
                    .items_center()
                    .child(
                        Button::new("playlist-back")
                            .label("Back")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.playlist_detail = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(dynamic_subtitle(is_light))
                            .child(format!(
                                "{total} saved songs · {} matching · {} unavailable",
                                indices.len(),
                                missing.len()
                            )),
                    ),
            )
            .child(search_bar(model))
            .child(smooth_scroll(
                format!("playlist-detail-scroll-{name}"),
                div()
                    .px(px(8.0))
                    .pb(px(12.0))
                    .children((total == 0).then(|| {
                        empty_state(
                            "This playlist is empty",
                            "Use Add beside a library song to add it here.",
                            is_light,
                        )
                    }))
                    .children((total > 0 && indices.is_empty()).then(|| {
                        div()
                            .p(px(12.0))
                            .text_sm()
                            .text_color(dynamic_subtitle(is_light))
                            .child("No available songs match the search.")
                    }))
                    .children(indices.iter().map(|&index| {
                        song_row(model, index, indices.clone(), Some(name.clone()), cx)
                    }))
                    .children(missing.into_iter().enumerate().map(|(index, path)| {
                        let name = name.clone();
                        h_flex()
                            .gap(px(12.0))
                            .items_center()
                            .p(px(10.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .text_color(dynamic_subtitle(is_light))
                                    .child(format!("Unavailable: {}", path.display())),
                            )
                            .child(
                                Button::new(format!("remove-missing-{index}"))
                                    .label("Remove")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.remove_from_playlist(&name, &path, cx);
                                    })),
                            )
                    })),
            ))
            .into_any_element()
    } else if model.store.playlists.is_empty() {
        empty_state(
            "No playlists yet",
            "Create a named playlist, then add songs from your library.",
            is_light,
        )
        .into_any_element()
    } else {
        smooth_scroll(
            "playlists-scroll",
            div().child(h_flex().flex_wrap().gap(px(14.0)).p(px(12.0)).children(
                model.store.playlists.iter().map(|playlist| {
                    let name = playlist.name.clone();
                    let available = model
                        .collection_indices(&Collection::Playlist(name.clone()))
                        .len();
                    v_flex()
                        .id(format!("playlist-{name}"))
                        .w(px(150.0))
                        .gap(px(8.0))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.85))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.playlist_detail = Some(name.clone());
                            cx.notify();
                        }))
                        .child(
                            div()
                                .size(px(150.0))
                                .rounded_2xl()
                                .bg(red_a(0.12))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(red_a(1.0))
                                .child(icon_text(MusicIcon::ListMusic, 52.0)),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(dynamic_text(is_light))
                                .child(playlist.name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(dynamic_subtitle(is_light))
                                .child(format!(
                                    "{} songs · {available} available",
                                    playlist.paths.len()
                                )),
                        )
                }),
            )),
        )
        .into_any_element()
    };
    v_flex()
        .size_full()
        .min_h_0()
        .relative()
        .child(top_fade(is_light))
        .child(
            v_flex()
                .size_full()
                .relative()
                .child(app_bar(detail.as_deref().unwrap_or("Playlists"), is_light))
                .child(tab_transition(format!("playlist-body-{detail:?}"), body))
                .child(h_flex().flex_shrink_0().p(px(12.0)).justify_end().child(
                    Button::new("new-playlist").label("New playlist").on_click(
                        cx.listener(|this, _, window, cx| this.create_playlist_dialog(window, cx)),
                    ),
                )),
        )
}
