use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::Input;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::views::ui::{
    dynamic_border, dynamic_subtitle, dynamic_surface, dynamic_text, icon_text, red, red_a,
    top_fade,
};

pub fn render_discover(model: &mut NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let query = model.discover_search.read(cx).value();
    let content = if model.discover_loading {
        div()
            .p_4()
            .text_color(dynamic_subtitle(is_light))
            .child("Searching Last.fm...")
            .into_any_element()
    } else if let Some(error) = &model.discover_error {
        div()
            .p_4()
            .text_color(red())
            .child(error.clone())
            .into_any_element()
    } else if model.discover_tracks.is_empty() {
        div()
            .p_4()
            .text_color(dynamic_subtitle(is_light))
            .child("No tracks found. Try another title or artist.")
            .into_any_element()
    } else {
        v_flex()
            .gap_2()
            .children(model.discover_tracks.iter().enumerate().map(|(i, track)| {
                let play = track.clone();
                let download = track.clone();
                let busy = model.discover_audio_busy();
                h_flex()
                    .gap_3()
                    .p_3()
                    .rounded_xl()
                    .bg(dynamic_surface(is_light))
                    .when(is_light, |d| {
                        d.border_1().border_color(dynamic_border(is_light))
                    })
                    .items_center()
                    .child(
                        div()
                            .size(px(48.0))
                            .rounded_lg()
                            .bg(red_a(0.12))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(red())
                            .child(icon_text(MusicIcon::Music4, 30.0)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(dynamic_text(is_light))
                                    .text_ellipsis()
                                    .child(track.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(dynamic_subtitle(is_light))
                                    .text_ellipsis()
                                    .child(track.artist.clone()),
                            ),
                    )
                    .child(
                        Button::new(format!("discover-play-{i}"))
                            .label("Play")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.discover_audio(play.clone(), false, cx);
                            })),
                    )
                    .child(
                        Button::new(format!("discover-download-{i}"))
                            .label("Download")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.discover_audio(download.clone(), true, cx);
                            })),
                    )
                    .when_some(track.url.clone(), |row, url| {
                        row.child(
                            Button::new(format!("discover-open-{i}"))
                                .label("Open on Last.fm")
                                .on_click(move |_, _, cx| {
                                    if crate::api::safe_track_url(&url) {
                                        cx.open_url(&url);
                                    }
                                }),
                        )
                    })
            }))
            .into_any_element()
    };
    v_flex()
        .size_full()
        .relative()
        .child(top_fade(is_light))
        .child(
            v_flex()
                .size_full()
                .relative()
                .child(
                    h_flex()
                        .w_full()
                        .h(px(78.0))
                        .px(px(24.0))
                        .flex_shrink_0()
                        .items_center()
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(dynamic_text(is_light))
                                .child("Discover"),
                        ),
                )
                .child(
                    h_flex()
                        .p_3()
                        .gap_2()
                        .items_center()
                        .flex_shrink_0()
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(&model.discover_search).cleanable(true)),
                        )
                        .child(Button::new("discover-search").label("Search").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.load_discover(
                                    this.discover_search.read(cx).value().to_string(),
                                    cx,
                                );
                            }),
                        ))
                        .child(
                            Button::new("discover-trending")
                                .label("Trending")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.discover_search
                                        .update(cx, |input, cx| input.set_value("", window, cx));
                                    this.load_discover(String::new(), cx);
                                })),
                        ),
                )
                .child(
                    div()
                        .px_4()
                        .text_sm()
                        .text_color(red())
                        .child(if query.trim().is_empty() {
                            "Trending tracks"
                        } else {
                            "Search results"
                        }),
                )
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .text_xs()
                        .text_color(dynamic_subtitle(is_light))
                        .child("Play prepares a cached download before local playback; it is not streaming. Download saves tagged audio to your Music folder."),
                )
                .children(model.discover_audio_status.clone().map(|status| {
                    h_flex()
                        .px_4()
                        .py_2()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .text_color(dynamic_text(is_light))
                                .child(status),
                        )
                        .when(model.discover_audio_busy(), |row| {
                            row.child(
                                Button::new("discover-cancel-audio")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.cancel_discover_audio(cx)
                                    })),
                            )
                        })
                }))
                .children(model.discover_audio_error.clone().map(|error| {
                    div().px_4().py_2().text_sm().text_color(red()).child(error)
                }))
                .child(crate::views::ui::smooth_scroll(
                    "discover-results",
                    div().p_3().child(content),
                )),
        )
}
