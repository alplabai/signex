//! Preview tab — header, read-only Symbol + Footprint panes, Pin Map
//! table, Alternate symbols and a Where-Used line, all built from the
//! shared dialog kit ([`super::kit`]).
//!
//! The Preview surface is purely read-only for primitives;
//! right-click on either render → context menu fires
//! [`crate::library::messages::LibraryMessage::OpenPrimitiveEditor`]
//! to open the standalone `.snxsym` / `.snxfpt` document tab.

use iced::widget::{Space, column, container, row, scrollable, text};
use iced::{Element, Length};
use signex_library::ComponentRow;
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress, LibraryState};
use super::kit::{self, SP, TS};

/// Height of the Symbol / Footprint panes: twenty body lines.
const PREVIEW_PANE_HEIGHT: f32 = TS.body * 20.0;

/// Shown in the header for a manufacturer / MPN the row does not carry.
const NOT_SET: &str = "Not set";
const NO_DESCRIPTION: &str = "No description";

pub fn view<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    canvas_colors: &CanvasColors,
    device_px_per_logical_px: f32,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let read_only = library_state.is_read_only(&state.library_path);
    let veil = super::read_only_veil(tokens);
    let body = column![
        header_row(&state.row, tokens),
        render_panes(
            state,
            library_state,
            tokens,
            canvas_colors,
            device_px_per_logical_px,
            &address,
            read_only,
        ),
        super::inert::unless_read_only_veiled(
            read_only,
            veil,
            pin_map_subsection(state, tokens, &address),
        ),
        super::inert::unless_read_only_veiled(
            read_only,
            veil,
            super::alternate_symbols::view(state, tokens, &address),
        ),
        where_used_footer(state, library_state, tokens),
    ]
    .spacing(SP.l)
    .width(Length::Fill);

    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Manufacturer and MPN as shown in the header; a blank one reads "Not set".
fn mpn_fields(manufacturer: &str, mpn: &str) -> (String, String) {
    let or_not_set = |v: &str| {
        let v = v.trim();
        if v.is_empty() { NOT_SET } else { v }.to_string()
    };
    (or_not_set(manufacturer), or_not_set(mpn))
}

fn description_text(description: &str) -> &str {
    let d = description.trim();
    if d.is_empty() { NO_DESCRIPTION } else { d }
}

/// The texts the Preview header card shows. The part number, class and
/// state are the shell header's (`editor::view_header`), not repeated here.
struct HeaderTexts {
    description: String,
    manufacturer: String,
    mpn: String,
}

fn header_texts(row_data: &ComponentRow) -> HeaderTexts {
    let (manufacturer, mpn) = mpn_fields(
        &row_data.primary_mpn.manufacturer,
        &row_data.primary_mpn.mpn,
    );
    HeaderTexts {
        description: description_text(&row_data.description).to_string(),
        manufacturer,
        mpn,
    }
}

fn header_row<'a>(row_data: &'a ComponentRow, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let HeaderTexts {
        description,
        manufacturer,
        mpn,
    } = header_texts(row_data);
    let description_color = if row_data.description.trim().is_empty() {
        muted
    } else {
        text_c
    };

    let identity = column![text(description).size(TS.body).color(description_color)]
        .width(Length::FillPortion(3));

    let labelled = |label: &'static str, value: String| {
        let value_color = if value == NOT_SET { muted } else { text_c };
        row![
            text(label).size(TS.body).color(muted),
            text(value).size(TS.body).color(value_color),
        ]
        .spacing(SP.m)
    };
    let supplier = column![labelled("Manufacturer", manufacturer), labelled("MPN", mpn)]
        .spacing(SP.xs)
        .width(Length::FillPortion(2));

    kit::card(
        row![identity, supplier]
            .spacing(SP.l)
            .align_y(iced::Alignment::Start),
        tokens,
    )
}

/// What a pane shows when it has no drawing.
fn pane_message(kind: PaneKind, unbound: bool) -> &'static str {
    match (kind, unbound) {
        (PaneKind::Symbol, true) => "No symbol bound",
        (PaneKind::Footprint, true) => "No footprint bound",
        (PaneKind::Symbol, false) => {
            "The symbol could not be loaded. It may be missing from the open libraries."
        }
        (PaneKind::Footprint, false) => {
            "The footprint could not be loaded. It may be missing from the open libraries."
        }
    }
}

