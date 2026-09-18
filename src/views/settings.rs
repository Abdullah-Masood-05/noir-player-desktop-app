use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::input::Input;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::views::ui::{backdrop_transition, icon_text, modal_transition, red, white};

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsCategory {
    All,
    Playback,
    Equalizer,
    Appearance,
    Library,
    About,
}

impl SettingsCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Playback => "Playback",
            Self::Equalizer => "Equalizer",
            Self::Appearance => "Appearance",
            Self::Library => "Library & Folders",
            Self::About => "About",
        }
    }
}

pub fn render_settings_modal(
    model: &mut NoirPlayerModel,
    cx: &mut Context<NoirPlayerModel>,
) -> impl IntoElement {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let selected_index = model.settings_selected_index;
    let query = model
        .settings_search
        .read(cx)
        .value()
        .to_string()
        .to_lowercase();
    let current_cat = model.settings_category;

    backdrop_transition(
        "settings-backdrop-anim",
        div()
            .id("settings-backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(if is_light { 0x00000066 } else { 0x000000CC }))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(|this, _, _, cx| {
                this.close_settings(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let k = event.keystroke.key.trim();
                let ctrl_or_cmd =
                    event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
                if k.eq_ignore_ascii_case("escape") || k.eq_ignore_ascii_case("esc") {
                    cx.stop_propagation();
                    this.close_settings(cx);
                } else if ctrl_or_cmd && (k == "," || k.eq_ignore_ascii_case("comma")) {
                    cx.stop_propagation();
                    this.close_settings(cx);
                } else if ctrl_or_cmd && k.eq_ignore_ascii_case("e") {
                    cx.stop_propagation();
                    this.close_settings(cx);
                    this.open_equalizer(cx);
                }
            }))
            .child(
                modal_transition(
                    "settings-modal-anim",
                    v_flex()
                        .id("settings-modal")
                        .track_focus(&model.settings_focus_handle)
                        .w(px(720.0))
                        .h(px(580.0))
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
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            let k = event.keystroke.key.trim();
                            let ctrl_or_cmd =
                                event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
                            if k.eq_ignore_ascii_case("escape") || k.eq_ignore_ascii_case("esc") {
                                cx.stop_propagation();
                                this.close_settings(cx);
                            } else if ctrl_or_cmd && (k == "," || k.eq_ignore_ascii_case("comma")) {
                                cx.stop_propagation();
                                this.close_settings(cx);
                            } else if ctrl_or_cmd && k.eq_ignore_ascii_case("e") {
                                cx.stop_propagation();
                                this.close_settings(cx);
                                this.open_equalizer(cx);
                            }
                        }))
                        .on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            window.focus(&this.settings_focus_handle, cx);
                        }))
                        .child(modal_header(model, is_light, cx))
                        .child(category_chips(model, is_light, cx))
                        .child(
                            div()
                                .id("settings-scroll")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scrollbar()
                                .px(px(16.0))
                                .py(px(10.0))
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .children(build_rows(
                                    model,
                                    &query,
                                    current_cat,
                                    selected_index,
                                    is_light,
                                    cx,
                                )),
                        )
                        .child(keyboard_hints(is_light)),
                ),
            ),
    )
}

fn modal_header(
    model: &NoirPlayerModel,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    h_flex()
        .w_full()
        .items_center()
        .gap(px(10.0))
        .px(px(16.0))
        .py(px(12.0))
        .border_b(px(1.0))
        .border_color(if is_light { c(0xE4E4E7) } else { c(0x222428) })
        .child(
            div()
                .text_color(if is_light { c(0x71717A) } else { white(0.4) })
                .flex_shrink_0()
                .child(icon_text(MusicIcon::Search, 16.0)),
        )
        .child(
            div()
                .flex_1()
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    let k = event.keystroke.key.trim();
                    if k.eq_ignore_ascii_case("escape") || k.eq_ignore_ascii_case("esc") {
                        cx.stop_propagation();
                        this.close_settings(cx);
                    }
                }))
                .child(Input::new(&model.settings_search)),
        )
        .child(
            div()
                .id("settings-close-x")
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
                    this.close_settings(cx);
                }))
                .child(icon_text(MusicIcon::X, 15.0)),
        )
}

