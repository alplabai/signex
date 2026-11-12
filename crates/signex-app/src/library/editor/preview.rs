//! Preview tab — header, read-only Symbol + Footprint panes, Pin Map
//! table, Alternate symbols and a Where-Used line, all built from the
//! shared dialog kit ([`super::kit`]).
//!
//! The Preview surface is purely read-only for primitives;
//! right-click on either render → context menu fires
//! [`crate::library::messages::LibraryMessage::OpenPrimitiveEditor`]
//! to open the standalone `.snxsym` / `.snxfpt` document tab.

use iced::widget::{Column, Space, Widget, column, container, row, scrollable};
use iced::{Element, Length};
use signex_i18n::{I18n, Localized, ids, t};
use signex_library::ComponentRow;
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::theme_ext;
use signex_widgets::{Label, raw, text};

use crate::panels::components_panel::details::symbol_canvas::SheetGrid;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress, LibraryState};
use super::kit::{self, SP, TS};

/// Height of the Symbol / Footprint panes: twenty body lines.
const PREVIEW_PANE_HEIGHT: f32 = TS.body * 20.0;

/// Shown in the header for a manufacturer / MPN the row does not carry.
const NO_DESCRIPTION: &str = "No description";

#[expect(
    clippy::too_many_arguments,
    reason = "the editor views take the language handle beside their existing seven inputs (#2225)"
)]
pub(crate) fn view<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    canvas_colors: &CanvasColors,
    grid: SheetGrid,
    device_px_per_logical_px: f32,
    address: EditorAddress,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let read_only = library_state.is_read_only(&state.library_path);
    let panes = render_panes(
        state,
        library_state,
        tokens,
        i18n,
        canvas_colors,
        (grid, device_px_per_logical_px),
        &address,
        read_only,
    );
    // The pin map and the alternates sit on their own tabs.
    let body = column![
        header_row(&state.row, tokens, i18n),
        panes,
        where_used_footer(state, library_state, tokens, i18n),
    ]
    .spacing(SP.l)
    .width(Length::Fill)
    .boxed();

    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// The Graphite Pin Map tab: the pin-to-pad table on its own.
