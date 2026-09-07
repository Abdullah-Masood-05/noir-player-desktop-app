use crate::app::{ActiveTab, NoirPlayerModel};
use crate::views::ui::{icon_text, nav_bg, red, selected_highlight, white};
use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub fn bottom_nav(active: ActiveTab, cx: &mut Context<NoirPlayerModel>) -> Div {
    let items = vec![
        (ActiveTab::Library, "Library", MusicIcon::LibraryBig),
        (ActiveTab::Player, "Player", MusicIcon::Music4),
        (ActiveTab::Playlists, "Playlists", MusicIcon::ListMusic),
        (ActiveTab::Discover, "Discover", MusicIcon::Compass),
    ];

    h_flex()
        .w_full()
        .h(px(66.0))
        .flex_shrink_0()
        .items_center()
        .justify_around()
        .bg(nav_bg())
        .border_t_1()
        .border_color(cx.theme().border)
        .children(items.into_iter().map(move |(tab, label, icon)| {
            let sel = active == tab;
            let handler_tab = tab;
            v_flex()
                .items_center()
                .justify_center()
                .gap(px(2.0))
                .flex_1()
                .h_full()
                .id(format!("nav-{:?}", tab))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_tab = handler_tab;
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
                            white(0.5)
                        })),
                ))
                .child(
                    div()
                        .text_xs()
                        .when(sel, |d| {
                            d.text_color(red()).font_weight(FontWeight::SEMIBOLD)
                        })
                        .when(!sel, |d| d.text_color(white(0.5)))
                        .child(label.to_string()),
                )
        }))
}
