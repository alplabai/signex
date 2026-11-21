//! The Component view (#2354, Graphite mockup row 94): the header names the
//! selection, the search leaves rows out, the sections hold the mockup row 94 rows,
//! the Parameters tab lists the shared parameters, the mismatch note offers
//! Update PCB, and the illustration draws pin 1 and the courtyard and links
//! its parts to their fields.

use signex_types::pcb::{ComponentType, Point};
use signex_types::pcb_layer::BoardSide;
use signex_widgets::property_illustration::{IllustrationMsg, Style};

use super::*;
use crate::app::Signex;
use crate::layout_probe::{lay_out_texts, messages_on_click};
use crate::panels::PcbBoardSummary;

const WINDOW: iced::Size = iced::Size::new(360.0, 2400.0);

/// One unlocked top-side component at (10, 5) with an MPN parameter.
pub(crate) fn one_component(designator: &str, footprint: &str) -> PcbComponentFacts {
    PcbComponentFacts {
        board: "board.snxpcb".into(),
        footprints: vec![Uuid::from_u128(1)],
        x: Shared::Same(10.0),
        y: Shared::Same(5.0),
        rotation: Shared::Same(90.0),
        locked: Shared::Same(false),
        side: Shared::Same(BoardSide::Top),
        designator: Shared::Same(designator.into()),
        designator_shown: Some(Shared::Same(true)),
        comment: Shared::Same("10k".into()),
        comment_shown: Some(Shared::Same(false)),
        area_mm2: Shared::Same(Some(2.5)),
        footprint_name: Shared::Same(footprint.into()),
        library: Shared::Same(None),
        schematic_path: Shared::Same(None),
        settings: ComponentSettingFacts::defaults(),
        identity: Shared::Same(PartIdentity::NotLinked),
        designator_mismatches: 0,
        parameters: vec![ParameterFacts {
            key: "MPN".into(),
            value: Shared::Same("RC0603".into()),
        }],
        drafts: BTreeMap::new(),
        read_only: false,
        search: String::new(),
        drawing: None,
        illustration_hover: IllustrationHover::default(),
    }
}

/// Two pads 2 mm either side of the origin, pin 1 on the left; a courtyard
/// rectangle when `courtyard`.
fn drawn(courtyard: bool) -> ComponentDrawing {
    let corners = [
        Point::new(-3.0, -1.0),
        Point::new(3.0, -1.0),
        Point::new(3.0, 1.0),
        Point::new(-3.0, 1.0),
    ];
    ComponentDrawing {
        side: BoardSide::Top,
        pads: vec![
            PadOutline::Ring(vec![
                Point::new(-2.5, -0.5),
                Point::new(-1.5, -0.5),
                Point::new(-1.5, 0.5),
                Point::new(-2.5, 0.5),
            ]),
            PadOutline::Disc {
                center: Point::new(2.0, 0.0),
                radius: 0.5,
            },
        ],
        pin_one: Some(Point::new(-2.0, 0.0)),
        courtyard: if courtyard {
            (0..4).map(|i| (corners[i], corners[(i + 1) % 4])).collect()
        } else {
            Vec::new()
        },
        outline: Vec::new(),
    }
}

/// The panel context showing `facts` on tab `tab`, with the sections of
/// `opened` toggled.
fn context(facts: PcbComponentFacts, tab: usize, opened: &[&str]) -> PanelContext {
    let (app, _boot) = Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.has_pcb = true;
    ctx.properties_tab = tab;
    for key in opened {
        ctx.collapsed_sections.insert((*key).to_string());
    }
    ctx.pcb_board = Some(Box::new(
        PcbBoardSummary::default().with_component(Some(facts)),
    ));
    ctx
}

