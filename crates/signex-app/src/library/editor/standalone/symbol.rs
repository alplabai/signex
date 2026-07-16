//! Standalone `.snxsym` symbol-editor document tab view builders.
//! Split from `library/editor/standalone.rs` as pure code motion.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_types::coord::Unit;
use signex_widgets::theme_ext;

use crate::app::SymbolEditorState;
use crate::library::editor::symbol::canvas::{self as sym_canvas, SymbolCanvas};
use crate::library::editor::symbol::state as sym_state;
use crate::library::messages::{
    GraphicHandleMsg, LibraryMessage, PrimitiveEdit, SymbolEditorMsg, SymbolRotatePivotMsg,
    SymbolSelectionMsg,
};
use crate::library::state::LibraryDisplaySettings;
use crate::panels::PanelContext;

// ── Symbol ──────────────────────────────────────────────────────────

/// Render the standalone Symbol editor for a `.snxsym` tab. Altium
/// SchLib parity: the canvas takes the full tab width; the right-dock
/// Properties panel renders symbol/pin properties driven by the
/// selection (see `panels::view_symbol_editor_properties`). The
/// in-tab properties column was retired in v0.9 phase 1 so the user
/// sees the same Properties surface whether editing a schematic or
/// a symbol library — single source of truth, no duplicated panes.
pub fn view_symbol<'a>(
    editor: &'a SymbolEditorState,
    panel_ctx: &'a PanelContext,
    display: LibraryDisplaySettings,
    theme_id: signex_types::theme::ThemeId,
    path: &'a std::path::PathBuf,
) -> Element<'a, LibraryMessage> {
    let tokens = &panel_ctx.tokens;
    let toolbar = view_symbol_toolbar(editor, panel_ctx);
    // Active bar moved to the app-view layer (view_main_for) so it
    // shares the schematic's window-absolute coordinate system — see
    // symbol::active_bar::{bar_items, dropdown_overlay}. The body here
    // renders just toolbar + canvas; the bar is layered on top there.
    let _ = theme_id;
    let canvas_widget = view_symbol_canvas(editor, panel_ctx, display);

    let body = column![toolbar, canvas_widget]
        .spacing(6)
        .width(Length::Fill)
        .height(Length::Fill);

    let status_line = view_symbol_status(editor, panel_ctx, display, path);

    let outer = column![body, Space::new().height(4), status_line]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

    container(outer)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into()
}

/// Bottom status footer for the .snxsym tab — Altium SchLib parity.
/// X / Y in the active unit (mm / mil), zoom %, grid spacing,
/// pin count + a hint string. Mirrors the global schematic
/// status bar so a user editing a symbol library has the same
/// metadata at the same place on screen.
fn view_symbol_status<'a>(
    editor: &'a SymbolEditorState,
    panel_ctx: &'a PanelContext,
    display: LibraryDisplaySettings,
    path: &'a std::path::PathBuf,
) -> Element<'a, LibraryMessage> {
    let tokens = &panel_ctx.tokens;
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);

    let unit = display.unit;
    let coord_text = match editor.cursor_mm {
        Some((x, y)) => format_coord(x, y, unit),
        None => "X: -.--   Y: -.--".to_string(),
    };
    let zoom_text = format!("{:.0}%", editor.camera.zoom_percent());
    let pin_count = format!("{} pins", editor.primitive().pins.len());
    let tool_text = format!("Tool: {}", editor.tool.label());

    let sep = || text("|").size(10).color(muted);

    // Grid toggle button.
    let grid_vis_label = if display.grid_visible {
        "Grid: ON"
    } else {
        "Grid: OFF"
    };
    let grid_toggle = button(text(grid_vis_label).size(11).color(text_c))
        .padding([2, 6])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Symbol(SymbolEditorMsg::ToggleGrid),
        })
        .style(symbol_tool_button_style(false, border));

    // Grid size cycle button.
    let grid_size_label = format!("{:.3} mm", display.grid_size_mm);
    let grid_cycle = button(text(grid_size_label).size(11).color(muted))
        .padding([2, 6])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Symbol(SymbolEditorMsg::CycleGridSize),
        })
        .style(symbol_tool_button_style(false, border));

    container(
        row![
            text(coord_text).size(11).color(text_c),
            sep(),
            text(zoom_text).size(11).color(muted),
            sep(),
            text(pin_count).size(11).color(muted),
            sep(),
            text(tool_text).size(11).color(muted),
            sep(),
            grid_toggle,
            grid_cycle,
            Space::new().width(Length::Fill),
            text(if editor.selected.is_some() {
                "Del removes · drag to move · scroll zooms · right-drag pans · Home fits"
            } else {
                "Ctrl+Z undo · Ctrl+Y redo · scroll zooms · right-drag pans · Home fits"
            })
            .size(10)
            .color(muted),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 12])
    .style(crate::styles::status_bar(tokens))
    .width(Length::Fill)
    .into()
}

