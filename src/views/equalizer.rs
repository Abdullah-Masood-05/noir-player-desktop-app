use gpui_kit::assets::IconName as MusicIcon;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::NoirPlayerModel;
use crate::media::EQ_PRESETS;
use crate::views::ui::{
    backdrop_transition, dynamic_inner_surface, dynamic_modal_surface, icon_text, modal_transition,
    red, white,
};

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub fn render_equalizer_modal(
    model: &mut NoirPlayerModel,
    cx: &mut Context<NoirPlayerModel>,
) -> impl IntoElement {
    let is_light = matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Light);
    let eq_enabled = model.store.equalizer_enabled;
    let gains = model.store.equalizer_gains;
    let active_preset = model.store.equalizer_preset.clone();

    backdrop_transition(
        "equalizer-backdrop-anim",
        div()
            .id("equalizer-backdrop")
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
                this.close_equalizer(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let k = event.keystroke.key.trim();
                let ctrl_or_cmd =
                    event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
                if k.eq_ignore_ascii_case("escape")
                    || k.eq_ignore_ascii_case("esc")
                    || (ctrl_or_cmd && k.eq_ignore_ascii_case("e"))
                {
                    cx.stop_propagation();
                    this.close_equalizer(cx);
                } else if ctrl_or_cmd && (k == "," || k.eq_ignore_ascii_case("comma")) {
                    cx.stop_propagation();
                    this.close_equalizer(cx);
                    this.open_settings(crate::views::settings::SettingsCategory::Equalizer, cx);
                }
            }))
            .child(modal_transition(
                "equalizer-modal-anim",
                v_flex()
                    .id("equalizer-modal")
                    .track_focus(&model.equalizer_focus_handle)
                    .w(px(660.0))
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
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        let k = event.keystroke.key.trim();
                        let ctrl_or_cmd =
                            event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
                        if k.eq_ignore_ascii_case("escape")
                            || k.eq_ignore_ascii_case("esc")
                            || (ctrl_or_cmd && k.eq_ignore_ascii_case("e"))
                        {
                            cx.stop_propagation();
                            this.close_equalizer(cx);
                        } else if ctrl_or_cmd && (k == "," || k.eq_ignore_ascii_case("comma")) {
                            cx.stop_propagation();
                            this.close_equalizer(cx);
                            this.open_settings(
                                crate::views::settings::SettingsCategory::Equalizer,
                                cx,
                            );
                        }
                    }))
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        window.focus(&this.equalizer_focus_handle, cx);
                    }))
                    .child(modal_header(is_light, cx))
                    .child(master_toggle_section(eq_enabled, is_light, cx))
                    .child(presets_section(&active_preset, is_light, cx))
                    .child(bands_section(gains, eq_enabled, is_light, cx))
                    .child(modal_footer(is_light)),
            )),
    )
}

fn modal_header(is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .px(px(20.0))
        .py(px(16.0))
        .border_b(px(1.0))
        .border_color(if is_light { c(0xE4E4E7) } else { c(0x1A1A1F) })
        .child(
            h_flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .size(px(36.0))
                        .rounded_xl()
                        .bg(if is_light { c(0xFDE8E8) } else { c(0x1F1418) })
                        .border(px(1.0))
                        .border_color(red().alpha(0.35))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(red())
                        .child(icon_text(MusicIcon::SlidersHorizontal, 20.0)),
                )
                .child(
                    v_flex()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_light { c(0x18181B) } else { white(0.95) })
                                .child("Audio Equalizer"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if is_light { c(0x71717A) } else { white(0.5) })
                                .child("5-Band Parametric Frequency Shaper"),
                        ),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .id("eq-reset-btn")
                        .px(px(10.0))
                        .py(px(5.0))
                        .rounded_lg()
                        .bg(dynamic_inner_surface(is_light))
                        .border(px(1.0))
                        .border_color(if is_light { c(0xDEDEE1) } else { c(0x222227) })
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if is_light { c(0x3F3F46) } else { white(0.7) })
                        .cursor_pointer()
                        .hover(|s| s.bg(c(0xFF000022)).text_color(red()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.store.equalizer_preset = "Flat".to_string();
                            this.store.equalizer_gains = [0.0; 5];
                            if let Some(p) = this.player.as_ref() {
                                p.set_equalizer_gains([0.0; 5]);
                            }
                            this.save_current_store(cx);
                        }))
                        .child("Reset Flat"),
                )
                .child(
                    div()
                        .id("equalizer-close-x")
                        .size(px(30.0))
                        .rounded_lg()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(if is_light { c(0x71717A) } else { white(0.5) })
                        .hover(|s| s.bg(c(0xFF000022)).text_color(red()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.close_equalizer(cx);
                        }))
                        .child(icon_text(MusicIcon::X, 16.0)),
                ),
        )
}