#[derive(Clone, Copy)]
enum PaneKind {
    Symbol,
    Footprint,
}

fn editor_event(address: &EditorAddress, msg: EditorMsg) -> LibraryMessage {
    LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    }
}

fn render_panes<'a>(
    state: &'a ComponentPreviewState,
    library_state: &LibraryState,
    tokens: &'a ThemeTokens,
    canvas_colors: &CanvasColors,
    device_px_per_logical_px: f32,
    address: &EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    // Binding a primitive writes the row, so a read-only mount (the
    // stock library) renders the Pick buttons disabled.
    // When the symbol_ref is the nil sentinel ("unbound") the user
    // hasn't picked a symbol yet — surface "Pick Symbol" instead of
    // "Open Symbol Editor". Same shape for footprint.
    let symbol_unbound = state.row.is_symbol_unbound();
    let footprint_unbound = state.row.is_footprint_unbound();
    let (mut symbol_msg, symbol_btn_label): (Option<LibraryMessage>, &'static str) =
        if symbol_unbound {
            (
                Some(LibraryMessage::OpenPrimitivePicker {
                    kind: signex_library::PrimitiveKind::Symbol,
                    target: super::super::state::PrimitivePickerTarget::PreviewRow(address.clone()),
                }),
                "Pick Symbol",
            )
        } else {
            (
                symbol_path(state, library_state)
                    .map(|path| LibraryMessage::OpenPrimitiveEditor { path }),
                "Open Symbol Editor",
            )
        };

    let (mut footprint_msg, footprint_btn_label): (Option<LibraryMessage>, &'static str) =
        if footprint_unbound {
            (
                Some(LibraryMessage::OpenPrimitivePicker {
                    kind: signex_library::PrimitiveKind::Footprint,
                    target: super::super::state::PrimitivePickerTarget::PreviewRow(address.clone()),
                }),
                "Pick Footprint",
            )
        } else {
            (
                footprint_path(state, library_state)
                    .map(|p| LibraryMessage::OpenPrimitiveEditor { path: p }),
                "Open Footprint Editor",
            )
        };

    if read_only {
        if symbol_unbound {
            symbol_msg = None;
        }
        if state.row.footprint_ref.is_none() {
            footprint_msg = None;
        }
    }

    let symbol_drawing = (!symbol_unbound)
        .then_some(state.symbol.as_ref())
        .flatten()
        .map(|sym| {
            crate::panels::components_panel::details::symbol_canvas::symbol_preview(
                sym.to_lib_symbol(sym.name.clone()),
                *canvas_colors,
                PREVIEW_PANE_HEIGHT,
                device_px_per_logical_px,
            )
            .map(|()| LibraryMessage::Noop)
        });
    let symbol_pane = render_pane(
        "Symbol",
        symbol_drawing,
        pane_message(PaneKind::Symbol, symbol_unbound),
        symbol_msg,
        symbol_btn_label,
        tokens,
    );
    // Drawn with the Components panel's footprint canvas, like the symbol.
    let footprint_drawing = (!footprint_unbound)
        .then_some(state.footprint.as_ref())
        .flatten()
        .map(|fp| {
            let program =
                crate::panels::components_panel::details::footprint_canvas::FootprintPreview::new(
                    fp.clone(),
                    tokens,
                );
            iced::widget::canvas(program)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        });
    let footprint_pane = render_pane(
        "Footprint",
        footprint_drawing,
        pane_message(PaneKind::Footprint, footprint_unbound),
        footprint_msg,
        footprint_btn_label,
        tokens,
    );

    row![
        container(symbol_pane).width(Length::FillPortion(1)),
        container(footprint_pane).width(Length::FillPortion(1)),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Start)
    .into()
}

fn render_pane<'a>(
    label: &'a str,
    drawing: Option<Element<'a, LibraryMessage>>,
    empty_message: &'static str,
    open_msg: Option<LibraryMessage>,
    button_label: &'a str,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let content: Element<'a, LibraryMessage> = match drawing {
        Some(drawing) => drawing,
        None => kit::empty_state(empty_message, None, tokens),
    };
    let header = kit::section_header(
        label,
        Some(kit::secondary_button(button_label, open_msg, tokens)),
        tokens,
    );
    column![
        header,
        kit::card(
            container(content)
                .width(Length::Fill)
                .height(Length::Fixed(PREVIEW_PANE_HEIGHT)),
            tokens,
        ),
    ]
    .spacing(SP.s)
    .width(Length::Fill)
    .into()
}

