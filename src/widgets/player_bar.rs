use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::base::ElementExt;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::{ActiveTab, NoirPlayerModel};
use crate::views::library::artwork_thumb;
use crate::views::ui::{
    bottom_aura, dynamic_border, dynamic_muted, dynamic_panel, dynamic_row_hover, dynamic_subtitle,
    dynamic_text, icon_text, red, red_a, waveform, white,
};

/// The persistent transport bar across the bottom of the window.
pub fn player_bar(model: &NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let is_playing = model.is_playing;
    let progress = model.progress();
    let (elapsed, total) = model.times();
    let has_track = model.current.is_some();
    let favourite = model.current.is_some_and(|index| model.is_favourite(index));
    let meter = model.audio_meter();

    h_flex()
        .w_full()
        .h(px(96.0))
        .flex_shrink_0()
        .items_center()
        .gap(px(16.0))
        .px(px(16.0))
        .relative()
        .overflow_hidden()
        .bg(dynamic_panel(is_light))
        .border_t_1()
        .border_color(dynamic_border(is_light))
        .child(bottom_aura(is_light))
        .child(now_playing_cell(model, favourite, is_light, cx))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(6.0))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_center()
                        .gap(px(18.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .justify_end()
                                .overflow_hidden()
                                .child(waveform(
                                    "bar-wave-left",
                                    26,
                                    2.0,
                                    2.0,
                                    26.0,
                                    is_playing,
                                    meter.clone(),
                                )),
                        )
                        .child(transport(model, is_playing, has_track, is_light, cx))
                        .child(div().flex_1().min_w_0().overflow_hidden().child(waveform(
                            "bar-wave-right",
                            26,
                            2.0,
                            2.0,
                            26.0,
                            is_playing,
                            meter.clone(),
                        ))),
                )
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap(px(10.0))
                        .child(
                            div()
                                .w(px(38.0))
                                .text_xs()
                                .text_color(dynamic_muted(is_light))
                                .child(elapsed),
                        )
                        .child(seek_bar(progress, is_light, cx))
                        .child(
                            div()
                                .w(px(38.0))
                                .text_xs()
                                .text_color(dynamic_muted(is_light))
                                .child(total),
                        ),
                ),
        )
        .child(secondary_controls(model, is_light, cx))
}

fn now_playing_cell(
    model: &NoirPlayerModel,
    favourite: bool,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let current = model.current;
    let (title, artist) = match model.now_playing() {
        Some(track) => (track.title.clone(), track.artist.clone()),
        None => ("Nothing playing".to_string(), "Pick a song".to_string()),
    };

    h_flex()
        .w(px(300.0))
        .flex_shrink_0()
        .items_center()
        .gap(px(12.0))
        .child(
            h_flex()
                .id("bar-open-player")
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(12.0))
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.current.is_some() {
                        this.active_tab = ActiveTab::Player;
                        if this.lyrics_open {
                            this.refresh_lyrics(cx);
                        }
                        cx.notify();
                    }
                }))
                .children(model.now_playing().map(|track| artwork_thumb(track, 56.0)))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(dynamic_text(is_light))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(dynamic_subtitle(is_light))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(artist),
                        ),
                ),
        )
        .child(
            div()
                .id("bar-favourite")
                .size(px(32.0))
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
                .hover(move |s| s.bg(dynamic_row_hover(is_light)).text_color(red()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(index) = current {
                        this.toggle_favourite(index, cx);
                    }
                }))
                .child(icon_text(MusicIcon::Heart, 18.0)),
        )
        .child(
            div()
                .id("bar-more")
                .size(px(32.0))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(dynamic_muted(is_light))
                .hover(move |s| {
                    s.bg(dynamic_row_hover(is_light))
                        .text_color(dynamic_text(is_light))
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(index) = current {
                        this.song_actions_dialog(index, None, window, cx);
                    }
                }))
                .child(icon_text(MusicIcon::Ellipsis, 18.0)),
        )
}

fn transport(
    model: &NoirPlayerModel,
    is_playing: bool,
    has_track: bool,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let shuffle = model.shuffle;
    let repeat_all = model.repeat_all;

    h_flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(14.0))
        .child(
            round_button("bar-shuffle", MusicIcon::Shuffle, 18.0, 34.0, is_light)
                .text_color(if shuffle {
                    red()
                } else {
                    dynamic_subtitle(is_light)
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.shuffle = !this.shuffle;
                    cx.notify();
                })),
        )
        .child(
            round_button("bar-prev", MusicIcon::SkipBack, 20.0, 36.0, is_light)
                .text_color(dynamic_text(is_light))
                .on_click(cx.listener(|this, _, _, cx| this.prev(cx))),
        )
        .child(
            div()
                .id("bar-play")
                .size(px(48.0))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(red())
                .text_color(white(1.0))
                .cursor_pointer()
                .shadow(vec![
                    BoxShadow::new(px(0.), px(4.), red_a(0.45)).blur_radius(px(16.0))
                ])
                .hover(|s| s.opacity(0.9))
                .when(has_track, |d| {
                    d.on_click(cx.listener(|this, _, _, cx| this.toggle_play(cx)))
                })
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
            round_button("bar-next", MusicIcon::SkipForward, 20.0, 36.0, is_light)
                .text_color(dynamic_text(is_light))
                .on_click(cx.listener(|this, _, _, cx| this.next(cx))),
        )
        .child(
            round_button("bar-repeat", MusicIcon::Repeat, 18.0, 34.0, is_light)
                .text_color(if repeat_all {
                    red()
                } else {
                    dynamic_subtitle(is_light)
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.repeat_all = !this.repeat_all;
                    cx.notify();
                })),
        )
}

