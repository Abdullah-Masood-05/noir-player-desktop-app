use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::views::ui::{
    dynamic_bg, dynamic_hover, dynamic_muted, dynamic_subtitle, dynamic_text, icon_text,
    img_from_bytes, red, red_a, white,
};

pub fn render_player(model: &mut NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let is_playing = model.is_playing;
    let progress = model.progress();
    let (elapsed, total) = model.times();
    let (title, artist, album) = match model.now_playing() {
        Some(t) => (t.title.clone(), t.artist.clone(), t.album.clone()),
        None => (
            "No song is currently playing".to_string(),
            String::new(),
            String::new(),
        ),
    };
    let artwork = model.now_playing().and_then(|t| t.artwork.clone());
    let shuffle = model.shuffle;
    let repeat_all = model.repeat_all;
    let has_track = model.current.is_some();

    let seek_interval = model.store.seek_interval_seconds;
    let eq_enabled = model.store.equalizer_enabled;

    v_flex()
        .size_full()
        .relative()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(300.0))
                .bg(linear_gradient(
                    180.0,
                    linear_color_stop(red_a(if is_light { 0.24 } else { 0.28 }), 0.0),
                    linear_color_stop(dynamic_bg(is_light).alpha(0.0), 1.0),
                )),
        )
        .child(
            v_flex()
                .size_full()
                .relative()
                .child(player_header(cx, eq_enabled, is_light))
                .child(
                    v_flex()
                        .flex_1()
                        .min_h_0()
                        .items_center()
                        .justify_center()
                        .gap(px(28.0))
                        .px(px(24.0))
                        .child(artwork_large(artwork, is_playing))
                        .child(
                            v_flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(dynamic_text(is_light))
                                        .text_center()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(red())
                                        .child(artist),
                                )
                                .when(!album.is_empty(), |d| {
                                    d.child(
                                        div()
                                            .text_xs()
                                            .text_color(dynamic_subtitle(is_light))
                                            .child(album),
                                    )
                                }),
                        ),
                )
                .child(
                    v_flex()
                        .w_full()
                        .flex_shrink_0()
                        .px(px(24.0))
                        .pb(px(20.0))
                        .gap(px(14.0))
                        .child(
                            v_flex()
                                .w_full()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(4.0))
                                        .rounded_full()
                                        .bg(red_a(0.18))
                                        .overflow_hidden()
                                        .child(div().h_full().bg(red()).w(relative(progress))),
                                )
                                .child(
                                    h_flex()
                                        .w_full()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(dynamic_muted(is_light))
                                                .child(elapsed),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(dynamic_muted(is_light))
                                                .child(total),
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_center()
                                .gap(px(24.0))
                                .child(
                                    div()
                                        .id("shuffle")
                                        .size(px(40.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .text_color(if shuffle {
                                            red()
                                        } else {
                                            dynamic_subtitle(is_light)
                                        })
                                        .hover(move |s| {
                                            s.bg(dynamic_hover(is_light)).text_color(if shuffle {
                                                red()
                                            } else {
                                                dynamic_text(is_light)
                                            })
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.shuffle = !this.shuffle;
                                            cx.notify();
                                        }))
                                        .child(icon_text(MusicIcon::Shuffle, 20.0)),
                                )
                                .child(prev_btn(is_light, cx))
                                .child(skip_back_btn(seek_interval, is_light, cx))
                                .child(
                                    div()
                                        .id("play-pause")
                                        .size(px(72.0))
                                        .rounded_full()
                                        .bg(red())
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(white(1.0))
                                        .shadow(vec![BoxShadow::new(px(0.), px(6.), red_a(0.45))
                                            .blur_radius(px(18.0))])
                                        .cursor_pointer()
                                        .hover(|s| s.opacity(0.88))
                                        .when(has_track, |d| {
                                            d.on_click(cx.listener(|this, _, _, cx| {
                                                this.toggle_play(cx);
                                            }))
                                        })
                                        .child(icon_text(
                                            if is_playing {
                                                MusicIcon::Pause
                                            } else {
                                                MusicIcon::Play
                                            },
                                            34.0,
                                        )),
                                )
                                .child(skip_fwd_btn(seek_interval, is_light, cx))
                                .child(next_btn(is_light, cx))
                                .child(
                                    div()
                                        .id("repeat")
                                        .size(px(40.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .text_color(if repeat_all {
                                            red()
                                        } else {
                                            dynamic_subtitle(is_light)
                                        })
                                        .hover(move |s| {
                                            s.bg(dynamic_hover(is_light)).text_color(
                                                if repeat_all {
                                                    red()
                                                } else {
                                                    dynamic_text(is_light)
                                                },
                                            )
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.repeat_all = !this.repeat_all;
                                            cx.notify();
                                        }))
                                        .child(icon_text(MusicIcon::Repeat, 20.0)),
                                ),
                        )
                        .child(volume_control_row(model.volume, is_light, cx)),
                ),
        )
}