/// The `.snxsym` a row's symbol ref points at, resolved through the
/// library named by `PrimitiveRef.library_id` (it may not be the row's own
/// library). `None` when that library is not open.
fn symbol_path(
    state: &ComponentPreviewState,
    library_state: &LibraryState,
) -> Option<std::path::PathBuf> {
    library_state.primitive_file(&state.row.symbol_ref, "symbols", "snxsym")
}

fn footprint_path(
    state: &ComponentPreviewState,
    library_state: &LibraryState,
) -> Option<std::path::PathBuf> {
    library_state.primitive_file(state.row.footprint_ref.as_ref()?, "footprints", "snxfpt")
}

/// One Pin Map table line: the pad a symbol pin lands on.
#[derive(Debug, PartialEq, Eq)]
struct PinLine {
    number: String,
    name: String,
    pad: String,
    is_override: bool,
}

/// Table lines for `pins`: a pin's pad is its override, else its own number.
fn pin_lines<'p>(
    pins: impl Iterator<Item = (&'p str, &'p str)>,
    overrides: &[signex_library::PinPadOverride],
) -> Vec<PinLine> {
    pins.map(|(number, name)| {
        let over = overrides.iter().find(|o| o.symbol_pin_number == number);
        PinLine {
            number: number.to_string(),
            name: name.to_string(),
            pad: over
                .map(|o| o.footprint_pad_number.clone())
                .unwrap_or_else(|| number.to_string()),
            is_override: over.is_some(),
        }
    })
    .collect()
}

fn pin_map_subsection<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let header = kit::section_header("Pin Map", None, tokens);

    let Some(sym) = state.symbol.as_ref() else {
        return column![
            header,
            kit::empty_state("The pin map shows once the symbol is loaded.", None, tokens),
        ]
        .into();
    };
    if sym.pins.is_empty() {
        return column![
            header,
            kit::empty_state(
                "This symbol has no pins yet. Open the Symbol Editor to add some.",
                None,
                tokens,
            ),
        ]
        .into();
    }

    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let widths = [
        Length::FillPortion(1),
        Length::FillPortion(2),
        Length::FillPortion(2),
        Length::FillPortion(2),
    ];
    let mut table = column![kit::table_header(
        &[
            ("Symbol pin", widths[0]),
            ("Symbol name", widths[1]),
            ("Pad", widths[2]),
            ("", widths[3]),
        ],
        tokens,
    )]
    .width(Length::Fill);

    for (index, line) in pin_lines(
        sym.pins
            .iter()
            .map(|p| (p.number.as_str(), p.name.as_str())),
        &state.row.pin_map_overrides,
    )
    .into_iter()
    .enumerate()
    {
        let pad_cell: Element<'a, LibraryMessage> = if line.is_override {
            row![
                text(line.pad.clone()).size(TS.body).color(text_c),
                text("override").size(TS.caption).color(muted),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            text(line.pad.clone()).size(TS.body).color(text_c).into()
        };
        let label = if line.is_override {
            "Edit override"
        } else {
            "Override"
        };
        let action = kit::secondary_button(
            label,
            Some(editor_event(
                address,
                EditorMsg::PinMapOpenOverrideEdit(line.number.clone()),
            )),
            tokens,
        );
        table = table.push(kit::table_row(
            vec![
                (
                    text(line.number.clone()).size(TS.body).color(text_c).into(),
                    widths[0],
                ),
                (
                    text(line.name.clone()).size(TS.body).color(text_c).into(),
                    widths[1],
                ),
                (pad_cell, widths[2]),
                (action, widths[3]),
            ],
            index,
            None,
            tokens,
        ));

        // Inline override editor for the expanded row.
        if state.pin_map_state.expanded_row.as_deref() == Some(line.number.as_str()) {
            table = table.push(override_editor_row(
                &line.number,
                &state.pin_map_state.override_buf,
                tokens,
                address,
            ));
        }
    }

    column![header, kit::card(table, tokens)].into()
}

