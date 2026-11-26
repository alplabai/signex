//! Layout probes of Change Component's one dialog (mockup row 155).

use std::collections::HashMap;
use std::path::PathBuf;

use iced::Size;
use signex_i18n::{Arg, I18n, ids};
use signex_library::{ComponentSummary, InternalPn, LifecycleState, PrimitiveRef};
use signex_types::theme::{ThemeId, theme_tokens};
use uuid::Uuid;

use super::view;
use crate::layout_probe::{lay_out_texts, messages_on_click};
use crate::library::messages::{
    LibraryMessage, PlacedPartMsg, PlacedPartTarget, ResolvedPlacement,
};
use crate::library::state::{
    ChangePreview, ChangeScope, ChangeSession, CompareField, CompareKind, CompareRow, LibraryState,
    OpenLibrary, PickerMode, PickerState,
};

const LIB: Uuid = Uuid::from_u128(0x11);
const SAME: Uuid = Uuid::from_u128(0x21);
const WIDE: Uuid = Uuid::from_u128(0x22);
const WINDOW: Size = Size::new(1400.0, 900.0);

fn summary(row_id: Uuid, mpn: &str, description: &str) -> ComponentSummary {
    ComponentSummary {
        row_id,
        internal_pn: InternalPn::new(mpn),
        mpn: mpn.into(),
        state: LifecycleState::Released,
        description: description.into(),
    }
}

/// Two listed parts: one with the placed part's 2 pins, one with 4.
fn state() -> LibraryState {
    let rows = [SAME, WIDE].map(|row_id| {
        let mut row = crate::library::component_preview::test_row();
        row.row_id = row_id;
        row.symbol_ref = PrimitiveRef::new(LIB, Uuid::from_u128(0x31));
        row
    });
    let mut state = LibraryState::default();
    state.open_libraries.push(OpenLibrary {
        root: PathBuf::from("/libs/passives.snxlib"),
        display_name: "passives".into(),
        library_id: LIB,
        tables: HashMap::from([("R".to_string(), rows.to_vec())]),
        cached_components: vec![
            summary(SAME, "RC0603FR-0731K6L", "31.6k · 1% · 0603"),
            summary(WIDE, "YC164-FR-0731K6L", "31.6k · 4 × array · 0612"),
        ],
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        own_edit_stamp: None,
        display: Default::default(),
        pricing: Default::default(),
    });
    state.touch_data();
    let target = PlacedPartTarget {
        sheet: PathBuf::from("/p/buck_channel.snxsch"),
        engine_instance: 1,
        symbol_id: Uuid::from_u128(1),
    };
    let mut session = ChangeSession::new(target, "R5".into());
    session.placed.part = "RC0402FR-0731K6L".into();
    session.placed.footprint = "R_0402".into();
    session.placed.pins = Some(2);
    session.row_pins = HashMap::from([(SAME, 2), (WIDE, 4)]);
    state.picker = Some(PickerState {
        mode: PickerMode::ChangeComponent(Box::new(session)),
        ..PickerState::default()
    });
    state.sync_view_caches();
    state
}

fn resolved() -> ResolvedPlacement {
    let lib_symbol = serde_json::from_value(serde_json::json!({ "id": "L:NEW" })).unwrap();
    ResolvedPlacement {
        lib_id: "L:NEW".into(),
        lib_symbol,
        library_id: LIB,
        row_id: SAME,
        library_version: "1".into(),
        stamp: signex_types::schematic::RowStamp::new(SAME),
        internal_pn: "R-0603-31K6".into(),
        value: "31.6k".into(),
        is_power: false,
        net_tie: false,
        prefix: "R".into(),
        class: "resistor".into(),
        prefix_override: None,
        footprint: "R_0603".into(),
        datasheet: String::new(),
        fields: vec![("MPN".into(), "RC0603FR-0731K6L".into())],
    }
}

/// The state with SAME picked and compared.
fn compared() -> LibraryState {
    let mut state = state();
    let picker = state.picker.as_mut().unwrap();
    let PickerMode::ChangeComponent(session) = &picker.mode else {
        unreachable!()
    };
    let session = (**session).clone();
    let row = |field, now: &str, after: &str, kind| CompareRow {
        field,
        now: now.into(),
        after: after.into(),
        kind,
    };
    picker.selected = Some((
        PathBuf::from("/libs/passives.snxlib"),
        summary(SAME, "RC0603FR-0731K6L", "31.6k · 1% · 0603"),
    ));
    picker.mode = PickerMode::ChangePreview(Box::new(ChangePreview {
        session,
        resolved: resolved(),
        preview: Default::default(),
        rows: vec![
            row(CompareField::Value, "31.6k", "31.6k", CompareKind::Same),
            row(
                CompareField::Footprint,
                "R_0402",
                "R_0603",
                CompareKind::Replaced,
            ),
            row(
                CompareField::Named("Note".into()),
                "FB divider",
                "",
                CompareKind::User,
            ),
        ],
        dropped: Vec::new(),
        scope: ChangeScope::ThisPart,
        pin_map: None,
        pins: Vec::new(),
        pin_choices: Vec::new(),
    }));
    state
}

