//! Standalone primitive editor tabs (`.snxsym` / `.snxfpt`).
//!
//! Renders `.snxsym` / `.snxfpt` files as main-window document tabs
//! using the canvas editor programs from
//! [`crate::library::editor::symbol`] and
//! [`crate::library::editor::footprint`] verbatim. Canvas events flow
//! through `LibraryMessage::PrimitiveEditorEvent { path, msg }` —
//! distinct from the Component Preview tab's
//! `LibraryMessage::EditorEvent { library_path, table, row_id, msg }`.
//!
//! Primitives are first-class document tabs alongside `.snxsch` /
//! `.snxpcb`. Dirty-tracking is keyed by `path`; persistence is plain
//! JSON via serde. The Save action lives on the parent dispatcher
//! (`save_primitive_tab_at`); the editor view only owns the widget
//! tree.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::app::{FootprintEditorState, SymbolEditorState};
use crate::library::editor::footprint::canvas::FootprintCanvas;
use crate::library::editor::footprint::layers::FpLayer;
use crate::library::editor::symbol::canvas::{self as sym_canvas, SymbolCanvas, SymbolTool};
use crate::library::editor::symbol::state as sym_state;
use crate::library::messages::{
    EditorMsg, GraphicHandleMsg, LibraryMessage, PrimitiveEditorMsg, SymbolSelectionMsg,
    SymbolToolMsg,
};

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
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    let toolbar = view_symbol_toolbar(editor, tokens);
    let canvas_widget = view_symbol_canvas(editor);

    let body = column![toolbar, canvas_widget]
        .spacing(10)
        .width(Length::Fill)
        .height(Length::Fill);

    let status_line = row![
        text(format!("{} pins", editor.primitive().pins.len()))
            .size(11)
            .color(muted),
        Space::new().width(Length::Fill),
        text(if editor.selected.is_some() {
            "Press Delete to remove the selected element."
        } else {
            "Click a pin to select; drag to move."
        })
        .size(11)
        .color(muted),
    ];

    let outer = column![body, Space::new().height(8), status_line]
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

fn view_symbol_toolbar<'a>(
    editor: &'a SymbolEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let path = editor.path.clone();

    let tool_button =
        |label: &str, tool: SymbolTool, msg: SymbolToolMsg| -> Element<'a, LibraryMessage> {
            let path_for_press = path.clone();
            button(text(label.to_string()).size(11).color(text_c))
                .padding([4, 10])
                .on_press(LibraryMessage::PrimitiveEditorEvent {
                    path: path_for_press,
                    msg: PrimitiveEditorMsg::SymbolSetTool(msg),
                })
                .style(symbol_tool_button_style(editor.tool == tool, border))
                .into()
        };

    let save_path = path.clone();
    let save_btn = button(
        text(if editor.dirty { "Save *" } else { "Save" })
            .size(11)
            .color(text_c),
    )
    .padding([4, 10])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: save_path,
        msg: PrimitiveEditorMsg::Save,
    })
    .style(symbol_tool_button_style(false, border));

    // Active Part picker. `← Part X / N →` reads + steps the active
    // sub-part. Arrows clamp at 1 / max — Tools ▸ New Part is the
    // way to add new parts, not the right arrow (mirrors Altium).
    let max_part = crate::library::editor::symbol::state::max_part_number(editor.primitive());
    let active_part = editor.active_part;
    let prev_path = path.clone();
    let next_path = path.clone();
    let prev_btn = button(text("\u{2190}").size(11).color(text_c))
        .padding([4, 8])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: prev_path,
            msg: PrimitiveEditorMsg::SymbolPrevPart,
        })
        .style(symbol_tool_button_style(false, border));
    let next_btn = button(text("\u{2192}").size(11).color(text_c))
        .padding([4, 8])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: next_path,
            msg: PrimitiveEditorMsg::SymbolNextPart,
        })
        .style(symbol_tool_button_style(false, border));
    let part_label = text(format!("Part {active_part} / {max_part}"))
        .size(11)
        .color(text_c);

    container(
        row![
            tool_button("Select", SymbolTool::Select, SymbolToolMsg::Select),
            tool_button("Add Pin", SymbolTool::AddPin, SymbolToolMsg::AddPin),
            Space::new().width(8),
            tool_button(
                "Rectangle",
                SymbolTool::PlaceRectangle,
                SymbolToolMsg::PlaceRectangle,
            ),
            tool_button("Line", SymbolTool::PlaceLine, SymbolToolMsg::PlaceLine),
            tool_button(
                "Circle",
                SymbolTool::PlaceCircle,
                SymbolToolMsg::PlaceCircle
            ),
            Space::new().width(Length::Fill),
            prev_btn,
            part_label,
            next_btn,
            Space::new().width(8),
            save_btn,
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

fn view_symbol_canvas<'a>(editor: &'a SymbolEditorState) -> Element<'a, LibraryMessage> {
    let program = SymbolCanvas::new(
        editor.primitive(),
        editor.selected,
        editor.tool,
        editor.active_part,
    );
    let widget: Element<'a, sym_canvas::CanvasAction> = iced::widget::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    let path = editor.path.clone();
    widget.map(move |action| LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: symbol_action_to_primitive_msg(action),
    })
}

