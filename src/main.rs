#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod actions;
mod album_color;
mod app;
mod artwork;
mod audio;
mod audio_spectrum;
mod bilibili;
mod components;
mod download;
mod formatting;
mod kugou;
mod local_music;
mod locale;
mod lyric_finder;
mod lyrics;
mod mode;
mod motion;
mod qqmusic;
mod shitease;
mod storage;
mod track;
mod updater;
mod windowing;
mod youtube;

use gpui_kit::{
    component::{Root, Theme, ThemeMode},
    *,
};

fn main() {
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        log::error!("CAPS: could not start its async runtime, exiting");
        return;
    };
    let _runtime_guard = runtime.enter();
    audio_spectrum::start_monitor();
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_kit::component::set_locale(&storage::load_state().settings.language);
            Theme::change(ThemeMode::Dark, None, cx);
            // Prefer the platform's own UI face, in the order Windows itself
            // uses. `Arial` used to win this list by accident, which is why the
            // capsule rendered like a document instead of like the system.
            let available = cx.text_system().all_font_names();
            let family = [
                "Segoe UI Variable Text",
                "Segoe UI Variable Display",
                "Segoe UI Variable",
                "Segoe UI",
                "Microsoft YaHei UI",
                "MiSans",
                "Noto Sans",
                "Arial",
            ]
            .into_iter()
            .find(|name| available.iter().any(|font| font == name))
            .unwrap_or("Segoe UI");
            cx.global_mut::<Theme>().font_family = family.into();
            let options = windowing::options(cx);
            let opened = cx.open_window(options, |window, cx| {
                window.set_window_title("CAPS");
                // The popup is a rectangle whose visible shapes are smaller
                // than its bounds. Its margins must carry real per-pixel alpha,
                // or the rounded, antialiased edges would composite against an
                // opaque plate instead of the desktop. (`windowing` hands the
                // painted shape to the system as a window region as well, which
                // is what keeps the non-client frame out of the picture.)
                window.set_background_appearance(WindowBackgroundAppearance::Transparent);
                windowing::install_mouse_handling(window);
                let view = cx.new(|cx| app::Caps::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx).bg(rgba(0x00000000)))
            });
            if let Err(err) = opened {
                log::error!("CAPS: could not open its window: {err}");
            }
        });
}
