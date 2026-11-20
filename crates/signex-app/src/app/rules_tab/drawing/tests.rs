//! The rule drawings: one label per value with its column, dropped values,
//! and commits from a typed label or a dragged handle.

use iced::Vector;
use signex_drc_dsl::table::{LimitField, RuleKind};
use signex_i18n::I18n;
use signex_widgets::dimension_label::DimensionLabelMessage as L;

use super::clearance::Object;
use super::labels::{Layout, dimension_labels, style};
use super::scene::BoardInk;
use super::{PreviewCommit, PreviewMsg, PreviewTarget, RulePreview, scene_of, update};
use crate::styles::tokens::contrast_ratio;

/// Every column of `kind`, each with its own value.
fn full(kind: RuleKind) -> Vec<(LimitField, f64)> {
    kind.fields()
        .into_iter()
        .enumerate()
        .map(|(i, f)| (f, 0.1 * (i + 1) as f64))
        .collect()
}

fn rule(kind: RuleKind, values: Vec<(LimitField, f64)>) -> PreviewTarget {
    PreviewTarget::Rule {
        kind,
        values,
        caption: String::new(),
    }
}

fn fields_of(target: &PreviewTarget) -> Vec<Option<LimitField>> {
    let scene = scene_of(target, BoardInk::new());
    scene.labels.iter().map(|l| l.field).collect()
}

#[test]
fn every_kind_labels_each_of_its_values_with_its_column() {
    for kind in RuleKind::ALL {
        let values = full(kind);
        let scene = scene_of(&rule(kind, values.clone()), BoardInk::new());
        let mut shown: Vec<(LimitField, f64)> = scene
            .labels
            .iter()
            .map(|l| (l.field.expect("a rule label has a column"), l.value_mm))
            .collect();
        shown.sort_by_key(|(f, _)| *f);
        let mut want = values;
        want.sort_by_key(|(f, _)| *f);
        assert_eq!(shown, want, "{kind:?}");
    }
}

#[test]
fn an_unset_value_has_no_label() {
    for kind in RuleKind::ALL {
        for dropped in kind.fields() {
            let unset: Vec<_> = full(kind)
                .into_iter()
                .filter(|(f, _)| *f != dropped)
                .collect();
            let fields = fields_of(&rule(kind, unset));
            assert!(!fields.contains(&Some(dropped)), "{kind:?} {dropped:?}");
            assert_eq!(fields.len(), kind.fields().len() - 1, "{kind:?}");
        }
    }
}

/// A 0 is a value (a mask expansion of 0, a sliver of 0): it keeps its
/// label at 0, so it can be changed on the drawing.
#[test]
fn a_zero_value_keeps_its_label() {
    for kind in RuleKind::ALL {
        for zeroed in kind.fields() {
            let values: Vec<_> = full(kind)
                .into_iter()
                .map(|(f, v)| (f, if f == zeroed { 0.0 } else { v }))
                .collect();
            let scene = scene_of(&rule(kind, values), BoardInk::new());
            let label = scene
                .labels
                .iter()
                .find(|l| l.field == Some(zeroed))
                .unwrap_or_else(|| panic!("{kind:?} {zeroed:?} keeps its label"));
            assert_eq!(label.value_mm, 0.0, "{kind:?} {zeroed:?}");
            assert_eq!(scene.labels.len(), kind.fields().len(), "{kind:?}");
        }
    }
}

#[test]
fn a_rule_with_no_values_still_draws_but_has_no_labels() {
    for kind in RuleKind::ALL {
        let scene = scene_of(&rule(kind, Vec::new()), BoardInk::new());
        assert!(scene.labels.is_empty(), "{kind:?}");
        assert!(!scene.parts.is_empty(), "{kind:?}");
    }
}

/// Opens label `index`, types `text` and presses Enter.
fn type_into(
    state: &mut RulePreview,
    target: &PreviewTarget,
    index: usize,
    text: &str,
) -> Option<PreviewCommit> {
    let steps = [
        L::LabelPressed(index),
        L::DraftEdited(text.to_owned()),
        L::DraftSubmitted,
    ];
    let mut last = None;
    for step in steps {
        last = update(state, target, PreviewMsg::Labels(step)).0;
    }
    last
}

#[test]
fn a_typed_label_commits_to_its_column() {
    for kind in RuleKind::ALL {
        let target = rule(kind, full(kind));
        for (index, field) in fields_of(&target).into_iter().enumerate() {
            let mut state = RulePreview::default();
            let commit = type_into(&mut state, &target, index, "0.375");
            assert_eq!(
                commit,
                Some(PreviewCommit {
                    field,
                    value_mm: 0.375,
                }),
                "{kind:?} label {index}"
            );
        }
    }
}

