//! Signex EDA — AI-first electronics design automation.
//!
//! Entry point for the Iced 0.14 + wgpu application.

mod active_bar;
mod app;
mod canvas;
mod chrome;
mod diagnostics;
mod dock;
mod find_replace;
mod fonts;
mod icons;
mod library;
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
const ROBOTO_REGULAR: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
const ROBOTO_BOLD: &[u8] = include_bytes!("../assets/fonts/Roboto-Bold.ttf");

fn main() -> iced::Result {
    if let Err(error) = diagnostics::init_logging() {
        eprintln!("[signex] failed to initialize logging: {error:#}");
    }

    // Read the persisted UI font preference (defaults to "Roboto").
    let ui_font_name = fonts::read_ui_font_pref();

    iced::daemon(Signex::new, Signex::update, Signex::view)
        .title(Signex::title)
        .theme(Signex::theme)
        .subscription(Signex::subscription)
        // Iosevka — schematic / PCB canvas text (monospace, tuned for EDA).
        .font(IOSEVKA_REGULAR)
        .font(IOSEVKA_BOLD)
        // Roboto — UI chrome (panels, toolbars, menus, dialogs).
        .font(ROBOTO_REGULAR)
        .font(ROBOTO_BOLD)
        // Default UI font resolves through the Preferences-panel pick. When
        // it matches a bundled family ("Roboto" / "Iosevka") iced uses the
        // embedded TTF directly; otherwise it falls back to a system font.
        .default_font(iced::Font::with_name(Box::leak(
            ui_font_name.into_boxed_str(),
        )))
        .run()
}
