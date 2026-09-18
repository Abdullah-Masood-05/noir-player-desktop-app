use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::input::Input;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::views::ui::{icon_text, red, white};

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
            Self::Library => "Library",
            Self::About => "About",
        }
    }
}

pub fn render_settings_modal(
    model: &mut NoirPlayerModel,
    cx: &mut Context<NoirPlayerModel>,
) -> impl IntoElement {
    let selected_index = model.settings_selected_index;
    let query = model
        .settings_search
        .read(cx)
        .value()
        .to_string()
        .to_lowercase();
    let current_cat = model.settings_category;

    div()
        .id("settings-backdrop")
        .absolute()
        .inset_0()
        .bg(rgba(0x000000CC))
        .flex()
        .items_center()
        .justify_center()
        .on_click(cx.listener(|this, _, _, cx| {
            this.close_settings(cx);
        }))
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
            let k = event.keystroke.key.trim();
            if k.eq_ignore_ascii_case("escape") || k.eq_ignore_ascii_case("esc") {
                cx.stop_propagation();
                this.close_settings(cx);
            }
        }))
        .child(
            v_flex()
                .id("settings-modal")
                .track_focus(&model.settings_focus_handle)
                .w(px(720.0))
                .h(px(560.0))
                .rounded_xl()
                .bg(c(0x111215))
                .border(px(1.0))
                .border_color(c(0x2A2D35))
                .shadow(vec![BoxShadow::new(px(0.0), px(24.0), c(0x000000))
                    .blur_radius(px(48.0))])
                .overflow_hidden()
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    let k = event.keystroke.key.trim();
                    if k.eq_ignore_ascii_case("escape") || k.eq_ignore_ascii_case("esc") {
                        cx.stop_propagation();
                        this.close_settings(cx);
                    }
                }))
                .on_click(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    window.focus(&this.settings_focus_handle, cx);
                }))
                .child(modal_header(model, cx))
                .child(category_chips(model, cx))
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
                        .gap(px(6.0))
                        .children(build_rows(model, &query, current_cat, selected_index, cx)),
                )
                .child(keyboard_hints()),
        )
}

fn modal_header(model: &NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    h_flex()
        .w_full()
        .items_center()
        .gap(px(10.0))
        .px(px(14.0))
        .py(px(10.0))
        .border_b(px(1.0))
        .border_color(c(0x222428))
        .child(
            div()
                .text_color(white(0.4))
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
                .size(px(26.0))
                .rounded_md()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(white(0.5))
                .hover(|s| s.text_color(white(1.0)))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.close_settings(cx);
                }))
                .child(icon_text(MusicIcon::X, 14.0)),
        )
}

