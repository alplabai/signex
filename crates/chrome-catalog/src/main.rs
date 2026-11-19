mod board_palette;
mod bom_modal;
mod catalog;
mod chips;
mod compact_group;
mod constraint_grid;
mod dimension_label;
mod enable_section;
mod flow_progress;
mod graphite_tokens;
mod icon;
mod illustration;
mod locale;
mod modal_card;
mod project_tree;
mod section;
mod tabs;
mod target_band;
mod theme;
mod theme_picker;
mod theme_pill;

fn main() -> iced::Result {
    catalog::run()
}
