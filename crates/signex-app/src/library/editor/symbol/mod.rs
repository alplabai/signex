//! Symbol tab — the minimum-viable interactive symbol editor scoped
//! to a single library component.
//!
//! Phase-1 scope (LIBRARY_PLAN §10):
//!
//! * Live canvas: pins + body rectangle parsed from
//!   `SchematicSide.symbol.sexpr` via [`state::SymbolDoc::parse`].
//! * Tools: Select (drag pins/fields, Delete to remove) and
//!   Add Pin (click to drop a pin with auto-incremented number).
//! * Body shape: rectangle only (line/arc/circle/polygon defer).
//! * Editable Designator + Value text fields (free-form drag on canvas
//!   plus a side text input).
//! * "AI-stub from datasheet" button — opens an `rfd` PDF picker,
//!   runs `signex_library::ai_stub::extract_pinout`, renders a preview,
//!   and replaces the current pin layout on Apply.
//! * Edits round-trip back to `SchematicSide.symbol.sexpr` via
//!   [`crate::library::messages::EditorMsg::SymbolEdited`].
//!
//! Things deferred to a follow-up workstream:
//! * Pan / zoom (the canvas auto-fits the body).
//! * Multi-unit symbols, alternate body styles.
//! * Free-form drawing tools (line, arc, circle, polygon).
//! * Pin metadata beyond `number` / `name` / coarse kind.

pub mod ai_stub;
pub mod canvas;
pub mod state;

#[cfg(test)]
mod tests;

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::ComponentEditorState;
use canvas::SymbolCanvas;
use state::SymbolDoc;

/// Render the Symbol tab. Pulls the editor's symbol document out of
/// `editor.symbol_doc` (the dispatcher rebuilds it whenever the
/// underlying sexpr changes) and lays the canvas + properties pane
/// side-by-side.
pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    // Toolbar — tool toggles + AI-from-PDF button.
    let toolbar = view_toolbar(editor, tokens, window_id);

    // Main split: canvas on the left, properties pane on the right.
    let canvas_widget = view_canvas(&editor.symbol_doc, editor.symbol_tool, window_id);
    let props = view_properties(&editor.symbol_doc, tokens, window_id);

    let split = row![
        container(canvas_widget)
            .width(Length::FillPortion(3))
            .height(Length::Fill),
        Space::new().width(10),
        container(props)
            .width(Length::FillPortion(2))
            .height(Length::Fill),
    ]
    .height(Length::Fill);

    // AI preview modal — drops in below the toolbar when active.
    let ai_preview = editor
        .symbol_ai_preview
        .as_ref()
        .map(|preview| view_ai_preview(preview, tokens, window_id));

    let mut body = column![toolbar].spacing(10).width(Length::Fill);
    if let Some(card) = ai_preview {
        body = body.push(card);
    }
    body = body.push(split);

    // Status line with pin count + confidence reminder.
    let status_line = row![
        text(format!("{} pins", editor.symbol_doc.pins.len()))
            .size(11)
            .color(muted),
        Space::new().width(Length::Fill),
        text(if editor.symbol_doc.selected.is_some() {
            "Press Delete to remove the selected element."
        } else {
            "Click a pin or field to select; drag to move."
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

fn view_toolbar<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);

    let tool_btn = |label: &'static str, tool: canvas::SymbolTool, active: bool| {
        let bg = if active {
            iced::Background::Color(iced::Color::from_rgb(0.00, 0.47, 0.84))
        } else {
            iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.05))
        };
        let fg = if active {
            iced::Color::WHITE
        } else {
            text_c
        };
        button(container(text(label).size(11).color(fg)).padding([4, 12]))
            .on_press(LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::SymbolSetTool(symbol_tool_msg(tool)),
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(bg),
                text_color: fg,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            })
    };

    let ai_btn = button(
        container(text("AI: From Datasheet PDF").size(11).color(text_c)).padding([4, 12]),
    )
    .on_press(LibraryMessage::EditorEvent {
        window_id,
        msg: EditorMsg::SymbolPickAiPdf,
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.30, 0.85, 0.95, 0.18,
        ))),
        text_color: text_c,
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: iced::Color::from_rgba(0.30, 0.85, 0.95, 0.55),
        },
        ..iced::widget::button::Style::default()
    });

    container(
        row![
            tool_btn(
                "Select",
                canvas::SymbolTool::Select,
                editor.symbol_tool == canvas::SymbolTool::Select,
            ),
            Space::new().width(6),
            tool_btn(
                "Add Pin",
                canvas::SymbolTool::AddPin,
                editor.symbol_tool == canvas::SymbolTool::AddPin,
            ),
            Space::new().width(Length::Fill),
            ai_btn,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 8])
    .into()
}