fn category_chips(model: &NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
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
        .gap(px(5.0))
        .px(px(14.0))
        .py(px(7.0))
        .border_b(px(1.0))
        .border_color(c(0x1E2026))
        .children(CATS.iter().map(|&cat| {
            let active = cat == current;
            div()
                .id(SharedString::from(format!("chip-{}", cat.label())))
                .px(px(10.0))
                .py(px(4.0))
                .rounded_md()
                .text_xs()
                .font_weight(if active { FontWeight::BOLD } else { FontWeight::NORMAL })
                .bg(if active { red() } else { c(0x1C1F26) })
                .text_color(if active { white(1.0) } else { white(0.6) })
                .cursor_pointer()
                .hover(|s| s.bg(c(0x26292F)).text_color(white(0.9)))
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

        // 1. Equalizer dedicated launcher button
        let active = idx == selected;
        rows.push(
            row_base(idx, active, cx)
                .justify_between()
                .child(label_cell(
                    "Equalizer Panel",
                    &format!("5-band parametric frequency shaper (Preset: {active_preset})"),
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

        // 2. EQ on/off quick toggle
        let active = idx == selected;
        rows.push(
            row_base(idx, active, cx)
                .justify_between()
                .child(label_cell(
                    "Equalizer Filter",
                    if eq_enabled {
                        "Active — real-time sound enhancement enabled"
                    } else {
                        "Bypassed — raw audio stream output"
                    },
                ))
                .child(toggle_switch("eq-toggle", eq_enabled, cx, |this, _, _, cx| {
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
            row_base(idx, active, cx)
                .justify_between()
                .child(label_cell(
                    "Skip Interval",
                    "Seconds to jump on forward/backward skip buttons (5s, 10s, 15s, 30s, 60s)",
                ))
                .child(stepper(
                    "seek",
                    &format!("{interval}s"),
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
            row_base(idx, active, cx)
                .justify_between()
                .child(label_cell(
                    "Resume on Startup",
                    "Automatically re-open and queue last played song on launch",
                ))
                .child(toggle_switch("resume-toggle", resume, cx, |this, _, _, cx| {
                    this.store.resume_last_song = !this.store.resume_last_song;
                    this.save_current_store(cx);
                }))
                .into_any_element(),
        );
        idx += 1;
    }

    // ── Appearance rows ─────────────────────────────────────────────────────────
    let app_visible = cat == SettingsCategory::All || cat == SettingsCategory::Appearance;
    let app_match = query.is_empty() || "appearance theme dark red color style".contains(query);

    if app_visible && app_match {
        let active = idx == selected;
        rows.push(
            row_base(idx, active, cx)
                .justify_between()
                .child(label_cell(
                    "Theme Palette",
                    "Dark Noir theme with signature crimson red accents",
                ))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .size(px(18.0))
                                .rounded_full()
                                .bg(red())
                                .border(px(1.0))
                                .border_color(white(0.3)),
                        )
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(3.0))
                                .rounded_md()
                                .bg(c(0x1C1F26))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(white(0.85))
                                .child("Noir Red Dark"),
                        ),
                )
                .into_any_element(),
        );
        idx += 1;
    }

    // ── Library rows ────────────────────────────────────────────────────────────
    let lib_visible = cat == SettingsCategory::All || cat == SettingsCategory::Library;
    let lib_match = query.is_empty() || "music folder library rescan files scan".contains(query);

    if lib_visible && lib_match {
        let folder = model
            .store
            .music_folder
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Default system Music folder".to_string());
        let active = idx == selected;
        rows.push(
            row_base(idx, active, cx)
                .justify_between()
                .child(label_cell("Music Folder", &folder))
                .child(
                    div()
                        .id("rescan-btn")
                        .px(px(14.0))
                        .py(px(6.0))
                        .rounded_lg()
                        .bg(red())
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(white(1.0))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.85))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.rescan(cx);
                        }))
                        .child("Rescan Library"),
                )
                .into_any_element(),
        );
        idx += 1;
    }

    // ── About rows ──────────────────────────────────────────────────────────────
    let ab_visible = cat == SettingsCategory::All || cat == SettingsCategory::About;
    let ab_match = query.is_empty() || "about noir player version info".contains(query);

    if ab_visible && ab_match {
        let active = idx == selected;
        rows.push(
            row_base(idx, active, cx)
                .justify_between()
                .child(label_cell(
                    "Noir Player",
                    "Version 1.1.3 — Desktop music player with GPUI & real-time audio engine",
                ))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .size(px(30.0))
                                .rounded_lg()
                                .bg(red())
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(white(1.0))
                                .child(icon_text(MusicIcon::Music4, 18.0)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(white(0.85))
                                .child("v1.1.3"),
                        ),
                )
                .into_any_element(),
        );
    }

    rows
}

// ── Reusable helpers ──────────────────────────────────────────────────────────

fn row_base(idx: usize, active: bool, cx: &mut Context<NoirPlayerModel>) -> Stateful<Div> {
    h_flex()
        .id(SharedString::from(format!("row-{idx}")))
        .w_full()
        .items_center()
        .px(px(14.0))
        .py(px(11.0))
        .rounded_xl()
        .bg(if active { c(0x18191F) } else { c(0x141519) })
        .border(px(1.5))
        .border_color(if active { red().alpha(0.7) } else { c(0x222428) })
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            this.settings_selected_index = idx;
            cx.notify();
        }))
}

fn label_cell(title: &str, subtitle: &str) -> Div {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap(px(2.0))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(white(0.92))
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_xs()
                .text_color(white(0.45))
                .child(subtitle.to_owned()),
        )
}

fn toggle_switch<H>(
    id: &str,
    on: bool,
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
        .bg(c(0x1C1F26))
        .border(px(1.0))
        .border_color(c(0x2A2D35))
        .overflow_hidden()
        .child(
            div()
                .id(SharedString::from(format!("{id}-dec")))
                .px(px(10.0))
                .py(px(5.0))
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(white(0.8))
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
                .text_color(white(0.95))
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
                .text_color(white(0.8))
                .cursor_pointer()
                .hover(|s| s.bg(c(0xFF000033)))
                .on_click(cx.listener(move |this, event, window, cx| {
                    cx.stop_propagation();
                    inc(this, event, window, cx);
                }))
                .child("+"),
        )
}

fn keyboard_hints() -> Div {
    h_flex()
        .w_full()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .py(px(8.0))
        .border_t(px(1.0))
        .border_color(c(0x1E2026))
        .child(hint_badge("Escape", "to close"))
}

fn hint_badge(key: &str, desc: &str) -> Div {
    h_flex()
        .items_center()
        .gap(px(5.0))
        .child(
            div()
                .px(px(6.0))
                .py(px(1.0))
                .rounded_md()
                .bg(c(0x1E2026))
                .border(px(1.0))
                .border_color(c(0x2A2D35))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(white(0.65))
                .child(key.to_owned()),
        )
        .child(
            div()
                .text_xs()
                .text_color(white(0.4))
                .child(desc.to_owned()),
        )
}