fn category_chips(
    model: &NoirPlayerModel,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    let current = model.settings_category;
    const CATS: [SettingsCategory; 6] = [
        SettingsCategory::All,
        SettingsCategory::Playback,
        SettingsCategory::Equalizer,
        SettingsCategory::Appearance,
        SettingsCategory::Library,
        SettingsCategory::About,
    ];

    h_flex()
        .w_full()
        .items_center()
        .gap(px(6.0))
        .px(px(16.0))
        .py(px(8.0))
        .border_b(px(1.0))
        .border_color(if is_light { c(0xEDEAEF) } else { c(0x1E2026) })
        .children(CATS.iter().map(|&cat| {
            let active = cat == current;
            div()
                .id(SharedString::from(format!("chip-{}", cat.label())))
                .px(px(10.0))
                .py(px(4.5))
                .rounded_lg()
                .text_xs()
                .font_weight(if active { FontWeight::BOLD } else { FontWeight::NORMAL })
                .bg(if active {
                    red()
                } else if is_light {
                    c(0xF0F1F3)
                } else {
                    c(0x1C1F26)
                })
                .text_color(if active {
                    white(1.0)
                } else if is_light {
                    c(0x52525B)
                } else {
                    white(0.65)
                })
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.settings_category = cat;
                    this.settings_selected_index = 0;
                    cx.notify();
                }))
                .child(cat.label())
        }))
}

