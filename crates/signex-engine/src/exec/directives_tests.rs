//! #2420: placing, editing, moving, turning and deleting a directive, each
//! one undo step.

use crate::test_support::test_sheet;
use crate::*;
use signex_types::schematic::{Point, SchDirective, SelectedItem, SelectedKind};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn blanket(class: Option<&str>) -> SchDirective {
    SchDirective::Blanket {
        uuid: id(1),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 5.0),
        net_class: class.map(str::to_string),
        erc_exclusions: Vec::new(),
        track_width: None,
        pour: signex_types::schematic::BlanketPour::Normal,
        include_edge_nets: false,
    }
}

fn placed(directive: SchDirective) -> Engine {
    let mut engine = Engine::new(test_sheet()).unwrap();
    engine
        .execute(Command::PlaceDirective { directive })
        .unwrap();
    engine
}

fn item() -> SelectedItem {
    SelectedItem::new(id(1), SelectedKind::Directive)
}

#[test]
fn placing_a_directive_is_one_undo_step() {
    let mut engine = placed(blanket(None));
    assert_eq!(engine.document().directives, [blanket(None)]);
    assert!(engine.can_undo());
    engine.undo().unwrap();
    assert!(engine.document().directives.is_empty());
    assert!(!engine.can_undo());
}

#[test]
fn a_region_without_area_or_a_non_finite_point_is_refused() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    let flat = SchDirective::CompileMask {
        uuid: id(2),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
    };
    assert!(matches!(
        engine.execute(Command::PlaceDirective { directive: flat }),
        Err(EngineError::InvalidDirectiveRegion)
    ));
    let lost = SchDirective::DiffPair {
        uuid: id(3),
        position: Point::new(f64::NAN, 0.0),
        class: None,
    };
    assert!(
        engine
            .execute(Command::PlaceDirective { directive: lost })
            .is_err()
    );
    assert!(engine.document().directives.is_empty());
}

#[test]
fn an_update_replaces_the_directive_and_typing_into_it_is_one_step() {
    let mut engine = placed(blanket(None));
    for class in ["H", "Hi", "High"] {
        let done = engine
            .execute(Command::UpdateDirective {
                directive: blanket(Some(class)),
            })
            .unwrap();
        assert!(done.changed);
    }
    assert_eq!(engine.document().directives, [blanket(Some("High"))]);
    // One step back undoes the whole run, a second the placement.
    engine.undo().unwrap();
    assert_eq!(engine.document().directives, [blanket(None)]);
    engine.undo().unwrap();
    assert!(engine.document().directives.is_empty());
}

#[test]
fn an_ended_run_makes_the_next_update_a_step_of_its_own() {
    let mut engine = placed(blanket(None));
    engine
        .execute(Command::UpdateDirective {
            directive: blanket(Some("A")),
        })
        .unwrap();
    engine.end_field_edit_run();
    engine
        .execute(Command::UpdateDirective {
            directive: blanket(Some("B")),
        })
        .unwrap();
    engine.undo().unwrap();
    assert_eq!(engine.document().directives, [blanket(Some("A"))]);
}

#[test]
fn an_update_of_a_missing_directive_or_to_another_kind_is_refused() {
    let mut engine = placed(blanket(None));
    let other_kind = SchDirective::CompileMask {
        uuid: id(1),
        start: Point::new(0.0, 0.0),
        end: Point::new(1.0, 1.0),
    };
    assert!(matches!(
        engine.execute(Command::UpdateDirective {
            directive: other_kind
        }),
        Err(EngineError::DirectiveNotFound { .. })
    ));
    let same = engine
        .execute(Command::UpdateDirective {
            directive: blanket(None),
        })
        .unwrap();
    assert!(!same.changed);
}

#[test]
fn a_directive_moves_turns_and_deletes_with_the_selection() {
    let mut engine = placed(blanket(None));
    engine
        .execute(Command::MoveSelection {
            items: vec![item()],
            dx: 2.0,
            dy: 3.0,
            keep_wires: false,
            wiring: signex_types::corner::DragWiringMode::Auto,
        })
        .unwrap();
    let moved = engine.document().directives[0].region().unwrap();
    assert_eq!((moved.min_x, moved.min_y), (2.0, 3.0));
    engine
        .execute(Command::TransformSelection {
            items: vec![item()],
            centre: Point::new(7.0, 5.5),
            transform: SelectionTransform::RotateClockwise,
            keep_wires: false,
        })
        .unwrap();
    let turned = engine.document().directives[0].region().unwrap();
    assert_eq!(
        (turned.width(), turned.height()),
        (5.0, 10.0),
        "a quarter turn swaps the sides"
    );
    engine
        .execute(Command::DeleteSelection {
            items: vec![item()],
        })
        .unwrap();
    assert!(engine.document().directives.is_empty());
    engine.undo().unwrap();
    assert_eq!(engine.document().directives.len(), 1);
}

