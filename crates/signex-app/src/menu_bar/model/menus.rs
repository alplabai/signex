//! The content of each root menu. Labels, order, gating and messages are the
//! menu contract pinned by `testdata/menu_bar.golden.tsv`.

use super::items::{
    cmd, cmd_item, cmd_item_gated, cmd_item_gated_as, cmd_toggle, item, item_if, panel_item, stub,
    submenu, unavailable,
};
use super::{MenuEntry, MenuMessage};
use crate::app::command::enablement::{new_part_menu_enabled, remove_part_menu_enabled};
use crate::app::update_pcb::PcbSyncLabel;
use crate::menu_bar::{MenuContext, enabled};
use crate::panels::PanelKind;

const SEP: MenuEntry = MenuEntry::Separator;

pub(super) fn file(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item(ctx, cmd("new_document", "New Project", "Ctrl+N")),
        // A board for the open project (#1312); greyed with no project.
        submenu(
            "New",
            vec![cmd_item_gated(ctx, cmd("new_pcb_document", "PCB", ""))],
        ),
        cmd_item(ctx, cmd("open_document", "Open...", "Ctrl+O")),
        submenu("Open Recent", open_recent(ctx)),
        SEP,
        cmd_item_gated(
            ctx,
            cmd("close_active_document", "Close Document", "Ctrl+F4"),
        ),
        SEP,
        // Save gating: docs/MENU_BAR.md#file-save
        cmd_item_gated(ctx, cmd("save_document", "Save", "Ctrl+S")),
        cmd_item_gated(ctx, cmd("save_document_as", "Save As...", "Ctrl+Shift+S")),
        // Greyed when nothing is unsaved; runs the shared Save All flow.
        item_if(
            "Save All",
            MenuMessage::SaveAll,
            ctx.commands.has_dirty_documents,
        ),
        SEP,
        // No New Component row here: docs/MENU_BAR.md#file-library
        submenu("Library", library(ctx)),
        SEP,
        submenu(
            "Import",
            vec![item("Altium Project...", MenuMessage::ImportAltium)],
        ),
        // Export > PDF and File > Print open the same modal; only Print shows Ctrl+P: docs/MENU_BAR.md#file-export-pdf
        submenu("Export", export(ctx)),
        cmd_item_gated(ctx, cmd("print", "Print...", "Ctrl+P")),
        SEP,
        item("Exit", MenuMessage::Exit),
    ]
}

/// File ▸ Open Recent: the file name of each project, newest first. A project
/// that has vanished since is still listed; choosing it drops it with a
/// notice (`handle_open_recent_project`).
fn open_recent(ctx: &MenuContext) -> Vec<MenuEntry> {
    let mut entries: Vec<MenuEntry> = ctx
        .recent_projects
        .iter()
        .map(|path| {
            let label = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            item(&label, MenuMessage::OpenRecentProject(path.clone()))
        })
        .collect();
    if entries.is_empty() {
        entries.push(unavailable("No recent projects"));
    }
    entries.push(SEP);
    entries.push(item_if(
        "Clear Recent",
        MenuMessage::ClearRecentProjects,
        !ctx.recent_projects.is_empty(),
    ));
    entries
}

fn library(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        item("Open Library...", MenuMessage::LibraryOpenLibrary),
        SEP,
        item_if(
            "Place Component...",
            MenuMessage::LibraryPlaceComponent,
            ctx.commands.has_schematic,
        ),
    ]
}

fn export(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        // No shortcut: Ctrl+P is shown on File > Print, which opens this
        // same modal. Gated like Print.
        item_if("PDF…", MenuMessage::ExportPdf, enabled(ctx, "print")),
    ]
}