fn symbol_action_to_primitive_msg(action: sym_canvas::CanvasAction) -> PrimitiveEditorMsg {
    use sym_canvas::CanvasAction;
    match action {
        CanvasAction::AddPin { x, y } => PrimitiveEditorMsg::SymbolAddPin { x, y },
        CanvasAction::AddRectangle { x, y } => PrimitiveEditorMsg::SymbolAddRectangle { x, y },
        CanvasAction::AddLine { x, y } => PrimitiveEditorMsg::SymbolAddLine { x, y },
        CanvasAction::AddCircle { x, y } => PrimitiveEditorMsg::SymbolAddCircle { x, y },
        CanvasAction::Select(sel) => PrimitiveEditorMsg::SymbolSelect(symbol_selection_to_msg(sel)),
        CanvasAction::Deselect => PrimitiveEditorMsg::SymbolDeselect,
        CanvasAction::Move { x, y } => PrimitiveEditorMsg::SymbolMoveSelected { x, y },
        CanvasAction::MoveGraphicHandle { idx, handle, x, y } => {
            PrimitiveEditorMsg::SymbolMoveGraphicHandle {
                idx,
                handle: graphic_handle_to_msg(handle),
                x,
                y,
            }
        }
        CanvasAction::DeleteSelected => PrimitiveEditorMsg::SymbolDeleteSelected,
    }
}

fn graphic_handle_to_msg(handle: sym_state::GraphicHandle) -> GraphicHandleMsg {
    use sym_state::GraphicHandle;
    match handle {
        GraphicHandle::RectCorner(c) => GraphicHandleMsg::RectCorner(c),
        GraphicHandle::LineEndpoint(e) => GraphicHandleMsg::LineEndpoint(e),
        GraphicHandle::CircleRadius => GraphicHandleMsg::CircleRadius,
        GraphicHandle::ArcStart => GraphicHandleMsg::ArcStart,
        GraphicHandle::ArcEnd => GraphicHandleMsg::ArcEnd,
        GraphicHandle::TextAnchor => GraphicHandleMsg::TextAnchor,
    }
}

fn symbol_selection_to_msg(sel: sym_state::SymbolSelection) -> SymbolSelectionMsg {
    use sym_state::{FieldKey, SymbolSelection};
    match sel {
        SymbolSelection::Pin(idx) => SymbolSelectionMsg::Pin(idx),
        SymbolSelection::Field(FieldKey::Reference) => SymbolSelectionMsg::FieldReference,
        SymbolSelection::Field(FieldKey::Value) => SymbolSelectionMsg::FieldValue,
        SymbolSelection::Graphic(idx) => SymbolSelectionMsg::Graphic(idx),
    }
}

// ── Footprint ───────────────────────────────────────────────────────

/// Render the standalone Footprint editor for a `.snxfpt` tab. Mirrors
/// the in-Component Editor footprint surface (toolbar + canvas +
/// footer) but skips the right-column Body 3D / 3D preview / STEP
/// attach panel — those edit Component-level fields that live on the
/// Footprint primitive's `body_3d` and `step_attachment` slots and the
/// view tree for them is reused via the Component Editor surface.
/// Pure pad-layout standalone editing is what `.snxfpt` needs first.
pub fn view_footprint<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let bg = crate::styles::ti(tokens.bg);
    let grid = crate::styles::ti(tokens.text_secondary);

    let toolbar = view_footprint_toolbar(editor, tokens);
    let canvas_area = view_footprint_canvas(editor, tokens, bg, grid);
    let footer = view_footprint_footer(editor, tokens);

    column![toolbar, canvas_area, footer]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_footprint_toolbar<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let layers = editor.state.layer_visibility;
    let auto_fit_on = editor.state.auto_fit_courtyard;

    let mut row_widget = row![text("Layers:").size(11).color(muted)]
        .spacing(6)
        .align_y(iced::Alignment::Center);

    for layer in FpLayer::ORDER {
        let on = layers.get(*layer);
        let swatch = layer.color();
        let label_color = if on { text_c } else { muted };
        let toggle_path = editor.path.clone();
        let layer_standard = layer.standard_name().to_string();
        let pill = button(
            row![
                container(text("").size(11))
                    .width(Length::Fixed(8.0))
                    .height(Length::Fixed(8.0))
                    .style(move |_: &Theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(swatch)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: iced::Color { a: 0.5, ..swatch },
                        },
                        ..iced::widget::container::Style::default()
                    }),
                text(layer.label()).size(11).color(label_color),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([3, 8])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: toggle_path,
            msg: PrimitiveEditorMsg::FootprintToggleLayer(layer_standard),
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: if on {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.05,
                )))
            } else {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.01,
                )))
            },
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: if on {
                    swatch
                } else {
                    iced::Color { a: 0.5, ..border }
                },
            },
            ..iced::widget::button::Style::default()
        });
        row_widget = row_widget.push(pill);
    }

    let auto_fit_path = editor.path.clone();
    let auto_fit_label = if auto_fit_on {
        "Auto-fit Courtyard \u{2713}"
    } else {
        "Auto-fit Courtyard"
    };
    let auto_fit_btn = button(text(auto_fit_label).size(11).color(text_c))
        .padding([3, 8])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: auto_fit_path,
            msg: PrimitiveEditorMsg::FootprintToggleAutoFit,
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        });

    let save_path = editor.path.clone();
    let save_btn = button(
        text(if editor.dirty { "Save *" } else { "Save" })
            .size(11)
            .color(text_c),
    )
    .padding([3, 8])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: save_path,
        msg: PrimitiveEditorMsg::Save,
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        ..iced::widget::button::Style::default()
    });

    row_widget = row_widget
        .push(Space::new().width(Length::Fill))
        .push(auto_fit_btn)
        .push(Space::new().width(8))
        .push(save_btn);

    container(row_widget)
        .padding([6, 10])
        .style(crate::styles::tab_bar_strip(tokens))
        .into()
}

