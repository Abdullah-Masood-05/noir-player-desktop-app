use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::base::ElementExt;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::{ActiveTab, NoirPlayerModel};
use crate::views::library::{album_line, artwork_thumb};
use crate::views::ui::{
    dynamic_bg, dynamic_border, dynamic_card, dynamic_muted, dynamic_row_hover, dynamic_subtitle,
    dynamic_text, format_duration, icon_text, img_from_image, red, red_a, smooth_scroll, waveform,
    white,
};
use crate::widgets::player_bar::{seek_bar, volume_icon};

pub fn render_player(
    model: &mut NoirPlayerModel,
    window: &mut Window,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let is_playing = model.is_playing;
    let progress = model.progress();
    let (elapsed, total) = model.times();
    let (title, artist, album) = match model.now_playing() {
        Some(track) => (track.title.clone(), track.artist.clone(), album_line(track)),
        None => (
            "No song is currently playing".to_string(),
            String::new(),
            String::new(),
        ),
    };
    let artwork = model.hero_cover(window, cx);
    let has_track = model.current.is_some();
    let meter = model.audio_meter();

    v_flex()
        .size_full()
        .min_h_0()
        .relative()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(320.0))
                .bg(linear_gradient(
                    180.0,
                    linear_color_stop(red_a(if is_light { 0.22 } else { 0.26 }), 0.0),
                    linear_color_stop(dynamic_bg(is_light).alpha(0.0), 1.0),
                )),
        )
        .child(
            v_flex()
                .relative()
                .size_full()
                .min_h_0()
                .child(header(model, is_light, cx))
                .child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .items_stretch()
                        .child(
                            div()
                                .w(px(320.0))
                                .flex_shrink_0()
                                .p(px(18.0))
                                .when(model.lyrics_open, |d| d.child(lyrics_card(model, is_light))),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .items_center()
                                .justify_center()
                                .gap(px(20.0))
                                .child(
                                    h_flex()
                                        .w_full()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(20.0))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .flex()
                                                .justify_end()
                                                .overflow_hidden()
                                                .child(waveform(
                                                    "player-wave-left",
                                                    34,
                                                    3.0,
                                                    4.0,
                                                    150.0,
                                                    is_playing,
                                                    meter.clone(),
                                                )),
                                        )
                                        .child(artwork_large(artwork, is_playing))
                                        .child(div().flex_1().min_w_0().overflow_hidden().child(
                                            waveform(
                                                "player-wave-right",
                                                34,
                                                3.0,
                                                4.0,
                                                150.0,
                                                is_playing,
                                                meter.clone(),
                                            ),
                                        )),
                                )
                                .child(
                                    v_flex()
                                        .items_center()
                                        .gap(px(6.0))
                                        .max_w(px(560.0))
                                        .child(
                                            div()
                                                .text_2xl()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(dynamic_text(is_light))
                                                .text_center()
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .child(title),
                                        )
                                        .when(!artist.is_empty(), |d| {
                                            d.child(
                                                div()
                                                    .text_base()
                                                    .text_color(dynamic_subtitle(is_light))
                                                    .child(artist),
                                            )
                                        })
                                        .when(!album.is_empty(), |d| {
                                            d.child(
                                                div()
                                                    .text_sm()
                                                    .text_color(dynamic_muted(is_light))
                                                    .child(album),
                                            )
                                        }),
                                )
                                .child(
                                    h_flex()
                                        .w_full()
                                        .max_w(px(760.0))
                                        .items_center()
                                        .gap(px(12.0))
                                        .child(
                                            div()
                                                .w(px(40.0))
                                                .text_xs()
                                                .text_color(dynamic_muted(is_light))
                                                .child(elapsed),
                                        )
                                        .child(seek_bar(progress, is_light, cx))
                                        .child(
                                            div()
                                                .w(px(40.0))
                                                .text_xs()
                                                .text_color(dynamic_muted(is_light))
                                                .child(total),
                                        ),
                                )
                                .child(transport(model, is_playing, has_track, is_light, cx))
                                .child(volume_row(model.volume, is_light, cx)),
                        )
                        .child(
                            div()
                                .w(px(320.0))
                                .flex_shrink_0()
                                .p(px(18.0))
                                .child(up_next_card(model, is_light, cx)),
                        ),
                )
                .child(action_bar(model, is_light, cx)),
        )
}