fn build_rows(
    model: &mut NoirPlayerModel,
    query: &str,
    cat: SettingsCategory,
    selected: usize,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Vec<AnyElement> {
    let mut rows: Vec<AnyElement> = Vec::new();
    let mut idx = 0_usize;

    // ── Equalizer rows ──────────────────────────────────────────────────────────
    let eq_visible = cat == SettingsCategory::All || cat == SettingsCategory::Equalizer;
    let eq_match =
        query.is_empty() || "equalizer eq sound audio preset bass treble vocal rock pop".contains(query);

    if eq_visible && eq_match {
        let eq_enabled = model.store.equalizer_enabled;
        let active_preset = model.store.equalizer_preset.clone();

        // Equalizer launcher button
        let active = idx == selected;
        rows.push(
            row_base(idx, active, is_light, cx)
                .justify_between()
                .child(label_cell(
                    "Equalizer Panel",
                    &format!("5-band parametric frequency shaper (Active Preset: {active_preset})"),
                    is_light,
                ))
                .child(
                    div()
                        .id("open-eq-modal-btn")
                        .px(px(12.0))
                        .py(px(6.0))
                        .rounded_lg()
                        .bg(red())
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(white(1.0))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.88))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.open_equalizer(cx);
                        }))
                        .child(icon_text(MusicIcon::SlidersHorizontal, 14.0))
                        .child("Open Equalizer"),
                )
                .into_any_element(),
        );
        idx += 1;

        // EQ filter toggle
        let active = idx == selected;
        rows.push(
            row_base(idx, active, is_light, cx)
                .justify_between()
                .child(label_cell(
                    "Equalizer Filter",
                    if eq_enabled {
                        "Active — real-time sound enhancement enabled"
                    } else {
                        "Bypassed — raw audio stream output"
                    },
                    is_light,
                ))
                .child(toggle_switch("eq-toggle", eq_enabled, is_light, cx, |this, _, _, cx| {
                    let next = !this.store.equalizer_enabled;
                    this.store.equalizer_enabled = next;
                    if let Some(p) = this.player.as_ref() {
                        p.set_equalizer_enabled(next);
                    }
                    this.save_current_store(cx);
                }))
                .into_any_element(),
        );
        idx += 1;
    }

    // ── Playback rows ───────────────────────────────────────────────────────────
    let pb_visible = cat == SettingsCategory::All || cat == SettingsCategory::Playback;
    let pb_match =
        query.is_empty() || "playback seek skip interval seconds resume startup".contains(query);

    if pb_visible && pb_match {
        let interval = model.store.seek_interval_seconds;
        let active = idx == selected;
        rows.push(
            row_base(idx, active, is_light, cx)
                .justify_between()
                .child(label_cell(
                    "Skip Interval",
                    "Seconds to jump on forward/backward skip buttons (5s, 10s, 15s, 30s, 60s)",
                    is_light,
                ))
                .child(stepper(
                    "seek",
                    &format!("{interval}s"),
                    is_light,
                    cx,
                    |this, _, _, cx| {
                        this.store.seek_interval_seconds =
                            match this.store.seek_interval_seconds {
                                60 => 30,
                                30 => 15,
                                15 => 10,
                                10 => 5,
                                _ => 5,
                            };
                        this.save_current_store(cx);
                    },
                    |this, _, _, cx| {
                        this.store.seek_interval_seconds =
                            match this.store.seek_interval_seconds {
                                5 => 10,
                                10 => 15,
                                15 => 30,
                                30 => 60,
                                _ => 60,
                            };
                        this.save_current_store(cx);
                    },
                ))
                .into_any_element(),
        );
        idx += 1;

        let resume = model.store.resume_last_song;
        let active = idx == selected;
        rows.push(
            row_base(idx, active, is_light, cx)
                .justify_between()
                .child(label_cell(
                    "Resume on Startup",
                    "Automatically re-open and queue last played song on launch",
                    is_light,
                ))
                .child(toggle_switch("resume-toggle", resume, is_light, cx, |this, _, _, cx| {
                    this.store.resume_last_song = !this.store.resume_last_song;
                    this.save_current_store(cx);
                }))
                .into_any_element(),
        );
        idx += 1;
    }

    // ── Appearance rows ─────────────────────────────────────────────────────────
    let app_visible = cat == SettingsCategory::All || cat == SettingsCategory::Appearance;
    let app_match = query.is_empty() || "appearance theme dark light red white color style".contains(query);

    if app_visible && app_match {
        let is_light_theme = model.store.theme_mode.eq_ignore_ascii_case("light");
        let active = idx == selected;
        rows.push(
            row_base(idx, active, is_light, cx)
                .justify_between()
                .child(label_cell(
                    "Theme Palette",
                    "Dark Noir (black & crimson red) or Light (clean white & red, matches mobile)",
                    is_light,
                ))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .id("theme-dark-btn")
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded_lg()
                                .bg(if !is_light_theme {
                                    red()
                                } else {
                                    c(0xF0F1F3)
                                })
                                .text_color(if !is_light_theme {
                                    white(1.0)
                                } else {
                                    c(0x52525B)
                                })
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.set_theme_mode("dark", cx);
                                }))
                                .child("Dark Noir"),
                        )
                        .child(
                            div()
                                .id("theme-light-btn")
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded_lg()
                                .bg(if is_light_theme {
                                    red()
                                } else {
                                    c(0x1C1F26)
                                })
                                .text_color(if is_light_theme {
                                    white(1.0)
                                } else {
                                    white(0.7)
                                })
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.set_theme_mode("light", cx);
                                }))
                                .child("Light Red & White"),
                        ),
                )
                .into_any_element(),
        );
        idx += 1;
    }

    // ── Library & Folders rows ──────────────────────────────────────────────────
    let lib_visible = cat == SettingsCategory::All || cat == SettingsCategory::Library;
    let lib_match = query.is_empty() || "music folder library rescan files scan download".contains(query);

    if lib_visible && lib_match {
        // 1. Add Music Folder & Rescan Actions
        let active = idx == selected;
        rows.push(
            row_base(idx, active, is_light, cx)
                .justify_between()
                .child(label_cell(
                    "Music Library Sources",
                    "Add folders containing audio tracks to your local library",
                    is_light,
                ))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .id("add-music-folder-btn")
                                .px(px(12.0))
                                .py(px(6.0))
                                .rounded_lg()
                                .bg(red())
                                .flex()
                                .items_center()
                                .gap(px(5.0))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(white(1.0))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.add_music_folder(cx);
                                }))
                                .child(icon_text(MusicIcon::FolderPlus, 14.0))
                                .child("+ Add Folder"),
                        )
                        .child(
                            div()
                                .id("rescan-btn")
                                .px(px(12.0))
                                .py(px(6.0))
                                .rounded_lg()
                                .bg(if is_light { c(0xF0F1F3) } else { c(0x1C1F26) })
                                .border(px(1.0))
                                .border_color(if is_light { c(0xDCDEE2) } else { c(0x2A2D35) })
                                .flex()
                                .items_center()
                                .gap(px(5.0))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_light { c(0x18181B) } else { white(0.9) })
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.rescan(cx);
                                }))
                                .child(icon_text(MusicIcon::RotateCw, 13.0))
                                .child("Rescan"),
                        ),
                )
                .into_any_element(),
        );
        idx += 1;

        // 2. Configured Folders list
        let configured_folders = model.store.music_folders.clone();
        if configured_folders.is_empty() {
            let default_path = crate::media::default_music_folder()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Music folder".to_string());
            let active = idx == selected;
            rows.push(
                row_base(idx, active, is_light, cx)
                    .justify_between()
                    .child(label_cell("Default Music Folder", &default_path, is_light))
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(3.0))
                            .rounded_md()
                            .bg(if is_light { c(0xEDEAEF) } else { c(0x1C1F26) })
                            .text_xs()
                            .text_color(if is_light { c(0x71717A) } else { white(0.5) })
                            .child("System Default"),
                    )
                    .into_any_element(),
            );
            idx += 1;
        } else {
            for folder in configured_folders {
                let active = idx == selected;
                let folder_clone = folder.clone();
                let display = folder.display().to_string();
                rows.push(
                    row_base(idx, active, is_light, cx)
                        .justify_between()
                        .child(
                            h_flex()
                                .items_center()
                                .gap(px(8.0))
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_color(red())
                                        .child(icon_text(MusicIcon::Folder, 16.0)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(if is_light { c(0x18181B) } else { white(0.9) })
                                        .truncate()
                                        .child(display),
                                ),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("remove-folder-{}", folder_clone.display())))
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .bg(c(0xFF000022))
                                .text_color(red())
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .cursor_pointer()
                                .hover(|s| s.bg(red()).text_color(white(1.0)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.remove_music_folder(&folder_clone, cx);
                                }))
                                .child("Remove"),
                        )
                        .into_any_element(),
                );
                idx += 1;
            }
        }

        // 3. Download Folder Configuration
        let download_dest = model
            .store
            .download_folder
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| {
                format!(
                    "Default: {}",
                    crate::media::default_music_folder()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "Music folder".to_string())
                )
            });
        let has_custom_download = model.store.download_folder.is_some();
        let active = idx == selected;
        rows.push(
            row_base(idx, active, is_light, cx)
                .justify_between()
                .child(label_cell(
                    "Download Folder",
                    &format!("Save destination for new songs from Discover ({download_dest})"),
                    is_light,
                ))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .id("change-download-folder-btn")
                                .px(px(12.0))
                                .py(px(6.0))
                                .rounded_lg()
                                .bg(red())
                                .flex()
                                .items_center()
                                .gap(px(5.0))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(white(1.0))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.change_download_folder(cx);
                                }))
                                .child(icon_text(MusicIcon::Download, 13.0))
                                .child("Change Folder"),
                        )
                        .when(has_custom_download, |d| {
                            d.child(
                                div()
                                    .id("reset-download-folder-btn")
                                    .px(px(10.0))
                                    .py(px(6.0))
                                    .rounded_lg()
                                    .bg(if is_light { c(0xF0F1F3) } else { c(0x1C1F26) })
                                    .border(px(1.0))
                                    .border_color(if is_light { c(0xDCDEE2) } else { c(0x2A2D35) })
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_light { c(0x71717A) } else { white(0.7) })
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.85))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.reset_download_folder(cx);
                                    }))
                                    .child("Reset to Default"),
                            )
                        }),
                )
                .into_any_element(),
        );
        idx += 1;
    }

    // ── About rows ──────────────────────────────────────────────────────────────
    let ab_visible = cat == SettingsCategory::All || cat == SettingsCategory::About;
    let ab_match = query.is_empty() || "about noir player version info help shortcuts".contains(query);

    if ab_visible && ab_match {
        let active = idx == selected;
        // Main About Card
        rows.push(
            row_base(idx, active, is_light, cx)
                .justify_between()
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .size(px(40.0))
                                .rounded_xl()
                                .bg(red())
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(white(1.0))
                                .child(icon_text(MusicIcon::Music4, 22.0)),
                        )
                        .child(
                            v_flex()
                                .gap(px(2.0))
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(if is_light { c(0x18181B) } else { white(0.95) })
                                                .child("Noir Player Desktop"),
                                        )
                                        .child(
                                            div()
                                                .px(px(6.0))
                                                .py(px(1.5))
                                                .rounded_md()
                                                .bg(red().alpha(0.2))
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(red())
                                                .child("v1.1.3"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if is_light { c(0x71717A) } else { white(0.5) })
                                        .child("Created by Abdullah Masood"),
                                ),
                        ),
                )
                .child(
                    div()
                        .id("open-github-btn")
                        .px(px(12.0))
                        .py(px(6.0))
                        .rounded_lg()
                        .bg(if is_light { c(0xF0F1F3) } else { c(0x1C1F26) })
                        .border(px(1.0))
                        .border_color(if is_light { c(0xDCDEE2) } else { c(0x2A2D35) })
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_light { c(0x18181B) } else { white(0.9) })
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.85))
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.stop_propagation();
                            cx.open_url("https://github.com/Abdullah-Masood-05/noir-player-desktop-app");
                        }))
                        .child(icon_text(MusicIcon::ExternalLink, 13.0))
                        .child("GitHub"),
                )
                .into_any_element(),
        );
        idx += 1;

        // Technology Description Card
        let active = idx == selected;
        rows.push(
            row_base(idx, active, is_light, cx)
                .justify_between()
                .child(label_cell(
                    "Architecture & Engine",
                    "Pure Rust native application with GPUI GPU acceleration & Rodio real-time DSP",
                    is_light,
                ))
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(3.0))
                        .rounded_md()
                        .bg(if is_light { c(0xEDEAEF) } else { c(0x1C1F26) })
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(red())
                        .child("Rust + GPUI"),
                )
                .into_any_element(),
        );
        idx += 1;

        // Keyboard Shortcuts Cheat Sheet
        let active = idx == selected;
        rows.push(
            row_base(idx, active, is_light, cx)
                .justify_between()
                .child(label_cell(
                    "Keyboard Shortcuts",
                    "Ctrl+, for Settings \u{2022} Ctrl+E for Equalizer \u{2022} Esc to Close Modals",
                    is_light,
                ))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(key_chip("Ctrl+,", is_light))
                        .child(key_chip("Ctrl+E", is_light))
                        .child(key_chip("Esc", is_light)),
                )
                .into_any_element(),
        );
    }

    rows
}

