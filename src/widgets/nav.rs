use crate::app::{ActiveTab, NoirPlayerModel};
use crate::views::ui::{icon_text, nav_bg, red, selected_highlight, white, LIGHT_MUTED, LIGHT_NAV_BG};
use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub fn bottom_nav(active: ActiveTab, settings_open: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let items = vec![
        (ActiveTab::Library, "Library", MusicIcon::LibraryBig),
        (ActiveTab::Player, "Player", MusicIcon::Music4),
        (ActiveTab::Playlists, "Playlists", MusicIcon::ListMusic),
        (ActiveTab::Discover, "Discover", MusicIcon::Compass),
    ];

    let unselected_color = if is_light {
        rgb(LIGHT_MUTED).into()
    } else {
        white(0.5)
    };

    let mut nav = h_flex()
        .w_full()
        .h(px(66.0))
        .flex_shrink_0()
        .items_center()
        .justify_around()
        .bg(if is_light {
            rgb(LIGHT_NAV_BG).into()
        } else {
            nav_bg()
        })
        .border_t_1()
        .border_color(cx.theme().border);

    for (tab, label, icon) in items {
        let sel = active == tab && !settings_open;
        let handler_tab = tab;
        nav = nav.child(
            v_flex()
                .items_center()
                .justify_center()
                .gap(px(2.0))
                .flex_1()
                .h_full()
                .id(format!("nav-{:?}", tab))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_tab = handler_tab;
                    if this.settings_open {
                        this.settings_open = false;
                    }
                    if handler_tab == ActiveTab::Discover && this.discover_request == 0 {
                        this.load_discover(String::new(), cx);
                    }
                    cx.notify();
                }))
                .cursor_pointer()
                .child(selected_highlight(
                    format!("nav-highlight-{tab:?}"),
                    sel,
                    div()
                        .w(px(64.0))
                        .h(px(32.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_text(icon, 28.0).text_color(if sel {
                            white(1.0)
                        } else {
                            unselected_color
                        })),
                ))
                .child(
                    div()
                        .text_xs()
                        .when(sel, |d| {
                            d.text_color(red()).font_weight(FontWeight::SEMIBOLD)
                        })
                        .when(!sel, |d| d.text_color(unselected_color))
                        .child(label.to_string()),
                ),
        );
    }

    nav.child(
        v_flex()
            .items_center()
            .justify_center()
            .gap(px(2.0))
            .flex_1()
            .h_full()
            .id("nav-settings")
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_settings(cx);
            }))
            .cursor_pointer()
            .child(selected_highlight(
                "nav-highlight-settings",
                settings_open,
                div()
                    .w(px(64.0))
                    .h(px(32.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_text(MusicIcon::Settings, 28.0).text_color(if settings_open {
                        white(1.0)
                    } else {
                        unselected_color
                    })),
            ))
            .child(
                div()
                    .text_xs()
                    .when(settings_open, |d| {
                        d.text_color(red()).font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!settings_open, |d| d.text_color(unselected_color))
                    .child("Settings"),
            ),
    )
}