#[test]
fn a_placed_directive_moves_the_connectivity_revision() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    let before = engine.connectivity_revision();
    engine
        .execute(Command::PlaceDirective {
            directive: blanket(None),
        })
        .unwrap();
    assert_ne!(engine.connectivity_revision(), before);
}

#[test]
fn the_properties_rows_name_the_directive_and_its_parameters() {
    let engine = placed(SchDirective::Blanket {
        uuid: id(1),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 5.0),
        net_class: Some("HighSpeed".into()),
        erc_exclusions: vec!["builtin::unused_pin".into(), "*".into()],
        track_width: None,
        pour: signex_types::schematic::BlanketPour::Normal,
        include_edge_nets: false,
    });
    let details = engine
        .describe_single_selection(&[item()])
        .expect("details");
    let row = |key: &str| {
        details
            .info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(row("Type").as_deref(), Some("Blanket"));
    assert_eq!(row("Net Class").as_deref(), Some("HighSpeed"));
    assert_eq!(
        row("ERC Exclusions").as_deref(),
        Some("builtin::unused_pin, *")
    );
}

fn net_parameters(width: Option<f64>) -> SchDirective {
    SchDirective::NetParameters {
        uuid: id(1),
        position: Point::new(5.0, 0.0),
        net_class: Some("POWER_IN".into()),
        diff_pair_class: None,
        rules: signex_types::schematic::NetRules {
            width_mm: width,
            ..Default::default()
        },
        show_on_flag: false,
    }
}

#[test]
fn a_net_parameters_directive_places_as_one_step_and_its_edits_are_a_typing_run() {
    let mut engine = placed(net_parameters(None));
    assert_eq!(engine.document().directives, [net_parameters(None)]);
    for width in [0.8, 0.85, 0.9] {
        let done = engine
            .execute(Command::UpdateDirective {
                directive: net_parameters(Some(width)),
            })
            .unwrap();
        assert!(done.changed);
        assert_eq!(
            done.patch_pair.map(|p| p.document),
            Some(DocumentPatch::DIRECTIVES)
        );
    }
    assert_eq!(engine.document().directives, [net_parameters(Some(0.9))]);
    // The typing run is one step; the placement another.
    engine.undo().unwrap();
    assert_eq!(engine.document().directives, [net_parameters(None)]);
    engine.undo().unwrap();
    assert!(engine.document().directives.is_empty());
}

#[test]
fn a_net_parameters_directive_with_a_non_finite_rule_is_refused() {
    let mut engine = placed(net_parameters(None));
    assert!(matches!(
        engine.execute(Command::UpdateDirective {
            directive: net_parameters(Some(f64::INFINITY)),
        }),
        Err(EngineError::InvalidDirectiveRegion)
    ));
    assert_eq!(engine.document().directives, [net_parameters(None)]);
}

#[test]
fn the_properties_rows_of_net_parameters_name_its_net_and_rules() {
    let mut sheet = test_sheet();
    sheet.wires.push(signex_types::schematic::Wire {
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet.labels.push(signex_types::schematic::Label::new(
        "VBUS",
        Point::new(0.0, 0.0),
        signex_types::schematic::LabelType::Net,
    ));
    let mut engine = Engine::new(sheet).unwrap();
    engine
        .execute(Command::PlaceDirective {
            directive: net_parameters(Some(0.8)),
        })
        .unwrap();
    let details = engine
        .describe_single_selection(&[item()])
        .expect("details");
    let row = |key: &str| {
        details
            .info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(row("Type").as_deref(), Some("Net Parameters"));
    assert_eq!(row("Net Class").as_deref(), Some("POWER_IN"));
    assert_eq!(row("Width").as_deref(), Some("0.80"));
    assert_eq!(row("Clearance").as_deref(), Some(""));
    assert_eq!(row("Show On Flag").as_deref(), Some("false"));
}