/// [`compared`] with `edit` applied to its comparison.
fn compared_with(edit: impl FnOnce(&mut ChangePreview)) -> LibraryState {
    let mut state = compared();
    if let Some(picker) = state.picker.as_mut()
        && let PickerMode::ChangePreview(change) = &mut picker.mode
    {
        edit(change);
    }
    state
}

/// R5's two pins mapped by number, on VOUT_3V3 and FB (mockup row 155).
fn on_two_nets(change: &mut ChangePreview) {
    use signex_types::schematic::{PinMatchedBy, PinPair};
    let row = |pin: &str, net: &str| signex_engine::PinMapRow {
        old: pin.into(),
        old_name: String::new(),
        new: Some((pin.into(), String::new())),
        net: Some(net.into()),
    };
    change.pins = vec![row("1", "VOUT_3V3"), row("2", "FB")];
    change.preview.mapping.pairs = ["1", "2"]
        .map(|pin| PinPair {
            old: pin.into(),
            new: pin.into(),
            by: PinMatchedBy::Number,
        })
        .to_vec();
}

/// The laid-out texts, without the bidi isolates the catalog puts around
/// arguments.
fn texts(state: &LibraryState) -> Vec<String> {
    let i18n = I18n::en_us_ref();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let card = view(state.picker.as_ref().unwrap(), &tokens, i18n, false).expect("a card");
    lay_out_texts(card, WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn clicks(state: &LibraryState, label: &str) -> Vec<LibraryMessage> {
    let i18n = I18n::en_us_ref();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let card = view(state.picker.as_ref().unwrap(), &tokens, i18n, false).expect("a card");
    messages_on_click(card, WINDOW, label)
}

#[test]
fn the_list_has_the_three_chips_and_a_pin_tag_on_every_row() {
    let shown = texts(&state());
    for label in [
        "Same symbol",
        "Same value",
        "In stock",
        "RC0603FR-0731K6L",
        "31.6k · 1% · 0603",
        "same pins",
        "4 pins, not 2",
        "Pick a part in the list to compare it with R5.",
        "R5 · buck_channel.snxsch",
    ] {
        assert!(shown.iter().any(|t| t == label), "{label:?} in {shown:#?}");
    }
}

#[test]
fn a_row_with_the_same_pins_is_picked_and_one_whose_pins_differ_is_not() {
    let state = state();
    let picked = clicks(&state, "RC0603FR-0731K6L");
    assert!(
        picked.iter().any(|m| matches!(
            m,
            LibraryMessage::PlacedPart(PlacedPartMsg::ChangeRowClicked(id)) if *id == SAME
        )),
        "{picked:?}"
    );
    assert!(clicks(&state, "YC164-FR-0731K6L").is_empty());
}

#[test]
fn the_comparison_shows_both_parts_the_table_kept_and_apply_to() {
    let shown = texts(&compared());
    let i18n = I18n::en_us_ref();
    let upper = |id| i18n.upper_for_display(&i18n.message_plain(id, &[]));
    for label in [
        "Placed now".to_string(),
        "Replace with".into(),
        "RC0402FR-0731K6L".into(),
        "R_0603".into(),
        "Keep now".into(),
        "not in part".into(),
        "user".into(),
        upper(ids::LIBRARY_CHANGE_COMPONENT_PARAMETERS),
        upper(ids::LIBRARY_CHANGE_COMPONENT_KEPT),
        upper(ids::LIBRARY_CHANGE_COMPONENT_APPLY_TO),
        "Change 1 part".into(),
        "One undo step. The PCB footprint changes on Update PCB.".into(),
    ] {
        assert!(shown.contains(&label), "{label:?} in {shown:#?}");
    }
    // A radio draws its own label, which the text probe does not list: the
    // Apply to option is the radio under the heading, labelled from the
    // catalog with the part's reference.
    let only = i18n
        .message(
            ids::LIBRARY_CHANGE_COMPONENT_SCOPE_THIS,
            &[("reference", Arg::from("R5"))],
        )
        .into_string()
        .replace(['\u{2068}', '\u{2069}'], "");
    assert_eq!(only, "Only R5 on this sheet");
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let state = compared();
    let card = || view(state.picker.as_ref().unwrap(), &tokens, i18n, false).unwrap();
    let laid = lay_out_texts(card(), WINDOW);
    let apply_to = upper(ids::LIBRARY_CHANGE_COMPONENT_APPLY_TO);
    let heading = crate::layout_probe::find(&laid, &apply_to).bounds;
    let radio = super::TS.heading;
    let at = iced::Point::new(
        heading.x + radio / 2.0,
        heading.y + heading.height + crate::styles::Spacing::DEFAULT.s + radio / 2.0,
    );
    let picked = crate::layout_probe::messages_on_click_at(card(), WINDOW, None, at);
    assert!(
        picked.iter().any(|m| matches!(
            m,
            LibraryMessage::PlacedPart(PlacedPartMsg::ChangeScopePicked(ChangeScope::ThisPart))
        )),
        "{picked:?}"
    );
}

#[test]
fn apply_to_offers_only_this_part_until_the_drawn_scopes_are_decided() {
    let shown = texts(&compared());
    assert!(
        !shown
            .iter()
            .any(|t| t.starts_with("Every ") || t.contains(" in all ")),
        "no scope beyond Only R5 in {shown:#?}"
    );
}

#[test]
fn change_confirms_only_while_the_list_shows_the_compared_part() {
    let i18n = I18n::en_us_ref();
    let label = i18n
        .message(
            ids::LIBRARY_CHANGE_COMPONENT_APPLY,
            &[("count", Arg::from(1_usize))],
        )
        .into_string();
    let state = compared();
    let confirmed = |messages: &[LibraryMessage]| {
        messages.iter().any(|m| {
            matches!(
                m,
                LibraryMessage::PlacedPart(PlacedPartMsg::ChangeConfirmed)
            )
        })
    };
    assert!(confirmed(&clicks(&state, &label)));

    let mut hidden = compared();
    if let Some(picker) = hidden.picker.as_mut()
        && let PickerMode::ChangePreview(change) = &mut picker.mode
    {
        change.session.hidden.insert(SAME);
    }
    assert!(!confirmed(&clicks(&hidden, &label)), "a hidden part");
}

#[test]
fn a_user_parameter_box_toggles_its_keep() {
    let mut state = compared();
    if let Some(picker) = state.picker.as_mut()
        && let PickerMode::ChangePreview(change) = &mut picker.mode
    {
        change.rows.retain(|r| r.kind == CompareKind::User);
    }
    let i18n = I18n::en_us_ref();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let card = view(state.picker.as_ref().unwrap(), &tokens, i18n, false).unwrap();
    let laid = lay_out_texts(card, WINDOW);
    let note = crate::layout_probe::find(&laid, "Note").bounds;
    let keep = crate::layout_probe::find(&laid, "Keep now").bounds;
    // The box sits in the Keep now column, on the Note row.
    let at = iced::Point::new(keep.x + keep.height / 2.0, note.center_y());
    let card = view(state.picker.as_ref().unwrap(), &tokens, i18n, false).unwrap();
    let toggled = crate::layout_probe::messages_on_click_at(card, WINDOW, None, at);
    assert!(
        toggled.iter().any(|m| matches!(
            m,
            LibraryMessage::PlacedPart(PlacedPartMsg::ChangeKeepToggled(name)) if name == "Note"
        )),
        "{toggled:?}"
    );
}

#[test]
fn kept_names_the_net_each_pin_keeps() {
    let shown = texts(&compared_with(on_two_nets));
    let line = "Designator R5 · position and rotation · pin 1 on VOUT_3V3, pin 2 on FB \
                (pins matched by number, 2 of 2)";
    assert!(shown.iter().any(|t| t == line), "{line:?} in {shown:#?}");
}

#[test]
fn kept_says_the_pins_are_mapped_by_hand() {
    let shown = texts(&compared_with(|change| {
        on_two_nets(change);
        change.pin_map = Some(change.preview.mapping.clone());
    }));
    let line = "Designator R5 · position and rotation · pin 1 on VOUT_3V3, pin 2 on FB \
                (pins as mapped, 2 of 2)";
    assert!(shown.iter().any(|t| t == line), "{line:?} in {shown:#?}");
}

#[test]
fn a_mapping_that_cannot_be_followed_says_why_and_change_does_nothing() {
    let i18n = I18n::en_us_ref();
    let label = i18n
        .message(
            ids::LIBRARY_CHANGE_COMPONENT_APPLY,
            &[("count", Arg::from(1_usize))],
        )
        .into_string();
    let why = "the carried wires of R5 would join another net; rewire them by hand";
    let state = compared_with(|change| {
        change.pin_map = Some(Default::default());
        change.preview.carried = vec!["2".into()];
        change.preview.refused = Some(why.into());
    });
    let shown = texts(&state);
    for line in [
        format!("The pin mapping cannot be followed: {why}"),
        "Connected pins whose wires move with them to the new pin: 2".to_string(),
    ] {
        assert!(shown.contains(&line), "{line:?} in {shown:#?}");
    }
    let confirmed = clicks(&state, &label).into_iter().any(|m| {
        matches!(
            m,
            LibraryMessage::PlacedPart(PlacedPartMsg::ChangeConfirmed)
        )
    });
    assert!(!confirmed, "Change is inert while the mapping is refused");
}

#[test]
fn the_dialog_builds_on_every_theme() {
    let i18n = I18n::en_us_ref();
    for state in [state(), compared()] {
        for id in ThemeId::BUILTINS {
            let tokens = theme_tokens(*id);
            assert!(view(state.picker.as_ref().unwrap(), &tokens, i18n, false).is_some());
        }
    }
}