fn secondary_controls(
    model: &NoirPlayerModel,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let lyrics_open = model.lyrics_open;
    let eq_enabled = model.store.equalizer_enabled;
    let queue_open = model.queue_open;
    let volume = model.volume;

    h_flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(6.0))
        .child(
            labelled_button("bar-lyrics", MusicIcon::FileText, "Lyrics", is_light)
                .when(lyrics_open, |d| d.text_color(red()))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_lyrics(cx))),
        )
        .child(
            labelled_button(
                "bar-equalizer",
                MusicIcon::SlidersHorizontal,
                "Equalizer",
                is_light,
            )
            .when(eq_enabled, |d| d.text_color(red()))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_equalizer(cx))),
        )
        .child(
            h_flex()
                .items_center()
                .gap(px(8.0))
                .pl(px(8.0))
                .child(
                    round_button("bar-mute", volume_icon(volume), 18.0, 30.0, is_light)
                        .text_color(if volume == 0.0 {
                            red()
                        } else {
                            dynamic_subtitle(is_light)
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            let next = if this.volume > 0.0 { 0.0 } else { 0.9 };
                            this.set_volume(next, cx);
                        })),
                )
                .child(volume_bar(volume, is_light, cx)),
        )
        .child(
            round_button("bar-queue", MusicIcon::ListMusic, 20.0, 34.0, is_light)
                .text_color(if queue_open {
                    red()
                } else {
                    dynamic_subtitle(is_light)
                })
                .on_click(cx.listener(|this, _, _, cx| this.toggle_queue_panel(cx))),
        )
}

pub fn volume_icon(volume: f32) -> MusicIcon {
    if volume == 0.0 {
        MusicIcon::VolumeX
    } else if volume < 0.5 {
        MusicIcon::Volume1
    } else {
        MusicIcon::Volume2
    }
}

/// A circular icon button used across the transport controls.
pub fn round_button(
    id: &'static str,
    icon: MusicIcon,
    icon_size: f32,
    size: f32,
    is_light: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(size))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |s| s.bg(dynamic_row_hover(is_light)))
        .child(icon_text(icon, icon_size))
}

fn labelled_button(
    id: &'static str,
    icon: MusicIcon,
    label: &'static str,
    is_light: bool,
) -> Stateful<Div> {
    v_flex()
        .id(id)
        .px(px(10.0))
        .py(px(4.0))
        .gap(px(2.0))
        .items_center()
        .rounded_lg()
        .cursor_pointer()
        .text_color(dynamic_subtitle(is_light))
        .hover(move |s| s.bg(dynamic_row_hover(is_light)))
        .child(icon_text(icon, 18.0))
        .child(div().text_xs().child(label))
}

/// Click-to-seek scrubber with a draggable-looking knob.
pub fn seek_bar(progress: f32, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Stateful<Div> {
    let entity = cx.entity();
    div()
        .id("seek-bar")
        .flex_1()
        .min_w_0()
        .h(px(16.0))
        .relative()
        .flex()
        .items_center()
        .cursor_pointer()
        .on_prepaint(move |bounds, _, cx| {
            entity.update(cx, |this, _| this.seek_bounds = Some(bounds));
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                let Some(bounds) = this.seek_bounds else {
                    return;
                };
                if bounds.size.width <= px(0.0) {
                    return;
                }
                let fraction = (event.position.x - bounds.left()) / bounds.size.width;
                this.seek_fraction(fraction, cx);
            }),
        )
        .child(
            div()
                .w_full()
                .h(px(4.0))
                .rounded_full()
                .bg(if is_light { red_a(0.16) } else { white(0.12) })
                .child(
                    div()
                        .h_full()
                        .rounded_full()
                        .bg(red())
                        .w(relative(progress.clamp(0.0, 1.0))),
                ),
        )
        .child(
            div()
                .absolute()
                .top(px(4.0))
                .left(relative(progress.clamp(0.0, 1.0)))
                .size(px(10.0))
                .ml(px(-5.0))
                .rounded_full()
                .bg(red())
                .shadow(vec![
                    BoxShadow::new(px(0.), px(1.), red_a(0.6)).blur_radius(px(6.0))
                ]),
        )
}

/// Click-to-set volume slider.
fn volume_bar(volume: f32, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Stateful<Div> {
    let entity = cx.entity();
    div()
        .id("volume-bar")
        .w(px(96.0))
        .h(px(16.0))
        .relative()
        .flex()
        .items_center()
        .cursor_pointer()
        .on_prepaint(move |bounds, _, cx| {
            entity.update(cx, |this, _| this.volume_bounds = Some(bounds));
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                let Some(bounds) = this.volume_bounds else {
                    return;
                };
                if bounds.size.width <= px(0.0) {
                    return;
                }
                let fraction = (event.position.x - bounds.left()) / bounds.size.width;
                this.set_volume(fraction.clamp(0.0, 1.0), cx);
            }),
        )
        .child(
            div()
                .w_full()
                .h(px(4.0))
                .rounded_full()
                .bg(if is_light { red_a(0.16) } else { white(0.12) })
                .child(
                    div()
                        .h_full()
                        .rounded_full()
                        .bg(red())
                        .w(relative(volume.clamp(0.0, 1.0))),
                ),
        )
        .child(
            div()
                .absolute()
                .top(px(4.0))
                .left(relative(volume.clamp(0.0, 1.0)))
                .size(px(10.0))
                .ml(px(-5.0))
                .rounded_full()
                .bg(red()),
        )
}
