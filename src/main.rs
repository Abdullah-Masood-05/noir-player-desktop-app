#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod app;
mod config;
mod media;
mod single_instance;
pub mod update;
mod views;
mod widgets;

use gpui_kit::component::Root;
use gpui_kit::*;

fn native_window_options(cx: &App) -> WindowOptions {
    let mut options = WindowOptions {
        app_id: Some("app.noirplayer.desktop".into()),
        ..Default::default()
    };
    options.titlebar = Some(gpui_kit::TitlebarOptions {
        title: Some("Noir Player".into()),
        ..Default::default()
    });
    // The desktop layout needs room for the rail, the list and the queue.
    options.window_min_size = Some(size(px(940.0), px(600.0)));
    if let Some(display) = cx.primary_display() {
        let available = display.bounds().size;
        let width = (f32::from(available.width) * 0.9).clamp(940.0, 1520.0);
        let height = (f32::from(available.height) * 0.88).clamp(600.0, 940.0);
        options.window_bounds = Some(WindowBounds::centered(size(px(width), px(height)), cx));
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(logo) = image::load_from_memory(include_bytes!("../Noir_Player_Logo.png")) {
            options.icon = Some(std::sync::Arc::new(logo.to_rgba8()));
        }
    }
    options
}

fn main() {
    // A shortcut, taskbar pin or `.desktop` file just runs the executable
    // again; ask any instance already running to come forward instead of
    // opening a second window onto the same library and the same audio
    // device.
    if single_instance::activate_existing() {
        return;
    }
    let activation = single_instance::listen_for_activation();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);

            let store = app::store::Store::default_path()
                .ok()
                .and_then(|p| app::store::Store::load(&p).ok())
                .unwrap_or_default();
            views::ui::apply_theme(&store.theme_mode, cx);

            let options = native_window_options(cx);
            cx.spawn(async move |cx| {
                let window = cx
                    .open_window(options, |window, cx| {
                        let view = cx.new(|cx| app::NoirPlayerModel::new(window, cx));
                        cx.new(|cx| Root::new(view, window, cx))
                    })
                    .expect("Failed to open window");

                if let Some(activation) = activation {
                    while activation.recv().await.is_ok() {
                        let _ = window.update(cx, |_, window, _| window.activate_window());
                    }
                }
            })
            .detach();
        });
}
