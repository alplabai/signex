/// Scan a library directory for standalone primitive files. Returns
/// `(symbols, footprints, sims)` triples — each `(stem, absolute_path)`.
/// Missing subdirectories are silently treated as empty so a fresh
/// library doesn't error; non-UTF-8 filenames and dotfiles are skipped.
///
/// Order is filename-stem-sorted so the project tree stays stable
/// across sessions (read_dir order is platform-dependent on Windows).
/// Project the active `.snxsym` editor's data into a panel-side
/// snapshot. Called from `refresh_panel_ctx` so the right-dock
/// Properties panel and the SCH-Library left-dock panel can render
/// context-aware content while the active tab is a Symbol editor.
/// Returns `None` for any other tab kind.
pub(super) fn build_symbol_editor_panel_ctx(
    app: &super::super::Signex,
) -> Option<crate::panels::SymbolEditorPanelContext> {
    use crate::library::editor::symbol::state as sym_state;
    use crate::panels::{
        GraphicKindSummary, GraphicSummary, SymbolDisplayOptions, SymbolEditorPanelContext,
        SymbolEditorSelection, SymbolFileEntry, SymbolPinDetails, SymbolPinSummary,
    };

    let active = app.document_state.tabs.get(app.document_state.active_tab)?;
    let path = match &active.kind {
        crate::app::TabKind::SymbolEditor(p) => p.clone(),
        _ => return None,
    };
    let editor = app.document_state.symbol_editors.get(&path)?;
    let sym = editor.primitive();

    let pins: Vec<SymbolPinSummary> = sym
        .pins
        .iter()
        .enumerate()
        .map(|(idx, pin)| SymbolPinSummary {
            idx,
            number: pin.number.clone(),
            name: pin.name.clone(),
            electrical: format!("{:?}", pin.electrical),
            position: pin.position,
            orientation: format!("{:?}", pin.orientation),
            length: pin.length,
            details: SymbolPinDetails {
                description: pin.description.clone(),
                function: pin.function.clone(),
                pin_package_length: pin.pin_package_length,
                propagation_delay_ns: pin.propagation_delay_ns,
                designator_visible: pin.designator_visible,
                name_visible: pin.name_visible,
                inside_symbol: pin.inside_symbol,
                inside_edge_symbol: pin.inside_edge_symbol,
                outside_edge_symbol: pin.outside_edge_symbol,
                outside_symbol: pin.outside_symbol,
                hidden: pin.hidden,
                locked: pin.locked,
                part_number: pin.part_number,
            },
        })
        .collect();

    let symbols_in_file: Vec<SymbolFileEntry> = editor
        .file
        .symbols
        .iter()
        .enumerate()
        .map(|(idx, s)| SymbolFileEntry {
            idx,
            name: s.name.clone(),
            uuid: s.uuid,
            pin_count: s.pins.len(),
            description: s.description.clone(),
        })
        .collect();

    let graphics: Vec<GraphicSummary> = sym
        .graphics
        .iter()
        .enumerate()
        .map(|(idx, g)| GraphicSummary {
            idx,
            kind: graphic_kind_to_summary(&g.kind),
            stroke_width: g.stroke_width,
        })
        .collect();

    let selected = match editor.selected.clone() {
        Some(sym_state::SymbolSelection::Pin(idx)) => pins
            .get(idx)
            .cloned()
            .map(SymbolEditorSelection::Pin)
            .unwrap_or(SymbolEditorSelection::None),
        Some(sym_state::SymbolSelection::Field(sym_state::FieldKey::Reference)) => {
            SymbolEditorSelection::FieldReference
        }
        Some(sym_state::SymbolSelection::Field(sym_state::FieldKey::Value)) => {
            SymbolEditorSelection::FieldValue
        }
        Some(sym_state::SymbolSelection::Graphic(idx)) => sym
            .graphics
            .get(idx)
            .map(|g| {
                SymbolEditorSelection::Graphic(GraphicSummary {
                    idx,
                    kind: graphic_kind_to_summary(&g.kind),
                    stroke_width: g.stroke_width,
                })
            })
            .unwrap_or(SymbolEditorSelection::None),
        Some(sym_state::SymbolSelection::All)
        | Some(sym_state::SymbolSelection::Multiple { .. })
        | None => SymbolEditorSelection::None,
    };

    let active_max_part = sym_state::max_part_number(sym);
    let active_has_part_zero = sym.pins.iter().any(|p| p.part_number == 0);

    // Resolve the containing `.snxlib` so the Properties panel's
    // Document Options branch can render real per-library values.
    // Lone-file edits (no mounted library) fall through to defaults.
    let display = match app.library.containing_library(&path) {
        Some(lib) => SymbolDisplayOptions {
            sheet_color: lib.display.sheet_color,
            grid_visible: lib.display.grid_visible,
            grid_size_mm: lib.display.grid_size_mm,
            unit: lib.display.unit,
            library_name: lib.display_name.clone(),
            library_symbol_count: Some(
                lib.cached_symbols.len() + lib.cached_footprints.len() + lib.cached_sims.len(),
            ),
        },
        None => SymbolDisplayOptions::default(),
    };

    Some(SymbolEditorPanelContext {
        path,
        symbol_name: sym.name.clone(),
        symbol_designator: sym.designator.clone(),
        symbol_comment: sym.comment.clone(),
        symbol_description: sym.description.clone(),
        symbol_component_type: sym.component_type,
        symbol_mirrored: sym.mirrored,
        symbol_local_fill_color: sym.local_fill_color,
        symbol_local_line_color: sym.local_line_color,
        symbol_local_pin_color: sym.local_pin_color,
        symbol_uuid: sym.uuid,
        pins,
        graphics,
        selected,
        symbols_in_file,
        active_idx: editor.active_idx,
        active_part: editor.active_part,
        active_max_part,
        active_has_part_zero,
        display,
    })
}

/// Project a `SymbolGraphicKind` into a [`GraphicKindSummary`] so the
/// Properties panel can render per-shape fields without depending on
/// the library type.
fn graphic_kind_to_summary(
    kind: &signex_library::SymbolGraphicKind,
) -> crate::panels::GraphicKindSummary {
    use crate::panels::GraphicKindSummary;
    use signex_library::SymbolGraphicKind;
    match kind {
        SymbolGraphicKind::Rectangle { from, to } => GraphicKindSummary::Rectangle {
            from: *from,
            to: *to,
        },
        SymbolGraphicKind::Line { from, to } => GraphicKindSummary::Line {
            from: *from,
            to: *to,
        },
        SymbolGraphicKind::Circle { center, radius } => GraphicKindSummary::Circle {
            center: *center,
            radius: *radius,
        },
        SymbolGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => GraphicKindSummary::Arc {
            center: *center,
            radius: *radius,
            start_deg: *start_deg,
            end_deg: *end_deg,
        },
        SymbolGraphicKind::Text {
            position,
            content,
            size,
        } => GraphicKindSummary::Text {
            position: *position,
            content: content.clone(),
            size: *size,
        },
    }
}