fn texts_in(ctx: &PanelContext) -> Vec<String> {
    let board = ctx.pcb_board.as_ref().expect("the board summary");
    lay_out_texts(super::super::pcb_board_properties::view(ctx, board), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn texts_of(facts: PcbComponentFacts, tab: usize) -> Vec<String> {
    texts_in(&context(facts, tab, &[]))
}

fn clicks(ctx: &PanelContext, label: &str) -> Vec<PanelMsg> {
    let board = ctx.pcb_board.as_ref().expect("the board summary");
    messages_on_click(
        super::super::pcb_board_properties::view(ctx, board),
        WINDOW,
        label,
    )
}

fn has(texts: &[String], wanted: &str) -> bool {
    texts.iter().any(|t| t == wanted)
}

fn i18n() -> std::sync::Arc<signex_i18n::I18n> {
    // The catalog the app's panels render with.
    let (app, _boot) = Signex::new();
    app.document_state.panel_ctx.i18n.clone()
}

#[test]
fn shared_values_are_kept_and_differing_ones_are_mixed() {
    assert_eq!(Shared::of([1.0, 1.0]), Some(Shared::Same(1.0)));
    assert_eq!(Shared::of([1.0, 2.0]), Some(Shared::Mixed));
    assert_eq!(Shared::<f64>::of([]), None);
}

#[test]
fn one_component_lists_every_section_and_its_rows() {
    let texts = texts_of(one_component("R7", "R_0603"), 0);
    for wanted in [
        "Component",
        "R7",
        "General",
        "Parameters",
        "Position",
        "X",
        "Y",
        "mm",
        "Rotation",
        "°",
        "Locked",
        "Layer",
        "Top Layer",
        "Bottom Layer",
        "Designator",
        "Comment",
        "Description",
        "This component has no Description parameter.",
        "Area",
        "2.500 mm²",
        "Footprint Name",
        "R_0603",
        "Change…",
        "Library",
        "Not from a library",
        "Type",
        "Height",
        "3D Opacity",
        "Lock",
        "Primitives",
        "Strings",
        "Hier. Path",
        "Not linked to a schematic part",
        "Channel Offset",
        "Design Item ID",
        "Source",
    ] {
        assert!(has(&texts, wanted), "{wanted} in {texts:?}");
    }
}

#[test]
fn swapping_options_start_closed_and_say_off() {
    let texts = texts_of(one_component("R7", "R_0603"), 0);
    assert!(has(&texts, "off"), "{texts:?}");
    assert!(!has(&texts, "Enable Pin Swapping"), "closed: {texts:?}");
    let opened = context(
        one_component("R7", "R_0603"),
        0,
        &["pcb_component_swapping.open"],
    );
    let texts = texts_in(&opened);
    for wanted in ["Enable Pin Swapping", "Enable Part Swapping"] {
        assert!(has(&texts, wanted), "{wanted} in {texts:?}");
    }
    let mut on = one_component("R7", "R_0603");
    on.settings.pin_swapping = Shared::Same(true);
    assert!(!has(&texts_of(on, 0), "off"), "a switch is on");
}

#[test]
fn the_parameters_tab_carries_the_parameter_count() {
    let mut facts = one_component("R7", "R_0603");
    facts.parameters.push(ParameterFacts {
        key: "Tolerance".into(),
        value: Shared::Same("1%".into()),
    });
    assert!(has(&texts_of(facts, 0), "2"));
}

#[test]
fn several_components_are_counted_and_edit_their_designators_one_at_a_time() {
    let mut facts = one_component("R7", "R_0603");
    facts.footprints.push(Uuid::from_u128(2));
    facts.designator = Shared::Mixed;
    facts.x = Shared::Mixed;
    let texts = texts_of(facts, 0);
    assert!(has(&texts, "2 components"), "{texts:?}");
    assert!(!has(&texts, "R7"), "{texts:?}");
    assert!(has(&texts, "One at a time"), "{texts:?}");
    assert_eq!(
        texts.iter().filter(|t| *t == "R_0603").count(),
        1,
        "only the Footprint Name row names it; the header does not for several: {texts:?}"
    );
}

#[test]
fn the_search_leaves_out_rows_whose_label_does_not_match() {
    let mut facts = one_component("R7", "R_0603");
    facts.search = "rot".into();
    let texts = texts_of(facts, 0);
    assert!(has(&texts, "Rotation"), "{texts:?}");
    for gone in ["Position", "Layer", "Footprint Name", "Channel Offset"] {
        assert!(!has(&texts, gone), "{gone} in {texts:?}");
    }
}

#[test]
fn the_search_finds_a_lock_by_its_full_name_and_opens_swapping() {
    let mut facts = one_component("R7", "R_0603");
    facts.search = "lock strings".into();
    let texts = texts_of(facts, 0);
    assert!(has(&texts, "Strings"), "{texts:?}");
    let mut facts = one_component("R7", "R_0603");
    facts.search = "pin swap".into();
    assert!(has(&texts_of(facts, 0), "Enable Pin Swapping"));
}

#[test]
fn the_parameters_tab_lists_the_shared_parameters() {
    let texts = texts_of(one_component("R7", "R_0603"), 1);
    assert!(has(&texts, "MPN"), "{texts:?}");
    assert!(!has(&texts, "Position"), "{texts:?}");
    let mut bare = one_component("R7", "R_0603");
    bare.parameters.clear();
    assert!(has(&texts_of(bare, 1), "No parameters"));
}

#[test]
fn placement_edits_stop_while_any_component_is_locked() {
    let mut facts = one_component("R7", "R_0603");
    assert!(facts.is_editable());
    facts.locked = Shared::Mixed;
    assert!(!facts.is_editable());
    facts.locked = Shared::Same(false);
    facts.read_only = true;
    assert!(!facts.is_editable());
}

#[test]
fn the_library_row_shows_the_library_name_or_that_it_is_not_open() {
    let mut facts = one_component("R7", "R_0603");
    facts.library = Shared::Same(Some(super::FootprintLibrary::Open("Stock".into())));
    assert!(has(&texts_of(facts.clone(), 0), "Stock"));
    let id = Uuid::from_u128(9);
    facts.library = Shared::Same(Some(super::FootprintLibrary::NotOpen(id)));
    let texts = texts_of(facts, 0);
    assert!(has(&texts, &format!("{id} (not open)")), "{texts:?}");
}

#[test]
fn the_stored_settings_show_their_values() {
    let mut facts = one_component("R7", "R_0603");
    facts.settings.component_type = Shared::Same(ComponentType::Mechanical);
    facts.settings.description = Shared::Same("Chip resistor".into());
    facts.settings.body_opacity = Shared::Same(0.4);
    let field = ComponentField::Setting(TypedSetting::Description);
    let stored = facts.settings.description.same().cloned();
    assert_eq!(
        super::general::shown_value(&facts, &field, stored),
        "Chip resistor",
        "an input shows the stored value"
    );
    // The opacity box shows the slider's draft while it is dragged.
    let opacity = ComponentField::Setting(TypedSetting::BodyOpacity);
    assert_eq!(
        super::general::shown_value(&facts, &opacity, Some("0.40".into())),
        "0.40"
    );
    facts.drafts.insert(opacity.clone(), "40%".into());
    assert_eq!(
        super::general::shown_value(&facts, &opacity, Some("0.40".into())),
        "40%",
        "a typed percentage stays in the box until Enter"
    );
    let texts = texts_of(facts, 0);
    for wanted in ["Mechanical", "Primitives", "Strings"] {
        assert!(has(&texts, wanted), "{wanted} in {texts:?}");
    }
}

#[test]
fn the_description_parameter_and_the_footprint_description_are_two_rows() {
    let mut facts = one_component("R7", "R_0603");
    facts.settings.description = Shared::Same("Chip resistor".into());
    facts.parameters.push(ParameterFacts {
        key: "Description".into(),
        value: Shared::Same("Thick film 10k".into()),
    });
    let texts = texts_of(facts, 0);
    assert_eq!(
        texts.iter().filter(|t| *t == "Description").count(),
        2,
        "Properties edits the parameter, Footprint the footprint's own: {texts:?}"
    );
    assert!(
        !has(&texts, "This component has no Description parameter."),
        "{texts:?}"
    );
}

#[test]
fn the_lock_checkboxes_and_change_button_send_their_edits() {
    let ctx = context(one_component("R7", "R_0603"), 0, &[]);
    let primitives = clicks(&ctx, "Primitives");
    assert!(
        primitives.iter().any(|m| matches!(
            m,
            PanelMsg::PcbComponent(PcbComponentMsg::SettingPicked {
                setting: ComponentSetting::LockPrimitives(true),
                ..
            })
        )),
        "{primitives:?}"
    );
    let change = clicks(&ctx, "Change…");
    assert!(
        change.iter().any(|m| matches!(
            m,
            PanelMsg::PcbComponent(PcbComponentMsg::FootprintPickRequested { .. })
        )),
        "{change:?}"
    );
    let bottom = clicks(&ctx, "Bottom Layer");
    assert!(
        bottom.iter().any(|m| matches!(
            m,
            PanelMsg::PcbComponent(PcbComponentMsg::SidePicked {
                side: BoardSide::Bottom,
                ..
            })
        )),
        "{bottom:?}"
    );
}

#[test]
fn a_read_part_shows_its_identity_and_a_designator_mismatch() {
    let mut facts = one_component("R7", "R_0603");
    facts.identity = Shared::Same(PartIdentity::Read {
        designator: "R9".into(),
        design_item_id: Some("RES-10K-0603".into()),
        source: Some("Passives".into()),
    });
    facts.designator_mismatches = 1;
    let texts = texts_of(facts, 0);
    for wanted in ["RES-10K-0603", "Passives", "R9"] {
        assert!(has(&texts, wanted), "{wanted} in {texts:?}");
    }
    assert!(
        texts
            .iter()
            .any(|t| t.contains("schematic names this component R9, the board R7")),
        "{texts:?}"
    );
}

#[test]
fn update_pcb_is_offered_only_on_a_designator_mismatch() {
    let mut facts = one_component("R7", "R_0603");
    facts.identity = Shared::Same(PartIdentity::Read {
        designator: "R7".into(),
        design_item_id: None,
        source: None,
    });
    assert!(!has(&texts_of(facts.clone(), 0), "Update PCB"));
    facts.identity = Shared::Same(PartIdentity::Read {
        designator: "R9".into(),
        design_item_id: None,
        source: None,
    });
    facts.designator_mismatches = 1;
    let ctx = context(facts, 0, &[]);
    assert!(has(&texts_in(&ctx), "Update PCB"));
    let pressed = clicks(&ctx, "Update PCB");
    assert!(
        pressed
            .iter()
            .any(|m| matches!(m, PanelMsg::PcbComponent(PcbComponentMsg::UpdatePcbPressed))),
        "{pressed:?}"
    );
}

#[test]
fn an_unread_part_says_to_open_its_sheet() {
    let mut facts = one_component("R7", "R_0603");
    facts.identity = Shared::Same(PartIdentity::NotRead);
    let texts = texts_of(facts, 0);
    assert!(
        has(&texts, "Open the schematic sheet to read this"),
        "{texts:?}"
    );
}

#[test]
fn differing_comments_get_a_note_that_enter_sets_all() {
    let mut facts = one_component("R7", "R_0603");
    facts.footprints.push(Uuid::from_u128(2));
    facts.designator = Shared::Mixed;
    facts.comment = Shared::Mixed;
    let texts = texts_of(facts, 0);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Enter sets the typed comment on all 2")),
        "{texts:?}"
    );
}

#[test]
fn the_parameters_tab_offers_an_add_row_while_editable() {
    let texts = texts_of(one_component("R7", "R_0603"), 1);
    assert!(has(&texts, "Add"), "{texts:?}");
    let mut locked = one_component("R7", "R_0603");
    locked.locked = Shared::Same(true);
    assert!(!has(&texts_of(locked, 1), "Add"));
}

#[test]
fn the_illustration_draws_pin_one_and_a_courtyard_only_when_there_is_one() {
    let i18n = i18n();
    let mut facts = one_component("R7", "R_0603");
    assert!(
        illustration(&facts, &i18n).is_none(),
        "nothing is drawn without a drawing (several components)"
    );
    facts.drawing = Some(drawn(false));
    let bare = illustration(&facts, &i18n).expect("one component is drawn");
    assert!(
        bare.visible_parts().any(|p| p.is_linked_to(&PIN_ONE_FIELD)),
        "pin 1"
    );
    assert!(
        !bare.visible_parts().any(|p| p.style == Style::dashed()),
        "no courtyard is made up"
    );
    facts.drawing = Some(drawn(true));
    let full = illustration(&facts, &i18n).expect("drawn");
    assert_eq!(
        full.visible_parts()
            .filter(|p| p.style == Style::dashed())
            .count(),
        4,
        "the courtyard's four sides"
    );
}

#[test]
fn hovering_a_field_lights_its_callouts_and_a_callout_lights_its_field() {
    let i18n = i18n();
    let mut facts = one_component("R7", "R_0603");
    facts.drawing = Some(drawn(true));
    facts
        .illustration_hover
        .update(IllustrationMsg::FieldHovered(DESIGNATOR_FIELD));
    let drawn = illustration(&facts, &i18n).expect("drawn");
    let lit: Vec<_> = drawn
        .visible_parts()
        .filter(|p| facts.illustration_hover.is_part_highlighted(p))
        .collect();
    assert_eq!(lit.len(), 1, "the designator text");
    assert!(lit[0].is_linked_to(&DESIGNATOR_FIELD));
    facts
        .illustration_hover
        .update(IllustrationMsg::CalloutHovered(LAYER_FIELD));
    assert!(facts.illustration_hover.is_field_highlighted(&LAYER_FIELD));
    let drawn = illustration(&facts, &i18n).expect("drawn");
    let lit = drawn
        .visible_parts()
        .filter(|p| facts.illustration_hover.is_part_highlighted(p))
        .count();
    assert_eq!(lit, 3, "the two pads and the side note");
}
