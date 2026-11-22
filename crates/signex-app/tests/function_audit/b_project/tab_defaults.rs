//! Rows m36 (Schematic defaults) and m37 (Project setup) of the Project tab.

use signex_app::app::{NewProjectMsg, ProjectSection, ProjectTabMsg};
use signex_types::designator::DesignatorStandard;
use signex_types::net_class::NetLineStyle;
use signex_types::project::{PcbField, SchematicDefaultField as Field, SetupProfile};
use signex_types::schematic::FillType;

use super::support::{Bench, bench};

fn on(section: ProjectSection) -> Bench {
    let mut b = bench(&[("root.snxsch", "")]);
    b.open_section(section);
    b
}

fn type_default(b: &mut Bench, field: Field, text: &str) {
    let project_id = b.id;
    b.tab(ProjectTabMsg::SchematicDefaultEdited {
        project_id,
        field,
        text: text.to_string(),
    });
}

fn stored(b: &Bench, field: Field) -> Option<f64> {
    b.data().schematic_defaults.get(field)
}

#[test]
fn m36_every_numeric_field_stores_a_typed_value() {
    let mut b = on(ProjectSection::SchematicDefaults);
    for field in Field::ALL {
        let value = *field.range().start() + 0.5;
        type_default(&mut b, field, &value.to_string());
        assert_eq!(stored(&b, field), Some(value), "{field:?}");
    }
    assert!(b.dirty());
}

#[test]
fn m36_undo_takes_a_typed_width_back_and_clears_the_dirty_mark() {
    let mut b = on(ProjectSection::SchematicDefaults);
    type_default(&mut b, Field::WireWidth, "0.3");
    b.undo();
    assert_eq!(stored(&b, Field::WireWidth), None);
    assert!(!b.dirty());
}

#[test]
fn m36_an_empty_field_goes_back_to_the_built_in_value() {
    let mut b = on(ProjectSection::SchematicDefaults);
    type_default(&mut b, Field::WireWidth, "0.3");
    type_default(&mut b, Field::WireWidth, "");
    assert_eq!(stored(&b, Field::WireWidth), None);
}

#[test]
fn m36_a_value_that_is_no_number_changes_nothing_and_stays_typed() {
    let mut b = on(ProjectSection::SchematicDefaults);
    type_default(&mut b, Field::WireWidth, "0.3");
    type_default(&mut b, Field::WireWidth, "abc");
    assert_eq!(stored(&b, Field::WireWidth), Some(0.3));
    assert_eq!(
        b.tab_state()
            .default_drafts
            .get(&Field::WireWidth)
            .map(String::as_str),
        Some("abc")
    );
}

#[test]
fn m36_zero_and_a_negative_width_are_refused() {
    let mut b = on(ProjectSection::SchematicDefaults);
    for bad in ["0", "-1"] {
        type_default(&mut b, Field::BusWidth, bad);
        assert_eq!(stored(&b, Field::BusWidth), None, "{bad}");
    }
    assert!(!b.dirty());
}

#[test]
fn m36_a_value_past_the_range_is_refused_and_the_text_stays() {
    let mut b = on(ProjectSection::SchematicDefaults);
    type_default(&mut b, Field::WireWidth, "99");
    assert_eq!(stored(&b, Field::WireWidth), None, "nothing is stored");
    assert_eq!(
        b.tab_state()
            .default_drafts
            .get(&Field::WireWidth)
            .map(String::as_str),
        Some("99"),
        "the box keeps what was typed"
    );
}

/// Row m36, 2026-11-18 pass: "The out-of-range junction (5 mm) draws at the
/// clamped 2 mm", under the note "Out of range (0.2 to 2): 2 is used".
#[test]
fn m36_a_5_mm_junction_is_refused() {
    let mut b = on(ProjectSection::SchematicDefaults);
    type_default(&mut b, Field::JunctionDiameter, "2");
    type_default(&mut b, Field::JunctionDiameter, "5");
    assert_eq!(stored(&b, Field::JunctionDiameter), Some(2.0));
}

#[test]
fn m36_the_font_is_stored_and_an_empty_font_clears_it() {
    let mut b = on(ProjectSection::SchematicDefaults);
    let project_id = b.id;
    b.tab(ProjectTabMsg::SchematicDefaultFontEdited {
        project_id,
        text: "Inter".into(),
    });
    assert_eq!(
        b.data().schematic_defaults.font_family.as_deref(),
        Some("Inter")
    );
    b.tab(ProjectTabMsg::SchematicDefaultFontEdited {
        project_id,
        text: String::new(),
    });
    assert_eq!(b.data().schematic_defaults.font_family, None);
}

#[test]
fn m36_the_wire_line_style_pick_is_stored_and_undone() {
    let mut b = on(ProjectSection::SchematicDefaults);
    let project_id = b.id;
    b.tab(ProjectTabMsg::SchematicDefaultLineStylePicked {
        project_id,
        line: Some(NetLineStyle::Dashed),
    });
    assert_eq!(
        b.data().schematic_defaults.wire_line_style,
        Some(NetLineStyle::Dashed)
    );
    b.undo();
    assert_eq!(b.data().schematic_defaults.wire_line_style, None);
}