fn override_editor_row<'a>(
    pin_number: &str,
    buf: &str,
    tokens: &ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    let pin_for_input = pin_number.to_string();
    let address_for_input = address.clone();
    let input = iced::widget::text_input("pad number…", buf)
        .on_input(move |s| {
            editor_event(
                &address_for_input,
                EditorMsg::PinMapOverrideBufChanged {
                    pin: pin_for_input.clone(),
                    value: s,
                },
            )
        })
        .padding([SP.s, SP.m])
        .size(TS.body)
        .width(Length::FillPortion(2));

    let save = kit::primary_button(
        "Save",
        Some(editor_event(
            address,
            EditorMsg::PinMapAddOverride {
                pin: pin_number.to_string(),
                pad: buf.to_string(),
            },
        )),
        tokens,
    );
    let cancel = kit::secondary_button(
        "Cancel",
        Some(editor_event(address, EditorMsg::PinMapCancelOverrideEdit)),
        tokens,
    );
    let remove = kit::danger_button(
        "Remove",
        Some(editor_event(
            address,
            EditorMsg::PinMapRemoveOverride {
                pin: pin_number.to_string(),
            },
        )),
        tokens,
    );

    container(
        row![
            text(format!("Override pin {pin_number} →"))
                .size(TS.body)
                .color(muted),
            input,
            Space::new().width(Length::Fill),
            save,
            cancel,
            remove,
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.s, SP.m])
    .into()
}

/// "Used by N sheet(s) in M project(s)."
fn where_used_label(sheets: usize, projects: usize) -> String {
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    format!(
        "Used by {sheets} sheet{} in {projects} project{}.",
        plural(sheets),
        plural(projects),
    )
}

fn where_used_footer<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let row_id = signex_library::RowId::from_uuid(state.row.row_id);
    let uses = library_state.where_used_for(row_id);
    let projects = uses
        .iter()
        .map(|u| &u.project_path)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let color = if uses.is_empty() {
        theme_ext::text_secondary(tokens)
    } else {
        theme_ext::text_primary(tokens)
    };
    kit::card(
        text(where_used_label(uses.len(), projects))
            .size(TS.body)
            .color(color),
        tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_card_does_not_repeat_the_shell_identity() {
        let mut row = crate::library::component_preview::test_row();
        row.description = "10k resistor".into();
        let texts = header_texts(&row);
        let shown = [&texts.description, &texts.manufacturer, &texts.mpn];
        let pn = row.internal_pn.as_str();
        assert!(!pn.is_empty());
        for text in shown {
            assert!(!text.contains(pn), "{text}");
            assert!(!text.contains(row.class.as_str()), "{text}");
        }
        assert_eq!(texts.description, "10k resistor");
    }

    #[test]
    fn blank_manufacturer_and_mpn_read_not_set() {
        assert_eq!(
            mpn_fields("", "  "),
            (NOT_SET.to_string(), NOT_SET.to_string())
        );
        assert_eq!(
            mpn_fields("TI", ""),
            ("TI".to_string(), NOT_SET.to_string())
        );
        assert_eq!(
            mpn_fields(" TI ", "LM358"),
            ("TI".to_string(), "LM358".to_string())
        );
    }

    #[test]
    fn blank_description_reads_no_description() {
        assert_eq!(description_text("  "), NO_DESCRIPTION);
        assert_eq!(description_text(" 100 nF "), "100 nF");
    }

    #[test]
    fn pane_message_is_plain_language() {
        assert_eq!(pane_message(PaneKind::Symbol, true), "No symbol bound");
        let missing = pane_message(PaneKind::Footprint, false);
        assert!(missing.contains("could not be loaded"));
        assert!(!missing.contains("uuid"));
    }

    #[test]
    fn pin_lines_one_per_pin_with_overrides_marked() {
        let overrides = [signex_library::PinPadOverride::new("2", "B2")];
        let lines = pin_lines([("1", "IN"), ("2", "OUT")].into_iter(), &overrides);
        assert_eq!(lines.len(), 2);
        assert_eq!((lines[0].pad.as_str(), lines[0].is_override), ("1", false));
        assert_eq!((lines[1].pad.as_str(), lines[1].is_override), ("B2", true));
    }

    #[test]
    fn where_used_pluralises() {
        assert_eq!(where_used_label(0, 0), "Used by 0 sheets in 0 projects.");
        assert_eq!(where_used_label(1, 1), "Used by 1 sheet in 1 project.");
        assert_eq!(where_used_label(3, 2), "Used by 3 sheets in 2 projects.");
    }
}
