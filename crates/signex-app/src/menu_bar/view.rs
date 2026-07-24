//! Menu-bar view builder. Split from `menu_bar.rs` as pure code motion.

use super::*;

// ─── View: Menu Bar ──────────────────────────────────────────

pub fn view(tokens: &ThemeTokens, ctx: MenuContext) -> Element<'static, MenuMessage> {
    let mc = MenuColors::from_tokens(tokens);
    // `leaf_if(enabled, ..)` wraps `leaf`/`leaf_stub` — enabled items
    // dispatch their message, disabled items render greyed-out like
    // the stub entries so Annotate / ERC / Save can't fire when no
    // schematic is loaded.
    let leaf_if = |label: &str,
                   shortcut: Option<String>,
                   msg: MenuMessage,
                   enabled: bool|
     -> Item<'static, MenuMessage, Theme, iced::Renderer> {
        if enabled {
            leaf(label, shortcut, msg, mc)
        } else {
            leaf_stub(label, shortcut, mc)
        }
    };

    let menu_template = |items| {
        Menu::new(items)
            .max_width(DROPDOWN_WIDTH)
            // Sit a couple of pixels below the bar — offset(0) overlaps
            // the bar's bottom row because the dropdown's 1px border
            // paints on the same pixel as the bar's baseline.
            .offset(2.0)
            .spacing(2.0)
            // iced_aw paints the dropdown's background quad at
            // `items.x - padding.left` (see `pad_rectangle` in
            // `iced_aw/src/widget/menu/menu_tree.rs`). Any positive
            // left-padding here drags the visible dropdown LEFT of the
            // root button's highlight box. Zero on the left keeps the
            // dropdown's left border flush with the root's layout
            // bounds, matching Altium's alignment.
            .padding(iced::Padding {
                top: 5.0,
                right: 5.0,
                bottom: 5.0,
                left: 0.0,
            })
    };

    let export_menu = Item::with_menu(
        submenu_item_btn("Export", mc),
        menu_template(vec![
            leaf_if(
                &cmd_label("print", "PDF…"),
                shortcut_for(&ctx, "print", "Ctrl+P"),
                MenuMessage::ExportPdf,
                ctx.has_schematic,
            ),
            leaf_if(
                "Netlist (Standard .net)...",
                None,
                MenuMessage::ExportNetlist,
                ctx.has_schematic,
            ),
            leaf_if(
                "Bill of Materials…",
                None,
                MenuMessage::ExportBom,
                ctx.has_schematic,
            ),
        ]),
    );

    // v0.9 Library submenu — open / place. New-component creation
    // moved to the project tree → right-click → Library ▸ Add New
    // ▸ Component flow, so File ▸ Library only carries "open
    // libraries outside the active project" + the canvas-side place
    // picker.
    let library_menu = Item::with_menu(
        submenu_item_btn("Library", mc),
        menu_template(vec![
            leaf("Open Library...", None, MenuMessage::LibraryOpenLibrary, mc),
            separator(mc),
            leaf_if(
                "Place Component...",
                None,
                MenuMessage::LibraryPlaceComponent,
                ctx.has_schematic,
            ),
        ]),
    );

    let file_menu = Item::with_menu(
        root_btn("File", mc),
        menu_template(vec![
            leaf(
                &cmd_label("new_document", "New Project"),
                shortcut_for(&ctx, "new_document", "Ctrl+N"),
                MenuMessage::NewProject,
                mc,
            ),
            leaf(
                &cmd_label("open_document", "Open..."),
                shortcut_for(&ctx, "open_document", "Ctrl+O"),
                MenuMessage::OpenProject,
                mc,
            ),
            separator(mc),
            // v0.14.2: Save / Save As also enabled for any standalone
            // primitive editor tab (`.snxsym` / `.snxfpt`). The
            // dispatcher in `save_active_document` already handles
            // those tab kinds; previously the menu greyed itself out
            // because the gate only checked for an active schematic.
            leaf_if(
                &cmd_label("save_document", "Save"),
                shortcut_for(&ctx, "save_document", "Ctrl+S"),
                MenuMessage::Save,
                ctx.has_schematic || ctx.has_symbol_editor || ctx.has_footprint_editor,
            ),
            leaf_if(
                &cmd_label("save_document_as", "Save As..."),
                shortcut_for(&ctx, "save_document_as", "Ctrl+Shift+S"),
                MenuMessage::SaveAs,
                ctx.has_schematic || ctx.has_symbol_editor || ctx.has_footprint_editor,
            ),
            separator(mc),
            library_menu,
            separator(mc),
            // Print Preview... previously lived here as a top-level
            // leaf duplicating Export → PDF's shortcut. Consolidated
            // under Export → PDF so there's one surface that opens the
            // same preview flow. Ctrl+P (the former top-level shortcut)
            // still fires `MenuMessage::PrintPreview` through the
            // global key handler; only the menu row moves.
            export_menu,
            separator(mc),
            leaf("Exit", None, MenuMessage::Exit, mc),
        ]),
    );

    let edit_menu = Item::with_menu(
        root_btn("Edit", mc),
        menu_template(vec![
            leaf_if(
                &cmd_label("undo", "Undo"),
                shortcut_for(&ctx, "undo", "Ctrl+Z"),
                MenuMessage::Undo,
                ctx.can_undo,
            ),
            leaf_if(
                &cmd_label("redo", "Redo"),
                shortcut_for(&ctx, "redo", "Ctrl+Y"),
                MenuMessage::Redo,
                ctx.can_redo,
            ),
            separator(mc),
            leaf_if(
                &cmd_label("cut", "Cut"),
                shortcut_for(&ctx, "cut", "Ctrl+X"),
                MenuMessage::Cut,
                ctx.has_selection,
            ),
            leaf_if(
                &cmd_label("copy", "Copy"),
                shortcut_for(&ctx, "copy", "Ctrl+C"),
                MenuMessage::Copy,
                ctx.has_selection,
            ),
            leaf_if(
                &cmd_label("paste", "Paste"),
                shortcut_for(&ctx, "paste", "Ctrl+V"),
                MenuMessage::Paste,
                ctx.has_schematic,
            ),
            leaf_if(
                &cmd_label("smart_paste", "Paste Special"),
                shortcut_for(&ctx, "smart_paste", "Shift+Ctrl+V"),
                MenuMessage::SmartPaste,
                ctx.has_schematic,
            ),
            leaf_if(
                &cmd_label("duplicate", "Duplicate"),
                shortcut_for(&ctx, "duplicate", "Ctrl+D"),
                MenuMessage::Duplicate,
                ctx.has_selection,
            ),
            leaf_if(
                &cmd_label("delete_selection", "Delete"),
                shortcut_for(&ctx, "delete_selection", "Del"),
                MenuMessage::Delete,
                ctx.has_selection,
            ),
            separator(mc),
            leaf_if(
                &cmd_label("select_all", "Select All"),
                shortcut_for(&ctx, "select_all", "Ctrl+A"),
                MenuMessage::SelectAll,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_if(
                &cmd_label("find", "Find"),
                shortcut_for(&ctx, "find", "Ctrl+F"),
                MenuMessage::Find,
                ctx.has_schematic,
            ),
            leaf_if(
                &cmd_label("find_and_replace", "Find and Replace"),
                shortcut_for(&ctx, "find_and_replace", "Ctrl+H"),
                MenuMessage::Replace,
                ctx.has_schematic,
            ),
        ]),
    );

    let view_menu = Item::with_menu(
        root_btn("View", mc),
        menu_template(vec![
            leaf_stub(
                &cmd_label("zoom_in_at_cursor", "Zoom In"),
                shortcut_for(&ctx, "zoom_in_at_cursor", "Ctrl+="),
                mc,
            ),
            leaf_stub(
                &cmd_label("zoom_out_at_cursor", "Zoom Out"),
                shortcut_for(&ctx, "zoom_out_at_cursor", "Ctrl+-"),
                mc,
            ),
            leaf_if(
                &cmd_label("zoom_to_fit", "Fit All"),
                shortcut_for(&ctx, "zoom_to_fit", "Home"),
                MenuMessage::ZoomFit,
                ctx.has_schematic || ctx.has_pcb,
            ),
            separator(mc),
            leaf_if(
                &cmd_label("toggle_visible_grid", "Toggle Grid"),
                shortcut_for(&ctx, "toggle_visible_grid", "Shift+Ctrl+G"),
                MenuMessage::ToggleGrid,
                ctx.has_schematic || ctx.has_pcb,
            ),
            leaf_if(
                &cmd_label("cycle_snap_grid_forward", "Cycle Grid Size"),
                shortcut_for(&ctx, "cycle_snap_grid_forward", "G"),
                MenuMessage::CycleGrid,
                ctx.has_schematic || ctx.has_pcb,
            ),
            leaf_if(
                &cmd_label("toggle_auto_focus", "AutoFocus (dim unselected)"),
                shortcut_for(&ctx, "toggle_auto_focus", "F9"),
                MenuMessage::ToggleAutoFocus,
                ctx.has_schematic,
            ),
            separator(mc),
            // Panel-open entries are always available — panels work
            // without an active document (show empty state).
            leaf("Projects", None, MenuMessage::OpenProjectsPanel, mc),
            leaf("Components", None, MenuMessage::OpenComponentsPanel, mc),
            leaf("Navigator", None, MenuMessage::OpenNavigatorPanel, mc),
            leaf("Properties", None, MenuMessage::OpenPropertiesPanel, mc),
            leaf("ERC", None, MenuMessage::OpenErcPanel, mc),
            leaf("Messages", None, MenuMessage::OpenMessagesPanel, mc),
            leaf("Signal", None, MenuMessage::OpenSignalPanel, mc),
        ]),
    );

    let place_menu = Item::with_menu(
        root_btn("Place", mc),
        menu_template(vec![
            leaf_if(
                &cmd_label("place_wire", "Wire"),
                shortcut_for(&ctx, "place_wire", "W"),
                MenuMessage::PlaceWire,
                ctx.has_schematic,
            ),
            leaf_if(
                &cmd_label("place_bus", "Bus"),
                shortcut_for(&ctx, "place_bus", "B"),
                MenuMessage::PlaceBus,
                ctx.has_schematic,
            ),
            leaf_if(
                &cmd_label("place_net_label", "Net Label"),
                shortcut_for(&ctx, "place_net_label", "L"),
                MenuMessage::PlaceLabel,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_if(
                &cmd_label("open_components_panel", "Component..."),
                shortcut_for(&ctx, "open_components_panel", "P"),
                MenuMessage::PlaceComponent,
                ctx.has_schematic,
            ),
            leaf_stub("Power Port", None, mc),
            separator(mc),
            leaf_stub("Text", None, mc),
            leaf_stub("No Connect", None, mc),
            leaf_stub("Sheet Entry", None, mc),
        ]),
    );

    // Design → Annotation submenu mirrors Altium's Annotation cascade.
    // Every entry gated on `has_schematic` — annotating without a
    // project open is nonsense.
    let annotation_submenu: Item<'static, MenuMessage, Theme, iced::Renderer> = Item::with_menu(
        submenu_item_btn("Annotation", mc),
        menu_template(vec![
            leaf_if(
                "Annotate Schematics...",
                None,
                MenuMessage::Annotate,
                ctx.has_schematic,
            ),
            leaf_if(
                "Reset Schematic Designators...",
                None,
                MenuMessage::AnnotateReset,
                ctx.has_schematic,
            ),
            leaf_if(
                "Reset Duplicate Schematic Designators...",
                None,
                MenuMessage::AnnotateResetDuplicates,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_if(
                &cmd_label("annotate_schematic_quietly", "Annotate Schematics Quietly"),
                shortcut_for(&ctx, "annotate_schematic_quietly", "Alt+A"),
                MenuMessage::AnnotateQuietly,
                ctx.has_schematic,
            ),
            leaf_if(
                &cmd_label(
                    "force_annotate_all_schematics",
                    "Force Annotate All Schematics",
                ),
                shortcut_for(&ctx, "force_annotate_all_schematics", "Shift+Alt+A"),
                MenuMessage::AnnotateForceAll,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_stub("Back Annotate Schematics...", None, mc),
            leaf_stub("Number Schematic Sheets...", None, mc),
        ]),
    );

    let design_menu = Item::with_menu(
        root_btn("Design", mc),
        menu_template(vec![
            annotation_submenu,
            separator(mc),
            leaf_if(
                &cmd_label("run_erc", "Electrical Rules Check"),
                shortcut_for(&ctx, "run_erc", "F8"),
                MenuMessage::Erc,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_stub("Generate BOM", None, mc),
            leaf_stub("Generate Netlist", None, mc),
        ]),
    );

    let tools_menu = Item::with_menu(
        root_btn("Tools", mc),
        menu_template(vec![
            leaf_stub("Assign Footprints", None, mc),
            leaf_stub("Library Editor", None, mc),
            separator(mc),
            leaf_stub("Design Rule Check", None, mc),
            leaf_stub("Net Inspector", None, mc),
            separator(mc),
            leaf(
                "Transmission Line Calculator...",
                None,
                MenuMessage::OpenTransmissionLineCalculator,
                mc,
            ),
            separator(mc),
            // Multi-part component flow — only meaningful when the
            // active tab is a `.snxsym` standalone editor; the
            // dispatcher silently no-ops on non-Symbol tabs.
            leaf("New Part", None, MenuMessage::ToolsNewPart, mc),
            leaf("Remove Part", None, MenuMessage::ToolsRemovePart, mc),
            separator(mc),
            // Document Options — Altium SchLib parity. Sheet color
            // / grid / unit per `.snxlib`. Greyed when not on a
            // primitive editor tab.
            leaf(
                "Document Options...",
                None,
                MenuMessage::ToolsDocumentOptions,
                mc,
            ),
            separator(mc),
            leaf(
                &cmd_label("open_preferences", "Preferences..."),
                shortcut_for(&ctx, "open_preferences", "Ctrl+,"),
                MenuMessage::OpenPreferences,
                mc,
            ),
        ]),
    );

    let window_menu = Item::with_menu(
        root_btn("Window", mc),
        menu_template(vec![
            leaf_stub("Tile Horizontally", None, mc),
            leaf_stub("Tile Vertically", None, mc),
            separator(mc),
            leaf_stub("Close All Documents", None, mc),
        ]),
    );

    let help_menu = Item::with_menu(
        root_btn("Help", mc),
        menu_template(vec![
            leaf_stub("About Signex", None, mc),
            separator(mc),
            leaf(
                &cmd_label("show_current_command_hotkeys", "Keyboard Shortcuts"),
                shortcut_for(&ctx, "show_current_command_hotkeys", "F1"),
                MenuMessage::OpenKeyboardShortcuts,
                mc,
            ),
        ]),
    );

    let mb: MenuBar<'static, MenuMessage, Theme, iced::Renderer> = MenuBar::new(vec![
        file_menu,
        edit_menu,
        view_menu,
        place_menu,
        design_menu,
        tools_menu,
        window_menu,
        help_menu,
    ])
    .spacing(1.0)
    .padding([1, 4])
    .close_on_item_click_global(true)
    .close_on_background_click_global(true)
    // `Backdrop` paints `styling.path` behind the active root while its
    // dropdown is open, so "File / Edit / Place / …" stays visibly lit
    // after the pointer leaves the root and enters the submenu — matches
    // Altium. `FakeHovering` (the default) only affects items inside the
    // dropdown and leaves the root dark.
    .draw_path(DrawPath::Backdrop)
    .style(move |_theme: &Theme, _status| menu_style::Style {
        bar_background: Background::Color(mc.toolbar_bg),
        bar_border: Border::default(),
        bar_shadow: iced::Shadow::default(),
        menu_background: Background::Color(mc.panel_bg),
        menu_border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: mc.border,
        },
        menu_shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(2.0, 4.0),
            blur_radius: 8.0,
        },
        path: Background::Color(mc.hover),
        path_border: Border {
            width: 1.0,
            radius: 2.0.into(),
            color: mc.border,
        },
    });

    // Wordmark — white on dark themes, near-black on light themes. Picked
    // by toolbar background luminance so custom themes also resolve
    // correctly. The asset is a PNG pre-rasterised at 1×/2×/3× the
    // on-screen 96×31 logical size; `wordmark_tier` picks the one that
    // matches the window's scale factor so text edges stay crisp at
    // 100 %, 125 %, 150 %, 200 %, 300 % Windows scaling. Filter method
    // is Linear so fractional in-between scales (e.g. 1.5×) downsample
    // cleanly from the 2× asset rather than hard-pixelating.
    let dark = is_dark_surface(tokens.toolbar_bg);
    let handle = match (dark, wordmark_tier(ctx.scale_factor)) {
        (true, 1) => (*BRAND_WORDMARK_WHITE_1X).clone(),
        (true, 2) => (*BRAND_WORDMARK_WHITE_2X).clone(),
        (true, _) => (*BRAND_WORDMARK_WHITE_3X).clone(),
        (false, 1) => (*BRAND_WORDMARK_BLACK_1X).clone(),
        (false, 2) => (*BRAND_WORDMARK_BLACK_2X).clone(),
        (false, _) => (*BRAND_WORDMARK_BLACK_3X).clone(),
    };
    let wordmark = image(handle)
        .width(WORDMARK_LOGICAL_W)
        .height(WORDMARK_LOGICAL_H)
        .filter_method(image::FilterMethod::Linear);

    // Just the wordmark + menu roots. The caller decides how to wrap this
    // (plain strip on secondary windows, draggable chrome with window
    // controls on the borderless main window).
    row![wordmark, mb]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .into()
}
