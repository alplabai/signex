//! Signex EDA — AI-first electronics design automation.
#![allow(dead_code, unused_imports)]
//!
//! Entry point for the Iced 0.14 + wgpu application.

mod app;
mod canvas;
mod dock;
mod styles;
mod menu_bar;
mod panels;
mod shortcuts;
mod status_bar;
mod tab_bar;
mod toolbar;
mod tree_view;
mod undo;

use app::Signex;

const IOSEVKA_REGULAR: &[u8] =
    include_bytes!("../assets/fonts/Iosevka-Regular.ttf");
const IOSEVKA_BOLD: &[u8] =
    include_bytes!("../assets/fonts/Iosevka-Bold.ttf");

fn main() -> iced::Result {
    iced::application(Signex::new, Signex::update, Signex::view)
        .title(Signex::title)
        .theme(Signex::theme)
        .subscription(Signex::subscription)
        .window_size(iced::Size::new(1400.0, 900.0))
        .font(IOSEVKA_REGULAR)
        .font(IOSEVKA_BOLD)
        .run()
}
