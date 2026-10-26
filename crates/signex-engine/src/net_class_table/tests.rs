use signex_types::net_class::{NetClass, NetClassStyle, NetLineStyle};
use signex_types::theme::Color;

use super::*;

fn add(name: &str) -> NetClassCommand {
    NetClassCommand::Add {
        class: NetClass::named(name),
    }
}

fn names(classes: &[NetClass]) -> Vec<&str> {
    classes.iter().map(|c| c.name.as_str()).collect()
}

#[test]
fn add_appends_and_undo_redo_round_trip() {
    let mut classes = Vec::new();
    let mut history = NetClassHistory::default();
    history.apply(&mut classes, add("Power")).unwrap();
    history.apply(&mut classes, add("Clock")).unwrap();
    assert_eq!(names(&classes), ["Power", "Clock"]);

    assert!(history.undo(&mut classes));
    assert_eq!(names(&classes), ["Power"]);
    assert!(history.redo(&mut classes));
    assert_eq!(names(&classes), ["Power", "Clock"]);
    assert!(!history.redo(&mut classes));
}

#[test]
fn a_new_edit_clears_the_redo_stack() {
    let mut classes = Vec::new();
    let mut history = NetClassHistory::default();
    history.apply(&mut classes, add("A")).unwrap();
    assert!(history.undo(&mut classes));
    history.apply(&mut classes, add("B")).unwrap();
    assert!(!history.can_redo());
}

#[test]
fn empty_and_duplicate_names_are_refused_and_leave_no_history() {
    let mut classes = Vec::new();
    let mut history = NetClassHistory::default();
    history.apply(&mut classes, add("Power")).unwrap();
    assert_eq!(
        history.apply(&mut classes, add("  ")),
        Err(NetClassError::EmptyName)
    );
    assert_eq!(
        history.apply(&mut classes, add("Power")),
        Err(NetClassError::DuplicateName("Power".into()))
    );
    assert_eq!(classes.len(), 1);
    assert!(history.undo(&mut classes));
    assert!(!history.can_undo());
}

#[test]
fn the_no_class_picker_entry_cannot_be_a_class_name() {
    let mut classes = Vec::new();
    let mut history = NetClassHistory::default();
    history.apply(&mut classes, add("Power")).unwrap();
    assert_eq!(
        history.apply(&mut classes, add("(none)")),
        Err(NetClassError::ReservedName("(none)".into()))
    );
    assert_eq!(
        history.apply(
            &mut classes,
            NetClassCommand::Rename {
                from: "Power".into(),
                to: "(none)".into()
            }
        ),
        Err(NetClassError::ReservedName("(none)".into()))
    );
    assert_eq!(names(&classes), ["Power"]);
}

#[test]
fn rename_keeps_position_and_members() {
    let mut classes = Vec::new();
    let mut history = NetClassHistory::default();
    history.apply(&mut classes, add("A")).unwrap();
    history.apply(&mut classes, add("B")).unwrap();
    history
        .apply(
            &mut classes,
            NetClassCommand::SetMembers {
                name: "A".into(),
                nets: vec!["GND".into(), " GND ".into(), "".into()],
                patterns: vec!["VCC*".into()],
            },
        )
        .unwrap();
    history
        .apply(
            &mut classes,
            NetClassCommand::Rename {
                from: "A".into(),
                to: "Rail".into(),
            },
        )
        .unwrap();
    assert_eq!(names(&classes), ["Rail", "B"]);
    assert_eq!(classes[0].nets, ["GND"]);
    assert_eq!(classes[0].patterns, ["VCC*"]);
}

#[test]
fn set_style_validates_the_width() {
    let mut classes = Vec::new();
    let mut history = NetClassHistory::default();
    history.apply(&mut classes, add("Power")).unwrap();
    let style = NetClassStyle {
        color: Some(Color::new(255, 0, 0, 255)),
        wire_width_mm: Some(0.4),
        line: NetLineStyle::Dashed,
    };
    history
        .apply(
            &mut classes,
            NetClassCommand::SetStyle {
                name: "Power".into(),
                style,
            },
        )
        .unwrap();
    assert_eq!(classes[0].style, style);

    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let result = history.apply(
            &mut classes,
            NetClassCommand::SetStyle {
                name: "Power".into(),
                style: NetClassStyle {
                    wire_width_mm: Some(bad),
                    ..style
                },
            },
        );
        assert_eq!(result, Err(NetClassError::InvalidWidth), "{bad}");
    }
    assert_eq!(classes[0].style, style);
}

#[test]
fn move_reorders_precedence_and_clamps() {
    let mut classes = Vec::new();
    let mut history = NetClassHistory::default();
    for n in ["A", "B", "C"] {
        history.apply(&mut classes, add(n)).unwrap();
    }
    history
        .apply(
            &mut classes,
            NetClassCommand::Move {
                name: "C".into(),
                to_index: 0,
            },
        )
        .unwrap();
    assert_eq!(names(&classes), ["C", "A", "B"]);
    history
        .apply(
            &mut classes,
            NetClassCommand::Move {
                name: "C".into(),
                to_index: 99,
            },
        )
        .unwrap();
    assert_eq!(names(&classes), ["A", "B", "C"]);
}