fn format_coord(x_mm: f64, y_mm: f64, unit: Unit) -> String {
    match unit {
        Unit::Mm => format!("X: {x_mm:>+8.3} mm   Y: {y_mm:>+8.3} mm"),
        Unit::Mil => format!(
            "X: {:>+8.1} mil  Y: {:>+8.1} mil",
            x_mm / 0.0254,
            y_mm / 0.0254
        ),
        Unit::Inch => format!("X: {:>+8.4} in   Y: {:>+8.4} in", x_mm / 25.4, y_mm / 25.4),
        Unit::Micrometer => format!(
            "X: {:>+8.0} um   Y: {:>+8.0} um",
            x_mm * 1000.0,
            y_mm * 1000.0
        ),
    }
}

fn view_symbol_toolbar<'a>(
    editor: &'a SymbolEditorState,
    panel_ctx: &'a PanelContext,
) -> Element<'a, LibraryMessage> {
    let tokens = &panel_ctx.tokens;
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let path = editor.path.clone();

    // Helper: create a small toolbar button that emits a PrimitiveEditorEvent.
    let btn = |label: &'static str, msg: PrimitiveEdit| {
        button(text(label).size(11).color(text_c))
            .padding([4, 10])
            .on_press(LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg,
            })
            .style(symbol_tool_button_style(false, border))
    };

    let max_part = crate::library::editor::symbol::state::max_part_number(editor.primitive());
    let active_part = editor.active_part;

    let save_label = if editor.dirty { "Save *" } else { "Save" };

    container(
        row![
            btn("Fit", PrimitiveEdit::Symbol(SymbolEditorMsg::Fit)),
            Space::new().width(Length::Fill),
            btn("\u{2190}", PrimitiveEdit::Symbol(SymbolEditorMsg::PrevPart)),
            text(format!("Part {active_part} / {max_part}"))
                .size(11)
                .color(text_c),
            btn("\u{2192}", PrimitiveEdit::Symbol(SymbolEditorMsg::NextPart)),
            Space::new().width(6),
            // Add-unit (+) / remove-unit (−). Wire the existing
            // NewPart / RemovePart messages; Phase B fixes their
            // semantics (real delete + persistent empty unit).
            btn("+", PrimitiveEdit::Symbol(SymbolEditorMsg::NewPart)),
            btn(
                "\u{2212}",
                PrimitiveEdit::Symbol(SymbolEditorMsg::RemovePart)
            ),
            Space::new().width(8),
            btn(save_label, PrimitiveEdit::Save),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 10])
    .style(crate::styles::tab_bar_strip(tokens))
    .into()
}

fn symbol_tool_button_style(
    active: bool,
    border: iced::Color,
) -> impl Fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style {
    move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(if active {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.02)
        })),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        ..iced::widget::button::Style::default()
    }
}

fn view_symbol_canvas<'a>(
    editor: &'a SymbolEditorState,
    panel_ctx: &'a PanelContext,
    display: LibraryDisplaySettings,
) -> Element<'a, LibraryMessage> {
    let tokens = &panel_ctx.tokens;
    let program = SymbolCanvas::new(
        editor.primitive(),
        editor.selected.clone(),
        editor.tool,
        editor.active_part,
        &editor.camera,
        display.grid_size_mm as f64,
        display.grid_visible,
        display.pin_selection.allows_label_grab(),
        display.sheet_color.to_color(),
        crate::styles::ti(tokens.accent),
        crate::styles::ti(tokens.text),
        crate::styles::ti(tokens.text),
        iced::Color {
            a: 0.18,
            ..crate::styles::ti(tokens.text_secondary)
        },
    );
    let widget: Element<'a, sym_canvas::CanvasAction> = iced::widget::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    let path = editor.path.clone();
    widget.map(move |action| LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Symbol(symbol_action_to_primitive_msg(action)),
    })
}

