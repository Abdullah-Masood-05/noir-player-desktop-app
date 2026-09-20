use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::update::UpdateStatus;
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
    let status = model.update_status.clone();

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
                                    .max_h(px(170.0))
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
                            )
                            .children(progress_section(&status, is_light)),
                    )
                    .child(modal_footer(&status, url, is_light, cx)),
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

/// Progress, verification and error detail below the release notes.
fn progress_section(status: &UpdateStatus, is_light: bool) -> Option<Div> {
    let note = |text: String, tone: Hsla| {
        div()
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(tone)
            .child(text)
    };
    let muted = if is_light { c(0x52525B) } else { white(0.7) };

    match status {
        UpdateStatus::Downloading { received, total } => {
            let fraction = if *total > 0 {
                (*received as f32 / *total as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let percent = (fraction * 100.0).round() as u32;
            Some(
                v_flex()
                    .w_full()
                    .gap(px(6.0))
                    .child(
                        div()
                            .w_full()
                            .h(px(6.0))
                            .rounded_full()
                            .bg(if is_light { c(0xE5E7EB) } else { c(0x24262C) })
                            .overflow_hidden()
                            .child(div().h_full().rounded_full().bg(red()).w(if *total > 0 {
                                relative(fraction)
                            } else {
                                relative(0.35)
                            })),
                    )
                    .child(note(
                        if *total > 0 {
                            format!(
                                "Downloading {} of {} ({percent}%)",
                                mib(*received),
                                mib(*total)
                            )
                        } else {
                            format!("Downloading {}", mib(*received))
                        },
                        muted,
                    )),
            )
        }
        UpdateStatus::Verifying => Some(
            v_flex()
                .w_full()
                .child(note("Checking the download against the published checksum...".to_string(), muted)),
        ),
        UpdateStatus::Ready(path) => Some(
            v_flex().w_full().gap(px(4.0)).child(note(
                if crate::update::installs_in_place() {
                    "Downloaded and verified. Noir Player will close while it installs, then reopen."
                        .to_string()
                } else {
                    "Downloaded and verified. Your package manager takes it from here.".to_string()
                },
                muted,
            ))
            .child(note(
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
                if is_light { c(0x8E8E93) } else { white(0.45) },
            )),
        ),
        UpdateStatus::Installing => Some(
            v_flex()
                .w_full()
                .child(note("Starting the installer...".to_string(), muted)),
        ),
        UpdateStatus::Error(message) => Some(
            v_flex()
                .w_full()
                .child(note(message.clone(), red())),
        ),
        _ => None,
    }
}

fn mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / 1_048_576.0)
}

fn secondary_button(id: &'static str, label: &'static str, is_light: bool) -> Stateful<Div> {
    div()
        .id(id)
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
        .child(label)
}

fn primary_button(
    id: &'static str,
    label: &'static str,
    icon: MusicIcon,
    is_light: bool,
) -> Stateful<Div> {
    let _ = is_light;
    h_flex()
        .id(id)
        .px(px(16.0))
        .py(px(7.0))
        .rounded_lg()
        .bg(red())
        .items_center()
        .gap(px(6.0))
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(white(1.0))
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .child(icon_text(icon, 14.0))
        .child(label)
}

fn modal_footer(
    status: &UpdateStatus,
    url: String,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let footer = h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap(px(10.0))
        .px(px(20.0))
        .py(px(14.0))
        .border_t(px(1.0))
        .border_color(if is_light { c(0xE4E4E7) } else { c(0x222428) })
        .child(
            div()
                .id("update-release-page")
                .text_xs()
                .text_color(if is_light { c(0x71717A) } else { white(0.45) })
                .cursor_pointer()
                .hover(|s| s.text_color(red()))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    cx.open_url(&url);
                })
                .child("Release page"),
        );

    let actions = h_flex().items_center().gap(px(10.0));

    let actions = match status {
        UpdateStatus::Downloading { .. } | UpdateStatus::Verifying => actions.child(
            secondary_button("update-cancel-btn", "Cancel", is_light).on_click(cx.listener(
                |this, _, _, cx| {
                    cx.stop_propagation();
                    this.cancel_update_download(cx);
                },
            )),
        ),
        UpdateStatus::Ready(_) => actions
            .child(
                secondary_button("update-later-btn", "Later", is_light).on_click(cx.listener(
                    |this, _, _, cx| {
                        cx.stop_propagation();
                        this.update_dialog_open = false;
                        cx.notify();
                    },
                )),
            )
            .child(
                primary_button(
                    "update-install-btn",
                    if crate::update::installs_in_place() {
                        "Restart and install"
                    } else {
                        "Open installer"
                    },
                    MusicIcon::RefreshCw,
                    is_light,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.install_update(cx);
                })),
            ),
        UpdateStatus::Installing => actions.child(
            div()
                .px(px(16.0))
                .py(px(7.0))
                .rounded_lg()
                .bg(if is_light { c(0xF0F1F3) } else { c(0x1C1F26) })
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(if is_light { c(0x71717A) } else { white(0.5) })
                .child("Installing..."),
        ),
        UpdateStatus::Error(_) => actions
            .child(
                secondary_button("update-dismiss-btn", "Close", is_light).on_click(cx.listener(
                    |this, _, _, cx| {
                        cx.stop_propagation();
                        this.update_dialog_open = false;
                        cx.notify();
                    },
                )),
            )
            .child(
                primary_button(
                    "update-retry-btn",
                    "Try again",
                    MusicIcon::RefreshCw,
                    is_light,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.start_update_download(cx);
                })),
            ),
        _ => actions
            .child(
                secondary_button("update-later-btn", "Later", is_light).on_click(cx.listener(
                    |this, _, _, cx| {
                        cx.stop_propagation();
                        this.update_dialog_open = false;
                        cx.notify();
                    },
                )),
            )
            .child(
                primary_button(
                    "update-download-btn",
                    "Download update",
                    MusicIcon::Download,
                    is_light,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.start_update_download(cx);
                })),
            ),
    };

    footer.child(actions)
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
