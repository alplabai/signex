//! The content of each root menu. Labels, order, gating and messages are the
//! menu contract pinned by `testdata/menu_bar.golden.tsv`.

use super::items::{
    cmd, cmd_item, cmd_item_gated, cmd_item_gated_as, cmd_toggle, item, item_if, item_if_command,
    item_named, panel_item, submenu, unavailable,
};
use super::{MenuEntry, MenuMessage};
use crate::app::command::enablement::{new_part_menu_enabled, remove_part_menu_enabled};
use crate::app::update_pcb::PcbSyncLabel;
use crate::menu_bar::{MenuContext, enabled};
use crate::panels::PanelKind;
use signex_i18n::{ids, menu_ids};

const SEP: MenuEntry = MenuEntry::Separator;

pub(super) fn file(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item(ctx, cmd("new_document", "Mod+N")),
        // A board (#1312) or a sheet (#2232) for the open project, each
        // through its setup dialog; greyed with no project.
        submenu(
            ctx,
            menu_ids::MENU_FILE_NEW,
            vec![
                cmd_item_gated(ctx, cmd("new_schematic_sheet", "")),
                cmd_item_gated(ctx, cmd("new_pcb_document", "")),
            ],
        ),
        cmd_item(ctx, cmd("open_document", "Mod+O")),
        submenu(ctx, menu_ids::MENU_FILE_OPEN_RECENT, open_recent(ctx)),
        SEP,
        cmd_item_gated(ctx, cmd("close_active_document", "Ctrl+F4")),
        SEP,
        // Save gating: docs/MENU_BAR.md#file-save
        cmd_item_gated(ctx, cmd("save_document", "Mod+S")),
        cmd_item_gated(ctx, cmd("save_document_as", "Mod+Shift+S")),
        // Greyed when nothing is unsaved; runs the shared Save All flow.
        item_if(
            ctx,
            menu_ids::MENU_FILE_SAVE_ALL,
            MenuMessage::SaveAll,
            ctx.commands.has_dirty_documents,
        ),
        SEP,
        // No New Component row here: docs/MENU_BAR.md#file-library
        submenu(ctx, menu_ids::MENU_FILE_LIBRARY, library(ctx)),
        SEP,
        submenu(
            ctx,
            menu_ids::MENU_FILE_IMPORT,
            vec![item(
                ctx,
                menu_ids::MENU_FILE_IMPORT_ALTIUM,
                MenuMessage::ImportAltium,
            )],
        ),
        // Export > PDF and File > Print open the same modal; only Print shows Mod+P: docs/MENU_BAR.md#file-export-pdf
        submenu(ctx, menu_ids::MENU_FILE_EXPORT, export(ctx)),
        cmd_item_gated(ctx, cmd("print", "Mod+P")),
        SEP,
        item(ctx, menu_ids::MENU_FILE_EXIT, MenuMessage::Exit),
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
            item_named(label, MenuMessage::OpenRecentProject(path.clone()))
        })
        .collect();
    if entries.is_empty() {
        entries.push(unavailable(ctx, ids::START_NO_RECENT_TITLE));
    }
    entries.push(SEP);
    entries.push(item_if(
        ctx,
        menu_ids::MENU_FILE_CLEAR_RECENT,
        MenuMessage::ClearRecentProjects,
        !ctx.recent_projects.is_empty(),
    ));
    entries
}

fn library(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        item(
            ctx,
            menu_ids::MENU_FILE_OPEN_LIBRARY,
            MenuMessage::LibraryOpenLibrary,
        ),
        SEP,
        item_if(
            ctx,
            menu_ids::MENU_FILE_PLACE_COMPONENT,
            MenuMessage::LibraryPlaceComponent,
            ctx.commands.has_schematic,
        ),
    ]
}

fn export(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        // No shortcut: Ctrl+P is shown on File > Print, which opens this
        // same modal. Gated like Print.
        item_if(
            ctx,
            menu_ids::MENU_FILE_EXPORT_PDF,
            MenuMessage::ExportPdf,
            enabled(ctx, "print"),
        ),
    ]
}

