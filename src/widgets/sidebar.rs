use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::{NavDestination, NoirPlayerModel};
use crate::views::ui::{
    bottom_aura, dynamic_border, dynamic_divider, dynamic_row_hover, dynamic_sidebar,
    dynamic_subtitle, dynamic_text, icon_text, red, red_a,
};

pub const SIDEBAR_WIDTH: f32 = 236.0;

const PRIMARY: [(NavDestination, MusicIcon); 7] = [
    (NavDestination::Library, MusicIcon::LibraryBig),
    (NavDestination::Favourites, MusicIcon::Heart),
    (NavDestination::Albums, MusicIcon::Disc3),
    (NavDestination::Artists, MusicIcon::Mic),
    (NavDestination::Folders, MusicIcon::Folder),
    (NavDestination::Playlists, MusicIcon::ListMusic),
    (NavDestination::Discover, MusicIcon::Compass),
];

/// The left navigation rail: brand mark, destinations, and settings.
pub fn sidebar(model: &NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let settings_open = model.settings_open;

    v_flex()
        .w(px(SIDEBAR_WIDTH))
        .flex_shrink_0()
        .h_full()
        .relative()
        .overflow_hidden()
        .bg(dynamic_sidebar(is_light))
        .border_r_1()
        .border_color(dynamic_border(is_light))
        .child(bottom_aura(is_light))
        .child(
            v_flex()
                .relative()
                .size_full()
                .child(brand(is_light))
                .child(v_flex().px(px(12.0)).gap(px(2.0)).children(
                    PRIMARY.map(|(destination, icon)| {
                        nav_item(model, destination, icon, is_light, cx)
                    }),
                ))
                .child(
                    div()
                        .mx(px(20.0))
                        .my(px(14.0))
                        .h(px(1.0))
                        .bg(dynamic_divider(is_light)),
                )
                .child(v_flex().px(px(12.0)).child(nav_item(
                    model,
                    NavDestination::RecentlyPlayed,
                    MusicIcon::Clock,
                    is_light,
                    cx,
                )))
                .child(div().flex_1())
                .child(
                    v_flex().px(px(12.0)).pb(px(16.0)).child(
                        h_flex()
                            .id("sidebar-settings")
                            .h(px(44.0))
                            .px(px(12.0))
                            .gap(px(12.0))
                            .items_center()
                            .rounded_lg()
                            .cursor_pointer()
                            .when(settings_open, |d| d.bg(red_a(0.16)))
                            .when(!settings_open, |d| {
                                d.hover(move |s| s.bg(dynamic_row_hover(is_light)))
                            })
                            .text_color(if settings_open {
                                red()
                            } else {
                                dynamic_subtitle(is_light)
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_settings(cx)))
                            .child(icon_text(MusicIcon::Settings, 20.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(if settings_open {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .child("Settings"),
                            ),
                    ),
                ),
        )
}

fn brand(is_light: bool) -> Div {
    h_flex()
        .px(px(20.0))
        .py(px(20.0))
        .gap(px(10.0))
        .items_center()
        .child(
            div()
                .text_color(red())
                .child(icon_text(MusicIcon::AudioLines, 26.0)),
        )
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(dynamic_text(is_light))
                .child("Noir Player"),
        )
}

fn nav_item(
    model: &NoirPlayerModel,
    destination: NavDestination,
    icon: MusicIcon,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let selected = model.is_current_destination(destination);
    div()
        .relative()
        .child(
            h_flex()
                .id(SharedString::from(format!("nav-{destination:?}")))
                .h(px(44.0))
                .px(px(12.0))
                .gap(px(12.0))
                .items_center()
                .rounded_lg()
                .cursor_pointer()
                .when(selected, |d| d.bg(red_a(0.16)))
                .when(!selected, |d| {
                    d.hover(move |s| s.bg(dynamic_row_hover(is_light)))
                })
                .text_color(if selected {
                    red()
                } else {
                    dynamic_subtitle(is_light)
                })
                .on_click(cx.listener(move |this, _, _, cx| this.navigate(destination, cx)))
                .child(icon_text(icon, 20.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(if selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .child(destination.label()),
                ),
        )
        .when(selected, |d| {
            d.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(10.0))
                    .bottom(px(10.0))
                    .w(px(3.0))
                    .rounded_full()
                    .bg(red()),
            )
        })
}