#[test]
fn a_dragged_handle_commits_to_its_column() {
    let i18n = I18n::en_us_ref();
    for kind in RuleKind::ALL {
        let target = rule(kind, full(kind));
        let scene = scene_of(&target, BoardInk::new());
        let shown = dimension_labels(&scene, Layout::new(false), i18n);
        for (index, label) in shown.iter().enumerate() {
            let Some(handle) = label.handle else {
                continue;
            };
            let mut state = RulePreview::default();
            let step = Vector::new(handle.axis.x * 20.0, handle.axis.y * 20.0);
            let mut commit = None;
            for message in [
                L::HandlePressed(index),
                L::PointerMoved(handle.at),
                L::PointerMoved(handle.at + step),
            ] {
                commit = update(&mut state, &target, PreviewMsg::Labels(message)).0;
            }
            let commit = commit.expect("a drag along the axis commits");
            assert_eq!(commit.field, scene.labels[index].field, "{kind:?}");
            assert!(
                commit.value_mm > label.value,
                "{kind:?} grows along its axis"
            );
        }
    }
}

#[test]
fn the_width_rule_has_a_handle_on_each_width() {
    let scene = scene_of(
        &rule(RuleKind::Track, full(RuleKind::Track)),
        BoardInk::new(),
    );
    assert!(scene.labels.iter().all(|l| l.handle.is_some()));
}

#[test]
fn pair_gaps_commit_with_no_column() {
    let targets = [
        PreviewTarget::ClassPair {
            a: "HV".into(),
            b: "PWR".into(),
            gap_mm: 0.8,
        },
        PreviewTarget::ObjectPair {
            objects: ["track".into(), "pad".into()],
            gap_mm: 1.0,
        },
    ];
    for target in targets {
        assert_eq!(fields_of(&target), vec![None]);
        let mut state = RulePreview::default();
        let commit = type_into(&mut state, &target, 0, "1.25");
        assert_eq!(
            commit,
            Some(PreviewCommit {
                field: None,
                value_mm: 1.25,
            })
        );
    }
}

#[test]
fn a_pair_at_zero_gap_has_no_label() {
    let target = PreviewTarget::ClassPair {
        a: "A".into(),
        b: "B".into(),
        gap_mm: 0.0,
    };
    assert!(fields_of(&target).is_empty());
}

#[test]
fn object_words_name_the_objects_drawn() {
    assert_eq!(Object::parse("track"), Object::Track);
    assert_eq!(Object::parse("Pad"), Object::Pad);
    assert_eq!(Object::parse("via"), Object::Via);
    assert_eq!(Object::parse("pour"), Object::Pour);
    assert_eq!(Object::parse("something"), Object::Track);
}

#[test]
fn an_untouched_label_commits_nothing() {
    let target = rule(RuleKind::Clearance, vec![(LimitField::MinClearance, 0.2)]);
    let mut state = RulePreview::default();
    let mut last = None;
    for step in [L::LabelPressed(0), L::DraftSubmitted] {
        last = update(&mut state, &target, PreviewMsg::Labels(step)).0;
    }
    assert_eq!(last, None);
}

#[test]
fn expand_toggles_and_grows_the_drawing() {
    let mut state = RulePreview::default();
    let target = rule(RuleKind::Clearance, Vec::new());
    let (commit, _none) = update(&mut state, &target, PreviewMsg::ExpandToggled);
    assert_eq!(commit, None);
    assert!(state.expanded);
    assert!(Layout::new(true).height() > Layout::new(false).height());
    let (_, _none) = update(&mut state, &target, PreviewMsg::ExpandToggled);
    assert!(!state.expanded);
}

/// The drawing is at least the approved 200 px tall at the default scale.
#[test]
fn the_drawing_is_at_least_two_hundred_pixels_tall() {
    assert!(Layout::new(false).height() >= 200.0);
}

/// Labels and notes are drawn on the board ground under both chrome
/// themes, so they must read on #111214 whatever the chrome is.
#[test]
fn labels_and_notes_read_on_the_board_ground() {
    let ink = BoardInk::new();
    let label = style();
    assert!(contrast_ratio(label.ink, ink.ground) >= 4.5);
    assert!(contrast_ratio(label.ink, label.fill) >= 4.5);
    assert!(contrast_ratio(ink.note, ink.ground) >= 4.5);
    assert!(contrast_ratio(ink.title, ink.ground) >= 4.5);
    let [r, g, b, _] = ink.ground.into_rgba8();
    assert_eq!((r, g, b), (0x11, 0x12, 0x14));
}
