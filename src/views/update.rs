use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::views::ui::{
    backdrop_transition, icon_text, modal_transition, red, smooth_scroll, white,
};

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub fn render_update_modal(
    model: &mut NoirPlayerModel,
    cx: &mut Context<NoirPlayerModel>,
) -> impl IntoElement {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let release = model.latest_release.clone();

    let (version, name, notes, url) = match release {
        Some(ref r) => (
            r.version.clone(),
            r.name.clone(),
            r.body.clone(),
            r.html_url.clone(),
        ),
        None => (
            "Unknown".to_string(),
            "New Update Available".to_string(),
            "A new version of Noir Player is available for download.".to_string(),
            "https://github.com/Abdullah-Masood-05/noir-player-desktop-app/releases".to_string(),
        ),
    };

    let current_version = env!("CARGO_PKG_VERSION");

    backdrop_transition(
        "update-backdrop-anim",
        div()
            .id("update-backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(if is_light { 0x00000066 } else { 0x000000CC }))
            .flex()
            .items_center()
            .justify_center()
            .on_scroll_wheel(|_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.update_dialog_open = false;
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let k = event.keystroke.key.trim();
                if k.eq_ignore_ascii_case("escape") || k.eq_ignore_ascii_case("esc") {
                    cx.stop_propagation();
                    this.update_dialog_open = false;
                    cx.notify();
                }
            }))
            .child(modal_transition(
                "update-modal-anim",
                v_flex()
                    .id("update-modal")
                    .w(px(540.0))
                    .max_h(px(520.0))
                    .rounded_2xl()
                    .bg(if is_light { c(0xFFFFFF) } else { c(0x111215) })
                    .border(px(1.0))
                    .border_color(if is_light { c(0xE4E4E7) } else { c(0x2A2D35) })
                    .shadow(vec![BoxShadow::new(
                        px(0.0),
                        px(24.0),
                        if is_light { c(0x00000033) } else { c(0x000000) },
                    )
                    .blur_radius(px(48.0))])
                    .overflow_hidden()
                    .on_scroll_wheel(|_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.stop_propagation();
                    }))
                    .child(modal_header(is_light, cx))
                    .child(
                        v_flex()
                            .w_full()
                            .px(px(20.0))
                            .py(px(16.0))
                            .gap(px(12.0))
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .px(px(8.0))
                                            .py(px(3.0))
                                            .rounded_md()
                                            .bg(if is_light { c(0xF0F1F3) } else { c(0x1C1F26) })
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(if is_light {
                                                c(0x71717A)
                                            } else {
                                                white(0.5)
                                            })
                                            .child(format!("Current: v{current_version}")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(if is_light {
                                                c(0x71717A)
                                            } else {
                                                white(0.4)
                                            })
                                            .child("→"),
                                    )
                                    .child(
                                        div()
                                            .px(px(8.0))
                                            .py(px(3.0))
                                            .rounded_md()
                                            .bg(red().alpha(0.18))
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(red())
                                            .child(format!("Latest: v{version}")),
                                    ),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_light { c(0x18181B) } else { white(0.95) })
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if is_light { c(0x71717A) } else { white(0.5) })
                                    .child("Release Highlights:"),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .max_h(px(220.0))
                                    .rounded_xl()
                                    .bg(if is_light { c(0xF8F9FA) } else { c(0x16181D) })
                                    .border(px(1.0))
                                    .border_color(if is_light { c(0xE5E7EB) } else { c(0x222428) })
                                    .p(px(12.0))
                                    .overflow_hidden()
                                    .child(smooth_scroll(
                                        "update-notes-scroll",
                                        div()
                                            .w_full()
                                            .text_xs()
                                            .line_height(relative(1.5))
                                            .text_color(if is_light {
                                                c(0x3F3F46)
                                            } else {
                                                white(0.8)
                                            })
                                            .children(render_notes_content(&notes, is_light)),
                                    )),
                            ),
                    )
                    .child(modal_footer(url, is_light, cx)),
            )),
    )
}

fn modal_header(is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .px(px(20.0))
        .py(px(14.0))
        .border_b(px(1.0))
        .border_color(if is_light { c(0xE4E4E7) } else { c(0x222428) })
        .child(
            h_flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .size(px(32.0))
                        .rounded_lg()
                        .bg(red().alpha(0.18))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(red())
                        .child(icon_text(MusicIcon::Sparkles, 18.0)),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_light { c(0x18181B) } else { white(0.95) })
                        .child("Software Update Available"),
                ),
        )
        .child(
            div()
                .id("update-close-btn")
                .size(px(28.0))
                .rounded_lg()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(if is_light { c(0x71717A) } else { white(0.5) })
                .hover(|s| s.bg(c(0xFF000022)).text_color(red()))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.update_dialog_open = false;
                    cx.notify();
                }))
                .child(icon_text(MusicIcon::X, 15.0)),
        )
}

fn modal_footer(url: String, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    h_flex()
        .w_full()
        .items_center()
        .justify_end()
        .gap(px(10.0))
        .px(px(20.0))
        .py(px(14.0))
        .border_t(px(1.0))
        .border_color(if is_light { c(0xE4E4E7) } else { c(0x222428) })
        .child(
            div()
                .id("update-dismiss-btn")
                .px(px(14.0))
                .py(px(7.0))
                .rounded_lg()
                .bg(if is_light { c(0xF0F1F3) } else { c(0x1C1F26) })
                .border(px(1.0))
                .border_color(if is_light { c(0xDCDEE2) } else { c(0x2A2D35) })
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(if is_light { c(0x52525B) } else { white(0.7) })
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.update_dialog_open = false;
                    cx.notify();
                }))
                .child("Later"),
        )
        .child(
            div()
                .id("update-download-btn")
                .px(px(16.0))
                .py(px(7.0))
                .rounded_lg()
                .bg(red())
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(white(1.0))
                .cursor_pointer()
                .hover(|s| s.opacity(0.9))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.update_dialog_open = false;
                    cx.open_url(&url);
                    cx.notify();
                }))
                .child(icon_text(MusicIcon::Download, 14.0))
                .child("Download & Install"),
        )
}

fn render_notes_content(notes: &str, is_light: bool) -> Vec<Div> {
    if notes.trim().is_empty() {
        return vec![div()
            .text_color(if is_light { c(0x71717A) } else { white(0.5) })
            .child("View release details on GitHub.")];
    }

    notes
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let line = line.trim();
            if line.starts_with("### ") || line.starts_with("## ") {
                let text = line.trim_start_matches('#').trim().to_string();
                div()
                    .pt(px(4.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_light { c(0x18181B) } else { white(0.95) })
                    .child(text)
            } else if line.starts_with("- ") || line.starts_with("* ") {
                let text = line[2..].trim().to_string();
                h_flex()
                    .gap(px(6.0))
                    .items_start()
                    .child(div().text_color(red()).child("\u{2022}"))
                    .child(div().flex_1().child(text))
            } else {
                div().child(line.to_string())
            }
        })
        .collect()
}
