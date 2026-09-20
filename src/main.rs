#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod app;
mod config;
mod media;
pub mod update;
mod views;
mod widgets;

use gpui_kit::component::Root;
use gpui_kit::*;

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

            let store = app::store::Store::default_path()
                .ok()
                .and_then(|p| app::store::Store::load(&p).ok())
                .unwrap_or_default();
            views::ui::apply_theme(&store.theme_mode, cx);

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
