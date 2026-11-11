//! Standalone `.snxsym` symbol-editor document tab view builders.
//! Split from `library/editor/standalone.rs` as pure code motion.

use iced::widget::{Space, column, container, row};
use iced::{Border, Element, Length, Theme, Widget};
use signex_widgets::theme_ext;
use signex_widgets::{button, text};

use crate::app::SymbolDocument;
use crate::library::editor::symbol::canvas::{self as sym_canvas, SymbolCanvas};
use crate::library::editor::symbol::state as sym_state;
use crate::library::messages::{
    GraphicHandleMsg, LibraryMessage, PrimitiveEdit, SymbolEditorMsg, SymbolRotatePivotMsg,
    SymbolSelectionMsg, SymbolToolMsg,
};
use crate::library::state::LibraryDisplaySettings;
use crate::panels::PanelContext;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

// ── Symbol ──────────────────────────────────────────────────────────

/// Render the standalone Symbol editor for a `.snxsym` tab. Altium
/// SchLib parity: the canvas takes the full tab width; the right-dock
/// Properties panel renders symbol/pin properties driven by the
/// selection (see `panels::view_symbol_editor_properties`). The
/// in-tab properties column was retired in v0.9 phase 1 so the user
/// sees the same Properties surface whether editing a schematic or
/// a symbol library — single source of truth, no duplicated panes.
pub fn view_symbol<'a>(
    editor: &'a SymbolDocument,
    panel_ctx: &'a PanelContext,
    display: LibraryDisplaySettings,
    path: &'a std::path::Path,
    device_px_per_logical_px: f32,
) -> Element<'a, LibraryMessage> {
    let tokens = &panel_ctx.tokens;
    // Active bar moved to the app-view layer (view_main_for) so it
    // shares the schematic's window-absolute coordinate system — see
    // symbol::active_bar::{bar_items, dropdown_overlay}. The body here
    // renders just toolbar + canvas; the bar is layered on top there,
    // which is also where the theme id for its icons is resolved.
    let canvas_widget = view_symbol_canvas(editor, panel_ctx, display, device_px_per_logical_px);

    // No header strip and no footer: the global status bar carries X / Y,
    // zoom, grid and the editor's part / pins / hint segment
    // (`symbol::status`). The canvas carries the body / part card at its
    // bottom-left and the zoom pill at its bottom-right.
    let card = container(graphite_toolbar(editor, path, tokens, &panel_ctx.i18n))
        .padding(SP.l)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Left)
        .align_y(iced::alignment::Vertical::Bottom);
    let pill = container(graphite_zoom_pill(editor, path, tokens))
        .padding(SP.l)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Bottom);
    let body = iced::widget::stack![canvas_widget, card.boxed(), pill.boxed()]
        .width(Length::Fill)
        .height(Length::Fill);

    container(body)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .boxed()
}

