mod api;
mod app;
mod config;
mod media;
mod views;
mod widgets;

use gpui_kit::component::{Root, Theme, ThemeMode};
use gpui_kit::*;

fn noir(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn native_window_options() -> WindowOptions {
    let mut options = WindowOptions {
        app_id: Some("app.noirplayer.desktop".into()),
        ..Default::default()
    };
    options.titlebar = Some(gpui_kit::TitlebarOptions {
        title: Some("Noir Player".into()),
        ..Default::default()
    });
    #[cfg(target_os = "linux")]
    {
        if let Ok(logo) = image::load_from_memory(include_bytes!("../Noir_Player_Logo.png")) {
            options.icon = Some(std::sync::Arc::new(logo.to_rgba8()));
        }
    }
    options
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);

            // Noir dark theme with the brand red as primary.
            Theme::change(ThemeMode::Dark, None, cx);
            let theme = Theme::global_mut(cx);
            theme.primary = noir(0xE53935);
            theme.primary_foreground = noir(0xFFFFFF);
            theme.background = noir(0x121212);
            theme.foreground = noir(0xFFFFFF);
            theme.popover = noir(0x1E1E1E);
            theme.secondary = noir(0x1E1E1E);
            theme.muted = noir(0x1E1E1E);
            theme.muted_foreground = noir(0xFFFFFF).alpha(0.55);
            theme.border = noir(0x2A2A2A);
            Theme::sync_base(cx);

            cx.spawn(async move |cx| {
                cx.open_window(native_window_options(), |window, cx| {
                    let view = cx.new(|cx| app::NoirPlayerModel::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("Failed to open window");
            })
            .detach();
        });
}