// ── Reusable helpers ──────────────────────────────────────────────────────────

fn row_base(
    idx: usize,
    active: bool,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Stateful<Div> {
    h_flex()
        .id(SharedString::from(format!("row-{idx}")))
        .w_full()
        .items_center()
        .px(px(14.0))
        .py(px(11.0))
        .rounded_xl()
        .bg(if active {
            if is_light {
                c(0xFDE8E8)
            } else {
                c(0x18191F)
            }
        } else if is_light {
            c(0xF9FAFB)
        } else {
            c(0x141519)
        })
        .border(px(1.5))
        .border_color(if active {
            red().alpha(0.75)
        } else if is_light {
            c(0xE5E7EB)
        } else {
            c(0x222428)
        })
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            this.settings_selected_index = idx;
            cx.notify();
        }))
}

fn label_cell(title: &str, subtitle: &str, is_light: bool) -> Div {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap(px(2.0))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if is_light { c(0x18181B) } else { white(0.92) })
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_xs()
                .text_color(if is_light { c(0x71717A) } else { white(0.45) })
                .child(subtitle.to_owned()),
        )
}

fn toggle_switch<H>(
    id: &str,
    on: bool,
    _is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
    handler: H,
) -> Stateful<Div>
where
    H: Fn(
            &mut NoirPlayerModel,
            &ClickEvent,
            &mut Window,
            &mut Context<NoirPlayerModel>,
        ) + 'static,
{
    div()
        .id(SharedString::from(id.to_owned()))
        .w(px(44.0))
        .h(px(24.0))
        .rounded_full()
        .bg(if on { red() } else { c(0x2E313A) })
        .p(px(2.0))
        .flex()
        .items_center()
        .cursor_pointer()
        .on_click(cx.listener(move |this, event, window, cx| {
            cx.stop_propagation();
            handler(this, event, window, cx);
        }))
        .child(
            div()
                .size(px(20.0))
                .rounded_full()
                .bg(white(1.0))
                .shadow(vec![
                    BoxShadow::new(px(0.0), px(1.0), c(0x000000)).blur_radius(px(3.0)),
                ])
                .when(on, |d| d.ml_auto())
                .flex()
                .items_center()
                .justify_center()
                .child(if on {
                    icon_text(MusicIcon::Check, 12.0).text_color(red())
                } else {
                    icon_text(MusicIcon::X, 10.0).text_color(c(0x555966))
                }),
        )
}

