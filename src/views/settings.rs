use gpui_kit::component::input::Input;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::media::EQ_PRESETS;
use crate::views::ui::{icon_text, red, white};

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsCategory {
    All,
    Equalizer,
    Playback,
    Appearance,
    Library,
    About,
}

impl SettingsCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Equalizer => "Equalizer",
            Self::Playback => "Playback",
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
            if event.keystroke.key == "escape" || event.keystroke.key == "Escape" {
                this.close_settings(cx);
            }
        }))
        .child(
            v_flex()
                .id("settings-modal")
                .w(px(720.0))
                .h(px(560.0))
                .rounded_xl()
                .bg(c(0x111215))
                .border(px(1.0))
                .border_color(c(0x2A2D35))
                .shadow(vec![BoxShadow::new(px(0.0), px(24.0), c(0x000000))
                    .blur_radius(px(48.0))])
                .overflow_hidden()
                .on_click(|_, _, _| {})
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
                .child(icon_text(gpui_kit::assets::IconName::Search, 16.0)),
        )
        .child(div().flex_1().child(Input::new(&model.settings_search)))
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
                .on_click(cx.listener(|this, _, _, cx| this.close_settings(cx)))
                .child(icon_text(gpui_kit::assets::IconName::X, 14.0)),
        )
}

fn category_chips(model: &NoirPlayerModel, cx: &mut Context<NoirPlayerModel>) -> Div {
    let current = model.settings_category;
    const CATS: [SettingsCategory; 6] = [
        SettingsCategory::All,
        SettingsCategory::Equalizer,
        SettingsCategory::Playback,
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

    let eq_visible = cat == SettingsCategory::All || cat == SettingsCategory::Equalizer;
    let eq_match =
        query.is_empty() || "equalizer eq sound preset bass treble vocal rock pop".contains(query);

    if eq_visible && eq_match {
        // EQ on/off toggle
        let eq_enabled = model.store.equalizer_enabled;
        let active = idx == selected;
        rows.push(
            row_base(idx, active, cx)
                .justify_between()
                .child(label_cell("Equalizer", "5-band real-time audio filter"))
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

        // EQ Presets
        let preset = model.store.equalizer_preset.clone();
        let active = idx == selected;
        rows.push(
            row_base(idx, active, cx)
                .flex_col()
                .items_start()
                .gap(px(8.0))
                .child(label_cell("Presets", &format!("Active: {preset}")))
                .child(preset_chips_row(&preset, cx))
                .into_any_element(),
        );
        idx += 1;

        // EQ Bands
        let active = idx == selected;
        let gains = model.store.equalizer_gains;
        let eq_on = model.store.equalizer_enabled;
        rows.push(
            row_base(idx, active, cx)
                .flex_col()
                .items_start()
                .gap(px(10.0))
                .child(label_cell("Frequency Bands", "Adjust −12 dB to +12 dB per band"))
                .child(eq_bands_row(gains, eq_on, cx))
                .into_any_element(),
        );
        idx += 1;
    }

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
                    "Seconds to jump on forward/backward skip",
                ))
                .child(stepper(
                    "seek",
                    &format!("{interval}s"),
                    cx,
                    |this, _, _, cx| {
                        this.store.seek_interval_seconds =
                            match this.store.seek_interval_seconds {
                                60 => 30, 30 => 15, 15 => 10, 10 => 5, _ => 5,
                            };
                        this.save_current_store(cx);
                    },
                    |this, _, _, cx| {
                        this.store.seek_interval_seconds =
                            match this.store.seek_interval_seconds {
                                5 => 10, 10 => 15, 15 => 30, 30 => 60, _ => 60,
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
                    "Re-open last played song on launch",
                ))
                .child(toggle_switch("resume-toggle", resume, cx, |this, _, _, cx| {
                    this.store.resume_last_song = !this.store.resume_last_song;
                    this.save_current_store(cx);
                }))
                .into_any_element(),
        );
        idx += 1;
    }

    let lib_visible = cat == SettingsCategory::All || cat == SettingsCategory::Library;
    let lib_match = query.is_empty() || "music folder library rescan".contains(query);

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
                        .py(px(5.0))
                        .rounded_lg()
                        .bg(red())
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(white(1.0))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.85))
                        .on_click(cx.listener(|this, _, _, cx| this.rescan(cx)))
                        .child("Rescan Library"),
                )
                .into_any_element(),
        );
        idx += 1;
    }

    let ab_visible = cat == SettingsCategory::All || cat == SettingsCategory::About;
    let ab_match = query.is_empty() || "about noir player version".contains(query);

    if ab_visible && ab_match {
        let active = idx == selected;
        rows.push(
            row_base(idx, active, cx)
                .justify_between()
                .child(label_cell(
                    "Noir Player",
                    "Version 1.1.3 — Desktop music player with GPUI",
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
                                .child(icon_text(gpui_kit::assets::IconName::Music4, 18.0)),
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
        // idx += 1;
    }

    rows
}

// ── Reusable helpers ──────────────────────────────────────────────────────────

fn row_base(idx: usize, active: bool, cx: &mut Context<NoirPlayerModel>) -> Stateful<Div> {
    h_flex()
        .id(SharedString::from(format!("row-{idx}")))
        .w_full()
        .flex_wrap()
        .items_center()
        .px(px(14.0))
        .py(px(11.0))
        .rounded_xl()
        .bg(if active { c(0x18191F) } else { c(0x141519) })
        .border(px(1.5))
        .border_color(if active { red().alpha(0.7) } else { c(0x222428) })
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
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
        .on_click(cx.listener(handler))
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
                    icon_text(gpui_kit::assets::IconName::Check, 12.0).text_color(red())
                } else {
                    icon_text(gpui_kit::assets::IconName::X, 10.0).text_color(c(0x555966))
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
                .on_click(cx.listener(dec))
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
                .on_click(cx.listener(inc))
                .child("+"),
        )
}

fn preset_chips_row(current: &str, cx: &mut Context<NoirPlayerModel>) -> Div {
    h_flex()
        .items_center()
        .gap(px(4.0))
        .flex_wrap()
        .children(EQ_PRESETS.iter().map(|&(name, gains)| {
            let active = current == name;
            div()
                .id(SharedString::from(format!("preset-{name}")))
                .px(px(9.0))
                .py(px(4.0))
                .rounded_md()
                .text_xs()
                .font_weight(if active { FontWeight::BOLD } else { FontWeight::NORMAL })
                .bg(if active { red() } else { c(0x1E2129) })
                .border(px(1.0))
                .border_color(if active { red() } else { c(0x2A2D38) })
                .text_color(if active { white(1.0) } else { white(0.65) })
                .cursor_pointer()
                .hover(|s| s.bg(c(0x26293A)).text_color(white(0.9)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.store.equalizer_preset = name.to_string();
                    this.store.equalizer_gains = gains;
                    this.store.equalizer_enabled = true;
                    if let Some(p) = this.player.as_ref() {
                        p.set_equalizer_enabled(true);
                        p.set_equalizer_gains(gains);
                    }
                    this.save_current_store(cx);
                }))
                .child(name)
        }))
}