pub(super) fn edit(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item_gated(ctx, cmd("undo", "Mod+Z")),
        cmd_item_gated(ctx, cmd("redo", "Ctrl+Y")),
        SEP,
        cmd_item_gated(ctx, cmd("cut", "Mod+X")),
        cmd_item_gated(ctx, cmd("copy", "Mod+C")),
        cmd_item_gated(ctx, cmd("paste", "Mod+V")),
        // Trunk spelling kept (not `Mod+`): the fallback is never shown while a keymap is active.
        cmd_item_gated(ctx, cmd("smart_paste", "Shift+Ctrl+V")),
        cmd_item_gated(ctx, cmd("duplicate", "Mod+D")),
        cmd_item_gated(ctx, cmd("delete_selection", "Del")),
        submenu(ctx, menu_ids::MENU_EDIT_MOVE, move_menu(ctx)),
        submenu(ctx, menu_ids::MENU_EDIT_ARRANGE, arrange(ctx)),
        SEP,
        cmd_item_gated(ctx, cmd("select_all", "Mod+A")),
        SEP,
        cmd_item_gated(ctx, cmd("find", "Mod+F")),
        cmd_item_gated(ctx, cmd("find_and_replace", "Ctrl+H")),
    ]
}

/// Edit ▸ Move: move, turn and mirror the selection, on a sheet and on a
/// board (#2240). Flip Selection puts footprints on the other board side.
/// Gated by the commands, so a locked footprint greys the board rows.
fn move_menu(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item_gated(ctx, cmd("move_object", "")),
        cmd_item_gated(ctx, cmd("move_selection", "")),
        SEP,
        cmd_item_gated(ctx, cmd("rotate_counterclockwise", "Space")),
        cmd_item_gated(ctx, cmd("rotate_clockwise", "Shift+Space")),
        cmd_item_gated(ctx, cmd("mirror_x", "X")),
        cmd_item_gated(ctx, cmd("mirror_y", "Y")),
        cmd_item_gated(ctx, cmd("flip_selection", "")),
    ]
}

/// Edit ▸ Arrange: order the selected drawings. Circuit elements in the
/// selection are ignored, and with no drawing selected the rows are greyed.
/// No default shortcut, so these are plain rows gated by their commands; a
/// binding the user adds in the keymap still runs the command.
fn arrange(ctx: &MenuContext) -> Vec<MenuEntry> {
    let row = |command: &str, action: MenuMessage| {
        item_if_command(ctx, command, action, enabled(ctx, command))
    };
    vec![
        row("bring_drawings_to_front", MenuMessage::BringDrawingsToFront),
        row("send_drawings_to_back", MenuMessage::SendDrawingsToBack),
        row("bring_drawings_forward", MenuMessage::BringDrawingsForward),
        row("send_drawings_backward", MenuMessage::SendDrawingsBackward),
    ]
}

pub(super) fn view(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item_gated(ctx, cmd("zoom_in_at_cursor", "Ctrl+=")),
        cmd_item_gated(ctx, cmd("zoom_out_at_cursor", "Ctrl+-")),
        cmd_item_gated(ctx, cmd("zoom_to_fit", "Home")),
        SEP,
        // Trunk spelling kept (not `Mod+`): the fallback is never shown while a keymap is active.
        cmd_item_gated(ctx, cmd("toggle_visible_grid", "Shift+Ctrl+G")),
        cmd_item_gated(ctx, cmd("next_grid", "")),
        cmd_item_gated(ctx, cmd("toggle_auto_focus", "F9")),
        SEP,
        // The board view: greyed unless a PCB is open.
        cmd_item_gated(ctx, cmd("toggle_ratsnest", "")),
        cmd_item_gated(ctx, cmd("next_layer", "")),
        cmd_item_gated(ctx, cmd("prev_layer", "")),
        cmd_item_gated(ctx, cmd("toggle_layer_visibility", "")),
        cmd_item_gated(ctx, cmd("show_all_layers", "")),
        SEP,
        // Panels work without an active document (they show an empty
        // state), so these are always available.
        panel_item(
            ctx,
            ids::PANELS_KIND_PROJECTS,
            PanelKind::Projects,
            MenuMessage::OpenProjectsPanel,
        ),
        panel_item(
            ctx,
            ids::PANELS_KIND_COMPONENTS,
            PanelKind::Components,
            MenuMessage::OpenComponentsPanel,
        ),
        panel_item(
            ctx,
            ids::PANELS_KIND_NAVIGATOR,
            PanelKind::Navigator,
            MenuMessage::OpenNavigatorPanel,
        ),
        panel_item(
            ctx,
            ids::PANELS_KIND_PROPERTIES,
            PanelKind::Properties,
            MenuMessage::OpenPropertiesPanel,
        ),
        panel_item(
            ctx,
            ids::PANELS_KIND_ERC,
            PanelKind::Erc,
            MenuMessage::OpenErcPanel,
        ),
        panel_item(
            ctx,
            ids::PANELS_KIND_MESSAGES,
            PanelKind::Messages,
            MenuMessage::OpenMessagesPanel,
        ),
        panel_item(
            ctx,
            ids::PANELS_KIND_SIGNEX_AI,
            PanelKind::SignexAi,
            MenuMessage::OpenSignexAiPanel,
        ),
        panel_item(
            ctx,
            ids::PANELS_KIND_LAYERS,
            PanelKind::Layers,
            MenuMessage::OpenLayersPanel,
        ),
    ]
}

