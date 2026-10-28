use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::update::UpdateStatus;
use crate::views::ui::{
    backdrop_transition, dynamic_inner_surface, dynamic_modal_surface, icon_text, modal_transition,
    red, smooth_scroll, white,
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
    let route = install_route(model.store.update_install_type);

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
            .occlude()
            .bg(rgba(if is_light { 0x00000066 } else { 0x000000CC }))
            .flex()
            .items_center()
            .justify_center()
            .on_scroll_wheel(|_, _, cx| {
                cx.stop_propagation();
            })
            // No click handler: a click outside the dialog does nothing. It
            // closes only with its own buttons, so a stray click can never
            // hide a download that is running.
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let k = event.keystroke.key.trim();
                if k.eq_ignore_ascii_case("escape") || k.eq_ignore_ascii_case("esc") {
                    cx.stop_propagation();
                    // A running download is stopped with Cancel, not
                    // dismissed by Escape.
                    if !this.update_status.is_busy() {
                        this.update_dialog_open = false;
                        cx.notify();
                    }
                }
            }))
            .child(modal_transition(
                "update-modal-anim",
                v_flex()
                    .id("update-modal")
                    .track_focus(&model.update_focus_handle)
                    .w(px(540.0))
                    .max_h(px(520.0))
                    .rounded_2xl()
                    .bg(dynamic_modal_surface(is_light))
                    .border(px(1.0))
                    .border_color(if is_light { c(0xE4E4E7) } else { c(0x2A2A2E) })
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
                                            .bg(dynamic_inner_surface(is_light))
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
                                v_flex()
                                    .w_full()
                                    .h(px(160.0))
                                    .rounded_xl()
                                    .bg(dynamic_inner_surface(is_light))
                                    .border(px(1.0))
                                    .border_color(if is_light { c(0xE4E4E7) } else { c(0x1E1E22) })
                                    .p(px(12.0))
                                    .overflow_hidden()
                                    .child(smooth_scroll(
                                        "update-notes-scroll",
                                        v_flex()
                                            .w_full()
                                            .gap(px(4.0))
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
                            .children(route_note(route, &status, is_light))
                            .children(progress_section(&status, route, is_light)),
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
        .border_color(if is_light { c(0xE4E4E7) } else { c(0x1E1E22) })
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

/// Where a Windows update installs, as far as the wording is concerned.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InstallRoute {
    /// Not Windows, or a per user copy updating itself: nothing to explain.
    InPlace,
    /// An all-users copy updating just for this account, with no prompt.
    JustForYou,
    /// An all-users copy whose owner opted into updating it for everyone.
    AllUsers,
}

fn install_route(preference: Option<crate::app::store::InstallPreference>) -> InstallRoute {
    #[cfg(target_os = "windows")]
    {
        use crate::update::{install_scope, InstallScope, UpdatePlan};
        match (
            install_scope(),
            crate::update::update_plan(install_scope(), preference),
        ) {
            (_, UpdatePlan::MachineMsi) => InstallRoute::AllUsers,
            (InstallScope::PerMachine, UpdatePlan::PerUserSetup) => InstallRoute::JustForYou,
            (InstallScope::PerUser, UpdatePlan::PerUserSetup) => InstallRoute::InPlace,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = preference;
        InstallRoute::InPlace
    }
}

/// One line saying how an all-users copy will update, before anything is
/// downloaded, so the administrator prompt never comes as a surprise.
fn route_note(route: InstallRoute, status: &UpdateStatus, is_light: bool) -> Option<Div> {
    if !matches!(status, UpdateStatus::Available(_) | UpdateStatus::Idle) {
        return None;
    }
    let text = match route {
        InstallRoute::InPlace => return None,
        InstallRoute::JustForYou => {
            "Installs just for you, with no administrator prompt. To update the copy for all \
             users instead, turn on Update for all users in Settings."
        }
        InstallRoute::AllUsers => {
            "Update for all users is on, so Windows will ask for administrator approval. Turn \
             it off in Settings to update just for you, with no prompt."
        }
    };
    Some(
        div()
            .text_xs()
            .line_height(relative(1.45))
            .text_color(if is_light { c(0x52525B) } else { white(0.6) })
            .child(text),
    )
}

/// Progress, verification and error detail below the release notes.
fn progress_section(status: &UpdateStatus, route: InstallRoute, is_light: bool) -> Option<Div> {
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
                            .bg(if is_light { c(0xE4E4E7) } else { c(0x1E1E22) })
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
                if cfg!(target_os = "linux") && crate::update::installs_in_place() {
                    "Downloaded and verified. Installing asks for your password, then Noir Player reopens."
                        .to_string()
                } else if route == InstallRoute::JustForYou {
                    "Downloaded and verified. Noir Player will close, install the update just for you with no administrator prompt, then reopen."
                        .to_string()
                } else if route == InstallRoute::AllUsers {
                    "Downloaded and verified. Noir Player will close and Windows will ask for administrator approval to update the copy for all users, then Noir Player reopens."
                        .to_string()
                } else if crate::update::installs_in_place() {
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
        UpdateStatus::Notice(message) => Some(
            v_flex()
                .w_full()
                .child(note(message.clone(), muted)),
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
        .bg(dynamic_inner_surface(is_light))
        .border(px(1.0))
        .border_color(if is_light { c(0xDEDEE1) } else { c(0x2A2A2E) })
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
        .border_color(if is_light { c(0xE4E4E7) } else { c(0x1E1E22) })
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
                    if cfg!(target_os = "linux") && crate::update::installs_in_place() {
                        "Install and restart"
                    } else if crate::update::installs_in_place() {
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
                .bg(dynamic_inner_surface(is_light))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(if is_light { c(0x71717A) } else { white(0.5) })
                .child("Installing..."),
        ),
        UpdateStatus::Notice(_) => actions.child(
            secondary_button("update-dismiss-btn", "Close", is_light).on_click(cx.listener(
                |this, _, _, cx| {
                    cx.stop_propagation();
                    this.update_dialog_open = false;
                    cx.notify();
                },
            )),
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
    let stripped = notes.trim();
    if stripped.is_empty() {
        return vec![div()
            .text_color(if is_light { c(0x71717A) } else { white(0.5) })
            .child("No release notes available. View release details on GitHub.")];
    }

    // Join continuation lines (indented lines following a bullet) into the
    // bullet text so multi-line entries render as a single item.
    let mut merged: Vec<String> = Vec::new();
    for raw in stripped.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let is_continuation = line.starts_with("  ")
            && !line.trim_start().starts_with('-')
            && !line.trim_start().starts_with('*')
            && !line.trim_start().starts_with('#');
        if is_continuation {
            if let Some(last) = merged.last_mut() {
                last.push(' ');
                last.push_str(line.trim());
                continue;
            }
        }
        merged.push(line.to_string());
    }

    // Strip markdown bold markers: **text** → text
    let strip_bold = |s: &str| -> String {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '*' && chars.peek() == Some(&'*') {
                chars.next(); // skip second *
            } else {
                out.push(ch);
            }
        }
        out
    };

    merged
        .iter()
        .map(|line| {
            let line = line.trim();
            if line.starts_with("### ") || line.starts_with("## ") {
                let text = strip_bold(line.trim_start_matches('#').trim());
                div()
                    .pt(px(4.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_light { c(0x18181B) } else { white(0.95) })
                    .child(text)
            } else if line.starts_with("- ") || line.starts_with("* ") {
                let text = strip_bold(line[2..].trim());
                h_flex()
                    .gap(px(6.0))
                    .items_start()
                    .child(div().text_color(red()).child("\u{2022}"))
                    .child(div().flex_1().child(text))
            } else {
                div().child(strip_bold(line))
            }
        })
        .collect()
}

/// Wording for the Settings choice. Only an all-users copy offers it.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub const ALL_USERS_TITLE: &str = "Update for all users (needs administrator approval)";
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub const ALL_USERS_OFF_DETAIL: &str = "Off: updates install just for you, with no administrator prompt, and Noir Player opens the newer copy from then on. The copy in Program Files stays as it is.";
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub const ALL_USERS_ON_DETAIL: &str = "On: updates upgrade the copy in Program Files for everyone on this PC, and Windows asks for administrator approval every time.";

/// How the running copy was installed, in plain words.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn install_scope_line(scope: crate::update::InstallScope) -> &'static str {
    match scope {
        crate::update::InstallScope::PerUser => {
            "This copy is installed just for you. Updates install with no administrator prompt."
        }
        crate::update::InstallScope::PerMachine => {
            "This copy is installed for all users. Updates install just for you, with no administrator prompt, unless you turn on Update for all users."
        }
    }
}

/// The one-time notice an update helper left behind. OK, Enter or Escape
/// dismiss it; a click outside it does nothing.
pub fn render_install_notice_modal(
    model: &mut NoirPlayerModel,
    cx: &mut Context<NoirPlayerModel>,
) -> impl IntoElement {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let message = model.install_notice.clone().unwrap_or_default();
    let title_color = if is_light { c(0x18181B) } else { white(0.95) };
    let body_color = if is_light { c(0x3F3F46) } else { white(0.8) };

    backdrop_transition(
        "install-notice-backdrop-anim",
        div()
            .id("install-notice-backdrop")
            .track_focus(&model.install_notice_focus_handle)
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(if is_light { 0x00000066 } else { 0x000000CC }))
            .flex()
            .items_center()
            .justify_center()
            .on_scroll_wheel(|_, _, cx| {
                cx.stop_propagation();
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let k = event.keystroke.key.trim();
                let escape = k.eq_ignore_ascii_case("escape") || k.eq_ignore_ascii_case("esc");
                let enter = k.eq_ignore_ascii_case("enter") || k.eq_ignore_ascii_case("return");
                // Every key stops here: the playback shortcuts underneath must
                // not fire while the notice is up.
                cx.stop_propagation();
                if escape || enter {
                    this.dismiss_install_notice(cx);
                }
            }))
            .child(modal_transition(
                "install-notice-modal-anim",
                v_flex()
                    .id("install-notice-modal")
                    .w(px(540.0))
                    .rounded_2xl()
                    .bg(dynamic_modal_surface(is_light))
                    .border(px(1.0))
                    .border_color(if is_light { c(0xE4E4E7) } else { c(0x2A2A2E) })
                    .shadow(vec![BoxShadow::new(
                        px(0.0),
                        px(24.0),
                        if is_light { c(0x00000033) } else { c(0x000000) },
                    )
                    .blur_radius(px(48.0))])
                    .overflow_hidden()
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.stop_propagation();
                    }))
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap(px(10.0))
                            .px(px(20.0))
                            .py(px(14.0))
                            .border_b(px(1.0))
                            .border_color(if is_light { c(0xE4E4E7) } else { c(0x1E1E22) })
                            .child(
                                div()
                                    .size(px(32.0))
                                    .rounded_lg()
                                    .bg(red().alpha(0.18))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(red())
                                    .child(icon_text(MusicIcon::Info, 18.0)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(title_color)
                                    .child("About your update"),
                            ),
                    )
                    .child(
                        v_flex().w_full().px(px(20.0)).py(px(16.0)).child(
                            div()
                                .text_sm()
                                .line_height(relative(1.5))
                                .text_color(body_color)
                                .child(message),
                        ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_end()
                            .px(px(20.0))
                            .py(px(14.0))
                            .border_t(px(1.0))
                            .border_color(if is_light { c(0xE4E4E7) } else { c(0x1E1E22) })
                            .child(
                                primary_button(
                                    "install-notice-ok",
                                    "OK",
                                    MusicIcon::Check,
                                    is_light,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.dismiss_install_notice(cx);
                                    },
                                )),
                            ),
                    ),
            )),
    )
}