fn header(model: &NoirPlayerModel, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    let eq_enabled = model.store.equalizer_enabled;

    h_flex()
        .w_full()
        .h(px(60.0))
        .px(px(18.0))
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .child(
            h_flex()
                .id("player-back")
                .w(px(180.0))
                .items_center()
                .gap(px(6.0))
                .cursor_pointer()
                .text_color(dynamic_subtitle(is_light))
                .hover(move |s| s.text_color(dynamic_text(is_light)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.active_tab = ActiveTab::Library;
                    cx.notify();
                }))
                .child(icon_text(MusicIcon::ChevronLeft, 20.0))
                .child(div().text_sm().child("Back to library")),
        )
        .child(
            v_flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(dynamic_text(is_light))
                        .child("Now Playing"),
                )
                .child(div().w(px(72.0)).h(px(3.0)).rounded_full().bg(red())),
        )
        .child(
            h_flex()
                .w(px(180.0))
                .items_center()
                .justify_end()
                .gap(px(4.0))
                .child(
                    header_button("player-eq-btn", MusicIcon::SlidersHorizontal, is_light)
                        .when(eq_enabled, |d| d.text_color(red()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle_equalizer(cx);
                        })),
                )
                .child(
                    header_button("player-settings-btn", MusicIcon::Settings, is_light).on_click(
                        cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle_settings(cx);
                        }),
                    ),
                ),
        )
}

fn header_button(id: &'static str, icon: MusicIcon, is_light: bool) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(36.0))
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
        .child(icon_text(icon, 20.0))
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
    let interval = model.store.seek_interval_seconds;

    h_flex()
        .items_center()
        .gap(px(20.0))
        .child(
            control("player-shuffle", MusicIcon::Shuffle, 20.0, 40.0, is_light)
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
            control("player-prev", MusicIcon::SkipBack, 22.0, 40.0, is_light)
                .text_color(dynamic_text(is_light))
                .on_click(cx.listener(|this, _, _, cx| this.prev(cx))),
        )
        .child(skip_button(
            "player-skip-back",
            MusicIcon::RotateCcw,
            interval,
            is_light,
            cx.listener(|this, _, _, cx| this.skip_backward(cx)),
        ))
        .child(
            div()
                .id("player-play")
                .size(px(72.0))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(red())
                .text_color(white(1.0))
                .cursor_pointer()
                .shadow(vec![
                    BoxShadow::new(px(0.), px(6.), red_a(0.45)).blur_radius(px(22.0))
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
                    34.0,
                )),
        )
        .child(skip_button(
            "player-skip-forward",
            MusicIcon::RotateCw,
            interval,
            is_light,
            cx.listener(|this, _, _, cx| this.skip_forward(cx)),
        ))
        .child(
            control("player-next", MusicIcon::SkipForward, 22.0, 40.0, is_light)
                .text_color(dynamic_text(is_light))
                .on_click(cx.listener(|this, _, _, cx| this.next(cx))),
        )
        .child(
            control("player-repeat", MusicIcon::Repeat, 20.0, 40.0, is_light)
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

fn control(
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

fn skip_button(
    id: &'static str,
    icon: MusicIcon,
    interval: u32,
    is_light: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(42.0))
        .rounded_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(red())
        .hover(move |s| s.bg(dynamic_row_hover(is_light)))
        .on_click(handler)
        .child(icon_text(icon, 20.0))
        .child(
            div()
                .text_size(px(9.0))
                .font_weight(FontWeight::BOLD)
                .text_color(dynamic_subtitle(is_light))
                .mt(px(-3.0))
                .child(format!("{interval}s")),
        )
}

fn volume_row(volume: f32, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    let entity = cx.entity();
    let pct = (volume * 100.0).round() as u32;

    h_flex()
        .items_center()
        .gap(px(12.0))
        .child(
            control("player-mute", volume_icon(volume), 20.0, 34.0, is_light)
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
        .child(
            div()
                .id("player-volume-track")
                .w(px(260.0))
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
                        .h(px(5.0))
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
                        .top(px(3.0))
                        .left(relative(volume.clamp(0.0, 1.0)))
                        .size(px(11.0))
                        .ml(px(-5.5))
                        .rounded_full()
                        .bg(red()),
                ),
        )
        .child(
            div()
                .w(px(44.0))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(dynamic_subtitle(is_light))
                .child(format!("{pct}%")),
        )
}

fn up_next_card(model: &NoirPlayerModel, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    let upcoming: Vec<usize> = model.upcoming_queue().into_iter().skip(1).take(3).collect();
    let queue = model.queue.clone();

    v_flex()
        .w_full()
        .p(px(10.0))
        .gap(px(6.0))
        .rounded_lg()
        // A translucent panel, so the card sits on the page's red wash rather
        // than punching an opaque block through it.
        .bg(if is_light {
            dynamic_card(is_light)
        } else {
            white(0.05)
        })
        .border_1()
        .border_color(dynamic_border(is_light))
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .px(px(4.0))
                .pb(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(dynamic_text(is_light))
                        .child("Up Next"),
                )
                .child(
                    div()
                        .text_color(dynamic_muted(is_light))
                        .child(icon_text(MusicIcon::ListMusic, 16.0)),
                ),
        )
        .children((upcoming.is_empty()).then(|| {
            div()
                .px(px(4.0))
                .py(px(2.0))
                .text_xs()
                .text_color(dynamic_muted(is_light))
                .child("Nothing queued after this song.")
        }))
        .children(upcoming.into_iter().map(|index| {
            let queue = queue.clone();
            let Some(track) = model.tracks.get(index) else {
                return div().id(SharedString::from(format!("player-next-missing-{index}")));
            };
            // No red edge here: in the queue rail that marks the playing
            // song, and everything in this card is still to come.
            h_flex()
                .id(SharedString::from(format!("player-next-{index}")))
                .items_center()
                .gap(px(10.0))
                .p(px(6.0))
                .rounded_md()
                .cursor_pointer()
                .hover(move |s| s.bg(dynamic_row_hover(is_light)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.play_collection(queue.clone(), index, cx);
                }))
                .child(artwork_thumb(track, 38.0))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(dynamic_text(is_light))
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
                                .child(format!(
                                    "{} \u{2022} {}",
                                    track.artist,
                                    format_duration(track.duration)
                                )),
                        ),
                )
                .child(
                    div()
                        .text_color(dynamic_muted(is_light))
                        .child(icon_text(MusicIcon::ChevronRight, 16.0)),
                )
        }))
}

