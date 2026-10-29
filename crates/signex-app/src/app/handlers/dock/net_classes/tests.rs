use signex_engine::NetClassHistory;
use signex_types::net_class::{NetClass, NetLineStyle};
use signex_types::theme::Color;

use super::*;
use crate::app::ProjectId;

const PROJECT: ProjectId = ProjectId::test_id(1);

struct Fixture {
    classes: Vec<NetClass>,
    history: NetClassHistory,
    panel: NetClassPanel,
}

impl Fixture {
    fn new() -> Self {
        Self {
            classes: Vec::new(),
            history: NetClassHistory::default(),
            panel: NetClassPanel::default(),
        }
    }

    fn send(&mut self, msg: NetClassMsg) -> bool {
        step(&mut self.classes, &mut self.history, &mut self.panel, &msg)
    }

    fn type_and_commit(&mut self, field: NetClassField, text: &str) -> bool {
        self.send(NetClassMsg::Draft(field, text.to_string()));
        self.send(NetClassMsg::Commit(field))
    }
}

#[test]
fn add_names_the_class_and_selects_it() {
    let mut f = Fixture::new();
    assert!(f.send(NetClassMsg::Add));
    assert!(f.send(NetClassMsg::Add));
    assert_eq!(f.classes[0].name, "Class 1");
    assert_eq!(f.classes[1].name, "Class 2");
    assert_eq!(f.panel.selected, 1);
    assert_eq!(f.panel.draft.name, "Class 2");
}

#[test]
fn committing_fields_edits_the_selected_class() {
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    assert!(f.type_and_commit(NetClassField::Name, "Power"));
    assert!(f.type_and_commit(NetClassField::Color, "#FF0000"));
    assert!(f.type_and_commit(NetClassField::Width, "0.4"));
    assert!(f.type_and_commit(NetClassField::Nets, "GND, +3V3"));
    assert!(f.type_and_commit(NetClassField::Patterns, "VCC*"));
    assert!(f.send(NetClassMsg::SetLine(NetLineStyle::Dashed)));

    let class = &f.classes[0];
    assert_eq!(class.name, "Power");
    assert_eq!(class.style.color, Some(Color::new(255, 0, 0, 255)));
    assert_eq!(class.style.wire_width_mm, Some(0.4));
    assert_eq!(class.style.line, NetLineStyle::Dashed);
    assert_eq!(class.nets, ["GND", "+3V3"]);
    assert_eq!(class.patterns, ["VCC*"]);
}

#[test]
fn typing_alone_changes_nothing_until_commit() {
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    assert!(!f.send(NetClassMsg::Draft(NetClassField::Name, "Pow".into())));
    assert_eq!(f.classes[0].name, "Class 1");
    assert_eq!(f.panel.draft.name, "Pow");
}

#[test]
fn a_bad_value_is_refused_with_a_notice_and_changes_nothing() {
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    let before = f.classes.clone();
    assert!(!f.type_and_commit(NetClassField::Color, "red"));
    assert!(f.panel.notice.is_some());
    assert!(!f.type_and_commit(NetClassField::Width, "wide"));
    assert!(!f.type_and_commit(NetClassField::Width, "-1"));
    assert!(f.panel.notice.is_some());
    assert_eq!(f.classes, before);

    f.send(NetClassMsg::Add);
    assert!(!f.type_and_commit(NetClassField::Name, "Class 1"));
    assert!(f.panel.notice.is_some());
}

#[test]
fn emptying_colour_and_width_clears_them() {
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    f.type_and_commit(NetClassField::Color, "#00FF00");
    f.type_and_commit(NetClassField::Width, "0.3");
    assert!(f.type_and_commit(NetClassField::Color, ""));
    assert!(f.type_and_commit(NetClassField::Width, " "));
    assert!(f.classes[0].style.is_default());
}

#[test]
fn undo_and_redo_restore_the_table_and_the_drafts() {
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    f.type_and_commit(NetClassField::Name, "Power");
    assert!(f.send(NetClassMsg::Undo));
    assert_eq!(f.classes[0].name, "Class 1");
    assert_eq!(f.panel.draft.name, "Class 1");
    assert!(f.send(NetClassMsg::Redo));
    assert_eq!(f.classes[0].name, "Power");
    assert!(f.send(NetClassMsg::Undo));
    assert!(f.send(NetClassMsg::Undo));
    assert!(f.classes.is_empty());
    assert!(!f.send(NetClassMsg::Undo));
}

#[test]
fn move_and_remove_keep_the_selection_on_a_valid_class() {
    let mut f = Fixture::new();
    for _ in 0..3 {
        f.send(NetClassMsg::Add);
    }
    assert!(f.send(NetClassMsg::MoveUp));
    assert_eq!(f.panel.selected, 1);
    assert_eq!(f.classes[1].name, "Class 3");
    assert!(f.send(NetClassMsg::RemoveSelected));
    assert_eq!(f.classes.len(), 2);
    assert!(f.panel.selected < f.classes.len());
    assert!(f.send(NetClassMsg::RemoveSelected));
    assert!(f.send(NetClassMsg::RemoveSelected));
    assert!(!f.send(NetClassMsg::RemoveSelected));
    assert_eq!(f.panel.draft, NetClassDraft::default());
}

