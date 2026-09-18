use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::views::ui::{icon_text, img_from_bytes, red, red_a, surface, white};

pub fn mini_player(model: &mut NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let Some(track) = model.now_playing() else {
        return div();
    };
    let is_playing = model.is_playing;
    let progress = model.progress();
    let title = track.title.clone();
    let artist = track.artist.clone();
    let artwork = track.artwork.clone();

    v_flex()
        .w_full()
        .flex_shrink_0()
        .bg(surface())
        .child(
            div()
                .w_full()
                .h(px(2.5))
                .bg(red_a(0.15))
                .overflow_hidden()
                .child(div().h_full().bg(red()).w(relative(progress))),
        )
        .child(
            h_flex()
                .id("mini-player-body")
                .items_center()
                .gap(px(12.0))
                .px(px(10.0))
                .py(px(8.0))
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.active_tab = crate::app::ActiveTab::Player;
                    cx.notify();
                }))
                .child(artwork_small(artwork))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .justify_center()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(white(0.55))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(artist),
                        ),
                )
                .child(
                    div()
                        .id("mini-play-pause")
                        .size(px(40.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(white(0.06)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.toggle_play(cx);
                        }))
                        .child(icon_text(
                            if is_playing {
                                MusicIcon::Pause
                            } else {
                                MusicIcon::Play
                            },
                            24.0,
                        )),
                )
                .child(
                    div()
                        .id("mini-next")
                        .size(px(40.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(white(0.06)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.next(cx);
                        }))
                        .child(icon_text(MusicIcon::SkipForward, 24.0)),
                ),
        )
}

fn artwork_small(artwork: Option<std::sync::Arc<[u8]>>) -> Div {
    let base = div().size(px(46.0)).rounded_lg().overflow_hidden();
    match artwork {
        Some(bytes) => base.child(img_from_bytes(bytes).size_full().rounded_lg()),
        None => base
            .bg(red_a(0.12))
            .flex()
            .items_center()
            .justify_center()
            .text_color(red())
            .child(icon_text(MusicIcon::Music4, 20.0)),
    }
}