fn master_toggle_section(enabled: bool, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .px(px(20.0))
        .py(px(14.0))
        .border_b(px(1.0))
        .border_color(if is_light { c(0xEDEDF0) } else { c(0x17171C) })
        .bg(dynamic_inner_surface(is_light))
        .child(
            h_flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .size(px(10.0))
                        .rounded_full()
                        .bg(if enabled {
                            red()
                        } else {
                            if is_light {
                                c(0xA1A1AA)
                            } else {
                                c(0x444448)
                            }
                        })
                        .when(enabled, |d| {
                            d.shadow(vec![
                                BoxShadow::new(px(0.0), px(0.0), red()).blur_radius(px(6.0))
                            ])
                        }),
                )
                .child(
                    v_flex()
                        .gap(px(1.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_light {
                                    if enabled {
                                        c(0x18181B)
                                    } else {
                                        c(0x71717A)
                                    }
                                } else if enabled {
                                    white(0.95)
                                } else {
                                    white(0.6)
                                })
                                .child(if enabled {
                                    "Equalizer Active"
                                } else {
                                    "Equalizer Bypassed (Off)"
                                }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if is_light { c(0x71717A) } else { white(0.4) })
                                .child("Apply real-time frequency adjustments to playback"),
                        ),
                ),
        )
        .child(
            div()
                .id("eq-master-toggle")
                .w(px(50.0))
                .h(px(26.0))
                .rounded_full()
                .bg(if enabled {
                    red()
                } else {
                    if is_light {
                        c(0xD4D4D8)
                    } else {
                        c(0x2C2C31)
                    }
                })
                .p(px(2.0))
                .flex()
                .items_center()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    let next = !this.store.equalizer_enabled;
                    this.store.equalizer_enabled = next;
                    if let Some(p) = this.player.as_ref() {
                        p.set_equalizer_enabled(next);
                    }
                    this.save_current_store(cx);
                }))
                .child(
                    div()
                        .size(px(22.0))
                        .rounded_full()
                        .bg(white(1.0))
                        .shadow(vec![
                            BoxShadow::new(px(0.0), px(1.0), c(0x000000)).blur_radius(px(3.0))
                        ])
                        .when(enabled, |d| d.ml_auto())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(if enabled {
                            icon_text(MusicIcon::Check, 13.0).text_color(red())
                        } else {
                            icon_text(MusicIcon::X, 11.0).text_color(c(0x555559))
                        }),
                ),
        )
}

fn presets_section(current: &str, is_light: bool, cx: &mut Context<NoirPlayerModel>) -> Div {
    v_flex()
        .w_full()
        .px(px(20.0))
        .py(px(12.0))
        .gap(px(8.0))
        .border_b(px(1.0))
        .border_color(if is_light { c(0xEDEDF0) } else { c(0x17171C) })
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_light { c(0x71717A) } else { white(0.5) })
                        .child("PRESETS"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(red())
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("Current: {current}")),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .gap(px(6.0))
                .flex_wrap()
                .children(EQ_PRESETS.iter().map(|&(name, gains)| {
                    let active = current == name;
                    div()
                        .id(SharedString::from(format!("eq-preset-{name}")))
                        .px(px(12.0))
                        .py(px(5.0))
                        .rounded_lg()
                        .text_xs()
                        .font_weight(if active {
                            FontWeight::BOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .bg(if active {
                            red()
                        } else if is_light {
                            c(0xF0F1F3)
                        } else {
                            c(0x131317)
                        })
                        .border(px(1.0))
                        .border_color(if active {
                            red()
                        } else if is_light {
                            c(0xDEDEE1)
                        } else {
                            c(0x222227)
                        })
                        .text_color(if active {
                            white(1.0)
                        } else if is_light {
                            c(0x3F3F46)
                        } else {
                            white(0.7)
                        })
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.85))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
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
                })),
        )
}