pub(crate) fn pin_map_tab<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    address: &EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let veil = super::read_only_veil(tokens);
    scrollable(super::inert::unless_read_only_veiled(
        read_only,
        veil,
        pin_map_subsection(state, tokens, i18n, address),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// (pins with a pad, pins) of the symbol; `None` before the symbol loads
/// or when it has no pins. A pin has a pad when it is overridden or the
/// footprint carries a pad of its number.
pub(crate) fn pin_map_counts(state: &ComponentPreviewState) -> Option<(usize, usize)> {
    let sym = state.symbol.as_ref()?;
    if sym.pins.is_empty() {
        return None;
    }
    let lines = pin_lines(
        sym.pins
            .iter()
            .map(|p| (p.number.as_str(), p.name.as_str())),
        &state.row.pin_map_overrides,
    );
    let mapped = lines
        .iter()
        .filter(|l| {
            l.is_override
                || state
                    .footprint
                    .as_ref()
                    .is_some_and(|f| f.pads.iter().any(|p| p.number == l.pad))
        })
        .count();
    Some((mapped, lines.len()))
}

/// Manufacturer and MPN as shown in the header, trimmed; an empty one is
/// shown as the catalog's "Not set", so a part really named "Not set" keeps
/// its own text.
fn mpn_fields(manufacturer: &str, mpn: &str) -> (String, String) {
    (manufacturer.trim().to_string(), mpn.trim().to_string())
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

fn header_row<'a>(
    row_data: &'a ComponentRow,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let HeaderTexts {
        description,
        manufacturer,
        mpn,
    } = header_texts(row_data);
    let description_blank = row_data.description.trim().is_empty();
    let (description, description_color): (Label<'a>, _) = if description_blank {
        (t!(i18n, ids::LIBRARY_PREVIEW_NO_DESCRIPTION).into(), muted)
    } else {
        (signex_i18n::UserText::new(description).into(), text_c)
    };

    let identity = column![
        text(description)
            .size(TS.body)
            .color(description_color)
            .boxed()
    ]
    .width(Length::FillPortion(3))
    .boxed();

    let labelled = |label: Localized, value: String| {
        let (value, value_color): (Label<'a>, _) = if value.is_empty() {
            (t!(i18n, ids::LIBRARY_PREVIEW_NOT_SET).into(), muted)
        } else {
            (raw(value), text_c)
        };
        row![
            text(label).size(TS.body).color(muted).boxed(),
            text(value).size(TS.body).color(value_color).boxed(),
        ]
        .spacing(SP.m)
        .boxed()
    };
    let supplier = column![
        labelled(
            t!(i18n, ids::LIBRARY_PREVIEW_HEADER_MANUFACTURER),
            manufacturer
        ),
        labelled(t!(i18n, ids::LIBRARY_PREVIEW_MPN), mpn),
    ]
    .spacing(SP.xs)
    .width(Length::FillPortion(2))
    .boxed();

    kit::card(
        row![identity, supplier]
            .spacing(SP.l)
            .align_y(iced::Alignment::Start)
            .boxed(),
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

#[expect(
    clippy::too_many_arguments,
    reason = "the editor views take the language handle beside their existing seven inputs (#2225)"
)]
fn render_panes<'a>(
    state: &'a ComponentPreviewState,
    library_state: &LibraryState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    canvas_colors: &CanvasColors,
    (grid, device_px_per_logical_px): (SheetGrid, f32),
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
                grid,
                PREVIEW_PANE_HEIGHT,
                device_px_per_logical_px,
            )
            .map(|()| LibraryMessage::Noop)
            .boxed()
        });
    let symbol_pane = render_pane(
        t!(i18n, ids::EDITORS_COMPONENT_SYMBOL_HEADING),
        symbol_drawing,
        pane_message(PaneKind::Symbol, symbol_unbound),
        symbol_msg,
        symbol_btn_label,
        tokens,
    );
    // Drawn with the Components panel's footprint canvas, like the symbol.
    let footprint_drawing = (!footprint_unbound)
        .then_some(state.footprint_preview())
        .flatten()
        .map(|model| {
            let program =
                crate::panels::components_panel::details::footprint_canvas::FootprintPreview::new(
                    model,
                )
                .with_theme_tokens(tokens);
            iced::widget::canvas(program)
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed()
        });
    let footprint_pane = render_pane(
        t!(i18n, ids::EDITORS_COMPONENT_FOOTPRINT_HEADING),
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
    .boxed()
}

fn render_pane<'a>(
    label: Localized,
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
                .height(Length::Fixed(PREVIEW_PANE_HEIGHT))
                .boxed(),
            tokens,
        ),
    ]
    .spacing(SP.s)
    .width(Length::Fill)
    .boxed()
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

/// Graphite Match cell: how the pin's pad was found.
fn match_pill<'a>(
    state: &ComponentPreviewState,
    line: &PinLine,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    use crate::styles::PillTone;
    if line.is_override {
        return kit::toned_pill("Override".to_string(), PillTone::Info, tokens);
    }
    let Some(footprint) = state.footprint.as_ref() else {
        return text(t!(i18n, ids::EDITORS_COMPONENT_NO_MATCH))
            .size(TS.body)
            .boxed();
    };
    if footprint.pads.iter().any(|p| p.number == line.pad) {
        kit::toned_pill("By number".to_string(), PillTone::Ok, tokens)
    } else {
        kit::toned_pill(format!("No pad '{}'", line.pad), PillTone::Warn, tokens)
    }
}

