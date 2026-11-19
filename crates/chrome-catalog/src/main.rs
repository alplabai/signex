mod board_palette;
mod bom_modal;
mod catalog;
mod graphite_tokens;
mod enable_section;
mod chips;
mod dimension_label;
mod icon;
mod illustration;
mod modal_card;
mod project_tree;
mod section;
mod tabs;
mod theme;
mod theme_picker;
mod theme_pill;

fn main() -> iced::Result {
    catalog::run()
}