#[test]
fn m36_the_shape_fill_pick_is_stored_and_built_in_clears_it() {
    let mut b = on(ProjectSection::SchematicDefaults);
    let project_id = b.id;
    b.tab(ProjectTabMsg::SchematicDefaultShapeFillPicked {
        project_id,
        fill: Some(FillType::Background),
    });
    assert_eq!(
        b.data().schematic_defaults.shape_fill,
        Some(FillType::Background)
    );
    b.tab(ProjectTabMsg::SchematicDefaultShapeFillPicked {
        project_id,
        fill: None,
    });
    assert_eq!(b.data().schematic_defaults.shape_fill, None);
}

fn setup(b: &mut Bench, msg: NewProjectMsg) {
    let project_id = b.id;
    b.tab(ProjectTabMsg::SetupEdited { project_id, msg });
}

#[test]
fn m37_a_card_pick_stores_its_rules_and_names_the_profile() {
    let mut b = on(ProjectSection::Setup);
    setup(&mut b, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    let defaults = &b.data().project_defaults;
    let pcb = defaults.pcb.as_ref().expect("stored rules");
    let hdi = SetupProfile::Hdi.pcb(pcb.copper_layers);
    assert_eq!(pcb.clearance_mm, hdi.clearance_mm);
    assert!(defaults.profile.is_some());
    assert!(b.dirty());
}

#[test]
fn m37_undo_takes_the_card_pick_back() {
    let mut b = on(ProjectSection::Setup);
    setup(&mut b, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    b.undo();
    assert!(b.data().project_defaults.pcb.is_none());
    assert!(b.data().project_defaults.profile.is_none());
    assert!(!b.dirty());
}

#[test]
fn m37_a_typed_rule_is_stored() {
    let mut b = on(ProjectSection::Setup);
    setup(
        &mut b,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "0.3".into()),
    );
    let pcb = b.data().project_defaults.pcb.as_ref().expect("stored");
    assert_eq!(pcb.clearance_mm, 0.3);
}

#[test]
fn m37_a_drill_not_smaller_than_the_via_is_not_stored() {
    let mut b = on(ProjectSection::Setup);
    // Standard is already the shown card: picking it stores nothing.
    setup(&mut b, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    let good = b.data().project_defaults.pcb.clone().expect("stored");
    setup(
        &mut b,
        NewProjectMsg::RuleEdited(PcbField::ViaDrill, "0.9".into()),
    );
    let pcb = b.data().project_defaults.pcb.as_ref().expect("stored");
    assert_eq!(pcb.via_drill_mm, good.via_drill_mm);
    assert!(
        b.tab_state().setup_draft.is_some(),
        "the box keeps the text"
    );
}

#[test]
fn m37_a_rule_that_is_no_number_is_not_stored() {
    let mut b = on(ProjectSection::Setup);
    setup(
        &mut b,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "wide".into()),
    );
    assert!(b.data().project_defaults.pcb.is_none());
    assert!(!b.dirty());
}

#[test]
fn m37_the_layer_stepper_stores_the_copper_count() {
    let mut b = on(ProjectSection::Setup);
    // Standard is already the shown card: picking it stores nothing.
    setup(&mut b, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    let before = b
        .data()
        .project_defaults
        .pcb
        .as_ref()
        .expect("stored")
        .copper_layers;
    setup(&mut b, NewProjectMsg::CardLayersStepped(SetupProfile::Hdi, 2));
    let pcb = b.data().project_defaults.pcb.as_ref().expect("stored");
    assert_eq!(pcb.copper_layers, before + 2);
}

#[test]
fn m37_the_sheet_fields_are_stored() {
    let mut b = on(ProjectSection::Setup);
    setup(&mut b, NewProjectMsg::SheetSizePicked("A2".into()));
    setup(&mut b, NewProjectMsg::TitleBlockPicked(false));
    let sheet = b.data().project_defaults.sheet.as_ref().expect("stored");
    assert_eq!(sheet.paper_size, "A2");
    assert!(!sheet.show_title_block);
}

#[test]
fn m37_the_designator_pick_is_stored() {
    let mut b = on(ProjectSection::Setup);
    setup(
        &mut b,
        NewProjectMsg::DesignatorsPicked(DesignatorStandard::Iec81346_2),
    );
    assert_eq!(
        b.data().designator_standard(),
        Some(DesignatorStandard::Iec81346_2)
    );
}

#[test]
fn m37_the_agent_journal_box_is_stored_and_undone() {
    let mut b = on(ProjectSection::Setup);
    let before = b.data().agent_journal.keep_in_git;
    let mut settings = b.data().agent_journal;
    settings.keep_in_git = !before;
    let project_id = b.id;
    b.tab(ProjectTabMsg::AgentJournalPicked {
        project_id,
        settings,
    });
    assert_eq!(b.data().agent_journal.keep_in_git, !before);
    b.undo();
    assert_eq!(b.data().agent_journal.keep_in_git, before);
}