fn pin_map_subsection<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let summary = pin_map_counts(state).map(|(mapped, total)| {
        kit::toned_pill(
            format!("{mapped} of {total} mapped"),
            if mapped == total {
                crate::styles::PillTone::Ok
            } else {
                crate::styles::PillTone::Warn
            },
            tokens,
        )
    });
    let header = kit::section_header(
        t!(i18n, ids::EDITORS_COMPONENT_PIN_TO_PAD_HEADING),
        summary,
        tokens,
    );

    let Some(sym) = state.symbol.as_ref() else {
        return column![
            header,
            kit::empty_state("The pin map shows once the symbol is loaded.", None, tokens),
        ]
        .boxed();
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
        .boxed();
    }

    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let widths = [
        Length::FillPortion(1),
        Length::FillPortion(2),
        Length::FillPortion(2),
        Length::FillPortion(2),
    ];
    let table_header = kit::table_header(
        &[
            (Some(ids::EDITORS_COMPONENT_COLUMN_PIN), widths[0]),
            (Some(ids::EDITORS_COMPONENT_COLUMN_NAME), widths[1]),
            (Some(ids::EDITORS_COMPONENT_COLUMN_TYPE), widths[1]),
            (Some(ids::EDITORS_COMPONENT_COLUMN_PAD), widths[0]),
            (Some(ids::EDITORS_COMPONENT_COLUMN_MATCH), widths[2]),
            (None, widths[3]),
        ],
        i18n,
        tokens,
    );
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let mut table: Column<Element<'a, LibraryMessage>> = column![table_header].width(Length::Fill);

    for (index, line) in pin_lines(
        sym.pins
            .iter()
            .map(|p| (p.number.as_str(), p.name.as_str())),
        &state.row.pin_map_overrides,
    )
    .into_iter()
    .enumerate()
    {
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
        let direction: Element<'a, LibraryMessage> = sym
            .pins
            .iter()
            .find(|p| p.number == line.number)
            .map_or_else(
                || Space::new().boxed(),
                |p| {
                    text(t!(i18n, super::pin_labels::direction_message(p.electrical)))
                        .size(TS.body)
                        .color(muted)
                        .boxed()
                },
            );
        let cells = vec![
            (
                text(signex_i18n::UserText::new(line.number.clone()))
                    .size(TS.body)
                    .font(mono)
                    .color(text_c)
                    .boxed(),
                widths[0],
            ),
            (
                text(signex_i18n::UserText::new(line.name.clone()))
                    .size(TS.body)
                    .font(mono)
                    .color(text_c)
                    .boxed(),
                widths[1],
            ),
            (direction, widths[1]),
            (
                text(signex_i18n::UserText::new(line.pad.clone()))
                    .size(TS.body)
                    .font(mono)
                    .color(text_c)
                    .boxed(),
                widths[0],
            ),
            (match_pill(state, &line, tokens, i18n), widths[2]),
            (action, widths[3]),
        ];
        table = table.push(kit::table_row(cells, index, None, tokens));

        // Inline override editor for the expanded row.
        if state.pin_map_state.expanded_row.as_deref() == Some(line.number.as_str()) {
            table = table.push(override_editor_row(
                &line.number,
                &state.pin_map_state.override_buf,
                tokens,
                i18n,
                address,
            ));
        }
    }

    column![header, kit::card(table.boxed(), tokens)].boxed()
}

fn override_editor_row<'a>(
    pin_number: &str,
    buf: &'a str,
    tokens: &ThemeTokens,
    i18n: &I18n,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    let pin_for_input = pin_number.to_string();
    let address_for_input = address.clone();
    let input = signex_widgets::text_input(t!(i18n, ids::LIBRARY_PREVIEW_PAD_PLACEHOLDER), buf)
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
        .width(Length::FillPortion(2))
        .boxed();

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
            text(t!(i18n, ids::LIBRARY_PREVIEW_OVERRIDE_PIN, "pin" => pin_number))
                .size(TS.body)
                .color(muted)
                .boxed(),
            input,
            Space::new().width(Length::Fill).boxed(),
            save,
            cancel,
            remove,
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .boxed(),
    )
    .padding([SP.s, SP.m])
    .boxed()
}

/// "Used by N sheet(s) in M project(s)."
fn where_used_label(i18n: &I18n, sheets: usize, projects: usize) -> Localized {
    t!(
        i18n,
        ids::LIBRARY_PREVIEW_WHERE_USED,
        "sheets" => sheets,
        "projects" => projects
    )
}

fn where_used_footer<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &ThemeTokens,
    i18n: &I18n,
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
        text(where_used_label(i18n, uses.len(), projects))
            .size(TS.body)
            .color(color)
            .boxed(),
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
        assert_eq!(mpn_fields("", "  "), (String::new(), String::new()));
        assert_eq!(mpn_fields("TI", ""), ("TI".to_string(), String::new()));
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
        // The counts are catalog arguments, wrapped in isolation marks.
        let i18n = I18n::en_us();
        let label = |sheets, projects| {
            where_used_label(&i18n, sheets, projects)
                .to_string()
                .replace(['\u{2068}', '\u{2069}'], "")
        };
        assert_eq!(label(0, 0), "Used by 0 sheets in 0 projects.");
        assert_eq!(label(1, 1), "Used by 1 sheet in 1 project.");
        assert_eq!(label(3, 2), "Used by 3 sheets in 2 projects.");
    }
}