pub(super) fn edit(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item_gated(ctx, cmd("undo", "Undo", "Ctrl+Z")),
        cmd_item_gated(ctx, cmd("redo", "Redo", "Ctrl+Y")),
        SEP,
        cmd_item_gated(ctx, cmd("cut", "Cut", "Ctrl+X")),
        cmd_item_gated(ctx, cmd("copy", "Copy", "Ctrl+C")),
        cmd_item_gated(ctx, cmd("paste", "Paste", "Ctrl+V")),
        cmd_item_gated(ctx, cmd("smart_paste", "Paste Special", "Shift+Ctrl+V")),
        cmd_item_gated(ctx, cmd("duplicate", "Duplicate", "Ctrl+D")),
        cmd_item_gated(ctx, cmd("delete_selection", "Delete", "Del")),
        submenu("Move", move_menu(ctx)),
        submenu("Arrange", arrange(ctx)),
        SEP,
        cmd_item_gated(ctx, cmd("select_all", "Select All", "Ctrl+A")),
        SEP,
        cmd_item_gated(ctx, cmd("find", "Find", "Ctrl+F")),
        cmd_item_gated(ctx, cmd("find_and_replace", "Find and Replace", "Ctrl+H")),
    ]
}

/// Edit ▸ Move: move, turn and mirror the selection, on a sheet and on a
/// board (#2240). Flip Selection puts footprints on the other board side.
/// Gated by the commands, so a locked footprint greys the board rows.
fn move_menu(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item_gated(ctx, cmd("move_object", "Move", "")),
        cmd_item_gated(ctx, cmd("move_selection", "Move Selection", "")),
        SEP,
        cmd_item_gated(ctx, cmd("rotate_counterclockwise", "Rotate", "Space")),
        cmd_item_gated(
            ctx,
            cmd("rotate_clockwise", "Rotate Clockwise", "Shift+Space"),
        ),
        cmd_item_gated(ctx, cmd("mirror_x", "Mirror X", "X")),
        cmd_item_gated(ctx, cmd("mirror_y", "Mirror Y", "Y")),
        cmd_item_gated(ctx, cmd("flip_selection", "Flip Selection", "")),
    ]
}

/// Edit ▸ Arrange: order the selected drawings. Circuit elements in the
/// selection are ignored, and with no drawing selected the rows are greyed.
/// No default shortcut, so these are plain rows gated by their commands; a
/// binding the user adds in the keymap still runs the command.
fn arrange(ctx: &MenuContext) -> Vec<MenuEntry> {
    let row = |label: &str, command: &str, action: MenuMessage| {
        item_if(label, action, enabled(ctx, command))
    };
    vec![
        row(
            "Bring to Front",
            "bring_drawings_to_front",
            MenuMessage::BringDrawingsToFront,
        ),
        row(
            "Send to Back",
            "send_drawings_to_back",
            MenuMessage::SendDrawingsToBack,
        ),
        row(
            "Bring Forward",
            "bring_drawings_forward",
            MenuMessage::BringDrawingsForward,
        ),
        row(
            "Send Backward",
            "send_drawings_backward",
            MenuMessage::SendDrawingsBackward,
        ),
    ]
}

pub(super) fn view(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item_gated(ctx, cmd("zoom_in_at_cursor", "Zoom In", "Ctrl+=")),
        cmd_item_gated(ctx, cmd("zoom_out_at_cursor", "Zoom Out", "Ctrl+-")),
        cmd_item_gated(ctx, cmd("zoom_to_fit", "Fit All", "Home")),
        SEP,
        cmd_item_gated(
            ctx,
            cmd("toggle_visible_grid", "Toggle Grid", "Shift+Ctrl+G"),
        ),
        cmd_item_gated(ctx, cmd("next_grid", "Cycle Grid Size", "")),
        cmd_item_gated(
            ctx,
            cmd("toggle_auto_focus", "AutoFocus (dim unselected)", "F9"),
        ),
        SEP,
        // The board view: greyed unless a PCB is open.
        cmd_item_gated(ctx, cmd("toggle_ratsnest", "Toggle Ratsnest", "")),
        cmd_item_gated(ctx, cmd("next_layer", "Next Layer", "")),
        cmd_item_gated(ctx, cmd("prev_layer", "Previous Layer", "")),
        cmd_item_gated(
            ctx,
            cmd("toggle_layer_visibility", "Toggle Layer Visibility", ""),
        ),
        cmd_item_gated(ctx, cmd("show_all_layers", "Show All Layers", "")),
        SEP,
        // Panels work without an active document (they show an empty
        // state), so these are always available.
        panel_item(
            ctx,
            "Projects",
            PanelKind::Projects,
            MenuMessage::OpenProjectsPanel,
        ),
        panel_item(
            ctx,
            "Components",
            PanelKind::Components,
            MenuMessage::OpenComponentsPanel,
        ),
        panel_item(
            ctx,
            "Navigator",
            PanelKind::Navigator,
            MenuMessage::OpenNavigatorPanel,
        ),
        panel_item(
            ctx,
            "Properties",
            PanelKind::Properties,
            MenuMessage::OpenPropertiesPanel,
        ),
        panel_item(ctx, "ERC", PanelKind::Erc, MenuMessage::OpenErcPanel),
        panel_item(
            ctx,
            "Messages",
            PanelKind::Messages,
            MenuMessage::OpenMessagesPanel,
        ),
        panel_item(
            ctx,
            "Signex AI",
            PanelKind::SignexAi,
            MenuMessage::OpenSignexAiPanel,
        ),
        panel_item(
            ctx,
            "Layers",
            PanelKind::Layers,
            MenuMessage::OpenLayersPanel,
        ),
    ]
}

