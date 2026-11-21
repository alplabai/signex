//! The embedded `.ftl` sources and the locales they make available.

/// The source language: every other locale falls back to it.
pub(crate) const EN_US: &str = "en-US";
/// The pseudolocale: `en-US` text, accented and lengthened, to find strings
/// that are hardcoded or that overflow.
pub(crate) const EN_XA: &str = "en-XA";

pub(crate) const EN_US_SOURCES: [&str; 54] = [
    include_str!("../locales/en-US/back_annotate.ftl"),
    include_str!("../locales/en-US/compare_board.ftl"),
    include_str!("../locales/en-US/board_query.ftl"),
    include_str!("../locales/en-US/board_status.ftl"),
    include_str!("../locales/en-US/board_view.ftl"),
    include_str!("../locales/en-US/board_sketch.ftl"),
    include_str!("../locales/en-US/board-checks.ftl"),
    include_str!("../locales/en-US/cross_probe.ftl"),
    include_str!("../locales/en-US/drc.ftl"),
    include_str!("../locales/en-US/drc_rules.ftl"),
    include_str!("../locales/en-US/menu.ftl"),
    include_str!("../locales/en-US/commands.ftl"),
    include_str!("../locales/en-US/pcb.ftl"),
    include_str!("../locales/en-US/pcb-board.ftl"),
    include_str!("../locales/en-US/pcb_component.ftl"),
    include_str!("../locales/en-US/pcb_primitive.ftl"),
    include_str!("../locales/en-US/pcb_stack.ftl"),
    include_str!("../locales/en-US/language.ftl"),
    include_str!("../locales/en-US/dialog.ftl"),
    include_str!("../locales/en-US/overlay.ftl"),
    include_str!("../locales/en-US/properties.ftl"),
    include_str!("../locales/en-US/preferences.ftl"),
    include_str!("../locales/en-US/new-project.ftl"),
    include_str!("../locales/en-US/document-setup.ftl"),
    include_str!("../locales/en-US/project.ftl"),
    include_str!("../locales/en-US/sheet-numbering.ftl"),
    include_str!("../locales/en-US/sheet_sync.ftl"),
    include_str!("../locales/en-US/library.ftl"),
    include_str!("../locales/en-US/editors.ftl"),
    include_str!("../locales/en-US/panels.ftl"),
    include_str!("../locales/en-US/update_pcb.ftl"),
    include_str!("../locales/en-US/keys.ftl"),
    include_str!("../locales/en-US/notice.ftl"),
    include_str!("../locales/en-US/file_dialog.ftl"),
    include_str!("../locales/en-US/fab_outputs.ftl"),
    include_str!("../locales/en-US/board_output.ftl"),
    include_str!("../locales/en-US/rules.ftl"),
    include_str!("../locales/en-US/dialog_text.ftl"),
    include_str!("../locales/en-US/bom.ftl"),
    include_str!("../locales/en-US/pdf_preview.ftl"),
    include_str!("../locales/en-US/find.ftl"),
    include_str!("../locales/en-US/chrome.ftl"),
    include_str!("../locales/en-US/placeholder.ftl"),
    include_str!("../locales/en-US/pcb_graphics.ftl"),
    include_str!("../locales/en-US/board_panel.ftl"),
    include_str!("../locales/en-US/panel_body.ftl"),
    include_str!("../locales/en-US/requirements.ftl"),
    include_str!("../locales/en-US/command_needs.ftl"),
    include_str!("../locales/en-US/gerber_viewer.ftl"),
    include_str!("../locales/en-US/passive_calculator.ftl"),
    include_str!("../locales/en-US/command_log.ftl"),
    include_str!("../locales/en-US/annotations.ftl"),
    include_str!("../locales/en-US/rooms.ftl"),
    include_str!("../locales/en-US/workspace_layouts.ftl"),
];

/// Every locale negotiation may pick, in language-code order.
pub(crate) const AVAILABLE: [&str; 2] = [EN_US, EN_XA];

/// The `.ftl` sources of `tag`; the pseudolocale reads the `en-US` ones.
pub(crate) fn sources(tag: &str) -> &'static [&'static str] {
    match tag {
        EN_US | EN_XA => &EN_US_SOURCES,
        _ => &[],
    }
}

/// The locales a user can pick, in language-code order. The pseudolocale is
/// a developer aid, so a release build leaves it out of the list; it stays
/// selectable by message and still resolves when saved.
pub(crate) fn listed() -> Vec<&'static str> {
    AVAILABLE
        .into_iter()
        .filter(|tag| *tag != EN_XA || cfg!(debug_assertions))
        .collect()
}