fn lyrics_card(model: &NoirPlayerModel, is_light: bool) -> Div {
    v_flex()
        .w_full()
        .h(px(420.0))
        .p(px(12.0))
        .gap(px(8.0))
        .rounded_lg()
        .bg(if is_light {
            dynamic_card(is_light)
        } else {
            white(0.05)
        })
        .border_1()
        .border_color(dynamic_border(is_light))
        .child(
            div()
                .px(px(4.0))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(dynamic_text(is_light))
                .child("Lyrics"),
        )
        .child(match model.lyrics.clone() {
            Some(lyrics) => smooth_scroll(
                "player-lyrics-scroll",
                div()
                    .text_sm()
                    .line_height(px(22.0))
                    .text_color(dynamic_subtitle(is_light))
                    .child(lyrics),
            )
            .into_any_element(),
            None => div()
                .px(px(4.0))
                .text_xs()
                .text_color(dynamic_muted(is_light))
                .child("No lyrics are saved in this file's tags.")
                .into_any_element(),
        })
}

fn action_bar(model: &NoirPlayerModel, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    let current = model.current;
    let favourite = current.is_some_and(|index| model.is_favourite(index));
    let lyrics_open = model.lyrics_open;
    let eq_enabled = model.store.equalizer_enabled;

    h_flex()
        .w_full()
        .h(px(64.0))
        .px(px(24.0))
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .border_t_1()
        .border_color(dynamic_border(is_light))
        .child(
            h_flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    action("player-liked", MusicIcon::Heart, "Liked", is_light)
                        .when(favourite, |d| d.text_color(red()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(index) = current {
                                this.toggle_favourite(index, cx);
                            }
                        })),
                )
                .child(
                    action("player-queue", MusicIcon::ListMusic, "Queue", is_light).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.queue_open = true;
                            this.active_tab = ActiveTab::Library;
                            cx.notify();
                        }),
                    ),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    action("player-lyrics", MusicIcon::FileText, "Lyrics", is_light)
                        .when(lyrics_open, |d| d.text_color(red()))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_lyrics(cx))),
                )
                .child(
                    action(
                        "player-equalizer",
                        MusicIcon::SlidersHorizontal,
                        "Equalizer",
                        is_light,
                    )
                    .when(eq_enabled, |d| d.text_color(red()))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_equalizer(cx))),
                )
                .child(
                    action("player-more", MusicIcon::Ellipsis, "More", is_light).on_click(
                        cx.listener(move |this, _, window, cx| {
                            if let Some(index) = current {
                                this.song_actions_dialog(index, None, window, cx);
                            }
                        }),
                    ),
                ),
        )
}

fn action(id: &'static str, icon: MusicIcon, label: &'static str, is_light: bool) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(px(36.0))
        .px(px(12.0))
        .items_center()
        .gap(px(8.0))
        .rounded_lg()
        .cursor_pointer()
        .text_color(dynamic_subtitle(is_light))
        .hover(move |s| s.bg(dynamic_row_hover(is_light)))
        .child(icon_text(icon, 18.0))
        .child(div().text_sm().child(label))
}

fn artwork_large(artwork: Option<std::sync::Arc<RenderImage>>, is_playing: bool) -> Div {
    let base = div()
        .size(px(276.0))
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
        Some(image) => base.child(img_from_image(image).size_full().rounded_3xl()),
        None => base
            .bg(red_a(0.12))
            .flex()
            .items_center()
            .justify_center()
            .text_color(red())
            .child(icon_text(MusicIcon::Music4, 110.0)),
    }
}
