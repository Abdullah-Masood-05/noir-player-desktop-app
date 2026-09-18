use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

#[path = "motion.rs"]
pub mod motion;

pub use motion::{
    backdrop_transition, modal_transition, selected_highlight, smooth_scroll, tab_transition,
};

pub const RED: u32 = 0xE53935;
pub const BG: u32 = 0x121212;
pub const SURFACE: u32 = 0x1E1E1E;
pub const NAV_BG: u32 = 0x181818;

pub const LIGHT_BG: u32 = 0xFAFAFA;
pub const LIGHT_SURFACE: u32 = 0xFFFFFF;
pub const LIGHT_NAV_BG: u32 = 0xFFFFFF;
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

pub fn surface() -> Hsla {
    rgb(SURFACE).into()
}

pub fn nav_bg() -> Hsla {
    rgb(NAV_BG).into()
}

pub fn dynamic_bg(is_light: bool) -> Hsla {
    if is_light { rgb(LIGHT_BG).into() } else { rgb(BG).into() }
}

pub fn dynamic_surface(is_light: bool) -> Hsla {
    if is_light { rgb(LIGHT_SURFACE).into() } else { rgb(SURFACE).into() }
}

pub fn dynamic_text(is_light: bool) -> Hsla {
    if is_light { rgb(LIGHT_FOREGROUND).into() } else { rgb(0xFFFFFF).into() }
}

pub fn dynamic_subtitle(is_light: bool) -> Hsla {
    if is_light { rgb(LIGHT_MUTED).into() } else { white(0.70) }
}

pub fn dynamic_muted(is_light: bool) -> Hsla {
    if is_light { rgb(0x8E8E93).into() } else { white(0.55) }
}

pub fn dynamic_border(is_light: bool) -> Hsla {
    if is_light { rgb(LIGHT_BORDER).into() } else { rgb(0x2A2A2A).into() }
}

pub fn dynamic_hover(is_light: bool) -> Hsla {
    if is_light { rgb(0xF0F1F3).into() } else { rgb(SURFACE).into() }
}

/// Red top wash: rich vibrant crimson in dark mode (matching the classic Noir design),
/// and a soft transparent crimson wash in light mode.
pub fn top_fade(is_light: bool) -> Div {
    let (start_color, end_color, height) = if is_light {
        (red_a(0.12), rgb(LIGHT_BG).alpha(0.0).into(), 130.0)
    } else {
        (red_a(0.24), bg_color(), 160.0)
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