pub(super) fn place(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item_gated(ctx, cmd("place_wire", "W")),
        cmd_item_gated(ctx, cmd("place_bus", "B")),
        cmd_item_gated(ctx, cmd("place_net_label", "L")),
        SEP,
        cmd_item_gated(ctx, cmd("open_components_panel", "P")),
        cmd_item_gated(ctx, cmd("place_power_symbol", "P")),
        cmd_item_gated(ctx, cmd("place_junction", "J")),
        SEP,
        item_if(
            ctx,
            menu_ids::MENU_PLACE_TEXT,
            MenuMessage::PlaceText,
            ctx.commands.has_schematic,
        ),
        item_if(
            ctx,
            menu_ids::MENU_PLACE_NO_CONNECT,
            MenuMessage::PlaceNoConnect,
            ctx.commands.has_schematic,
        ),
        cmd_item_gated(ctx, cmd("place_sheet_entry", "")),
    ]
}

pub(super) fn design(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        update_pcb_row(ctx),
        update_schematics_row(ctx),
        // #2234: enter or leave the sketch of the board in view; greyed
        // with no PCB.
        cmd_item_gated(ctx, cmd("board_sketch", "")),
        SEP,
        cmd_item_gated(ctx, cmd("open_parameter_manager", "")),
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
        cmd("update_pcb_from_schematic", ""),
        label.into_string(),
    )
}

/// Design ▸ Update Schematics in `<project>` (#1312): Back Annotate, the
/// board-side mirror of the row above. The project is named on a board.
fn update_schematics_row(ctx: &MenuContext) -> MenuEntry {
    use signex_i18n::{ids, t};
    let i18n = &ctx.i18n;
    let label = match &ctx.pcb_sync {
        Some(PcbSyncLabel::ImportFrom(project)) => {
            t!(i18n, ids::BACK_ANNOTATE_MENU_UPDATE_IN, "project" => project.as_str())
        }
        _ => t!(i18n, ids::BACK_ANNOTATE_MENU_UPDATE),
    };
    cmd_item_gated_as(
        ctx,
        cmd("back_annotate_schematics", ""),
        label.into_string(),
    )
}

pub(super) fn project(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        // F8 (`run_erc`) runs ERC at once; this row opens the dialog.
        cmd_item_gated(ctx, cmd("open_erc_dialog", "")),
        SEP,
        // Gated like Close Project: with no project there is no tab to open.
        item_if_command(
            ctx,
            "open_project_options",
            MenuMessage::OpenProjectOptions,
            ctx.commands.has_open_project,
        ),
        SEP,
        item_if(
            ctx,
            menu_ids::MENU_PROJECT_CLOSE,
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
        item_if(
            ctx,
            menu_ids::MENU_REPORTS_BOM,
            MenuMessage::ExportBom,
            has_schematic,
        ),
        item_if(
            ctx,
            menu_ids::MENU_REPORTS_NETLIST,
            MenuMessage::ExportNetlist,
            has_schematic,
        ),
        item_if(
            ctx,
            menu_ids::MENU_REPORTS_ERC,
            MenuMessage::ExportErcReport,
            has_schematic,
        ),
    ]
}

/// Tools ▸ Annotation. The first three rows need an open schematic:
/// annotating without one is meaningless.
fn annotation(ctx: &MenuContext) -> Vec<MenuEntry> {
    let has_schematic = ctx.commands.has_schematic;
    vec![
        item_if(
            ctx,
            menu_ids::MENU_TOOLS_ANNOTATE,
            MenuMessage::Annotate,
            has_schematic,
        ),
        item_if(
            ctx,
            menu_ids::MENU_TOOLS_RESET_DESIGNATORS,
            MenuMessage::AnnotateReset,
            has_schematic,
        ),
        item_if(
            ctx,
            menu_ids::MENU_TOOLS_RESET_DUPLICATE_DESIGNATORS,
            MenuMessage::AnnotateResetDuplicates,
            has_schematic,
        ),
        SEP,
        cmd_item_gated(ctx, cmd("annotate_schematic_quietly", "Alt+A")),
        cmd_item_gated(ctx, cmd("force_annotate_all_schematics", "Shift+Alt+A")),
        SEP,
        // Back Annotate: docs/MENU_BAR.md#back-annotate
        cmd_item_gated(ctx, cmd("back_annotate_schematics", "")),
        // The editable numbering dialog: docs/MENU_BAR.md#number-schematic-sheets
        cmd_item_gated(ctx, cmd("number_schematic_sheets", "")),
    ]
}