fn stepper<Dm, Dp>(
    id: &str,
    label: &str,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
    dec: Dm,
    inc: Dp,
) -> Div
where
    Dm: Fn(
            &mut NoirPlayerModel,
            &ClickEvent,
            &mut Window,
            &mut Context<NoirPlayerModel>,
        ) + 'static,
    Dp: Fn(
            &mut NoirPlayerModel,
            &ClickEvent,
            &mut Window,
            &mut Context<NoirPlayerModel>,
        ) + 'static,
{
    h_flex()
        .items_center()
        .rounded_lg()
        .bg(if is_light { c(0xF0F1F3) } else { c(0x1C1F26) })
        .border(px(1.0))
        .border_color(if is_light { c(0xDCDEE2) } else { c(0x2A2D35) })
        .overflow_hidden()
        .child(
            div()
                .id(SharedString::from(format!("{id}-dec")))
                .px(px(10.0))
                .py(px(5.0))
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(if is_light { c(0x3F3F46) } else { white(0.8) })
                .cursor_pointer()
                .hover(|s| s.bg(c(0xFF000033)))
                .on_click(cx.listener(move |this, event, window, cx| {
                    cx.stop_propagation();
                    dec(this, event, window, cx);
                }))
                .child("\u{2212}"),
        )
        .child(
            div()
                .px(px(12.0))
                .py(px(5.0))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if is_light { c(0x18181B) } else { white(0.95) })
                .min_w(px(38.0))
                .flex()
                .items_center()
                .justify_center()
                .child(label.to_owned()),
        )
        .child(
            div()
                .id(SharedString::from(format!("{id}-inc")))
                .px(px(10.0))
                .py(px(5.0))
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(if is_light { c(0x3F3F46) } else { white(0.8) })
                .cursor_pointer()
                .hover(|s| s.bg(c(0xFF000033)))
                .on_click(cx.listener(move |this, event, window, cx| {
                    cx.stop_propagation();
                    inc(this, event, window, cx);
                }))
                .child("+"),
        )
}

