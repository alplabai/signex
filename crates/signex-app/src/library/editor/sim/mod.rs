//! Sim tab — SPICE model section + per-pin SPICE node table.
//!
//! Backed by the typed `SimModel` primitive bound through
//! `ComponentRow::sim_ref`. The view operates on `state.sim`
//! (resolved lazily by the dispatcher's `PreviewTab::Simulation` arm
//! via `LibrarySet::resolve_sim`) and `state.sim_body`, the live
//! `text_editor::Content` mirror of the SPICE deck.
//!
//! The pin/node table iterates `state.symbol.pins`; the SPICE-node
//! buffer for a given pin is read from
//! `sim.default_node_map[pin_number]` and edits flow back through
//! [`EditorMsg::SimSetPinNode`] which empty-removes / non-empty-
//! inserts the mapping.

pub mod state;

use iced::widget::{
    Widget, column, container, pick_list, row, scrollable, text, text_editor, text_input,
};
use iced::{Element, Length};
use signex_library::SimKind;
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress};
use super::kit::{self, SP, TS};

const SIM_KIND_OPTS: [SimKind; 4] = [
    SimKind::Spice3,
    SimKind::Ngspice,
    SimKind::LtSpice,
    SimKind::VerilogA,
];

/// Shown when no SPICE model is bound to the row.
const EMPTY_MESSAGE: &str = "No SPICE model is bound to this component.";
/// Shown in the node table while the symbol has not resolved.
const SYMBOL_PENDING: &str = "Open the Symbol tab first to list the pins.";
/// Shown in the node table when the symbol has no pins.
const NO_PINS: &str = "The symbol has no pins yet.";
/// Pin-name cell for an unnamed pin.
const UNNAMED_PIN: &str = "\u{2014}";

/// Table columns: pin number, symbol pin name, SPICE node.
const PIN_COLUMNS: [(&str, Length); 3] = [
    ("Pin", Length::FillPortion(1)),
    ("Symbol", Length::FillPortion(2)),
    ("SPICE Node", Length::FillPortion(3)),
];

/// Height of the deck editor, in lines of body text.
const DECK_LINES: f32 = 16.0;

/// Pick-list adapter so `SimKind` can sit on the iced `pick_list`
/// without needing a `Display` impl on the public type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SimKindPick(SimKind);

impl std::fmt::Display for SimKindPick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // SimKind is `#[non_exhaustive]` — fall through to `Debug`
        // for any new dialect added by signex-library so the picker
        // doesn't regress into "fail to compile" the moment a new
        // variant lands.
        let s = match self.0 {
            SimKind::Spice3 => "Spice3",
            SimKind::Ngspice => "Ngspice",
            SimKind::LtSpice => "LtSpice",
            SimKind::VerilogA => "Verilog-A",
            other => return write!(f, "{other:?}"),
        };
        f.write_str(s)
    }
}

/// One line of the pin/node table, as plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PinNodeLine {
    number: String,
    name: String,
    node: String,
}

/// The table's lines, one per symbol pin in symbol order; an unnamed pin
/// shows a dash and an unmapped pin an empty node.
fn pin_node_lines(
    sim: &signex_library::SimModel,
    pins: &[signex_library::SymbolPin],
) -> Vec<PinNodeLine> {
    pins.iter()
        .map(|pin| PinNodeLine {
            number: pin.number.clone(),
            name: if pin.name.is_empty() {
                UNNAMED_PIN.to_string()
            } else {
                pin.name.clone()
            },
            node: sim
                .default_node_map
                .get(pin.number.as_str())
                .cloned()
                .unwrap_or_default(),
        })
        .collect()
}

fn event(address: &EditorAddress, msg: EditorMsg) -> LibraryMessage {
    LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    }
}