fn bands_section(
    gains: [f32; 5],
    enabled: bool,
    is_light: bool,
    cx: &mut Context<NoirPlayerModel>,
) -> Div {
    const BAND_INFO: [(&str, &str); 5] = [
        ("60Hz", "Sub Bass"),
        ("230Hz", "Bass"),
        ("910Hz", "Midrange"),
        ("3.6kHz", "Presence"),
        ("14kHz", "Brilliance"),
    ];

    v_flex()
        .w_full()
        .px(px(20.0))
        .py(px(16.0))
        .gap(px(10.0))
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_light { c(0x71717A) } else { white(0.5) })
                        .child("FREQUENCY BANDS (−12 dB to +12 dB)"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(if is_light { c(0x71717A) } else { white(0.4) })
                        .child("Adjust with + / − buttons"),
                ),
        )
        .child(
            h_flex()
                .w_full()
                .gap(px(10.0))
                .children((0..5usize).map(|bi| {
                    let gain = gains[bi];
                    let (freq, desc) = BAND_INFO[bi];
                    let gain_str = if gain > 0.0 {
                        format!("+{gain:.0} dB")
                    } else {
                        format!("{gain:.0} dB")
                    };
                    let is_active = enabled && gain.abs() > 0.05;

                    v_flex()
                        .flex_1()
                        .items_center()
                        .gap(px(6.0))
                        .p(px(10.0))
                        .rounded_xl()
                        .bg(dynamic_inner_surface(is_light))
                        .border(px(1.0))
                        .border_color(if is_active {
                            red().alpha(0.5)
                        } else if is_light {
                            c(0xE4E4E7)
                        } else {
                            c(0x1D1D22)
                        })
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_light { c(0x18181B) } else { white(0.85) })
                                .child(freq),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if is_light { c(0x71717A) } else { white(0.4) })
                                .child(desc),
                        )
                        // Gain readout
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(3.0))
                                .rounded_md()
                                .bg(if is_active {
                                    red().alpha(0.2)
                                } else if is_light {
                                    c(0xF0F1F3)
                                } else {
                                    c(0x0C0C10)
                                })
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_active {
                                    red()
                                } else if is_light {
                                    c(0x3F3F46)
                                } else {
                                    white(0.55)
                                })
                                .child(gain_str),
                        )
                        // Visual level bar
                        .child(level_indicator(gain, enabled, is_light))
                        // Plus / Minus Stepper
                        .child(
                            h_flex()
                                .items_center()
                                .rounded_lg()
                                .bg(dynamic_inner_surface(is_light))
                                .border(px(1.0))
                                .border_color(if is_light { c(0xDEDEE1) } else { c(0x202025) })
                                .overflow_hidden()
                                .child(
                                    div()
                                        .id(SharedString::from(format!("eq-band-dec-{bi}")))
                                        .px(px(8.0))
                                        .py(px(4.0))
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if is_light { c(0x3F3F46) } else { white(0.8) })
                                        .cursor_pointer()
                                        .hover(|s| s.bg(c(0xFF000033)).text_color(red()))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
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
                                        .id(SharedString::from(format!("eq-band-inc-{bi}")))
                                        .px(px(8.0))
                                        .py(px(4.0))
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if is_light { c(0x3F3F46) } else { white(0.8) })
                                        .cursor_pointer()
                                        .hover(|s| s.bg(c(0xFF000033)).text_color(red()))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
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
                })),
        )
}

fn level_indicator(gain: f32, enabled: bool, is_light: bool) -> Div {
    let clamped = gain.clamp(-12.0, 12.0);
    let ratio = clamped / 12.0;
    let bar_height = (ratio.abs() * 26.0).max(2.0);
    let is_pos = ratio >= 0.0;

    div()
        .w(px(14.0))
        .h(px(60.0))
        .rounded_md()
        .bg(if is_light { c(0xEDEDF0) } else { c(0x0A0A0E) })
        .border(px(1.0))
        .border_color(if is_light { c(0xDEDEE1) } else { c(0x1A1A1F) })
        .relative()
        .overflow_hidden()
        // Center zero line
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(29.0))
                .h(px(2.0))
                .bg(if is_light { c(0xA1A1AA) } else { c(0x353539) }),
        )
        // Active bar
        .child(
            div()
                .absolute()
                .left(px(2.0))
                .right(px(2.0))
                .when(is_pos, |d| d.bottom(px(30.0)).h(px(bar_height)))
                .when(!is_pos, |d| d.top(px(30.0)).h(px(bar_height)))
                .rounded_sm()
                .bg(if enabled && clamped.abs() > 0.1 {
                    red()
                } else if is_light {
                    c(0xA1A1AA)
                } else {
                    c(0x3B3B40)
                }),
        )
}

fn modal_footer(is_light: bool) -> Div {
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .px(px(20.0))
        .py(px(10.0))
        .border_t(px(1.0))
        .border_color(if is_light { c(0xE4E4E7) } else { c(0x17171C) })
        .child(
            div()
                .text_xs()
                .text_color(if is_light { c(0x71717A) } else { white(0.4) })
                .child("Real-time biquad audio engine"),
        )
        .child(
            h_flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded_md()
                                .bg(dynamic_inner_surface(is_light))
                                .border(px(1.0))
                                .border_color(if is_light { c(0xDEDEE1) } else { c(0x2A2A2E) })
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_light { c(0x3F3F46) } else { white(0.65) })
                                .child("Ctrl + E"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if is_light { c(0x71717A) } else { white(0.4) })
                                .child("toggle"),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded_md()
                                .bg(dynamic_inner_surface(is_light))
                                .border(px(1.0))
                                .border_color(if is_light { c(0xDEDEE1) } else { c(0x2A2A2E) })
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_light { c(0x3F3F46) } else { white(0.65) })
                                .child("Escape"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if is_light { c(0x71717A) } else { white(0.4) })
                                .child("close"),
                        ),
                ),
        )
}
