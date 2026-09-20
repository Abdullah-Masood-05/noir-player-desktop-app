use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::{NoirPlayerModel, SongMenu};
use crate::views::library::artwork_thumb;
use crate::views::ui::{
    backdrop_transition, dynamic_border, dynamic_panel, dynamic_row_hover, dynamic_subtitle,
    dynamic_text, format_duration, icon_text, modal_transition, red, red_a,
};

/// The per song action sheet. Built here rather than with the component
/// dialog so it carries the app's own surfaces and its own close button.
pub fn render_song_menu(model: &NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> AnyElement {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let Some(menu) = model.song_menu.clone() else {
        return div().into_any_element();
    };
    let SongMenu { index, playlist } = menu;
    let Some(track) = model.tracks.get(index) else {
        return div().into_any_element();
    };

    let title = track.title.clone();
    let subtitle = format!(
        "{} \u{2022} {} \u{2022} {}",
        track.artist,
        track.album,
        format_duration(track.duration)
    );
    let favourite = model.is_favourite(index);
    let remove_path = track.path.clone();
    let artwork = artwork_thumb(track, 44.0);

    backdrop_transition(
        "song-menu-backdrop-anim",
        div()
            .id("song-menu-backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(if is_light { 0x00000055 } else { 0x000000B8 }))
            .flex()
            .items_center()
            .justify_center()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, _, cx| this.close_song_menu(cx)))
            .child(modal_transition(
                "song-menu-anim",
                v_flex()
                    .id("song-menu")
                    .w(px(360.0))
                    .p(px(8.0))
                    .rounded_xl()
                    .bg(dynamic_panel(is_light))
                    .border_1()
                    .border_color(dynamic_border(is_light))
                    .shadow(vec![BoxShadow::new(
                        px(0.0),
                        px(20.0),
                        hsla(0.0, 0.0, 0.0, 0.55),
                    )
                    .blur_radius(px(44.0))])
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap(px(10.0))
                            .px(px(6.0))
                            .py(px(6.0))
                            .child(artwork)
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
                                            .child(subtitle),
                                    ),
                            )
                            .child(
                                div()
                                    .id("song-menu-close")
                                    .size(px(28.0))
                                    .rounded_md()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_color(dynamic_subtitle(is_light))
                                    .hover(move |s| s.bg(red_a(0.16)).text_color(red()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.close_song_menu(cx);
                                    }))
                                    .child(icon_text(MusicIcon::X, 15.0)),
                            ),
                    )
                    .child(
                        div()
                            .mx(px(6.0))
                            .my(px(4.0))
                            .h(px(1.0))
                            .bg(dynamic_border(is_light)),
                    )
                    .child(action(
                        "song-menu-play",
                        MusicIcon::Play,
                        "Play now",
                        false,
                        is_light,
                        cx.listener(move |this, _, _, cx| {
                            this.play_index(index, cx);
                            this.close_song_menu(cx);
                        }),
                    ))
                    .child(action(
                        "song-menu-next",
                        MusicIcon::ListStart,
                        "Play next",
                        false,
                        is_light,
                        cx.listener(move |this, _, _, cx| {
                            this.play_next(index, cx);
                            this.close_song_menu(cx);
                        }),
                    ))
                    .child(action(
                        "song-menu-queue",
                        MusicIcon::ListPlus,
                        "Add to queue",
                        false,
                        is_light,
                        cx.listener(move |this, _, _, cx| {
                            this.add_to_queue(index, cx);
                            this.close_song_menu(cx);
                        }),
                    ))
                    .child(action(
                        "song-menu-favourite",
                        MusicIcon::Heart,
                        if favourite {
                            "Remove from favourites"
                        } else {
                            "Add to favourites"
                        },
                        false,
                        is_light,
                        cx.listener(move |this, _, _, cx| {
                            this.toggle_favourite(index, cx);
                            this.close_song_menu(cx);
                        }),
                    ))
                    .child(action(
                        "song-menu-playlist",
                        MusicIcon::ListMusic,
                        "Add to playlist",
                        false,
                        is_light,
                        cx.listener(move |this, _, window, cx| {
                            this.close_song_menu(cx);
                            this.add_to_playlist_dialog(index, window, cx);
                        }),
                    ))
                    .when_some(playlist, |sheet, name| {
                        sheet.child(action(
                            "song-menu-remove",
                            MusicIcon::Trash,
                            "Remove from this playlist",
                            true,
                            is_light,
                            cx.listener(move |this, _, _, cx| {
                                this.remove_from_playlist(&name, &remove_path, cx);
                                this.close_song_menu(cx);
                            }),
                        ))
                    }),
            )),
    )
}

fn action(
    id: &'static str,
    icon: MusicIcon,
    label: &'static str,
    danger: bool,
    is_light: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .w_full()
        .h(px(40.0))
        .px(px(10.0))
        .gap(px(10.0))
        .items_center()
        .rounded_lg()
        .cursor_pointer()
        .text_color(if danger {
            red()
        } else {
            dynamic_text(is_light)
        })
        .hover(move |s| {
            s.bg(if danger {
                red_a(0.12)
            } else {
                dynamic_row_hover(is_light)
            })
        })
        .on_click(handler)
        .child(icon_text(icon, 16.0))
        .child(div().text_sm().child(label))
}