pub(super) fn place(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item_gated(ctx, cmd("place_wire", "Wire", "W")),
        cmd_item_gated(ctx, cmd("place_bus", "Bus", "B")),
        cmd_item_gated(ctx, cmd("place_net_label", "Net Label", "L")),
        SEP,
        cmd_item_gated(ctx, cmd("open_components_panel", "Component...", "P")),
        cmd_item_gated(ctx, cmd("place_power_symbol", "Power Port", "P")),
        cmd_item_gated(ctx, cmd("place_junction", "Junction", "J")),
        SEP,
        item_if("Text", MenuMessage::PlaceText, ctx.commands.has_schematic),
        item_if(
            "No Connect",
            MenuMessage::PlaceNoConnect,
            ctx.commands.has_schematic,
        ),
        cmd_item_gated(ctx, cmd("place_sheet_entry", "Sheet Entry", "")),
    ]
}

pub(super) fn design(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        update_pcb_row(ctx),
        // #2234: enter or leave the sketch of the board in view; greyed
        // with no PCB.
        cmd_item_gated(ctx, cmd("board_sketch", "Board Sketch", "")),
        SEP,
        cmd_item_gated(
            ctx,
            cmd("open_parameter_manager", "Parameter Manager...", ""),
        ),
    ]
}

/// Design ▸ Update PCB Document `<board>` on a schematic, Import Changes
/// From `<project>` on a board (#1312): one command, worded for the
/// document in view.
fn update_pcb_row(ctx: &MenuContext) -> MenuEntry {
    use signex_i18n::{ids, t};
    let i18n = &ctx.i18n;
    let label = match &ctx.pcb_sync {
        Some(PcbSyncLabel::UpdateBoard(Some(board))) => {
            t!(i18n, ids::UPDATE_PCB_MENU_UPDATE_BOARD, "board" => board.as_str())
        }
        Some(PcbSyncLabel::ImportFrom(project)) => {
            t!(i18n, ids::UPDATE_PCB_MENU_IMPORT, "project" => project.as_str())
        }
        Some(PcbSyncLabel::UpdateBoard(None)) | None => t!(i18n, ids::UPDATE_PCB_MENU_UPDATE),
    };
    cmd_item_gated_as(
        ctx,
        cmd("update_pcb_from_schematic", "Update PCB Document", ""),
        label.into_string(),
    )
}

pub(super) fn project(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        // F8 (`run_erc`) runs ERC at once; this row opens the dialog.
        cmd_item_gated(ctx, cmd("open_erc_dialog", "Electrical Rules Check", "")),
        SEP,
        // Gated like Close Project: with no project there is no tab to open.
        item_if(
            "Project Options...",
            MenuMessage::OpenProjectOptions,
            ctx.commands.has_open_project,
        ),
        SEP,
        item_if(
            "Close Project",
            MenuMessage::CloseProject,
            ctx.commands.has_open_project,
        ),
    ]
}

