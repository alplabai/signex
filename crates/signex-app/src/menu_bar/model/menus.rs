//! The content of each root menu. Labels, order, gating and messages are the
//! menu contract pinned by `testdata/menu_bar.golden.tsv`.

use super::items::{
    cmd, cmd_item, cmd_item_gated, cmd_item_gated_as, cmd_item_if, item, item_if, panel_item, stub,
    submenu, unavailable,
};
use super::{MenuEntry, MenuMessage};
use crate::app::command::enablement::cycle_grid_menu_enabled;
use crate::menu_bar::{MenuContext, enabled};
use crate::panels::PanelKind;

const SEP: MenuEntry = MenuEntry::Separator;

pub(super) fn file(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item(
            ctx,
            cmd("new_document", "New Project", "Ctrl+N"),
            MenuMessage::NewProject,
        ),
        cmd_item(
            ctx,
            cmd("open_document", "Open...", "Ctrl+O"),
            MenuMessage::OpenProject,
        ),
        submenu("Open Recent", open_recent(ctx)),
        SEP,
        // Both act on what is open and do nothing when nothing is.
        item("Close Document", MenuMessage::CloseDocument),
        item("Close Project", MenuMessage::CloseProject),
        SEP,
        // Save gating: docs/MENU_BAR.md#file-save
        cmd_item_gated(
            ctx,
            cmd("save_document", "Save", "Ctrl+S"),
            MenuMessage::Save,
        ),
        cmd_item_gated(
            ctx,
            cmd("save_document_as", "Save As...", "Ctrl+Shift+S"),
            MenuMessage::SaveAs,
        ),
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
        // Print Preview lives under Export > PDF: docs/MENU_BAR.md#file-export-pdf
        submenu("Export", export(ctx)),
        cmd_item_gated_as(ctx, cmd("print", "Print...", "Ctrl+P"), MenuMessage::Print),
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
    let has_schematic = ctx.commands.has_schematic;
    vec![
        cmd_item_gated(ctx, cmd("print", "PDF…", "Ctrl+P"), MenuMessage::ExportPdf),
        item_if(
            "Netlist (Protel .NET)...",
            MenuMessage::ExportNetlist,
            has_schematic,
        ),
        item_if("Bill of Materials…", MenuMessage::ExportBom, has_schematic),
        item_if("ERC Report…", MenuMessage::ExportErcReport, has_schematic),
    ]
}

pub(super) fn edit(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item_gated(ctx, cmd("undo", "Undo", "Ctrl+Z"), MenuMessage::Undo),
        cmd_item_gated(ctx, cmd("redo", "Redo", "Ctrl+Y"), MenuMessage::Redo),
        SEP,
        cmd_item_gated(ctx, cmd("cut", "Cut", "Ctrl+X"), MenuMessage::Cut),
        cmd_item_gated(ctx, cmd("copy", "Copy", "Ctrl+C"), MenuMessage::Copy),
        cmd_item_gated(ctx, cmd("paste", "Paste", "Ctrl+V"), MenuMessage::Paste),
        cmd_item_gated(
            ctx,
            cmd("smart_paste", "Paste Special", "Shift+Ctrl+V"),
            MenuMessage::SmartPaste,
        ),
        cmd_item_gated(
            ctx,
            cmd("duplicate", "Duplicate", "Ctrl+D"),
            MenuMessage::Duplicate,
        ),
        cmd_item_gated(
            ctx,
            cmd("delete_selection", "Delete", "Del"),
            MenuMessage::Delete,
        ),
        submenu("Arrange", arrange(ctx)),
        SEP,
        cmd_item_gated(
            ctx,
            cmd("select_all", "Select All", "Ctrl+A"),
            MenuMessage::SelectAll,
        ),
        SEP,
        cmd_item_gated(ctx, cmd("find", "Find", "Ctrl+F"), MenuMessage::Find),
        cmd_item_gated(
            ctx,
            cmd("find_and_replace", "Find and Replace", "Ctrl+H"),
            MenuMessage::Replace,
        ),
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
        cmd_item_gated(
            ctx,
            cmd("zoom_in_at_cursor", "Zoom In", "Ctrl+="),
            MenuMessage::ZoomIn,
        ),
        cmd_item_gated(
            ctx,
            cmd("zoom_out_at_cursor", "Zoom Out", "Ctrl+-"),
            MenuMessage::ZoomOut,
        ),
        cmd_item_gated(
            ctx,
            cmd("zoom_to_fit", "Fit All", "Home"),
            MenuMessage::ZoomFit,
        ),
        SEP,
        cmd_item_gated(
            ctx,
            cmd("toggle_visible_grid", "Toggle Grid", "Shift+Ctrl+G"),
            MenuMessage::ToggleGrid,
        ),
        cmd_item_if(
            ctx,
            cmd("cycle_snap_grid_forward", "Cycle Grid Size", "G"),
            MenuMessage::CycleGrid,
            cycle_grid_menu_enabled(&ctx.commands),
        ),
        cmd_item_gated(
            ctx,
            cmd("toggle_auto_focus", "AutoFocus (dim unselected)", "F9"),
            MenuMessage::ToggleAutoFocus,
        ),
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
            "Signal",
            PanelKind::Signal,
            MenuMessage::OpenSignalPanel,
        ),
    ]
}

