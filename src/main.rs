// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cli;
mod compare;
mod core;
mod games;
mod mods;
mod pages;
mod setup;
mod sound;
mod textmod;
mod patches;
mod theme;
mod tweaks;
mod ui;
mod workspace;

use std::borrow::Cow;

use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowOptions, prelude::*, px, size,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = cli::run(&args) {
        std::process::exit(code);
    }
    Application::new().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(theme::FONT_FILES.iter().map(|f| Cow::Borrowed(*f)).collect())
            .expect("bundled fonts should load");
        theme::set_palette(workspace::AppSettings::load().palette);

        let bounds = Bounds::centered(None, size(px(1380.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Vault Patcher".into()),
                    appears_transparent: true,
                    traffic_light_position: None,
                }),
                window_min_size: Some(size(px(1100.), px(700.))),
                window_background: WindowBackgroundAppearance::Opaque,
                app_id: Some("VaultPatcher".into()),
                ..Default::default()
            },
            |_, cx| cx.new(app::Shell::new),
        )
        .expect("failed to open the main window");
        cx.activate(true);
    });
}
