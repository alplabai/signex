//! Preview tab — read-only Symbol + Footprint render side-by-side,
//! inline Pin Map subsection, Where-Used footer line.
//!
//! The Preview surface is purely read-only for primitives;
//! right-click on either render → context menu fires
//! [`crate::library::messages::LibraryMessage::OpenPrimitiveEditor`]
//! to open the standalone `.snxsym` / `.snxfpt` document tab.
//!
//! The Symbol render uses a minimal preview canvas — no pan/zoom, no
//! drag/drop, just an auto-fit body rectangle + pin stubs. Same for
//! the Footprint render: pad outlines on a flat board with no input
//! capture.

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Border, Element, Length, Theme};
use signex_library::{Footprint, Symbol, SymbolGraphicKind};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress, LibraryState};

const PREVIEW_PANE_HEIGHT: f32 = 220.0;

pub fn view<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let body = column![
        header_row(state, tokens),
        Space::new().height(10),
        render_panes(state, tokens, &address),
        Space::new().height(10),
        pin_map_subsection(state, tokens, &address),
        Space::new().height(10),
        where_used_footer(state, library_state, tokens),
    ]
    .spacing(0)
    .width(Length::Fill);

    container(scrollable(body).width(Length::Fill).height(Length::Fill))
        .padding(14)
        .style(crate::styles::modal_card(tokens))
        .into()
}

fn header_row<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    row![
        text(state.row.internal_pn.as_str()).size(15).color(text_c),
        Space::new().width(12),
        text(format!("class: {}", state.row.class.as_str()))
            .size(11)
            .color(muted),
        Space::new().width(12),
        text(format!("{:?}", state.row.state)).size(11).color(muted),
        Space::new().width(Length::Fill),
        text(format!(
            "MPN: {} / {}",
            state.row.primary_mpn.manufacturer, state.row.primary_mpn.mpn
        ))
        .size(11)
        .color(muted),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

fn render_panes<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let _ = address; // address only flows into messages from sub-helpers below.
    let symbol_msg = Some(LibraryMessage::OpenPrimitiveEditor {
        path: symbol_path(state),
    });
    let footprint_msg =
        footprint_path(state).map(|p| LibraryMessage::OpenPrimitiveEditor { path: p });

    let symbol_pane = render_pane(
        "Symbol",
        symbol_summary_text(state.symbol.as_ref()),
        symbol_msg,
        "Open Symbol Editor",
        tokens,
    );
    let footprint_pane = render_pane(
        "Footprint",
        footprint_summary_text(state.footprint.as_ref()),
        footprint_msg,
        "Open Footprint Editor",
        tokens,
    );

    row![
        container(symbol_pane)
            .width(Length::FillPortion(1))
            .height(Length::Fixed(PREVIEW_PANE_HEIGHT)),
        Space::new().width(8),
        container(footprint_pane)
            .width(Length::FillPortion(1))
            .height(Length::Fixed(PREVIEW_PANE_HEIGHT)),
    ]
    .align_y(iced::Alignment::Start)
    .into()
}

fn render_pane<'a>(
    label: &'a str,
    summary: String,
    open_msg: Option<LibraryMessage>,
    button_label: &'a str,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = row![
        text(label).size(11).color(muted),
        Space::new().width(Length::Fill),
        open_btn(button_label, open_msg, tokens),
    ]
    .align_y(iced::Alignment::Center);

    let body = container(text(summary).size(11).color(text_c))
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..Default::default()
        });

    column![header, Space::new().height(6), body]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn symbol_summary_text(sym: Option<&Symbol>) -> String {
    match sym {
        None => "Unresolved — symbol primitive not loaded.\n\nLoad happens lazily on tab switch; if this persists, the bound `symbol_ref.uuid` may not exist in any mounted library.".to_string(),
        Some(sym) => {
            let body = sym
                .graphics
                .iter()
                .find_map(|g| match &g.kind {
                    SymbolGraphicKind::Rectangle { from, to } => Some((from, to)),
                    _ => None,
                })
                .map(|(f, t)| format!("body: ({:.2}, {:.2}) → ({:.2}, {:.2}) mm\n", f[0], f[1], t[0], t[1]))
                .unwrap_or_else(|| "body: <no rectangle graphic>\n".to_string());
            let pins = sym
                .pins
                .iter()
                .take(8)
                .map(|p| format!("  pin {}  {}  ({}, {})", p.number, p.name, p.position[0], p.position[1]))
                .collect::<Vec<_>>()
                .join("\n");
            let more = if sym.pins.len() > 8 {
                format!("\n  … +{} more pins", sym.pins.len() - 8)
            } else {
                String::new()
            };
            format!(
                "{}\n{} pins:\n{}{}",
                body,
                sym.pins.len(),
                pins,
                more,
            )
        }
    }
}