pub(super) fn tools(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        submenu(ctx, menu_ids::MENU_TOOLS_ANNOTATION, annotation(ctx)),
        SEP,
        // The project's parts and their row's footprints:
        // docs/MENU_BAR.md#tools-assign-footprints
        cmd_item_gated(ctx, cmd("assign_footprints", "")),
        // Opens or focuses a Library Browser tab: docs/MENU_BAR.md#tools-library-editor
        cmd_item_gated(ctx, cmd("open_library_browser", "")),
        SEP,
        // Runs `run_drc` on the PCB in view; greyed with no PCB open.
        cmd_item_gated(ctx, cmd("run_drc", "")),
        // Positional Re-Annotate of the PCB in view (#1312):
        // docs/MENU_BAR.md#tools-reannotate-board
        cmd_item_gated(ctx, cmd("reannotate_board", "")),
        // Opens the Net Inspector panel: docs/MENU_BAR.md#tools-net-inspector
        cmd_item_gated(ctx, cmd("open_net_inspector", "")),
        SEP,
        // #1312: Cross Probe shows the selection on the other side; Cross
        // Select Mode (checked while on) mirrors every selection there.
        cmd_item_gated(ctx, cmd("cross_probe", "")),
        cmd_toggle(
            ctx,
            cmd("toggle_cross_select_mode", "Mod+Shift+X"),
            ctx.cross_select,
        ),
        SEP,
        item_if(
            ctx,
            menu_ids::MENU_TOOLS_UP_HIERARCHY,
            MenuMessage::AscendToParentSheet,
            enabled(ctx, "navigate_up_hierarchy"),
        ),
        SEP,
        // Multi-part flow: acts only on a `.snxsym` editor tab, and Remove
        // Part only on a part after the first.
        item_if(
            ctx,
            menu_ids::MENU_TOOLS_NEW_PART,
            MenuMessage::ToolsNewPart,
            new_part_menu_enabled(&ctx.commands),
        ),
        item_if(
            ctx,
            menu_ids::MENU_TOOLS_REMOVE_PART,
            MenuMessage::ToolsRemovePart,
            remove_part_menu_enabled(&ctx.commands),
        ),
        SEP,
        // Opens the options of the library the active editor tab is in.
        item_if(
            ctx,
            menu_ids::MENU_TOOLS_DOCUMENT_OPTIONS,
            MenuMessage::ToolsDocumentOptions,
            ctx.commands.has_library_editor,
        ),
        SEP,
        item(
            ctx,
            menu_ids::MENU_TOOLS_PASSIVE_CALCULATOR,
            MenuMessage::OpenPassiveCalculator,
        ),
        SEP,
        cmd_item(ctx, cmd("open_preferences", "Mod+,")),
    ]
}

pub(super) fn window(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        // Main window plus undocked document windows; greyed with none
        // undocked: docs/MENU_BAR.md#window-tile
        cmd_item_gated(ctx, cmd("tile_windows_horizontally", "")),
        cmd_item_gated(ctx, cmd("tile_windows_vertically", "")),
        SEP,
        cmd_item_gated(ctx, cmd("close_all_documents", "")),
    ]
}

pub(super) fn help(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        item(ctx, menu_ids::MENU_HELP_ABOUT, MenuMessage::OpenAbout),
        item(
            ctx,
            menu_ids::MENU_HELP_DOCUMENTATION,
            MenuMessage::OpenDocumentation,
        ),
        item(
            ctx,
            menu_ids::MENU_HELP_REPORT_ISSUE,
            MenuMessage::ReportIssue,
        ),
        item(ctx, ids::START_OPEN_SAMPLE, MenuMessage::OpenSampleProject),
        SEP,
        cmd_item(ctx, cmd("show_current_command_hotkeys", "F1")),
    ]
}