fn volume_control_row(volume: f32, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    let pct = (volume * 100.0).round() as u32;
    let icon = if volume == 0.0 {
        MusicIcon::VolumeX
    } else if volume < 0.5 {
        MusicIcon::Volume1
    } else {
        MusicIcon::Volume2
    };

    h_flex()
        .w_full()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .pt(px(4.0))
        .child(
            div()
                .id("player-mute-toggle")
                .size(px(32.0))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(if volume == 0.0 {
                    red()
                } else {
                    dynamic_subtitle(is_light)
                })
                .hover(move |s| {
                    s.bg(dynamic_hover(is_light))
                        .text_color(dynamic_text(is_light))
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.volume > 0.0 {
                        this.set_volume(0.0, cx);
                    } else {
                        this.set_volume(0.9, cx);
                    }
                }))
                .child(icon_text(icon, 18.0)),
        )
        .child(
            div()
                .id("player-vol-down")
                .size(px(28.0))
                .rounded_lg()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(dynamic_subtitle(is_light))
                .hover(move |s| {
                    s.bg(dynamic_hover(is_light))
                        .text_color(dynamic_text(is_light))
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.adjust_volume(-0.05, cx);
                }))
                .child("−"),
        )
        .child(
            div()
                .id("player-vol-track")
                .w(px(160.0))
                .h(px(6.0))
                .rounded_full()
                .bg(red_a(0.18))
                .overflow_hidden()
                .child(div().h_full().bg(red()).rounded_full().w(relative(volume))),
        )
        .child(
            div()
                .id("player-vol-up")
                .size(px(28.0))
                .rounded_lg()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(dynamic_subtitle(is_light))
                .hover(move |s| {
                    s.bg(dynamic_hover(is_light))
                        .text_color(dynamic_text(is_light))
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.adjust_volume(0.05, cx);
                }))
                .child("+"),
        )
        .child(
            div()
                .w(px(40.0))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(dynamic_subtitle(is_light))
                .child(format!("{pct}%")),
        )
}

fn player_header(cx: &mut Context<NoirPlayerModel>, eq_enabled: bool, is_light: bool) -> Div {
    h_flex()
        .w_full()
        .h(px(56.0))
        .px(px(16.0))
        .items_center()
        .justify_between()
        .flex_shrink_0()
        .child(div().size(px(36.0)).flex().items_center().justify_center())
        .child(
            div()
                .text_center()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(dynamic_text(is_light))
                .child("Now Playing"),
        )
        .child(
            h_flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .id("player-eq-btn")
                        .size(px(36.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(if eq_enabled {
                            red()
                        } else {
                            dynamic_subtitle(is_light)
                        })
                        .cursor_pointer()
                        .hover(move |s| {
                            s.bg(dynamic_hover(is_light)).text_color(if eq_enabled {
                                red()
                            } else {
                                dynamic_text(is_light)
                            })
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle_equalizer(cx);
                        }))
                        .child(icon_text(MusicIcon::SlidersHorizontal, 20.0)),
                )
                .child(
                    div()
                        .id("player-settings-btn")
                        .size(px(36.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(dynamic_subtitle(is_light))
                        .cursor_pointer()
                        .hover(move |s| {
                            s.bg(dynamic_hover(is_light))
                                .text_color(dynamic_text(is_light))
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle_settings(cx);
                        }))
                        .child(icon_text(MusicIcon::Settings, 20.0)),
                ),
        )
}

fn skip_back_btn(
    interval: u32,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Stateful<Div> {
    div()
        .id("skip-back")
        .size(px(40.0))
        .rounded_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .text_color(dynamic_text(is_light))
        .cursor_pointer()
        .hover(move |s| s.bg(dynamic_hover(is_light)))
        .on_click(cx.listener(|this, _, _, cx| this.skip_backward(cx)))
        .child(icon_text(MusicIcon::RotateCcw, 18.0))
        .child(
            div()
                .text_size(px(9.0))
                .font_weight(FontWeight::BOLD)
                .text_color(dynamic_subtitle(is_light))
                .mt(px(-2.0))
                .child(format!("{interval}s")),
        )
}

fn skip_fwd_btn(interval: u32, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Stateful<Div> {
    div()
        .id("skip-forward")
        .size(px(40.0))
        .rounded_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .text_color(dynamic_text(is_light))
        .cursor_pointer()
        .hover(move |s| s.bg(dynamic_hover(is_light)))
        .on_click(cx.listener(|this, _, _, cx| this.skip_forward(cx)))
        .child(icon_text(MusicIcon::RotateCw, 18.0))
        .child(
            div()
                .text_size(px(9.0))
                .font_weight(FontWeight::BOLD)
                .text_color(dynamic_subtitle(is_light))
                .mt(px(-2.0))
                .child(format!("{interval}s")),
        )
}

fn prev_btn(is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Stateful<Div> {
    div()
        .id("prev")
        .size(px(40.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(dynamic_text(is_light))
        .cursor_pointer()
        .hover(move |s| s.bg(dynamic_hover(is_light)))
        .on_click(cx.listener(|this, _, _, cx| this.prev(cx)))
        .child(icon_text(MusicIcon::SkipBack, 22.0))
}

fn next_btn(is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Stateful<Div> {
    div()
        .id("next")
        .size(px(40.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(dynamic_text(is_light))
        .cursor_pointer()
        .hover(move |s| s.bg(dynamic_hover(is_light)))
        .on_click(cx.listener(|this, _, _, cx| this.next(cx)))
        .child(icon_text(MusicIcon::SkipForward, 22.0))
}

fn artwork_large(artwork: Option<std::sync::Arc<[u8]>>, is_playing: bool) -> Div {
    let base = div()
        .size(px(280.0))
        .rounded_3xl()
        .overflow_hidden()
        .flex_shrink_0()
        .when(is_playing, |d| {
            d.shadow(vec![
                BoxShadow::new(px(0.), px(14.), red_a(0.35)).blur_radius(px(50.0))
            ])
        })
        .when(!is_playing, |d| {
            d.opacity(0.93).shadow(vec![BoxShadow::new(
                px(0.),
                px(14.),
                hsla(0., 0., 0., 0.45),
            )
            .blur_radius(px(28.0))])
        });

    match artwork {
        Some(bytes) => base.child(img_from_bytes(bytes).size_full().rounded_3xl()),
        None => base
            .bg(red_a(0.12))
            .flex()
            .items_center()
            .justify_center()
            .text_color(red())
            .child(icon_text(MusicIcon::Music4, 110.0)),
    }
}