/// Reports: every output of the open schematic. The BOM row is the one
/// working BOM dialog (#1251); the old Design > Generate BOM duplicate row is folded in.
pub(super) fn reports(ctx: &MenuContext) -> Vec<MenuEntry> {
    let has_schematic = ctx.commands.has_schematic;
    vec![
        item_if("Bill of Materials…", MenuMessage::ExportBom, has_schematic),
        item_if(
            "Netlist (Protel .NET)...",
            MenuMessage::ExportNetlist,
            has_schematic,
        ),
        item_if("ERC Report…", MenuMessage::ExportErcReport, has_schematic),
    ]
}

/// Tools ▸ Annotation. The first three rows need an open schematic:
/// annotating without one is meaningless.
fn annotation(ctx: &MenuContext) -> Vec<MenuEntry> {
    let has_schematic = ctx.commands.has_schematic;
    vec![
        item_if(
            "Annotate Schematics...",
            MenuMessage::Annotate,
            has_schematic,
        ),
        item_if(
            "Reset Schematic Designators...",
            MenuMessage::AnnotateReset,
            has_schematic,
        ),
        item_if(
            "Reset Duplicate Schematic Designators...",
            MenuMessage::AnnotateResetDuplicates,
            has_schematic,
        ),
        SEP,
        cmd_item_gated(
            ctx,
            cmd(
                "annotate_schematic_quietly",
                "Annotate Schematics Quietly",
                "Alt+A",
            ),
        ),
        cmd_item_gated(
            ctx,
            cmd(
                "force_annotate_all_schematics",
                "Force Annotate All Schematics",
                "Shift+Alt+A",
            ),
        ),
        SEP,
        stub("Back Annotate Schematics..."),
        stub("Number Schematic Sheets..."),
    ]
}

pub(super) fn tools(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        submenu("Annotation", annotation(ctx)),
        SEP,
        stub("Assign Footprints"),
        // Opens or focuses a Library Browser tab: docs/MENU_BAR.md#tools-library-editor
        cmd_item_gated(ctx, cmd("open_library_browser", "Library Editor", "")),
        SEP,
        // Runs `run_drc` on the PCB in view; greyed with no PCB open.
        cmd_item_gated(ctx, cmd("run_drc", "Design Rule Check", "")),
        stub("Net Inspector"),
        SEP,
        // #1312: Cross Probe shows the selection on the other side; Cross
        // Select Mode (checked while on) mirrors every selection there.
        cmd_item_gated(ctx, cmd("cross_probe", "Cross Probe", "")),
        cmd_toggle(
            ctx,
            cmd(
                "toggle_cross_select_mode",
                "Cross Select Mode",
                "Ctrl+Shift+X",
            ),
            ctx.cross_select,
        ),
        SEP,
        item_if(
            "Up Hierarchy",
            MenuMessage::AscendToParentSheet,
            enabled(ctx, "navigate_up_hierarchy"),
        ),
        SEP,
        // Multi-part flow: acts only on a `.snxsym` editor tab, and Remove
        // Part only on a part after the first.
        item_if(
            "New Part",
            MenuMessage::ToolsNewPart,
            new_part_menu_enabled(&ctx.commands),
        ),
        item_if(
            "Remove Part",
            MenuMessage::ToolsRemovePart,
            remove_part_menu_enabled(&ctx.commands),
        ),
        SEP,
        // Opens the options of the library the active editor tab is in.
        item_if(
            "Document Options...",
            MenuMessage::ToolsDocumentOptions,
            ctx.commands.has_library_editor,
        ),
        SEP,
        item(
            "Passive Network Calculator...",
            MenuMessage::OpenPassiveCalculator,
        ),
        SEP,
        cmd_item(ctx, cmd("open_preferences", "Preferences...", "Ctrl+,")),
    ]
}

pub(super) fn window(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        stub("Tile Horizontally"),
        stub("Tile Vertically"),
        SEP,
        cmd_item_gated(ctx, cmd("close_all_documents", "Close All Documents", "")),
    ]
}

pub(super) fn help(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        item("About Signex", MenuMessage::OpenAbout),
        item("Documentation", MenuMessage::OpenDocumentation),
        item("Report an Issue", MenuMessage::ReportIssue),
        item("Open Sample Project", MenuMessage::OpenSampleProject),
        SEP,
        cmd_item(
            ctx,
            cmd("show_current_command_hotkeys", "Keyboard Shortcuts", "F1"),
        ),
    ]
}