fn view_footprint_canvas<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
    bg: iced::Color,
    grid: iced::Color,
) -> Element<'a, LibraryMessage> {
    let border = theme_ext::border_color(tokens);

    // The canvas program publishes `LibraryMessage::EditorEvent { ...,
    // msg: EditorMsg::Footprint* }`; we translate those into the
    // standalone primitive-editor envelope via `Element::map`. The
    // `EditorAddress` we stamp on the program is a sentinel — its
    // `library_path` field is the tab path (so dirty-tracking still
    // resolves), and the `table` / `row_id` are nil-shaped since
    // standalone tabs don't carry a Component.
    let address = crate::library::state::EditorAddress::new(
        editor.path.clone(),
        String::new(),
        signex_library::RowId::from_uuid(uuid::Uuid::nil()),
    );
    let prog = FootprintCanvas {
        state: &editor.state,
        address,
        bg_color: bg,
        grid_color: grid,
        cache: &editor.canvas_cache,
    };
    let canvas_widget: Element<'a, LibraryMessage> = iced::widget::canvas(prog)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    let path = editor.path.clone();
    let translated = canvas_widget.map(move |msg| match msg {
        LibraryMessage::EditorEvent { msg, .. } => LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: editor_msg_to_primitive_msg(msg),
        },
        other => other,
    });

    container(translated)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 0.0.into(),
                color: border,
            },
            ..iced::widget::container::Style::default()
        })
        .into()
}

/// Translate a Footprint canvas `EditorMsg` into the standalone
/// primitive-editor envelope. Only the canvas-emitted variants are
/// ever produced here — non-footprint variants fall through to a
/// no-op `Save` (the dispatcher discards on path-keyed lookup
/// mismatch anyway).
fn editor_msg_to_primitive_msg(msg: EditorMsg) -> PrimitiveEditorMsg {
    match msg {
        EditorMsg::FootprintAddPad { x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintAddPad { x_mm, y_mm }
        }
        EditorMsg::FootprintMovePad { idx, x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintMovePad { idx, x_mm, y_mm }
        }
        EditorMsg::FootprintCursorAt { x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintCursorAt { x_mm, y_mm }
        }
        EditorMsg::FootprintSelectPad(sel) => PrimitiveEditorMsg::FootprintSelectPad(sel),
        EditorMsg::FootprintDeleteSelected => PrimitiveEditorMsg::FootprintDeleteSelected,
        EditorMsg::FootprintToggleLayer(name) => PrimitiveEditorMsg::FootprintToggleLayer(name),
        EditorMsg::FootprintToggleAutoFit => PrimitiveEditorMsg::FootprintToggleAutoFit,
        // Anything not emitted by the footprint canvas is dropped via
        // a benign "save of the wrong tab" — the path-keyed dispatcher
        // ignores mismatches.
        _ => PrimitiveEditorMsg::Save,
    }
}

fn view_footprint_footer<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let cursor_label = match editor.state.cursor_mm {
        Some((x, y)) => format!("X {x:>+8.3} mm   Y {y:>+8.3} mm"),
        None => "X    -.--- mm   Y    -.--- mm".to_string(),
    };
    let pad_count = editor.state.pads.len();
    let selected_label = match editor.state.selected_pad {
        Some(i) => match editor.state.pads.get(i) {
            Some(pad) => format!(
                "Pad {} — {:.2} × {:.2} mm @ ({:+.3}, {:+.3})",
                pad.number, pad.size_mm.0, pad.size_mm.1, pad.position_mm.0, pad.position_mm.1
            ),
            None => format!("Pads: {pad_count}"),
        },
        None => {
            format!("Pads: {pad_count}   ·   Click empty area to add, drag to move, Del to remove")
        }
    };

    container(
        row![
            text(cursor_label).size(11).color(muted),
            Space::new().width(20),
            text(selected_label).size(11).color(text_c),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 12])
    .style(crate::styles::modal_footer_strip(tokens))
    .width(Length::Fill)
    .into()
}