fn eq_bands_row(gains: [f32; 5], enabled: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    const LABELS: [&str; 5] = ["60Hz", "230Hz", "910Hz", "3.6k", "14k"];

    h_flex()
        .w_full()
        .gap(px(5.0))
        .children((0..5usize).map(|bi| {
            let gain = gains[bi];
            let freq = LABELS[bi];
            let gain_str = if gain > 0.0 {
                format!("+{gain:.0}dB")
            } else {
                format!("{gain:.0}dB")
            };
            let hot = enabled && gain.abs() > 0.1;

            v_flex()
                .flex_1()
                .items_center()
                .gap(px(4.0))
                .p(px(7.0))
                .rounded_lg()
                .bg(c(0x18191F))
                .border(px(1.0))
                .border_color(if hot { red().alpha(0.45) } else { c(0x222428) })
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if hot { red() } else { white(0.5) })
                        .child(gain_str),
                )
                .child(
                    h_flex()
                        .items_center()
                        .rounded_md()
                        .bg(c(0x111215))
                        .border(px(1.0))
                        .border_color(c(0x2A2D35))
                        .overflow_hidden()
                        .child(
                            div()
                                .id(SharedString::from(format!("band-m-{bi}")))
                                .px(px(5.0))
                                .py(px(3.0))
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(white(0.75))
                                .cursor_pointer()
                                .hover(|s| s.bg(c(0xFF000033)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let mut g = this.store.equalizer_gains;
                                    g[bi] = (g[bi] - 1.0).max(-12.0);
                                    this.store.equalizer_gains = g;
                                    this.store.equalizer_preset = "Custom".into();
                                    if let Some(p) = this.player.as_ref() {
                                        p.set_equalizer_gains(g);
                                    }
                                    this.save_current_store(cx);
                                }))
                                .child("\u{2212}"),
                        )
                        .child(
                            div()
                                .px(px(4.0))
                                .py(px(3.0))
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(white(0.6))
                                .child(freq),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("band-p-{bi}")))
                                .px(px(5.0))
                                .py(px(3.0))
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(white(0.75))
                                .cursor_pointer()
                                .hover(|s| s.bg(c(0xFF000033)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let mut g = this.store.equalizer_gains;
                                    g[bi] = (g[bi] + 1.0).min(12.0);
                                    this.store.equalizer_gains = g;
                                    this.store.equalizer_preset = "Custom".into();
                                    if let Some(p) = this.player.as_ref() {
                                        p.set_equalizer_gains(g);
                                    }
                                    this.save_current_store(cx);
                                }))
                                .child("+"),
                        ),
                )
        }))
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