/// A muted label over its control, so every field of the model card reads
/// the same.
fn labelled<'a>(
    label: &'a str,
    control: Element<'a, LibraryMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    column![
        text(label)
            .size(TS.caption)
            .color(theme_ext::text_secondary(tokens))
            .boxed(),
        control,
    ]
    .spacing(SP.s)
    .boxed()
}

pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let Some(sim) = state.sim.as_ref() else {
        // No model bound: a header and an empty state with the one action.
        let add = kit::primary_button(
            "Add SPICE model",
            Some(event(&address, EditorMsg::SimSetEnabled(true))),
            tokens,
        );
        let body = column![
            kit::section_header("Model", None, tokens),
            kit::card(kit::empty_state(EMPTY_MESSAGE, Some(add), tokens), tokens),
        ]
        .spacing(SP.m)
        .width(Length::Fill)
        .boxed();
        return scrollable(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed();
    };

    let remove = kit::danger_button(
        "Remove model",
        Some(event(&address, EditorMsg::SimSetEnabled(false))),
        tokens,
    );

    let kind_address = address.clone();
    let kind_picker = pick_list(
        Some(SimKindPick(sim.kind)),
        SIM_KIND_OPTS.map(SimKindPick),
        |k: &SimKindPick| k.to_string(),
    )
    .graphite()
    .on_select(move |SimKindPick(k)| event(&kind_address, EditorMsg::SimSetKind(k)))
    .padding([SP.s, SP.m])
    .boxed();

    let name_address = address.clone();
    let name_input = text_input("LM358_DUAL", sim.name.as_str())
        .on_input(move |s| event(&name_address, EditorMsg::SimSetName(s)))
        .padding([SP.s, SP.m])
        .size(TS.body)
        .boxed();

    // SPICE deck — multi-line text_editor backed by state.sim_body.
    let deck: Element<'a, LibraryMessage> = if let Some(content) = state.sim_body.as_ref() {
        let deck_address = address.clone();
        text_editor(content)
            .placeholder(".SUBCKT NAME 1 2 3 4\n  …\n.ENDS")
            .padding(SP.m)
            .size(TS.body)
            .height(Length::Fixed(TS.body * DECK_LINES))
            .on_action(move |a| event(&deck_address, EditorMsg::SimBodyAction(a)))
            .boxed()
    } else {
        // sim_body is seeded by the SelectTab(Sim) arm; this is the
        // safety fallback for a tab reached without it.
        text(sim.body.clone())
            .size(TS.body)
            .color(theme_ext::text_primary(tokens))
            .boxed()
    };

    let model_card = kit::card(
        column![
            row![
                container(labelled("Model kind", kind_picker, tokens))
                    .width(Length::FillPortion(1))
                    .boxed(),
                container(labelled("Model name", name_input, tokens))
                    .width(Length::FillPortion(2))
                    .boxed(),
            ]
            .spacing(SP.m)
            .boxed(),
            labelled("SPICE deck", deck, tokens),
        ]
        .spacing(SP.m)
        .boxed(),
        tokens,
    );

    let table: Element<'a, LibraryMessage> = match state.symbol.as_ref() {
        Some(symbol) => view_pin_node_table(sim, &symbol.pins, tokens, &address),
        None => kit::empty_state(SYMBOL_PENDING, None, tokens).boxed(),
    };

    scrollable(
        column![
            kit::section_header("Model", Some(remove), tokens),
            model_card,
            kit::section_header("Pin / Node Map", None, tokens),
            table,
        ]
        .spacing(SP.m)
        .width(Length::Fill)
        .boxed(),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

fn view_pin_node_table<'a>(
    sim: &'a signex_library::SimModel,
    pins: &'a [signex_library::SymbolPin],
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    if pins.is_empty() {
        return kit::empty_state(NO_PINS, None, tokens);
    }

    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let mut col: iced::widget::Column<Element<'a, LibraryMessage>> =
        column![kit::table_header(&PIN_COLUMNS, tokens)];
    for (index, line) in pin_node_lines(sim, pins).into_iter().enumerate() {
        let pin_number = line.number.clone();
        let node_address = address.clone();
        let input = text_input("net", line.node.clone())
            .padding([SP.xs, SP.s])
            .size(TS.body)
            .on_input(move |s| {
                event(
                    &node_address,
                    EditorMsg::SimSetPinNode {
                        pin_number: pin_number.clone(),
                        value: s,
                    },
                )
            });
        let cells: Vec<(Element<'a, LibraryMessage>, Length)> = vec![
            (
                text(line.number).size(TS.body).color(text_c).boxed(),
                PIN_COLUMNS[0].1,
            ),
            (
                text(line.name).size(TS.body).color(muted).boxed(),
                PIN_COLUMNS[1].1,
            ),
            (input.boxed(), PIN_COLUMNS[2].1),
        ];
        col = col.push(kit::table_row(cells, index, None, tokens));
    }
    kit::card(col.boxed(), tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::messages::EditorMsg;
    use crate::library::state::ComponentPreviewState;
    use signex_library::{
        ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
        ParamMap, PlmReserved, PrimitiveRef,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    fn fixture_editor() -> ComponentPreviewState {
        let lib_id = Uuid::now_v7();
        let row = ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: Uuid::now_v7(),
            internal_pn: InternalPn::new("TEST_PN"),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(lib_id, Uuid::now_v7()),
            footprint_ref: None,
            alternate_footprints: Vec::new(),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
            content_hash: [0u8; 32],
            server_revision: None,
        };
        ComponentPreviewState::from_row(PathBuf::from("/tmp/test.snxlib"), "generics".into(), row)
    }

    fn apply(editor: &mut ComponentPreviewState, msg: EditorMsg) {
        // Drive the same code path the dispatcher uses for inline edits.
        // `apply_inline_edit` is `pub(crate)` so the unit tests in this
        // module can exercise the SimSet*/SimBodyAction handlers
        // without standing up the full Signex update tree.
        crate::library::component_preview::apply_inline_edit(editor, msg);
    }

    /// One table line per pin, in symbol order; unnamed pins show a dash
    /// and unmapped pins an empty node.
    #[test]
    fn pin_node_lines_follow_pins_and_node_map() {
        let mut e = fixture_editor();
        apply(&mut e, EditorMsg::SimSetEnabled(true));
        apply(
            &mut e,
            EditorMsg::SimSetPinNode {
                pin_number: "2".to_string(),
                value: "GND".to_string(),
            },
        );
        let pins = vec![
            signex_library::SymbolPin::new("1", "IN"),
            signex_library::SymbolPin::new("2", ""),
        ];
        let lines = pin_node_lines(e.sim.as_ref().unwrap(), &pins);
        assert_eq!(lines.len(), pins.len());
        assert_eq!(lines[0].name, "IN");
        assert_eq!(lines[0].node, "");
        assert_eq!(lines[1].name, UNNAMED_PIN);
        assert_eq!(lines[1].node, "GND");
    }

    #[test]
    fn pin_columns_are_pin_symbol_node() {
        let titles: Vec<&str> = PIN_COLUMNS.iter().map(|(t, _)| *t).collect();
        assert_eq!(titles, ["Pin", "Symbol", "SPICE Node"]);
    }

    /// Enable → disable round-trip clears every Sim-side field.
    #[test]
    fn sim_set_enabled_round_trip_clears_state() {
        let mut e = fixture_editor();
        assert!(e.sim.is_none());
        assert!(e.sim_body.is_none());
        assert!(e.row.sim_ref.is_none());

        apply(&mut e, EditorMsg::SimSetEnabled(true));
        assert!(e.sim.is_some(), "enable should construct a SimModel");
        assert!(
            e.sim_body.is_some(),
            "enable should seed text_editor::Content"
        );
        assert!(e.row.sim_ref.is_some(), "enable should bind sim_ref");
        let bound_uuid = e.sim.as_ref().unwrap().uuid;
        assert_eq!(
            e.row.sim_ref.unwrap().uuid,
            bound_uuid,
            "sim_ref must point at the constructed SimModel"
        );

        apply(&mut e, EditorMsg::SimSetEnabled(false));
        assert!(e.sim.is_none(), "disable should clear sim");
        assert!(e.sim_body.is_none(), "disable should clear sim_body");
        assert!(e.row.sim_ref.is_none(), "disable should clear sim_ref");
    }

    /// Setting a pin-node mapping persists into `default_node_map`.
    #[test]
    fn sim_set_pin_node_persists_into_default_node_map() {
        let mut e = fixture_editor();
        apply(&mut e, EditorMsg::SimSetEnabled(true));
        apply(
            &mut e,
            EditorMsg::SimSetPinNode {
                pin_number: "1".to_string(),
                value: "VCC".to_string(),
            },
        );
        let sim = e.sim.as_ref().unwrap();
        assert_eq!(
            sim.default_node_map.get("1").map(String::as_str),
            Some("VCC")
        );
    }

    /// Empty value removes the key from `default_node_map`.
    #[test]
    fn sim_set_pin_node_empty_removes_key() {
        let mut e = fixture_editor();
        apply(&mut e, EditorMsg::SimSetEnabled(true));
        // Seed a value, then clear it.
        apply(
            &mut e,
            EditorMsg::SimSetPinNode {
                pin_number: "2".to_string(),
                value: "GND".to_string(),
            },
        );
        assert!(e.sim.as_ref().unwrap().default_node_map.contains_key("2"));
        apply(
            &mut e,
            EditorMsg::SimSetPinNode {
                pin_number: "2".to_string(),
                value: String::new(),
            },
        );
        assert!(
            !e.sim.as_ref().unwrap().default_node_map.contains_key("2"),
            "empty value must remove the key"
        );
    }

    /// Whitespace-only values trim to empty and remove the key —
    /// mirrors the trimming that the SaveDraft path does on text input.
    #[test]
    fn sim_set_pin_node_whitespace_removes_key() {
        let mut e = fixture_editor();
        apply(&mut e, EditorMsg::SimSetEnabled(true));
        apply(
            &mut e,
            EditorMsg::SimSetPinNode {
                pin_number: "3".to_string(),
                value: "OUT".to_string(),
            },
        );
        apply(
            &mut e,
            EditorMsg::SimSetPinNode {
                pin_number: "3".to_string(),
                value: "   ".to_string(),
            },
        );
        assert!(
            !e.sim.as_ref().unwrap().default_node_map.contains_key("3"),
            "whitespace-only value must remove the key"
        );
    }

    /// SimSetKind / SimSetName mutate the bound model in place.
    #[test]
    fn sim_set_kind_and_name_mutate_in_place() {
        let mut e = fixture_editor();
        apply(&mut e, EditorMsg::SimSetEnabled(true));
        apply(&mut e, EditorMsg::SimSetKind(SimKind::Ngspice));
        apply(&mut e, EditorMsg::SimSetName("LM358".to_string()));
        let sim = e.sim.as_ref().unwrap();
        assert_eq!(sim.kind, SimKind::Ngspice);
        assert_eq!(sim.name, "LM358");
    }

    /// Enable on an editor that already has a sim is idempotent
    /// (no clobber).
    #[test]
    fn sim_set_enabled_true_is_idempotent_when_already_bound() {
        let mut e = fixture_editor();
        apply(&mut e, EditorMsg::SimSetEnabled(true));
        let first_uuid = e.sim.as_ref().unwrap().uuid;
        apply(&mut e, EditorMsg::SimSetEnabled(true));
        let second_uuid = e.sim.as_ref().unwrap().uuid;
        assert_eq!(first_uuid, second_uuid, "re-enabling must not re-construct");
    }
}
