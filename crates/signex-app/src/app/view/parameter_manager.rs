//! The Parameter Manager table: one row per placed symbol, one column per
//! custom parameter in use. Cells write through as you type (one undo step
//! per cell); each parameter column carries a delete button.
//!
//! Edits go through `ParameterManagerMsg`, so they are undoable and
//! dirty-flagged like any other schematic edit.

use super::*;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Whether a row survives the filter: a case-insensitive substring of its
/// reference, value or footprint. An empty filter keeps every row.
fn row_matches_filter(filter: &str, sym: &signex_types::schematic::Symbol) -> bool {
    let needle = filter.trim().to_lowercase();
    needle.is_empty()
        || [&sym.reference, &sym.value, &sym.footprint]
            .iter()
            .any(|field| field.to_lowercase().contains(&needle))
}

impl Signex {
    pub(super) fn view_parameter_manager_body(&self) -> Element<'_, Message> {
        use iced::widget::{button, column, container, row, scrollable, text, text_input};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let table = &self.ui_state.parameter_table;

        let Some(engine) = self.document_state.active_engine() else {
            return container(
                column![
                    container(text("No active schematic.").size(TS.body).color(text_muted))
                        .padding([14, 14]),
                ]
                .width(iced::Length::Fixed(900.0))
                .height(iced::Length::Fixed(560.0)),
            )
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .into();
        };
        let doc = engine.document();

        // Only the parameters someone actually uses get a column.
        let mut keys: Vec<String> = doc
            .symbols
            .iter()
            .filter(|s| !s.is_power)
            .flat_map(|s| s.fields.keys().cloned())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        keys.sort();

        let toolbar = row![
            text_input("Filter rows", &table.filter)
                .on_input(
                    |text| Message::ParameterManager(ParameterManagerMsg::FilterChanged(text))
                )
                .padding([SP.xs, 6.0])
                .size(TS.body)
                .width(iced::Length::Fixed(220.0)),
            text_input("New parameter", &table.new_key)
                .on_input(
                    |text| Message::ParameterManager(ParameterManagerMsg::NewKeyChanged(text))
                )
                .on_submit(Message::ParameterManager(ParameterManagerMsg::ColumnAdded))
                .padding([SP.xs, 6.0])
                .size(TS.body)
                .width(iced::Length::Fixed(180.0)),
            button(
                container(text("Add column").size(TS.body).color(text_c)).padding([SP.xs, 10.0])
            )
            .on_press(Message::ParameterManager(ParameterManagerMsg::ColumnAdded)),
        ]
        .spacing(SP.s)
        .padding([SP.s, SP.m]);

        let header_row = {
            let mut r = row![
                text("Reference")
                    .size(TS.caption)
                    .color(text_muted)
                    .width(iced::Length::Fixed(100.0)),
                text("Value")
                    .size(TS.caption)
                    .color(text_muted)
                    .width(iced::Length::Fixed(160.0)),
                text("Footprint")
                    .size(TS.caption)
                    .color(text_muted)
                    .width(iced::Length::Fixed(200.0)),
            ];
            for k in &keys {
                r = r.push(
                    row![
                        text(k.clone())
                            .size(TS.caption)
                            .color(text_muted)
                            .width(iced::Length::Fill),
                        button(text("×").size(TS.caption).color(text_c))
                            .padding([0, 6])
                            .on_press(Message::ParameterManager(
                                ParameterManagerMsg::ParameterRemoved { key: k.clone() }
                            )),
                    ]
                    .width(iced::Length::Fixed(140.0)),
                );
            }
            r.padding([SP.s, SP.m])
        };

        let mut rows_col = column![].spacing(SP.xs);
        rows_col = rows_col.push(header_row);
        for sym in doc
            .symbols
            .iter()
            .filter(|s| !s.is_power && row_matches_filter(&table.filter, s))
        {
            let mut r = row![
                text(sym.reference.clone())
                    .size(TS.body)
                    .color(text_c)
                    .width(iced::Length::Fixed(100.0)),
                text(sym.value.clone())
                    .size(TS.body)
                    .color(text_c)
                    .width(iced::Length::Fixed(160.0)),
                text(sym.footprint.clone())
                    .size(TS.body)
                    .color(text_muted)
                    .width(iced::Length::Fixed(200.0)),
            ];
            for k in &keys {
                let shown = sym.fields.get(k).cloned().unwrap_or_default();
                let sym_uuid = sym.uuid;
                let k_str = k.clone();
                r = r.push(
                    text_input("", &shown)
                        .on_input(move |text| {
                            Message::ParameterManager(ParameterManagerMsg::Edit {
                                symbol_uuid: sym_uuid,
                                key: k_str.clone(),
                                value: text,
                            })
                        })
                        .on_submit(Message::ParameterManager(
                            ParameterManagerMsg::CellSubmitted,
                        ))
                        .padding([SP.xs, 6.0])
                        .size(TS.body)
                        .width(iced::Length::Fixed(140.0)),
                );
            }
            rows_col = rows_col.push(r.padding([SP.xs, SP.m]));
        }

        container(
            column![
                toolbar,
                container(
                    scrollable(rows_col)
                        .direction(scrollable::Direction::Both {
                            vertical: scrollable::Scrollbar::default(),
                            horizontal: scrollable::Scrollbar::default(),
                        })
                        .height(iced::Length::Fill),
                )
                .padding([14, 14])
                .height(iced::Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    border: iced::Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border_c,
                    },
                    ..container::Style::default()
                }),
            ]
            .width(iced::Length::Fixed(900.0))
            .height(iced::Length::Fixed(560.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }
}