#[test]
fn assigning_a_net_from_a_label_puts_it_in_one_class_and_undoes() {
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    f.send(NetClassMsg::Add);

    assert!(f.send(NetClassMsg::AssignNet {
        project: PROJECT,
        net: "VCC".into(),
        class: Some("Class 1".into()),
    }));
    assert_eq!(f.classes[0].nets, ["VCC"]);

    assert!(f.send(NetClassMsg::AssignNet {
        project: PROJECT,
        net: "VCC".into(),
        class: Some("Class 2".into()),
    }));
    assert!(f.classes[0].nets.is_empty());
    assert_eq!(f.classes[1].nets, ["VCC"]);

    assert!(f.send(NetClassMsg::Undo));
    assert_eq!(f.classes[0].nets, ["VCC"]);
}

#[test]
fn assigning_to_none_clears_the_membership() {
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    f.send(NetClassMsg::AssignNet {
        project: PROJECT,
        net: "VCC".into(),
        class: Some("Class 1".into()),
    });

    assert!(f.send(NetClassMsg::AssignNet {
        project: PROJECT,
        net: "VCC".into(),
        class: None,
    }));

    assert!(f.classes[0].nets.is_empty());
}

#[test]
fn an_earlier_pattern_that_still_matches_is_reported() {
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    f.type_and_commit(NetClassField::Patterns, "VCC*");
    f.send(NetClassMsg::Add);

    f.send(NetClassMsg::AssignNet {
        project: PROJECT,
        net: "VCC_3V3".into(),
        class: Some("Class 2".into()),
    });

    assert!(
        f.panel
            .notice
            .as_deref()
            .is_some_and(|n| n.contains("Class 1"))
    );
}

#[test]
fn choosing_none_for_a_net_a_pattern_still_matches_is_reported() {
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    f.type_and_commit(NetClassField::Patterns, "VCC*");

    f.send(NetClassMsg::AssignNet {
        project: PROJECT,
        net: "VCC_3V3".into(),
        class: None,
    });

    assert!(
        f.panel
            .notice
            .as_deref()
            .is_some_and(|n| n.contains("Class 1") && n.contains("pattern"))
    );
}

#[test]
fn choosing_none_for_an_unmatched_net_says_nothing() {
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    f.send(NetClassMsg::AssignNet {
        project: PROJECT,
        net: "VCC".into(),
        class: Some("Class 1".into()),
    });

    f.send(NetClassMsg::AssignNet {
        project: PROJECT,
        net: "VCC".into(),
        class: None,
    });

    assert!(f.panel.notice.is_none());
}

#[test]
fn choices_list_none_then_the_classes_and_show_the_first_match() {
    use crate::panels::{NetClassChoice, net_class_choice, net_class_choices};
    let mut f = Fixture::new();
    f.send(NetClassMsg::Add);
    f.send(NetClassMsg::AssignNet {
        project: PROJECT,
        net: "GND".into(),
        class: Some("Class 1".into()),
    });

    assert_eq!(
        net_class_choices(&f.classes),
        [
            NetClassChoice::NoClass,
            NetClassChoice::Class("Class 1".into())
        ]
    );
    assert_eq!(
        net_class_choice(&f.classes, "GND"),
        NetClassChoice::Class("Class 1".into())
    );
    assert_eq!(net_class_choice(&f.classes, "SDA"), NetClassChoice::NoClass);
}

#[test]
fn a_loaded_class_named_none_is_still_a_class_when_picked() {
    use crate::panels::{NetClassChoice, net_class_choice, net_class_choices};
    // The engine refuses to Add or Rename a class to `(none)`, but a `.snxprj`
    // written by hand or by an older build can carry one.
    let dir = std::env::temp_dir().join(format!("signex-none-class-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let path = dir.join("p.snxprj");
    std::fs::write(
        &path,
        r#"{"name":"p","dir":"","schematic_root":null,"pcb_file":null,"net_classes":[{"name":"(none)","nets":["GND"]}]}"#,
    )
    .expect("write project");
    let data = signex_types::project::parse_project(&path).expect("parse project");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(data.net_classes.len(), 1, "the class loaded");

    let shown = net_class_choice(&data.net_classes, "GND");
    assert_eq!(shown, NetClassChoice::Class("(none)".into()));
    assert_eq!(
        shown.into_class(),
        Some("(none)".to_string()),
        "picking it assigns that class instead of clearing the net"
    );
    let choices = net_class_choices(&data.net_classes);
    assert_eq!(choices.len(), 2);
    assert_ne!(choices[0], choices[1], "the no-class entry is distinct");
    assert_eq!(NetClassChoice::NoClass.into_class(), None);
}