pub(super) fn place(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        cmd_item_gated(ctx, cmd("place_wire", "Wire", "W"), MenuMessage::PlaceWire),
        cmd_item_gated(ctx, cmd("place_bus", "Bus", "B"), MenuMessage::PlaceBus),
        cmd_item_gated(
            ctx,
            cmd("place_net_label", "Net Label", "L"),
            MenuMessage::PlaceLabel,
        ),
        SEP,
        cmd_item_gated(
            ctx,
            cmd("open_components_panel", "Component...", "P"),
            MenuMessage::PlaceComponent,
        ),
        cmd_item_gated(
            ctx,
            cmd("place_power_symbol", "Power Port", "P"),
            MenuMessage::PlacePowerPort,
        ),
        cmd_item_gated(
            ctx,
            cmd("place_junction", "Junction", "J"),
            MenuMessage::PlaceJunction,
        ),
        SEP,
        item_if("Text", MenuMessage::PlaceText, ctx.commands.has_schematic),
        item_if(
            "No Connect",
            MenuMessage::PlaceNoConnect,
            ctx.commands.has_schematic,
        ),
        stub("Sheet Entry"),
    ]
}

pub(super) fn design(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        submenu("Annotation", annotation(ctx)),
        SEP,
        cmd_item_gated(
            ctx,
            cmd("run_erc", "Electrical Rules Check", "F8"),
            MenuMessage::Erc,
        ),
        SEP,
        // The same working BOM dialog File > Export > Bill of Materials
        // opens (#1251).
        item_if(
            "Generate BOM",
            MenuMessage::ExportBom,
            ctx.commands.has_schematic,
        ),
    ]
}

/// Design ▸ Annotation. The first three rows need an open schematic:
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
            MenuMessage::AnnotateQuietly,
        ),
        cmd_item_gated(
            ctx,
            cmd(
                "force_annotate_all_schematics",
                "Force Annotate All Schematics",
                "Shift+Alt+A",
            ),
            MenuMessage::AnnotateForceAll,
        ),
        SEP,
        stub("Back Annotate Schematics..."),
        stub("Number Schematic Sheets..."),
    ]
}

pub(super) fn tools(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        stub("Assign Footprints"),
        stub("Library Editor"),
        SEP,
        stub("Design Rule Check"),
        stub("Net Inspector"),
        SEP,
        item_if(
            "Up Hierarchy",
            MenuMessage::AscendToParentSheet,
            ctx.commands.has_schematic,
        ),
        SEP,
        // Multi-part flow: only acts on a `.snxsym` editor tab; the
        // dispatcher no-ops elsewhere.
        item("New Part", MenuMessage::ToolsNewPart),
        item("Remove Part", MenuMessage::ToolsRemovePart),
        SEP,
        item("Document Options...", MenuMessage::ToolsDocumentOptions),
        SEP,
        item(
            "Passive Network Calculator...",
            MenuMessage::OpenPassiveCalculator,
        ),
        SEP,
        cmd_item(
            ctx,
            cmd("open_preferences", "Preferences...", "Ctrl+,"),
            MenuMessage::OpenPreferences,
        ),
    ]
}

pub(super) fn window() -> Vec<MenuEntry> {
    vec![
        stub("Tile Horizontally"),
        stub("Tile Vertically"),
        SEP,
        stub("Close All Documents"),
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
            MenuMessage::OpenKeyboardShortcuts,
        ),
    ]
}
