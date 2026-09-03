//! Shared Noir theme helpers: brand palette, icon and image helpers.
use gpui_kit::*;

#[path = "motion.rs"]
pub mod motion;

pub use motion::{selected_highlight, smooth_scroll, tab_transition};

pub const RED: u32 = 0xE53935;
pub const BG: u32 = 0x121212;
pub const SURFACE: u32 = 0x1E1E1E;
pub const NAV_BG: u32 = 0x181818;

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
