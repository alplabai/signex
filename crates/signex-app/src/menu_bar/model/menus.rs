//! The content of each root menu. Labels, order, gating and messages are the
//! menu contract pinned by `testdata/menu_bar.golden.tsv`.

use super::items::{
    cmd, cmd_item, cmd_item_gated, cmd_item_if, cmd_stub, item, item_if, menu, stub, submenu,
};
use super::{Menu, MenuEntry, MenuMessage};
use crate::app::command::enablement::cycle_grid_menu_enabled;
use crate::menu_bar::MenuContext;

const SEP: MenuEntry = MenuEntry::Separator;

pub(super) fn file(ctx: &MenuContext) -> Menu {
    menu(
        "File",
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
            // Save / Save As also act on a standalone symbol or footprint
            // editor tab, which `save_active_document` already handles.
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
            SEP,
            submenu("Library", library(ctx)),
            SEP,
            submenu(
                "Import",
                vec![item("Altium Project...", MenuMessage::ImportAltium)],
            ),
            submenu("Export", export(ctx)),
            SEP,
            item("Exit", MenuMessage::Exit),
        ],
    )
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
        entries.push(stub("No recent projects"));
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

pub(super) fn edit(ctx: &MenuContext) -> Menu {
    menu(
        "Edit",
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
        ],
    )
}

pub(super) fn view(ctx: &MenuContext) -> Menu {
    menu(
        "View",
        vec![
            cmd_stub(ctx, cmd("zoom_in_at_cursor", "Zoom In", "Ctrl+=")),
            cmd_stub(ctx, cmd("zoom_out_at_cursor", "Zoom Out", "Ctrl+-")),
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
            item("Projects", MenuMessage::OpenProjectsPanel),
            item("Components", MenuMessage::OpenComponentsPanel),
            item("Navigator", MenuMessage::OpenNavigatorPanel),
            item("Properties", MenuMessage::OpenPropertiesPanel),
            item("ERC", MenuMessage::OpenErcPanel),
            item("Messages", MenuMessage::OpenMessagesPanel),
            item("Signal", MenuMessage::OpenSignalPanel),
        ],
    )
}

pub(super) fn place(ctx: &MenuContext) -> Menu {
    menu(
        "Place",
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
            stub("Power Port"),
            SEP,
            stub("Text"),
            stub("No Connect"),
            stub("Sheet Entry"),
        ],
    )
}

pub(super) fn design(ctx: &MenuContext) -> Menu {
    menu(
        "Design",
        vec![
            submenu("Annotation", annotation(ctx)),
            SEP,
            cmd_item_gated(
                ctx,
                cmd("run_erc", "Electrical Rules Check", "F8"),
                MenuMessage::Erc,
            ),
            SEP,
            stub("Generate BOM"),
        ],
    )
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

pub(super) fn tools(ctx: &MenuContext) -> Menu {
    menu(
        "Tools",
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
        ],
    )
}

pub(super) fn window() -> Menu {
    menu(
        "Window",
        vec![
            stub("Tile Horizontally"),
            stub("Tile Vertically"),
            SEP,
            stub("Close All Documents"),
        ],
    )
}

pub(super) fn help(ctx: &MenuContext) -> Menu {
    menu(
        "Help",
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
        ],
    )
}
