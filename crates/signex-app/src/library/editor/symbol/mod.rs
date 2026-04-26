//! Symbol tab — interactive symbol editor scoped to a single library
//! component.
//!
//! WS-F refactor: the editor now mutates a typed
//! [`signex_library::Symbol`] primitive in-place. No more
//! `(symbol …)` sexpr round-trip — pin operations land directly on
//! `Symbol::pins: Vec<SymbolPin>`. Save dispatches
//! [`crate::library::messages::EditorMsg::SaveSymbol`] which routes
//! through the dispatcher's `save_symbol` helper to the `LibrarySet`.
//!
//! Tabs supported by the canvas:
//! * Select — drag pins, Delete to remove.
//! * Add Pin — click to drop a pin with auto-incremented number.
//!
//! Things deferred to a follow-up workstream:
//! * Pan / zoom (the canvas auto-fits the body).
//! * Designator + Value drag — those fields live on the Component
//!   binding now, not the Symbol primitive (WS-E owns rebuilding the
//!   on-canvas drag-edit flow).
//! * Multi-unit symbols, alternate body styles, line/arc/polygon tools.

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
use super::super::state::{ComponentEditorState, EditorAddress};
use canvas::SymbolCanvas;

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    let toolbar = view_toolbar(editor, tokens, window_id);
    let canvas_widget = view_canvas(
        &editor.symbol,
        editor.symbol_selected,
        editor.symbol_tool,
        window_id,
    );
    let props = view_properties(editor, tokens, window_id);

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

    let ai_preview = editor
        .symbol_ai_preview
        .as_ref()
        .map(|preview| view_ai_preview(preview, tokens, address.clone()));

    let mut body = column![toolbar].spacing(10).width(Length::Fill);
    if let Some(card) = ai_preview {
        body = body.push(card);
    }
    body = body.push(split);

    let status_line = row![
        text(format!("{} pins", editor.symbol.pins.len()))
            .size(11)
            .color(muted),
        Space::new().width(Length::Fill),
        text(if editor.symbol_selected.is_some() {
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

fn view_toolbar<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);

    let select_addr = address.clone();
    let add_pin_addr = address.clone();
    let ai_addr = address;

    let select_active = editor.symbol_tool == canvas::SymbolTool::Select;
    let add_pin_active = editor.symbol_tool == canvas::SymbolTool::AddPin;

    let select_btn = tool_button(
        "Select",
        select_active,
        text_c,
        border,
        LibraryMessage::EditorEvent {
            library_path: select_addr.library_path,
            component_id: select_addr.component_id,
            msg: EditorMsg::SymbolSetTool(super::super::messages::SymbolToolMsg::Select),
        },
    );
    let add_pin_btn = tool_button(
        "Add Pin",
        add_pin_active,
        text_c,
        border,
        LibraryMessage::EditorEvent {
            library_path: add_pin_addr.library_path,
            component_id: add_pin_addr.component_id,
            msg: EditorMsg::SymbolSetTool(super::super::messages::SymbolToolMsg::AddPin),
        },
    );

    let ai_btn = button(
        container(text("AI: From Datasheet PDF").size(11).color(text_c)).padding([4, 12]),
    )
    .on_press(LibraryMessage::EditorEvent {
        library_path: ai_addr.library_path,
        component_id: ai_addr.component_id,
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
            select_btn,
            Space::new().width(6),
            add_pin_btn,
            Space::new().width(Length::Fill),
            ai_btn,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 8])
    .into()
}

fn view_canvas<'a>(
    sym: &'a signex_library::Symbol,
    selected: Option<state::SymbolSelection>,
    tool: canvas::SymbolTool,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let program = SymbolCanvas::new(sym, selected, tool);
    let widget: Element<'a, canvas::CanvasAction> = iced::widget::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    widget.map(move |action| LibraryMessage::EditorEvent {
        window_id,
        msg: action_to_msg(action),
    })
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

fn view_properties<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let mut col = column![
        text("Symbol Properties").size(13).color(text_c),
        Space::new().height(6),
        text(format!("Name: {}", editor.symbol.name))
            .size(11)
            .color(text_c),
        text(format!("UUID: {}", editor.symbol.uuid))
            .size(10)
            .color(muted),
        Space::new().height(8),
        text("Pins").size(13).color(text_c),
        Space::new().height(6),
    ]
    .spacing(0)
    .width(Length::Fill);

    if editor.symbol.pins.is_empty() {
        col = col.push(
            text("No pins yet — switch to Add Pin and click on the canvas.")
                .size(11)
                .color(muted),
        );
    } else {
        col = col.push(view_pin_table(&editor.symbol.pins, window_id, tokens));
    }

    container(scrollable(col).width(Length::Fill).height(Length::Fill))
        .padding(10)
        .style(crate::styles::modal_card(tokens))
        .into()
}

fn view_pin_table<'a>(
    pins: &'a [signex_library::SymbolPin],
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

    for (idx, pin) in pins.iter().enumerate() {
        let kind = state::PinKind::from_electrical(pin.electrical);
        let row_widget = row![
            text(format!("{idx}"))
                .size(11)
                .color(text_c)
                .width(Length::Fixed(28.0)),
            text_input("", &pin.number)
                .on_input(move |s| LibraryMessage::EditorEvent {
                    library_path: num_addr.library_path.clone(),
                    component_id: num_addr.component_id,
                    msg: EditorMsg::SymbolSetPinNumber { idx, number: s },
                })
                .padding([2, 6])
                .size(11)
                .width(Length::FillPortion(2)),
            text_input("", &pin.name)
                .on_input(move |s| LibraryMessage::EditorEvent {
                    library_path: name_addr.library_path.clone(),
                    component_id: name_addr.component_id,
                    msg: EditorMsg::SymbolSetPinName { idx, name: s },
                })
                .padding([2, 6])
                .size(11)
                .width(Length::FillPortion(3)),
            text(kind.label())
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
    address: EditorAddress,
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

    let cancel_addr = address.clone();
    let apply_addr = address;

    let cancel_btn = button(container(text("Cancel").size(11).color(text_c)).padding([4, 12]))
        .on_press(LibraryMessage::EditorEvent {
            library_path: cancel_addr.library_path,
            component_id: cancel_addr.component_id,
            msg: EditorMsg::SymbolDismissAiPreview,
        });
    let apply_btn = button(
        container(text("Apply").size(11).color(iced::Color::WHITE)).padding([4, 14]),
    )
    .on_press(LibraryMessage::EditorEvent {
        library_path: apply_addr.library_path,
        component_id: apply_addr.component_id,
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
