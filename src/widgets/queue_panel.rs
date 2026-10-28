use std::rc::Rc;

use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::base::v_virtual_list;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::views::library::{artwork_thumb, keyed_virtual_scroll, with_virtual_scrollbar};
use crate::views::ui::{
    bottom_aura, dynamic_border, dynamic_muted, dynamic_panel, dynamic_row_hover, dynamic_subtitle,
    dynamic_text, format_duration, icon_text, red, red_a, waveform,
};

pub const QUEUE_WIDTH: f32 = 302.0;
/// Every queue row is pinned to this height so the virtual list's fixed
/// sizes match what is drawn: 8px padding around the three-line text column.
const QUEUE_ROW_HEIGHT: f32 = 82.0;
const QUEUE_LIST_PADDING: f32 = 10.0;

/// The docked "Up Next" rail beside the library.
pub fn queue_panel(
    model: &NoirPlayerModel,
    window: &mut Window,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let upcoming = model.upcoming_queue();
    let shuffle = model.store.shuffle;
    let has_upcoming = !upcoming.is_empty();

    v_flex()
        .w(px(QUEUE_WIDTH))
        .flex_shrink_0()
        .h_full()
        .min_h_0()
        .relative()
        .overflow_hidden()
        .bg(dynamic_panel(is_light))
        .border_l_1()
        .border_color(dynamic_border(is_light))
        .child(bottom_aura(is_light))
        .child(
            h_flex()
                .flex_shrink_0()
                .h(px(64.0))
                .px(px(16.0))
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(dynamic_text(is_light))
                        .child("Up Next"),
                )
                .child(
                    h_flex()
                        .gap(px(4.0))
                        .child(
                            icon_button(
                                "queue-shuffle",
                                MusicIcon::Shuffle,
                                is_light,
                                cx.listener(|this, _, _, cx| this.toggle_shuffle(cx)),
                            )
                            .when(shuffle, |d| d.text_color(red())),
                        )
                        .child(icon_button(
                            "queue-hide",
                            MusicIcon::PanelRightClose,
                            is_light,
                            cx.listener(|this, _, _, cx| this.toggle_queue_panel(cx)),
                        )),
                ),
        )
        .child(if !has_upcoming {
            v_flex()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .px(px(24.0))
                .child(
                    div()
                        .text_color(red_a(0.35))
                        .child(icon_text(MusicIcon::ListMusic, 42.0)),
                )
                .child(
                    div()
                        .text_sm()
                        .text_center()
                        .text_color(dynamic_subtitle(is_light))
                        .child("The queue is empty"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_center()
                        .text_color(dynamic_muted(is_light))
                        .child("Play a song, or add songs from the list."),
                )
                .into_any_element()
        } else {
            queue_list(upcoming, window, is_light, cx)
        })
        .when(has_upcoming, |panel| {
            panel.child(
                div().flex_shrink_0().p(px(12.0)).child(
                    h_flex()
                        .id("queue-clear")
                        .h(px(40.0))
                        .w_full()
                        .items_center()
                        .justify_center()
                        .gap(px(8.0))
                        .rounded_lg()
                        .cursor_pointer()
                        .border_1()
                        .border_color(dynamic_border(is_light))
                        .text_color(dynamic_subtitle(is_light))
                        .hover(move |s| s.bg(red_a(0.12)).text_color(red()))
                        .on_click(cx.listener(|this, _, _, cx| this.clear_queue(cx)))
                        .child(icon_text(MusicIcon::Trash, 16.0))
                        .child(div().text_sm().child("Clear Queue")),
                ),
            )
        })
}