fn view_canvas<'a>(
    doc: &'a SymbolDoc,
    tool: canvas::SymbolTool,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    // The `iced::widget::canvas` constructor function shares its name
    // with our `canvas` submodule, so we reach for it via the
    // fully-qualified path.
    let program = SymbolCanvas::new(doc, tool);
    let widget: Element<'a, canvas::CanvasAction> = iced::widget::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    widget.map_with_window(window_id)
}

/// Helper trait so `iced::widget::Canvas<CanvasAction>` can be lifted
/// into `LibraryMessage` without a turbofish dance at every call site.
trait MapWithWindow<'a> {
    fn map_with_window(self, window_id: iced::window::Id) -> Element<'a, LibraryMessage>;
}

impl<'a> MapWithWindow<'a> for Element<'a, canvas::CanvasAction> {
    fn map_with_window(self, window_id: iced::window::Id) -> Element<'a, LibraryMessage> {
        self.map(move |action| LibraryMessage::EditorEvent {
            window_id,
            msg: action_to_msg(action),
        })
    }
}

fn action_to_msg(action: canvas::CanvasAction) -> EditorMsg {
    use canvas::CanvasAction;
    match action {
        CanvasAction::AddPin { x, y } => EditorMsg::SymbolAddPin { x, y },
        CanvasAction::Select(sel) => EditorMsg::SymbolSelect(selection_to_msg(sel)),
        CanvasAction::Deselect => EditorMsg::SymbolDeselect,
        CanvasAction::Move { x, y } => EditorMsg::SymbolMoveSelected { x, y },
        CanvasAction::DeleteSelected => EditorMsg::SymbolDeleteSelected,
    }
}

fn selection_to_msg(sel: state::SymbolSelection) -> super::super::messages::SymbolSelectionMsg {
    use super::super::messages::SymbolSelectionMsg;
    use state::{FieldKey as F, SymbolSelection as S};
    match sel {
        S::Pin(idx) => SymbolSelectionMsg::Pin(idx),
        S::Field(F::Reference) => SymbolSelectionMsg::FieldReference,
        S::Field(F::Value) => SymbolSelectionMsg::FieldValue,
    }
}

fn symbol_tool_msg(tool: canvas::SymbolTool) -> super::super::messages::SymbolToolMsg {
    use super::super::messages::SymbolToolMsg;
    match tool {
        canvas::SymbolTool::Select => SymbolToolMsg::Select,
        canvas::SymbolTool::AddPin => SymbolToolMsg::AddPin,
    }
}

fn view_properties<'a>(
    doc: &'a SymbolDoc,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let mut col = column![
        text("Symbol Properties").size(13).color(text_c),
        Space::new().height(6),
    ]
    .spacing(0)
    .width(Length::Fill);

    // Designator + Value text inputs.
    let designator_input = text_input("U?", &doc.designator.value)
        .on_input(move |s| LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::SymbolSetField {
                key: super::super::messages::FieldKeyMsg::Reference,
                value: s,
            },
        })
        .padding([4, 8])
        .size(12);
    let value_input = text_input("Value", &doc.value_field.value)
        .on_input(move |s| LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::SymbolSetField {
                key: super::super::messages::FieldKeyMsg::Value,
                value: s,
            },
        })
        .padding([4, 8])
        .size(12);

    col = col.push(text("Designator").size(10).color(muted));
    col = col.push(designator_input);
    col = col.push(Space::new().height(8));
    col = col.push(text("Value").size(10).color(muted));
    col = col.push(value_input);
    col = col.push(Space::new().height(12));
    col = col.push(text("Pins").size(13).color(text_c));
    col = col.push(Space::new().height(6));

    if doc.pins.is_empty() {
        col = col.push(
            text("No pins yet — switch to Add Pin and click on the canvas.")
                .size(11)
                .color(muted),
        );
    } else {
        col = col.push(view_pin_table(doc, window_id, tokens));
    }

    container(scrollable(col).width(Length::Fill).height(Length::Fill))
        .padding(10)
        .style(crate::styles::modal_card(tokens))
        .into()
}