fn symbol_action_to_primitive_msg(action: sym_canvas::CanvasAction) -> SymbolEditorMsg {
    use sym_canvas::CanvasAction;
    match action {
        CanvasAction::AddPin { x, y } => SymbolEditorMsg::AddPin { x, y },
        CanvasAction::AddRectangle {
            from_x,
            from_y,
            to_x,
            to_y,
        } => SymbolEditorMsg::AddRectangle {
            from_x,
            from_y,
            to_x,
            to_y,
        },
        CanvasAction::AddLine {
            from_x,
            from_y,
            to_x,
            to_y,
        } => SymbolEditorMsg::AddLine {
            from_x,
            from_y,
            to_x,
            to_y,
        },
        CanvasAction::AddCircle { cx, cy, radius } => SymbolEditorMsg::AddCircle { cx, cy, radius },
        CanvasAction::AddArc {
            cx,
            cy,
            radius,
            start_deg,
            end_deg,
        } => SymbolEditorMsg::AddArc {
            cx,
            cy,
            radius,
            start_deg,
            end_deg,
        },
        CanvasAction::AddText { x, y } => SymbolEditorMsg::AddText { x, y },
        CanvasAction::AddPolygon { vertices } => SymbolEditorMsg::AddPolygon { vertices },
        CanvasAction::Select(sel) => SymbolEditorMsg::Select(symbol_selection_to_msg(sel)),
        CanvasAction::Deselect => SymbolEditorMsg::Deselect,
        CanvasAction::Move { x, y } => SymbolEditorMsg::MoveSelected { x, y },
        CanvasAction::MoveAll { dx, dy } => SymbolEditorMsg::MoveAll { dx, dy },
        CanvasAction::MoveGraphicHandle { idx, handle, x, y } => {
            SymbolEditorMsg::MoveGraphicHandle {
                idx,
                handle: graphic_handle_to_msg(handle),
                x,
                y,
            }
        }
        CanvasAction::RotateSelected {
            clockwise,
            pivot_mode,
        } => SymbolEditorMsg::RotateSelected {
            clockwise,
            pivot: rotate_pivot_to_msg(pivot_mode),
        },
        CanvasAction::DeleteSelected => SymbolEditorMsg::DeleteSelected,
        CanvasAction::Pan { dx, dy } => SymbolEditorMsg::Pan { dx, dy },
        CanvasAction::Zoom { sx, sy, delta } => SymbolEditorMsg::Zoom { sx, sy, delta },
        CanvasAction::Fit => SymbolEditorMsg::Fit,
        CanvasAction::CursorAt { x_mm, y_mm } => SymbolEditorMsg::CursorAt { x_mm, y_mm },
        CanvasAction::DragCommit => SymbolEditorMsg::DragCommit,
        CanvasAction::Undo => SymbolEditorMsg::Undo,
        CanvasAction::Redo => SymbolEditorMsg::Redo,
    }
}

fn graphic_handle_to_msg(handle: sym_state::GraphicHandle) -> GraphicHandleMsg {
    use sym_state::GraphicHandle;
    match handle {
        GraphicHandle::RectCorner(c) => GraphicHandleMsg::RectCorner(c),
        GraphicHandle::RectEdge(e) => GraphicHandleMsg::RectEdge(e),
        GraphicHandle::LineEndpoint(e) => GraphicHandleMsg::LineEndpoint(e),
        GraphicHandle::CircleRadius => GraphicHandleMsg::CircleRadius,
        GraphicHandle::ArcStart => GraphicHandleMsg::ArcStart,
        GraphicHandle::ArcEnd => GraphicHandleMsg::ArcEnd,
        GraphicHandle::TextAnchor => GraphicHandleMsg::TextAnchor,
        GraphicHandle::PolygonVertex(i) => GraphicHandleMsg::PolygonVertex(i),
    }
}

fn rotate_pivot_to_msg(pivot_mode: sym_canvas::RotatePivotMode) -> SymbolRotatePivotMsg {
    match pivot_mode {
        sym_canvas::RotatePivotMode::WorldOrigin => SymbolRotatePivotMsg::WorldOrigin,
        sym_canvas::RotatePivotMode::GeometryCenter => SymbolRotatePivotMsg::GeometryCenter,
    }
}

fn symbol_selection_to_msg(sel: sym_state::SymbolSelection) -> SymbolSelectionMsg {
    use sym_state::{FieldKey, SymbolSelection};
    match sel {
        SymbolSelection::Pin(idx) => SymbolSelectionMsg::Pin(idx),
        SymbolSelection::Field(FieldKey::Reference) => SymbolSelectionMsg::FieldReference,
        SymbolSelection::Field(FieldKey::Value) => SymbolSelectionMsg::FieldValue,
        SymbolSelection::Graphic(idx) => SymbolSelectionMsg::Graphic(idx),
        SymbolSelection::All => SymbolSelectionMsg::All,
        SymbolSelection::Multiple {
            pin_indices,
            graphic_indices,
        } => SymbolSelectionMsg::Multiple {
            pin_indices,
            graphic_indices,
        },
    }
}