#[test]
fn unknown_class_is_an_error_and_remove_is_undoable() {
    let mut classes = Vec::new();
    let mut history = NetClassHistory::default();
    assert_eq!(
        history.apply(&mut classes, NetClassCommand::Remove { name: "X".into() }),
        Err(NetClassError::Unknown("X".into()))
    );
    history.apply(&mut classes, add("A")).unwrap();
    history
        .apply(&mut classes, NetClassCommand::Remove { name: "A".into() })
        .unwrap();
    assert!(classes.is_empty());
    assert!(history.undo(&mut classes));
    assert_eq!(names(&classes), ["A"]);
}

#[test]
fn a_no_op_command_adds_no_history_step() {
    let mut classes = Vec::new();
    let mut history = NetClassHistory::default();
    history.apply(&mut classes, add("A")).unwrap();
    history
        .apply(
            &mut classes,
            NetClassCommand::Rename {
                from: "A".into(),
                to: "A".into(),
            },
        )
        .unwrap();
    assert!(history.undo(&mut classes));
    assert!(classes.is_empty());
    assert!(!history.can_undo());
}

#[test]
fn commands_round_trip_through_json() {
    let command = NetClassCommand::SetStyle {
        name: "Power".into(),
        style: NetClassStyle {
            color: Some(Color::new(0x12, 0x34, 0x56, 255)),
            wire_width_mm: Some(0.5),
            line: NetLineStyle::Dotted,
        },
    };
    let json = serde_json::to_string(&command).unwrap();
    assert!(json.contains("\"command\":\"set_style\""), "{json}");
    assert!(json.contains("#123456"), "{json}");
    assert_eq!(
        serde_json::from_str::<NetClassCommand>(&json).unwrap(),
        command
    );
}

#[test]
fn undo_depth_is_bounded() {
    let mut classes = Vec::new();
    let mut history = NetClassHistory::default();
    for i in 0..(HISTORY_LIMIT + 20) {
        history.apply(&mut classes, add(&format!("C{i}"))).unwrap();
    }
    let mut steps = 0;
    while history.undo(&mut classes) {
        steps += 1;
    }
    assert_eq!(steps, HISTORY_LIMIT);
}

fn with_nets(name: &str, nets: &[&str]) -> NetClass {
    let mut class = NetClass::named(name);
    class.nets = nets.iter().map(|n| n.to_string()).collect();
    class
}

#[test]
fn assign_net_moves_a_net_between_classes_in_one_undo_step() {
    let mut classes = vec![with_nets("Power", &["VCC"]), with_nets("Clock", &[])];
    let mut history = NetClassHistory::default();

    history
        .apply(
            &mut classes,
            NetClassCommand::AssignNet {
                net: "VCC".into(),
                class: Some("Clock".into()),
            },
        )
        .unwrap();

    assert!(classes[0].nets.is_empty());
    assert_eq!(classes[1].nets, ["VCC"]);
    assert!(history.undo(&mut classes));
    assert_eq!(classes[0].nets, ["VCC"]);
    assert!(classes[1].nets.is_empty());
    assert!(!history.can_undo(), "the move was a single step");
}

#[test]
fn assign_net_to_none_removes_it_from_every_class() {
    let mut classes = vec![with_nets("A", &["X"]), with_nets("B", &["X"])];
    let mut history = NetClassHistory::default();

    history
        .apply(
            &mut classes,
            NetClassCommand::AssignNet {
                net: " X ".into(),
                class: None,
            },
        )
        .unwrap();

    assert!(classes.iter().all(|c| c.nets.is_empty()));
}

#[test]
fn assign_net_to_an_unknown_class_is_refused_and_changes_nothing() {
    let mut classes = vec![with_nets("A", &["X"])];
    let before = classes.clone();
    let mut history = NetClassHistory::default();

    let result = history.apply(
        &mut classes,
        NetClassCommand::AssignNet {
            net: "X".into(),
            class: Some("Nope".into()),
        },
    );

    assert_eq!(result, Err(NetClassError::Unknown("Nope".into())));
    assert_eq!(classes, before);
    assert!(!history.can_undo());
}

#[test]
fn assigning_a_net_already_in_the_class_is_a_no_op() {
    let mut classes = vec![with_nets("A", &["X", "Y"])];
    let mut history = NetClassHistory::default();

    history
        .apply(
            &mut classes,
            NetClassCommand::AssignNet {
                net: "X".into(),
                class: Some("A".into()),
            },
        )
        .unwrap();

    assert!(!history.can_undo());
}

#[test]
fn assign_net_round_trips_through_json() {
    let command = NetClassCommand::AssignNet {
        net: "VCC".into(),
        class: Some("Power".into()),
    };
    let json = serde_json::to_string(&command).unwrap();
    assert_eq!(
        serde_json::from_str::<NetClassCommand>(&json).unwrap(),
        command
    );
}
