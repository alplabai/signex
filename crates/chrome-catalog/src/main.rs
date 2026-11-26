mod board_palette;
mod bom_modal;
mod calculator;
mod catalog;
mod chips;
mod compact_group;
mod constraint_grid;
mod dimension_label;
mod doc_tab_shell;
mod enable_section;
mod flow_progress;
mod graphite_tokens;
mod history_graph;
mod icon;
mod illustration;
mod locale;
mod modal_card;
mod project_tree;
mod section;
mod stat_tiles;
mod tabs;
mod target_band;
mod theme;
mod theme_picker;
mod theme_pill;

fn main() -> iced::Result {
    catalog::run()
}
