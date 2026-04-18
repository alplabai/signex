//! Signex EDA — AI-first electronics design automation.
//!
//! Entry point for the Iced 0.14 + wgpu application.

mod active_bar;
mod app;
mod canvas;
mod diagnostics;
mod dock;
mod find_replace;
mod fonts;
mod menu_bar;
mod panels;
mod pcb_canvas;
mod preferences;
mod shortcuts;
mod status_bar;
mod styles;
mod tab_bar;
mod toolbar;
mod undo;

use app::Signex;

const IOSEVKA_REGULAR: &[u8] = include_bytes!("../assets/fonts/Iosevka-Regular.ttf");
const IOSEVKA_BOLD: &[u8] = include_bytes!("../assets/fonts/Iosevka-Bold.ttf");

fn main() -> iced::Result {
    if let Err(error) = diagnostics::init_logging() {
        eprintln!("[signex] failed to initialize logging: {error:#}");
    }

    // Read the persisted UI font preference (defaults to "Roboto").
    let ui_font_name = fonts::read_ui_font_pref();

    iced::application(Signex::new, Signex::update, Signex::view)
        .title(Signex::title)
        .theme(Signex::theme)
        .subscription(Signex::subscription)
        .window_size(iced::Size::new(1400.0, 900.0))
        // Iosevka is bundled — schematic / PCB canvas text.
        .font(IOSEVKA_REGULAR)
        .font(IOSEVKA_BOLD)
        // UI default font: use whatever is configured (falls back to system
        // sans-serif if the named font is not installed).
        .default_font(iced::Font::with_name(Box::leak(
            ui_font_name.into_boxed_str(),
        )))
        .run()
}
