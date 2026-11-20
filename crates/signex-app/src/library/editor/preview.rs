//! The General tab's entry point and the Pin Map tab, built from the shared
//! dialog kit ([`super::kit`]). The General layout itself (Symbol,
//! Footprint, PIN TO PAD) lives in [`super::general`]; this file keeps the
//! pin-to-pad lines, the Pin Map tab with its override editor and the
//! primitive paths both tabs use.
//!
//! Symbol and footprint are read-only here: Open fires
//! [`crate::library::messages::LibraryMessage::OpenPrimitiveEditor`] to
//! open the standalone `.snxsym` / `.snxfpt` document tab.

use iced::widget::{Column, Space, Widget, column, container, row, scrollable};
use iced::{Element, Length};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::text;
use signex_widgets::theme_ext;

use crate::panels::components_panel::details::symbol_canvas::SheetGrid;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress, LibraryState};
use super::kit::{self, SP, TS};

/// The General tab (mockup row 128): see [`super::general::view`].
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
    super::general::view(
        state,
        library_state,
        tokens,
        canvas_colors,
        grid,
        device_px_per_logical_px,
        address,
        i18n,
    )
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
    scrollable(signex_widgets::inert::unless_read_only_veiled(
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

pub(super) fn editor_event(address: &EditorAddress, msg: EditorMsg) -> LibraryMessage {
    LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    }
}

/// The `.snxsym` a row's symbol ref points at, resolved through the
/// library named by `PrimitiveRef.library_id` (it may not be the row's own
/// library). `None` when that library is not open.
pub(super) fn symbol_path(
    state: &ComponentPreviewState,
    library_state: &LibraryState,
) -> Option<std::path::PathBuf> {
    library_state.primitive_file(&state.row.symbol_ref, "symbols", "snxsym")
}

pub(super) fn footprint_path(
    state: &ComponentPreviewState,
    library_state: &LibraryState,
) -> Option<std::path::PathBuf> {
    library_state.primitive_file(state.row.footprint_ref.as_ref()?, "footprints", "snxfpt")
}

/// One Pin Map table line: the pad a symbol pin lands on.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct PinLine {
    pub(super) number: String,
    pub(super) name: String,
    pub(super) pad: String,
    pub(super) is_override: bool,
}

/// Table lines for `pins`: a pin's pad is its override, else its own number.
pub(super) fn pin_lines<'p>(
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
            kit::empty_state(
                t!(i18n, ids::EDITORS_COMPONENT_PIN_MAP_PENDING),
                None,
                tokens
            ),
        ]
        .boxed();
    };
    if sym.pins.is_empty() {
        return column![
            header,
            kit::empty_state(
                t!(i18n, ids::EDITORS_COMPONENT_PIN_MAP_NO_PINS),
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

/// The inline override editor under a pin's row: the pad number input,
/// Save, Cancel and Remove. Shared by the Pin Map tab and the General
/// tab's pin-to-pad table.
pub(super) fn override_editor_row<'a>(
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
pub(super) fn where_used_label(i18n: &I18n, sheets: usize, projects: usize) -> Localized {
    t!(
        i18n,
        ids::LIBRARY_PREVIEW_WHERE_USED,
        "sheets" => sheets,
        "projects" => projects
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
