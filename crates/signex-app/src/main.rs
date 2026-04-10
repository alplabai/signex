//! Signex EDA — AI-first electronics design automation.
#![allow(dead_code, unused_imports)]
//!
//! Entry point for the Iced 0.14 + wgpu application.

mod app;
mod canvas;
mod dock;
mod menu_bar;
mod panels;
mod shortcuts;
mod status_bar;
mod tab_bar;
mod toolbar;
mod tree_view;

use app::Signex;

fn main() -> iced::Result {
    iced::application(Signex::new, Signex::update, Signex::view)
        .title(Signex::title)
        .theme(Signex::theme)
        .subscription(Signex::subscription)
        .window_size(iced::Size::new(1400.0, 900.0))
        .run()
}
