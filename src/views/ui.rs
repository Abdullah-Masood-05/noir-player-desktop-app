use gpui_kit::component::{h_flex, Theme, ThemeMode};
use gpui_kit::*;

#[path = "motion.rs"]
pub mod motion;

#[allow(unused_imports)]
pub use motion::{
    backdrop_transition, bubble_pop_item, bubbly_pop, bubbly_spring, menu_transition,
    modal_transition, smooth_scroll, tab_transition,
};

pub const RED: u32 = 0xE53935;
pub const BG: u32 = 0x0A0A0C;
pub const SURFACE: u32 = 0x17171C;

/// Left rail behind the navigation.
pub const SIDEBAR_BG: u32 = 0x0C0C0F;
/// Docked panels such as the queue rail.
pub const PANEL_BG: u32 = 0x0D0D11;
/// Raised cards inside a view.
pub const CARD_BG: u32 = 0x151519;

pub const LIGHT_SIDEBAR_BG: u32 = 0xFFFFFF;
pub const LIGHT_PANEL_BG: u32 = 0xFFFFFF;
pub const LIGHT_CARD_BG: u32 = 0xFFFFFF;

pub const LIGHT_BG: u32 = 0xFAFAFA;
pub const LIGHT_SURFACE: u32 = 0xFFFFFF;
pub const LIGHT_FOREGROUND: u32 = 0x1A1A1A;
pub const LIGHT_MUTED: u32 = 0x757575;
pub const LIGHT_BORDER: u32 = 0xE4E4E7;

pub fn apply_theme(mode: &str, cx: &mut App) {
    let is_light = mode.eq_ignore_ascii_case("light");
    if is_light {
        Theme::change(ThemeMode::Light, None, cx);
        let theme = Theme::global_mut(cx);
        theme.primary = rgb(0xE53935).into();
        theme.primary_foreground = rgb(0xFFFFFF).into();
        theme.background = rgb(LIGHT_BG).into();
        theme.foreground = rgb(LIGHT_FOREGROUND).into();
        theme.popover = rgb(LIGHT_SURFACE).into();
        theme.secondary = rgb(0xF0F1F3).into();
        theme.muted = rgb(0xF4F4F5).into();
        theme.muted_foreground = rgb(LIGHT_MUTED).into();
        theme.border = rgb(LIGHT_BORDER).into();
    } else {
        Theme::change(ThemeMode::Dark, None, cx);
        let theme = Theme::global_mut(cx);
        theme.primary = rgb(0xE53935).into();
        theme.primary_foreground = rgb(0xFFFFFF).into();
        theme.background = rgb(BG).into();
        theme.foreground = rgb(0xFFFFFF).into();
        theme.popover = rgb(SURFACE).into();
        theme.secondary = rgb(SURFACE).into();
        theme.muted = rgb(SURFACE).into();
        theme.muted_foreground = rgb(0xFFFFFF).alpha(0.55).into();
        theme.border = rgb(0x2A2A2A).into();
    }
    Theme::sync_base(cx);
}

pub fn red() -> Hsla {
    rgb(RED).into()
}

pub fn red_a(a: f32) -> Hsla {
    let mut c: Hsla = rgb(RED).into();
    c.a = a;
    c
}

pub fn white(a: f32) -> Hsla {
    let mut c: Hsla = rgb(0xFFFFFF).into();
    c.a = a;
    c
}

pub fn bg_color() -> Hsla {
    rgb(BG).into()
}

pub fn dynamic_bg(is_light: bool) -> Hsla {
    if is_light {
        rgb(LIGHT_BG).into()
    } else {
        rgb(BG).into()
    }
}

pub fn dynamic_surface(is_light: bool) -> Hsla {
    if is_light {
        rgb(LIGHT_SURFACE).into()
    } else {
        rgb(SURFACE).into()
    }
}

pub fn dynamic_text(is_light: bool) -> Hsla {
    if is_light {
        rgb(LIGHT_FOREGROUND).into()
    } else {
        rgb(0xFFFFFF).into()
    }
}

pub fn dynamic_subtitle(is_light: bool) -> Hsla {
    if is_light {
        rgb(LIGHT_MUTED).into()
    } else {
        white(0.70)
    }
}

pub fn dynamic_muted(is_light: bool) -> Hsla {
    if is_light {
        rgb(0x8E8E93).into()
    } else {
        white(0.55)
    }
}

pub fn dynamic_border(is_light: bool) -> Hsla {
    if is_light {
        rgb(LIGHT_BORDER).into()
    } else {
        rgb(0x2A2A2A).into()
    }
}

