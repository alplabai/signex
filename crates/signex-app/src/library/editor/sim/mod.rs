//! Sim tab — SPICE deck editor + per-pin SPICE node mapping.
//!
//! WS-L: backed by the typed `SimModel` primitive bound through
//! `Revision::sim_ref`. The view operates entirely on
//! `state.sim` (resolved lazily by the dispatcher's
//! `EditorTab::Sim` arm via `LibrarySet::resolve_sim`) and
//! `state.sim_body`, the live `text_editor::Content` mirror of
//! the SPICE deck.
//!
//! The pin/node table iterates `editor.symbol.pins`; the SPICE-node
//! buffer for a given pin is read from
//! `sim.default_node_map[pin_number]` and edits flow back through
//! [`EditorMsg::SimSetPinNode`] which empty-removes / non-empty-
//! inserts the mapping.

pub mod state;

use iced::widget::{
    Space, checkbox, column, container, pick_list, row, scrollable, text, text_editor, text_input,
};
use iced::{Border, Element, Length, Theme};
use signex_library::SimKind;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress};

const SIM_KIND_OPTS: [SimKind; 4] = [
    SimKind::Spice3,
    SimKind::Ngspice,
    SimKind::LtSpice,
    SimKind::VerilogA,
];

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

pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let lib_path_for_toggle = address.library_path.clone();
    let table_for_toggle = address.table.clone();
    let row_id = address.row_id;
    let has_sim = state.sim.is_some();
    let toggle = checkbox(has_sim)
        .label("Has SPICE Model")
        .on_toggle(move |v| LibraryMessage::EditorEvent {
            library_path: lib_path_for_toggle.clone(),
            table: table_for_toggle.clone(),
            row_id,
            msg: EditorMsg::SimSetEnabled(v),
        })
        .size(14)
        .text_size(12)
        .spacing(6);

    let Some(sim) = state.sim.as_ref() else {
        // No sim model bound — show only the toggle and a muted hint.
        let body = column![
            row![toggle].align_y(iced::Alignment::Center),
            Space::new().height(10),
            text("No SPICE model bound to this component.")
                .size(11)
                .color(muted),
            text("Toggle \u{201C}Has SPICE Model\u{201D} to add one.")
                .size(11)
                .color(muted),
        ]
        .spacing(0)
        .width(Length::Fill);
        return container(body)
            .padding(14)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(crate::styles::modal_card(tokens))
            .into();
    };

    // Header — checkbox + kind picker.
    let lib_path_for_kind = address.library_path.clone();
    let table_for_kind = address.table.clone();
    let kind_picker = pick_list(
        SIM_KIND_OPTS.map(SimKindPick),
        Some(SimKindPick(sim.kind)),
        move |SimKindPick(k)| LibraryMessage::EditorEvent {
            library_path: lib_path_for_kind.clone(),
            table: table_for_kind.clone(),
            row_id,
            msg: EditorMsg::SimSetKind(k),
        },
    )
    .text_size(12)
    .padding([4, 8]);

    let header_row =
        row![toggle, Space::new().width(20), kind_picker,].align_y(iced::Alignment::Center);

    // Name row.
    let lib_path_for_name = address.library_path.clone();
    let table_for_name = address.table.clone();
    let name_input = text_input("LM358_DUAL", sim.name.as_str())
        .on_input(move |s| LibraryMessage::EditorEvent {
            library_path: lib_path_for_name.clone(),
            table: table_for_name.clone(),
            row_id,
            msg: EditorMsg::SimSetName(s),
        })
        .padding([4, 8])
        .size(12);
    let name_block = column![text("Name").size(10).color(muted), name_input,].spacing(4);

    // SPICE deck — multi-line text_editor backed by state.sim_body.
    let deck: Element<'a, LibraryMessage> = if let Some(content) = state.sim_body.as_ref() {
        let lib_path_for_deck = address.library_path.clone();
        let table_for_deck = address.table.clone();
        let editor_widget = text_editor(content)
            .placeholder(".SUBCKT NAME 1 2 3 4\n  …\n.ENDS")
            .padding(8)
            .size(12)
            .height(Length::Fixed(220.0))
            .on_action(move |a| LibraryMessage::EditorEvent {
                library_path: lib_path_for_deck.clone(),
                table: table_for_deck.clone(),
                row_id,
                msg: EditorMsg::SimBodyAction(a),
            });
        editor_widget.into()
    } else {
        // sim_body should have been seeded by the SelectTab(Sim) arm.
        // This branch is the safety fallback for tabs that landed on
        // Sim without going through SelectTab — show a passive
        // multi-line view of whatever sim.body has.
        text(sim.body.clone()).size(11).color(text_c).into()
    };
    let deck_block = column![text("SPICE deck").size(10).color(muted), deck,].spacing(4);

    // Pin/Node map table. Defensive fallback: if `editor.symbol` is
    // None (the symbol primitive failed to resolve, or the user
    // landed on the Sim tab without going through SelectTab) we show
    // a muted hint instead of crashing on an empty pin slice. The
    // canonical lazy-load happens in
    // `dispatch::library::handle_select_editor_tab`.
    let table: Element<'a, LibraryMessage> = match state.symbol.as_ref() {
        Some(symbol) => view_pin_node_table(sim, &symbol.pins, tokens, &address),
        None => container(
            text("(load Symbol tab first to populate pin/node map)")
                .size(11)
                .color(muted),
        )
        .padding([6, 0])
        .into(),
    };
    let table_block = column![
        text("Pin / Node Map").size(10).color(muted),
        Space::new().height(4),
        table,
    ]
    .spacing(0);

    let body = scrollable(
        column![
            header_row,
            Space::new().height(10),
            name_block,
            Space::new().height(14),
            deck_block,
            Space::new().height(14),
            table_block,
        ]
        .spacing(0)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    container(body)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into()
}