fn footprint_summary_text(fp: Option<&Footprint>) -> String {
    match fp {
        None => "Unresolved — footprint primitive not loaded.\n\nLoad happens lazily on tab switch; if this persists, the bound `footprint_ref.uuid` may not exist in any mounted library.".to_string(),
        Some(fp) => {
            let pads = fp
                .pads
                .iter()
                .take(8)
                .map(|p| format!("  pad {}  ({:.2}, {:.2}) mm", p.number, p.position[0], p.position[1]))
                .collect::<Vec<_>>()
                .join("\n");
            let more = if fp.pads.len() > 8 {
                format!("\n  … +{} more pads", fp.pads.len() - 8)
            } else {
                String::new()
            };
            format!(
                "name: {}\n{} pads:\n{}{}",
                fp.name,
                fp.pads.len(),
                pads,
                more,
            )
        }
    }
}

fn open_btn<'a>(
    label: &'a str,
    msg: Option<LibraryMessage>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let mut btn = button(
        container(
            text(label)
                .size(11)
                .color(if msg.is_some() { text_c } else { muted }),
        )
        .padding([3, 10]),
    );
    if let Some(m) = msg {
        btn = btn.on_press(m);
    }
    btn.padding(0)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => {
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                }
                _ => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04),
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

fn symbol_path(state: &ComponentPreviewState) -> std::path::PathBuf {
    state
        .library_path
        .join("symbols")
        .join(format!("{}.snxsym", state.row.symbol_ref.uuid))
}

fn footprint_path(state: &ComponentPreviewState) -> Option<std::path::PathBuf> {
    let fp_ref = state.row.footprint_ref.as_ref()?;
    Some(
        state
            .library_path
            .join("footprints")
            .join(format!("{}.snxfpt", fp_ref.uuid)),
    )
}

fn pin_map_subsection<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let mut col = column![
        text("Pin Map").size(11).color(muted),
        Space::new().height(6),
    ]
    .spacing(0)
    .width(Length::Fill);

    let Some(sym) = state.symbol.as_ref() else {
        col = col.push(
            text("Pin map view requires the symbol to be resolved.")
                .size(11)
                .color(muted),
        );
        return col.into();
    };

    if sym.pins.is_empty() {
        col = col.push(
            text("Symbol has no pins yet — open the Symbol Editor to add some.")
                .size(11)
                .color(muted),
        );
        return col.into();
    }

    // Header row.
    col = col.push(
        row![
            text("Sym pin")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(1)),
            text("Sym name")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            text("Pad")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(1)),
            text("").size(10).width(Length::FillPortion(1)),
        ]
        .align_y(iced::Alignment::Center)
        .padding([2, 4]),
    );

    for pin in &sym.pins {
        let override_pad = state
            .row
            .pin_map_overrides
            .iter()
            .find(|o| o.symbol_pin_number == pin.number)
            .map(|o| o.footprint_pad_number.clone())
            .unwrap_or_else(|| pin.number.clone());
        let is_override = state
            .row
            .pin_map_overrides
            .iter()
            .any(|o| o.symbol_pin_number == pin.number);

        let pad_label = if is_override {
            format!("{} (override)", override_pad)
        } else {
            override_pad.clone()
        };

        col = col.push(
            row![
                text(pin.number.clone())
                    .size(11)
                    .color(text_c)
                    .width(Length::FillPortion(1)),
                text(pin.name.clone())
                    .size(11)
                    .color(text_c)
                    .width(Length::FillPortion(2)),
                text(pad_label)
                    .size(11)
                    .color(text_c)
                    .width(Length::FillPortion(1)),
                container(override_action_btn(
                    pin.number.clone(),
                    is_override,
                    tokens,
                    address,
                ))
                .width(Length::FillPortion(1)),
            ]
            .align_y(iced::Alignment::Center)
            .padding([2, 4]),
        );

        // Inline override editor for the expanded row.
        if state.pin_map_state.expanded_row.as_deref() == Some(pin.number.as_str()) {
            col = col.push(override_editor_row(
                &pin.number,
                &state.pin_map_state.override_buf,
                tokens,
                address,
            ));
        }
    }

    col.into()
}

