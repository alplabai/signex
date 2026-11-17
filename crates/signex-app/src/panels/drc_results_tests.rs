//! #2399: the DRC results panel's grouping, filter and row actions.

use signex_drc::ViolationKind;
use signex_types::pcb_layer::CopperLayer;
use signex_types::violation::Severity;

use super::*;
use crate::layout_probe::{lay_out_texts, messages_on_click};

fn row(index: usize, kind: ViolationKind, rule: &str, waiver: Option<&str>) -> DrcRow {
    DrcRow {
        index,
        kind,
        a: "GND".into(),
        b: "VCC".into(),
        measured_mm: "0.100".into(),
        required_mm: "0.150".into(),
        over_maximum: false,
        x_mm: "1.000".into(),
        y_mm: "2.000".into(),
        layer: Some(CopperLayer::Top),
        layers: vec![CopperLayer::Top],
        key: format!("k{index}"),
        rule: rule.into(),
        severity: Some(Severity::Error),
        objects: 2,
        waiver: waiver.map(str::to_string),
    }
}

fn context(rows: Vec<DrcRow>, filter: DrcFilter) -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.pcb_drc = Some(DrcPanel {
        ran: true,
        rows,
        filter,
        ..DrcPanel::default()
    });
    ctx
}

fn texts(ctx: &PanelContext) -> Vec<String> {
    lay_out_texts(view(ctx), iced::Size::new(1400.0, 1200.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn rows_group_by_rule_in_report_order() {
    let rows = [
        row(0, ViolationKind::Short, "builtin::short", None),
        row(1, ViolationKind::Clearance, "user::hv", None),
        row(2, ViolationKind::Short, "builtin::short", None),
    ];
    let refs: Vec<&DrcRow> = rows.iter().collect();
    let grouped = groups(&refs);
    let shape: Vec<(&str, Vec<usize>)> = grouped
        .iter()
        .map(|(rule, members)| (*rule, members.iter().map(|r| r.index).collect()))
        .collect();
    assert_eq!(
        shape,
        [("builtin::short", vec![0, 2]), ("user::hv", vec![1])]
    );
}

#[test]
fn the_filter_takes_rule_layer_severity_and_waived() {
    let plain = row(0, ViolationKind::Short, "builtin::short", None);
    let waived = row(1, ViolationKind::Short, "builtin::short", Some("ok"));
    let all = DrcFilter::default();
    assert!(all.shows(&plain) && !all.shows(&waived));
    let with_waived = DrcFilter {
        show_waived: true,
        ..DrcFilter::default()
    };
    assert!(with_waived.shows(&waived));
    let other_rule = DrcFilter {
        rule: Some("builtin::clearance".into()),
        ..DrcFilter::default()
    };
    assert!(!other_rule.shows(&plain));
    let bottom = DrcFilter {
        layer: Some(CopperLayer::Bottom),
        ..DrcFilter::default()
    };
    assert!(!bottom.shows(&plain));
    let warnings = DrcFilter {
        severity: Some(Severity::Warning),
        ..DrcFilter::default()
    };
    assert!(!warnings.shows(&plain));
}

/// Each group heads its rows with the rule and its count; a written rule is
/// named by its name.
#[test]
fn groups_are_headed_by_the_rule_and_its_count() {
    let ctx = context(
        vec![
            row(0, ViolationKind::Short, "builtin::short", None),
            row(1, ViolationKind::Short, "builtin::short", None),
            row(2, ViolationKind::Clearance, "user::hv_gap", None),
        ],
        DrcFilter::default(),
    );
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t == "Short (2)"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "hv_gap (1)"), "{shown:?}");
}

/// A waived row is hidden and counted, and the filters can hide every row.
#[test]
fn a_waived_row_is_counted_and_hidden_until_shown() {
    let ctx = context(
        vec![row(
            0,
            ViolationKind::Short,
            "builtin::short",
            Some("on purpose"),
        )],
        DrcFilter::default(),
    );
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t == "1 violation waived"), "{shown:?}");
    assert!(
        shown
            .iter()
            .any(|t| t == "No violations match the filters."),
        "{shown:?}"
    );
    let ctx = context(
        vec![row(
            0,
            ViolationKind::Short,
            "builtin::short",
            Some("on purpose"),
        )],
        DrcFilter {
            show_waived: true,
            ..DrcFilter::default()
        },
    );
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t == "Waived: on purpose"), "{shown:?}");
}

/// A row's Waive opens its note; Unwaive on a waived row takes it back.
#[test]
fn waive_and_unwaive_are_row_buttons() {
    let window = iced::Size::new(1400.0, 1200.0);
    let ctx = context(
        vec![row(0, ViolationKind::Short, "builtin::short", None)],
        DrcFilter::default(),
    );
    let pressed = messages_on_click(view(&ctx), window, "Waive");
    assert!(
        matches!(pressed.as_slice(), [PanelMsg::Drc(DrcPanelMsg::WaiveStarted(k))] if k == "k0"),
        "{pressed:?}"
    );
    let ctx = context(
        vec![row(0, ViolationKind::Short, "builtin::short", Some(""))],
        DrcFilter {
            show_waived: true,
            ..DrcFilter::default()
        },
    );
    let pressed = messages_on_click(view(&ctx), window, "Unwaive");
    assert!(
        matches!(pressed.as_slice(), [PanelMsg::Drc(DrcPanelMsg::Unwaived(k))] if k == "k0"),
        "{pressed:?}"
    );
}