pub fn dynamic_hover(is_light: bool) -> Hsla {
    if is_light {
        rgb(0xF0F1F3).into()
    } else {
        rgb(SURFACE).into()
    }
}

/// Background of the left navigation rail.
pub fn dynamic_sidebar(is_light: bool) -> Hsla {
    if is_light {
        rgb(LIGHT_SIDEBAR_BG).into()
    } else {
        rgb(SIDEBAR_BG).into()
    }
}

/// Background of a docked panel, such as the queue rail.
pub fn dynamic_panel(is_light: bool) -> Hsla {
    if is_light {
        rgb(LIGHT_PANEL_BG).into()
    } else {
        rgb(PANEL_BG).into()
    }
}

/// Background of a raised card inside a view.
pub fn dynamic_card(is_light: bool) -> Hsla {
    if is_light {
        rgb(LIGHT_CARD_BG).into()
    } else {
        rgb(CARD_BG).into()
    }
}

/// A restrained hover tint for dense list rows.
pub fn dynamic_row_hover(is_light: bool) -> Hsla {
    if is_light {
        hsla(0.0, 0.0, 0.0, 0.04)
    } else {
        white(0.05)
    }
}

/// A hairline that separates rows and sections.
pub fn dynamic_divider(is_light: bool) -> Hsla {
    if is_light {
        rgb(0xEDEDF0).into()
    } else {
        white(0.06)
    }
}

/// The crimson glow that washes the bottom of the navigation rail.
pub fn bottom_aura(is_light: bool) -> Div {
    div()
        .absolute()
        .bottom_0()
        .left_0()
        .right_0()
        .h(px(340.0))
        .bg(linear_gradient(
            0.0,
            linear_color_stop(red_a(if is_light { 0.10 } else { 0.16 }), 0.0),
            linear_color_stop(red_a(0.0), 1.0),
        ))
}

/// Deterministic 0..1 noise so decorative waveforms stay stable between frames.
fn noise(seed: u32) -> f32 {
    let mut value = seed.wrapping_mul(2_654_435_761);
    value ^= value >> 15;
    value = value.wrapping_mul(2_246_822_519);
    value ^= value >> 13;
    (value % 1000) as f32 / 1000.0
}

/// A decorative audio waveform. `phase` animates the envelope while a song
/// plays, `intensity` scales the bar heights and colour.
pub fn waveform(
    bars: usize,
    bar_width: f32,
    gap: f32,
    height: f32,
    phase: f32,
    intensity: f32,
) -> Div {
    h_flex()
        .items_center()
        .gap(px(gap))
        .h(px(height))
        .flex_shrink_0()
        .children((0..bars).map(|index| {
            let position = index as f32 / bars.max(1) as f32;
            // A raised-cosine envelope keeps the strip tallest in the middle.
            let envelope = (position * std::f32::consts::PI).sin().powf(0.65);
            let wobble = ((index as f32 * 0.8 + phase * 6.0).sin() * 0.5 + 0.5) * 0.55
                + noise(index as u32 * 7 + 13) * 0.45;
            let value = (envelope * wobble * intensity).clamp(0.05, 1.0);
            div()
                .w(px(bar_width))
                .h(px((height * value).max(2.0)))
                .rounded_full()
                .bg(red_a(0.25 + value * 0.55))
        }))
}

/// Red top wash: rich vibrant crimson in dark mode (matching the classic Noir design),
/// and a warmer, more vibrant crimson wash in light mode.
pub fn top_fade(is_light: bool) -> Div {
    let (start_color, end_color, height) = if is_light {
        (red_a(0.22), rgb(LIGHT_BG).alpha(0.0).into(), 155.0)
    } else {
        (red_a(0.25), bg_color(), 160.0)
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(height))
        .bg(linear_gradient(
            180.0,
            linear_color_stop(start_color, 0.0),
            linear_color_stop(end_color, 1.0),
        ))
}

pub fn icon(kind: gpui_kit::assets::IconName) -> gpui_kit::component::Icon {
    gpui_kit::component::Icon::new(kind)
}

pub fn icon_text(kind: gpui_kit::assets::IconName, size: f32) -> Div {
    div()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .child(icon(kind).size(px(size * 0.66)))
}

pub fn img_from_bytes(bytes: std::sync::Arc<[u8]>) -> Img {
    let format = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        ImageFormat::Png
    } else {
        ImageFormat::Jpeg
    };
    img(std::sync::Arc::new(Image::from_bytes(
        format,
        bytes.to_vec(),
    )))
}

pub fn format_duration(d: std::time::Duration) -> String {
    format!("{}:{:02}", d.as_secs() / 60, d.as_secs() % 60)
}