fn override_action_btn<'a>(
    pin_number: String,
    is_override: bool,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let lib_path = address.library_path.clone();
    let table = address.table.clone();
    let row_id = address.row_id;

    let label = if is_override {
        "[Edit / Remove]"
    } else {
        "[Override]"
    };

    let pin_for_msg = pin_number.clone();
    button(container(text(label).size(11).color(text_c)).padding([2, 8]))
        .padding(0)
        .on_press_with(move || LibraryMessage::EditorEvent {
            library_path: lib_path.clone(),
            table: table.clone(),
            row_id,
            msg: EditorMsg::PinMapOpenOverrideEdit(pin_for_msg.clone()),
        })
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => {
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                }
                _ => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.03),
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

fn override_editor_row<'a>(
    pin_number: &str,
    buf: &str,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let pin_owned = pin_number.to_string();
    let lib_path_input = address.library_path.clone();
    let table_input = address.table.clone();
    let row_id = address.row_id;

    let pin_for_input = pin_owned.clone();
    let input = iced::widget::text_input("pad number…", buf)
        .on_input(move |s| LibraryMessage::EditorEvent {
            library_path: lib_path_input.clone(),
            table: table_input.clone(),
            row_id,
            msg: EditorMsg::PinMapOverrideBufChanged {
                pin: pin_for_input.clone(),
                value: s,
            },
        })
        .padding([4, 8])
        .size(11)
        .width(Length::Fixed(120.0));

    let lib_path_save = address.library_path.clone();
    let table_save = address.table.clone();
    let pin_for_save = pin_owned.clone();
    let buf_for_save = buf.to_string();
    let save_btn =
        button(container(text("Save").size(11).color(iced::Color::WHITE)).padding([3, 10]))
            .on_press(LibraryMessage::EditorEvent {
                library_path: lib_path_save,
                table: table_save,
                row_id,
                msg: EditorMsg::PinMapAddOverride {
                    pin: pin_for_save,
                    pad: buf_for_save,
                },
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
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

    let lib_path_cancel = address.library_path.clone();
    let table_cancel = address.table.clone();
    let cancel_btn = button(container(text("Cancel").size(11).color(text_c)).padding([3, 10]))
        .on_press(LibraryMessage::EditorEvent {
            library_path: lib_path_cancel,
            table: table_cancel,
            row_id,
            msg: EditorMsg::PinMapCancelOverrideEdit,
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: text_c,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        });

    let lib_path_remove = address.library_path.clone();
    let table_remove = address.table.clone();
    let pin_for_remove = pin_owned;
    let remove_btn = button(container(text("Remove").size(11).color(muted)).padding([3, 10]))
        .on_press(LibraryMessage::EditorEvent {
            library_path: lib_path_remove,
            table: table_remove,
            row_id,
            msg: EditorMsg::PinMapRemoveOverride {
                pin: pin_for_remove,
            },
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: muted,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        });

    container(
        row![
            text(format!("override pin {pin_number} → "))
                .size(11)
                .color(muted),
            input,
            Space::new().width(8),
            save_btn,
            Space::new().width(4),
            cancel_btn,
            Space::new().width(4),
            remove_btn,
        ]
        .align_y(iced::Alignment::Center)
        .spacing(0),
    )
    .padding([4, 14])
    .into()
}

fn where_used_footer<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let row_id = signex_library::RowId::from_uuid(state.row.row_id);
    let uses = library_state.where_used_for(row_id);
    let label = if uses.is_empty() {
        "Used by 0 sheets in 0 projects.".to_string()
    } else {
        let project_count = uses
            .iter()
            .map(|u| &u.project_path)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        format!(
            "Used by {} sheet{} in {} project{}.",
            uses.len(),
            if uses.len() == 1 { "" } else { "s" },
            project_count,
            if project_count == 1 { "" } else { "s" },
        )
    };

    text(label).size(10).color(muted).into()
}