fn view_pin_table<'a>(
    doc: &'a SymbolDoc,
    window_id: iced::window::Id,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let header = row![
        text("#").size(10).color(muted).width(Length::Fixed(28.0)),
        text("Number")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        text("Name")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(3)),
        text("Kind")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
    ]
    .spacing(6);

    let mut col = column![header, Space::new().height(4)].spacing(2);

    for (idx, pin) in doc.pins.iter().enumerate() {
        let row_widget = row![
            text(format!("{idx}"))
                .size(11)
                .color(text_c)
                .width(Length::Fixed(28.0)),
            text_input("", &pin.number)
                .on_input(move |s| LibraryMessage::EditorEvent {
                    window_id,
                    msg: EditorMsg::SymbolSetPinNumber { idx, number: s },
                })
                .padding([2, 6])
                .size(11)
                .width(Length::FillPortion(2)),
            text_input("", &pin.name)
                .on_input(move |s| LibraryMessage::EditorEvent {
                    window_id,
                    msg: EditorMsg::SymbolSetPinName { idx, name: s },
                })
                .padding([2, 6])
                .size(11)
                .width(Length::FillPortion(3)),
            text(pin.kind.label())
                .size(11)
                .color(muted)
                .width(Length::FillPortion(2)),
        ]
        .spacing(6);
        col = col.push(row_widget);
    }
    col.into()
}

fn view_ai_preview<'a>(
    preview: &'a ai_stub::AiPinoutPreview,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let confidence_pct = (preview.confidence * 100.0).round() as i32;

    let warn = if preview.is_low_confidence() {
        Some(
            text("Low confidence — review the suggested pinout manually before applying.")
                .size(11)
                .color(iced::Color::from_rgb(0.95, 0.62, 0.30)),
        )
    } else {
        None
    };

    let mut list = column![].spacing(2);
    if preview.pins.is_empty() {
        list = list.push(
            text("Heuristic recovered no pins. Try another datasheet or add pins manually.")
                .size(11)
                .color(muted),
        );
    } else {
        for p in &preview.pins {
            list = list.push(
                row![
                    text(p.number.clone())
                        .size(11)
                        .color(text_c)
                        .width(Length::FillPortion(1)),
                    text(p.name.clone())
                        .size(11)
                        .color(text_c)
                        .width(Length::FillPortion(3)),
                    text(p.kind.label())
                        .size(11)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                ]
                .spacing(6),
            );
        }
    }

    let cancel_btn = button(container(text("Cancel").size(11).color(text_c)).padding([4, 12]))
        .on_press(LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::SymbolDismissAiPreview,
        });
    let apply_btn = button(
        container(text("Apply").size(11).color(iced::Color::WHITE)).padding([4, 14]),
    )
    .on_press(LibraryMessage::EditorEvent {
        window_id,
        msg: EditorMsg::SymbolApplyAiPreview,
    })
    .style(|_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgb(
            0.00, 0.47, 0.84,
        ))),
        text_color: iced::Color::WHITE,
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    });

    let mut card = column![
        row![
            text(format!(
                "AI-stub preview — {} pins, confidence {}%",
                preview.pins.len(),
                confidence_pct,
            ))
            .size(12)
            .color(text_c),
            Space::new().width(Length::Fill),
            cancel_btn,
            Space::new().width(8),
            apply_btn,
        ]
        .align_y(iced::Alignment::Center),
        Space::new().height(6),
    ]
    .spacing(0)
    .width(Length::Fill);
    if let Some(w) = warn {
        card = card.push(w);
        card = card.push(Space::new().height(6));
    }
    card = card.push(list);
    container(card)
        .padding(10)
        .style(crate::styles::modal_card(tokens))
        .width(Length::Fill)
        .into()
}