fn view_pin_node_table<'a>(
    sim: &'a signex_library::SimModel,
    pins: &'a [signex_library::SymbolPin],
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    if pins.is_empty() {
        return container(text("(symbol has no pins yet)").size(11).color(muted))
            .padding([6, 0])
            .into();
    }

    let header = row![
        text("Pin").size(10).color(muted).width(Length::Fixed(56.0)),
        text("Symbol")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        text("SPICE Node")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(3)),
    ]
    .padding([4, 8]);

    let mut col = column![header].spacing(2);
    for pin in pins {
        col = col.push(pin_node_row(sim, pin, tokens, address));
    }

    container(col)
        .padding(4)
        .style(move |_: &Theme| iced::widget::container::Style {
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
}

fn pin_node_row<'a>(
    sim: &'a signex_library::SimModel,
    pin: &'a signex_library::SymbolPin,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let buf = sim
        .default_node_map
        .get(pin.number.as_str())
        .cloned()
        .unwrap_or_default();

    let pin_for_input = pin.number.clone();
    let lib_path_for_input = address.library_path.clone();
    let table_for_input = address.table.clone();
    let row_id = address.row_id;
    let input = text_input("net", &buf)
        .padding([3, 6])
        .size(11)
        .on_input(move |s| LibraryMessage::EditorEvent {
            library_path: lib_path_for_input.clone(),
            table: table_for_input.clone(),
            row_id,
            msg: EditorMsg::SimSetPinNode {
                pin_number: pin_for_input.clone(),
                value: s,
            },
        });

    container(
        row![
            text(pin.number.clone())
                .size(11)
                .color(text_c)
                .width(Length::Fixed(56.0)),
            text(if pin.name.is_empty() {
                "—".to_string()
            } else {
                pin.name.clone()
            })
            .size(11)
            .color(muted)
            .width(Length::FillPortion(2)),
            container(input).width(Length::FillPortion(3)),
        ]
        .align_y(iced::Alignment::Center)
        .padding([3, 8]),
    )
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 1.0,
            radius: 2.0.into(),
            color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06),
        },
        ..Default::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::messages::EditorMsg;
    use crate::library::state::ComponentEditorState;
    use signex_library::{
        Component, ComponentClass, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
        ParamMap, PlmReserved, PrimitiveRef, Revision, Version,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    fn fixture_editor() -> ComponentEditorState {
        let lib_id = Uuid::new_v4();
        let comp_id = Uuid::now_v7();
        let rev = Revision {
            version: Version::new(0, 1),
            state: LifecycleState::Draft,
            created: chrono::Utc::now(),
            author: "test@example.com".into(),
            message: "init".into(),
            symbol_ref: PrimitiveRef::new(lib_id, Uuid::new_v4()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            datasheet: DatasheetRef::default(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            content_hash: [0u8; 32],
        };
        let comp = Component {
            uuid: comp_id,
            internal_pn: InternalPn::new("TEST_PN"),
            class: ComponentClass::generic(),
            category: PathBuf::from("Generic"),
            family: None,
            revisions: vec![rev],
            head: Version::new(0, 1),
        };
        ComponentEditorState::from_head(PathBuf::from("/tmp/test.snxlib"), comp, false)
    }

    fn apply(editor: &mut ComponentEditorState, msg: EditorMsg) {
        // Drive the same code path the dispatcher uses for inline edits.
        // `apply_inline_edit` is `pub(crate)` so the unit tests in this
        // module can exercise the SimSet*/SimBodyAction handlers
        // without standing up the full Signex update tree.
        crate::app::dispatch::library::apply_inline_edit(editor, msg);
    }

    /// WS-L: enable → disable round-trip clears every Sim-side field.
    #[test]
    fn sim_set_enabled_round_trip_clears_state() {
        let mut e = fixture_editor();
        assert!(e.sim.is_none());
        assert!(e.sim_body.is_none());
        assert!(e.draft.sim_ref.is_none());

        apply(&mut e, EditorMsg::SimSetEnabled(true));
        assert!(e.sim.is_some(), "enable should construct a SimModel");
        assert!(
            e.sim_body.is_some(),
            "enable should seed text_editor::Content"
        );
        assert!(e.draft.sim_ref.is_some(), "enable should bind sim_ref");
        let bound_uuid = e.sim.as_ref().unwrap().uuid;
        assert_eq!(
            e.draft.sim_ref.unwrap().uuid,
            bound_uuid,
            "sim_ref must point at the constructed SimModel"
        );

        apply(&mut e, EditorMsg::SimSetEnabled(false));
        assert!(e.sim.is_none(), "disable should clear sim");
        assert!(e.sim_body.is_none(), "disable should clear sim_body");
        assert!(e.draft.sim_ref.is_none(), "disable should clear sim_ref");
    }

    /// WS-L: setting a pin-node mapping persists into
    /// `default_node_map`.
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

    /// WS-L: empty value removes the key from `default_node_map`.
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

    /// WS-L: whitespace-only values trim to empty and remove the key —
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

    /// WS-L: SimSetKind / SimSetName mutate the bound model in place.
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

    /// WS-L: enable on an editor that already has a sim is idempotent
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