/// Graphite body / part card over the canvas: a segmented Normal / Alternate
/// control, the body assignment buttons, the part stepper and an accent Save.
/// Fit lives on the zoom pill.
fn graphite_toolbar<'a>(
    editor: &SymbolDocument,
    path: &std::path::Path,
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, LibraryMessage> {
    use crate::styles::tokens::resolve;
    use iced::widget::button::Status;
    use signex_i18n::{Localized, ids, t};
    use signex_types::schematic::BodyStyle;
    let r = resolve(tokens);
    let ink = theme_ext::text_primary(tokens);
    let (ink3, raised, sunken, line, hover) =
        (r.ink3, r.raised, r.sunken, r.line_strong, r.accent_soft);
    let send = |msg: PrimitiveEdit| LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg,
    };
    let sym = |m: SymbolEditorMsg| send(PrimitiveEdit::Symbol(m));
    let ghost_style = move |_: &Theme, status: Status| iced::widget::button::Style {
        background: matches!(status, Status::Hovered | Status::Pressed)
            .then_some(iced::Background::Color(hover)),
        text_color: ink,
        border: Border {
            width: crate::styles::HAIRLINE,
            radius: 7.0.into(),
            color: line,
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    };
    let ghost = |label: Localized, msg: LibraryMessage| {
        button(text(label).size(TS.caption))
            .padding([SP.xs, SP.m])
            .on_press(msg)
            .style(ghost_style)
    };
    let cell = |label: Localized, active: bool, msg: LibraryMessage| {
        button(text(label).size(TS.caption))
            .padding([SP.xs, SP.m])
            .on_press(msg)
            .style(
                move |_: &Theme, status: Status| iced::widget::button::Style {
                    background: if active {
                        Some(iced::Background::Color(raised))
                    } else if matches!(status, Status::Hovered) {
                        Some(iced::Background::Color(hover))
                    } else {
                        None
                    },
                    text_color: if active { ink } else { ink3 },
                    border: Border {
                        radius: 6.0.into(),
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                },
            )
    };
    let view = editor.view();
    let segmented = container(
        row![
            cell(
                t!(i18n, ids::EDITORS_SYMBOL_BODY_NORMAL),
                view.body == BodyStyle::Normal,
                sym(SymbolEditorMsg::BodyStyleSelected(BodyStyle::Normal)),
            ),
            cell(
                t!(i18n, ids::EDITORS_SYMBOL_BODY_ALTERNATE),
                view.body == BodyStyle::Alternate,
                sym(SymbolEditorMsg::BodyStyleSelected(BodyStyle::Alternate)),
            ),
        ]
        .spacing(crate::styles::Spacing::DEFAULT.xs),
    )
    .padding(crate::styles::Spacing::DEFAULT.xs)
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(sunken)),
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..iced::widget::container::Style::default()
    });
    let max_part = sym_state::max_part_number(editor.primitive());
    let save_label = if editor.is_dirty() {
        ids::EDITORS_SYMBOL_SAVE_DIRTY
    } else {
        ids::EDITORS_SYMBOL_SAVE
    };
    let save = button(text(t!(i18n, save_label)).size(TS.caption))
        .padding([SP.xs, SP.l])
        .on_press(send(PrimitiveEdit::Save))
        .style(crate::styles::primary_button(tokens));

    let body_row = row![
        text(t!(i18n, ids::EDITORS_SYMBOL_BODY))
            .size(TS.caption)
            .color(ink3),
        segmented,
        ghost(
            t!(i18n, ids::EDITORS_SYMBOL_BODY_BOTH),
            sym(SymbolEditorMsg::SelectionBodyAssigned(
                signex_library::BODY_COMMON
            )),
        ),
        ghost(
            t!(i18n, ids::EDITORS_SYMBOL_BODY_THIS_ONLY),
            sym(SymbolEditorMsg::SelectionBodyAssigned(view.body_tag())),
        ),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    let part_row = row![
        ghost(
            t!(i18n, ids::EDITORS_SYMBOL_PREVIOUS_PART),
            sym(SymbolEditorMsg::PrevPart)
        ),
        text(t!(
            i18n,
            ids::EDITORS_SYMBOL_PART_OF,
            "part" => u32::from(editor.active_part),
            "parts" => u32::from(max_part)
        ))
        .size(TS.body)
        .color(ink),
        ghost(
            t!(i18n, ids::EDITORS_SYMBOL_NEXT_PART),
            sym(SymbolEditorMsg::NextPart)
        ),
        ghost(
            t!(i18n, ids::EDITORS_SYMBOL_ADD_PART),
            sym(SymbolEditorMsg::NewPart)
        ),
        ghost(
            t!(i18n, ids::EDITORS_SYMBOL_REMOVE_PART),
            sym(SymbolEditorMsg::RemovePart)
        ),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    // A copy of a stock symbol can be made again from the editor (#813).
    let part_row = match &editor.file.copied_from {
        Some(_) => part_row.push(
            ghost(
                t!(i18n, ids::EDITORS_SYMBOL_COPY_AGAIN),
                LibraryMessage::StockCopy(
                    crate::library::stock_copy::StockCopyMsg::CopyAgainRequested(
                        path.to_path_buf(),
                    ),
                ),
            )
            .boxed(),
        ),
        None => part_row,
    }
    .push(Space::new().width(SP.m).boxed())
    .push(save.boxed());
    container(column![body_row, part_row].spacing(SP.s))
        .padding(SP.m)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(raised)),
            border: Border {
                width: crate::styles::HAIRLINE,
                radius: crate::styles::Radii::GRAPHITE.card.into(),
                color: line,
                ..Border::default()
            },
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

/// Graphite zoom pill of the symbol editor. It zooms about the symbol origin
/// (the canvas size is only known to the canvas) and Fit queues a fit.
fn graphite_zoom_pill<'a>(
    editor: &SymbolDocument,
    path: &std::path::Path,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let anchor = editor.camera.offset;
    let sym = |m: SymbolEditorMsg| LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Symbol(m),
    };
    let zoom = |factor: f32| {
        sym(SymbolEditorMsg::ZoomFactor {
            sx: anchor.x,
            sy: anchor.y,
            factor,
        })
    };
    crate::canvas::zoom_pill::view(
        tokens,
        editor.camera.zoom_percent(),
        zoom(1.0 / ZOOM_BUTTON_STEP),
        zoom(ZOOM_BUTTON_STEP),
        sym(SymbolEditorMsg::Fit),
    )
}

/// Scale factor of one zoom-pill click.
const ZOOM_BUTTON_STEP: f32 = 1.25;