/// The collapsed queue: a slim rail that opens the "Up Next" panel again.
pub fn queue_handle(model: &NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let queued = model.upcoming_queue().len().saturating_sub(1);

    v_flex()
        .w(px(46.0))
        .flex_shrink_0()
        .h_full()
        .items_center()
        .gap(px(8.0))
        .pt(px(17.0))
        .relative()
        .overflow_hidden()
        .bg(dynamic_panel(is_light))
        .border_l_1()
        .border_color(dynamic_border(is_light))
        .child(bottom_aura(is_light))
        .child(icon_button(
            "queue-show",
            MusicIcon::PanelRightOpen,
            is_light,
            cx.listener(|this, _, _, cx| this.toggle_queue_panel(cx)),
        ))
        .child(
            v_flex()
                .id("queue-show-label")
                .items_center()
                .gap(px(4.0))
                .cursor_pointer()
                .text_color(dynamic_subtitle(is_light))
                .hover(move |s| s.text_color(red()))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_queue_panel(cx)))
                .child(icon_text(MusicIcon::ListMusic, 18.0))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{queued}")),
                ),
        )
}

/// The upcoming songs as a virtual list: only the visible rows are built,
/// however long the queue is. The scroll handle is keyed, so the list keeps
/// its place across the per-tick re-renders while a song plays.
fn queue_list(
    upcoming: Vec<usize>,
    window: &mut Window,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let list_id = ElementId::from("queue-scroll");
    let handle = keyed_virtual_scroll(&list_id, window, cx);
    let sizes = Rc::new(vec![
        size(
            px(QUEUE_WIDTH - 2.0 * QUEUE_LIST_PADDING),
            px(QUEUE_ROW_HEIGHT)
        );
        upcoming.len()
    ]);
    let list = v_virtual_list(
        cx.entity(),
        list_id.clone(),
        sizes,
        move |this, range, _, cx| {
            range
                .map(|position| queue_row(this, upcoming[position], is_light, cx))
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(&handle)
    .px(px(QUEUE_LIST_PADDING))
    .pb(px(QUEUE_LIST_PADDING));

    div()
        .w_full()
        .flex_1()
        .min_h_0()
        .child(with_virtual_scrollbar(list, &handle, list_id))
        .into_any_element()
}

fn queue_row(
    model: &NoirPlayerModel,
    index: usize,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> AnyElement {
    let Some(track) = model.tracks.get(index) else {
        return div().into_any_element();
    };
    let playing = model.current == Some(index);
    let queue = model.queue.clone();
    let meter = model.audio_meter();
    let playing_now = model.is_playing;

    h_flex()
        // The queue holds each track at most once, so the track index alone
        // is a stable, unique id as songs advance and rows shift.
        .id(("queue-row", index))
        .w_full()
        .h(px(QUEUE_ROW_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .gap(px(10.0))
        .p(px(8.0))
        .rounded_lg()
        .cursor_pointer()
        .when(playing, |d| d.bg(red_a(0.12)))
        .when(!playing, |d| {
            d.hover(move |s| s.bg(dynamic_row_hover(is_light)))
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.play_collection(queue.clone(), index, cx);
        }))
        .child(div().w(px(3.0)).h(px(40.0)).rounded_full().bg(if playing {
            red()
        } else {
            red_a(0.0)
        }))
        .child(artwork_thumb(track, 44.0))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
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
                        .text_xs()
                        .text_color(dynamic_subtitle(is_light))
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(track.artist.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(dynamic_muted(is_light))
                        .child(format_duration(track.duration)),
                ),
        )
        .child(if playing {
            div()
                .flex_shrink_0()
                .w(px(20.0))
                .child(waveform(
                    &format!("queue-wave-{index}"),
                    4,
                    2.5,
                    2.0,
                    16.0,
                    playing_now,
                    meter.clone(),
                ))
                .into_any_element()
        } else {
            div()
                .id(SharedString::from(format!("queue-remove-{index}")))
                .flex_shrink_0()
                .p(px(4.0))
                .rounded_md()
                .cursor_pointer()
                .text_color(dynamic_muted(is_light))
                .hover(move |s| s.text_color(red()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.remove_from_queue(index, cx);
                }))
                .child(icon_text(MusicIcon::X, 16.0))
                .into_any_element()
        })
        .into_any_element()
}

fn icon_button(
    id: &'static str,
    icon: MusicIcon,
    is_light: bool,
    handler: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(30.0))
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(dynamic_subtitle(is_light))
        .hover(move |s| {
            s.bg(dynamic_row_hover(is_light))
                .text_color(dynamic_text(is_light))
        })
        .on_click(handler)
        .child(icon_text(icon, 18.0))
}