fn key_chip(key: &str, is_light: bool) -> Div {
    div()
        .px(px(7.0))
        .py(px(2.0))
        .rounded_md()
        .bg(if is_light { c(0xF0F1F3) } else { c(0x1E2026) })
        .border(px(1.0))
        .border_color(if is_light { c(0xD4D4D8) } else { c(0x2A2D35) })
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(if is_light { c(0x27272A) } else { white(0.7) })
        .child(key.to_owned())
}

fn keyboard_hints(is_light: bool) -> Div {
    h_flex()
        .w_full()
        .items_center()
        .justify_center()
        .gap(px(14.0))
        .py(px(8.0))
        .border_t(px(1.0))
        .border_color(if is_light { c(0xE4E4E7) } else { c(0x1E2026) })
        .child(hint_badge("Ctrl + ,", "Settings", is_light))
        .child(hint_badge("Ctrl + E", "Equalizer", is_light))
        .child(hint_badge("Escape", "to close", is_light))
}

fn hint_badge(key: &str, desc: &str, is_light: bool) -> Div {
    h_flex()
        .items_center()
        .gap(px(5.0))
        .child(
            div()
                .px(px(6.0))
                .py(px(1.5))
                .rounded_md()
                .bg(if is_light { c(0xF0F1F3) } else { c(0x1E2026) })
                .border(px(1.0))
                .border_color(if is_light { c(0xD4D4D8) } else { c(0x2A2D35) })
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(if is_light { c(0x27272A) } else { white(0.65) })
                .child(key.to_owned()),
        )
        .child(
            div()
                .text_xs()
                .text_color(if is_light { c(0x71717A) } else { white(0.4) })
                .child(desc.to_owned()),
        )
}