/// The canvas program the view draws: the editor's document and the window's
/// display scale (#1887).
fn symbol_canvas<'a>(
    editor: &'a SymbolDocument,
    panel_ctx: &'a PanelContext,
    display: LibraryDisplaySettings,
    device_px_per_logical_px: f32,
) -> SymbolCanvas<'a> {
    let tokens = &panel_ctx.tokens;
    SymbolCanvas::new(
        editor.primitive(),
        editor.selected.clone(),
        editor.tool,
        &editor.polygon_vertices,
        editor.view(),
        editor.context_menu.is_some(),
        &editor.camera,
        crate::canvas::grid::pitch_mm_exact(display.grid_size_mm),
        display.grid_visible,
        panel_ctx.symbol_grid_style,
        display.pin_selection.allows_label_grab(),
        display.sheet_color.to_color(),
        crate::styles::ti(tokens.accent),
        crate::styles::ti(tokens.text),
        crate::styles::ti(tokens.text),
        iced::Color {
            a: 0.18,
            ..crate::styles::ti(tokens.text_secondary)
        },
    )
    .with_graphite(tokens)
    .with_selection_filter(editor.selection_filter)
    .with_view_pending(editor.pending_view.is_some())
    .with_trackpad_pan(editor.trackpad_pan)
    .with_wheel_mode(editor.wheel_mode)
    .with_device_px_per_logical_px(device_px_per_logical_px)
}

fn view_symbol_canvas<'a>(
    editor: &'a SymbolDocument,
    panel_ctx: &'a PanelContext,
    display: LibraryDisplaySettings,
    device_px_per_logical_px: f32,
) -> Element<'a, LibraryMessage> {
    let program = symbol_canvas(editor, panel_ctx, display, device_px_per_logical_px);
    let path = editor.path.clone();
    iced::widget::Canvas::<_, sym_canvas::CanvasAction>::new(program)
        .width(Length::Fill)
        .height(Length::Fill)
        .map(move |action| LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Symbol(symbol_action_to_primitive_msg(action)),
        })
        .boxed()
}

pub(crate) fn symbol_action_to_primitive_msg(action: sym_canvas::CanvasAction) -> SymbolEditorMsg {
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
        CanvasAction::ArcSweepRejected => SymbolEditorMsg::ArcSweepRejected,
        CanvasAction::AddText { x, y } => SymbolEditorMsg::AddText { x, y },
        CanvasAction::PolygonClick { x, y } => SymbolEditorMsg::PolygonClick { x, y },
        CanvasAction::PolygonCommit => SymbolEditorMsg::PolygonCommit,
        CanvasAction::PolygonCancel => SymbolEditorMsg::PolygonCancel,
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
        CanvasAction::ZoomFactor { sx, sy, factor } => {
            SymbolEditorMsg::ZoomFactor { sx, sy, factor }
        }
        CanvasAction::Fit => SymbolEditorMsg::Fit,
        CanvasAction::ViewportMeasured {
            width,
            height,
            then,
        } => SymbolEditorMsg::ViewportMeasured {
            width,
            height,
            then: then.map(|action| Box::new(symbol_action_to_primitive_msg(*action))),
        },
        CanvasAction::CursorAt { x_mm, y_mm } => SymbolEditorMsg::CursorAt { x_mm, y_mm },
        CanvasAction::DragCommit => SymbolEditorMsg::DragCommit,
        CanvasAction::Undo => SymbolEditorMsg::Undo,
        CanvasAction::Redo => SymbolEditorMsg::Redo,
        CanvasAction::ShowContextMenu { x, y, target } => SymbolEditorMsg::ShowContextMenu {
            x,
            y,
            target: symbol_context_target_to_msg(target),
        },
        CanvasAction::CloseContextMenu => SymbolEditorMsg::CloseContextMenu,
        CanvasAction::LeaveTool => SymbolEditorMsg::SetTool(SymbolToolMsg::Select),
    }
}

fn symbol_context_target_to_msg(
    target: sym_state::SymbolContextTarget,
) -> crate::library::messages::SymbolContextTargetMsg {
    use crate::library::messages::SymbolContextTargetMsg;
    use sym_state::SymbolContextTarget;
    match target {
        SymbolContextTarget::Empty => SymbolContextTargetMsg::Empty,
        SymbolContextTarget::Pin(idx) => SymbolContextTargetMsg::Pin(idx),
        SymbolContextTarget::Graphic(idx) => SymbolContextTargetMsg::Graphic(idx),
        SymbolContextTarget::Field(key) => SymbolContextTargetMsg::Field(key),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The view hands the window's display scale to the canvas it draws.
    #[test]
    fn the_view_builds_the_canvas_with_the_windows_display_scale() {
        let (app, _task) = crate::app::Signex::new();
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
        let editor = SymbolDocument::new(std::path::PathBuf::from("s.snxsym"), file);
        let panel_ctx = &app.document_state.panel_ctx;
        let build = |dpi| symbol_canvas(&editor, panel_ctx, LibraryDisplaySettings::default(), dpi);
        assert_eq!(build(1.0).device_px_per_logical_px, 1.0);
        assert_eq!(build(2.0).device_px_per_logical_px, 2.0);
    }
}
