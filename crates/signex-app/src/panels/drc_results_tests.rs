//! #2399: the DRC results panel's grouping, filter and row actions.

use signex_drc::ViolationKind;
use signex_types::pcb_layer::CopperLayer;
use signex_types::violation::Severity;

use super::*;
use crate::layout_probe::{lay_out_inputs, lay_out_texts, messages_on_click};

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
    assert!(
        shown.iter().any(|t| t == "(1 violation waived)"),
        "{shown:?}"
    );
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

fn hover(ctx: &mut PanelContext, key: &str) {
    if let Some(drc) = ctx.pcb_drc.as_mut() {
        drc.view.hovered = Some(key.to_string());
    }
}

/// A row's Waive opens the waiver form; Unwaive on a waived row takes it
/// back. Both show on hover (or selection) only (#88).
#[test]
fn waive_and_unwaive_are_row_actions_on_hover() {
    let window = iced::Size::new(1400.0, 1200.0);
    let mut ctx = context(
        vec![row(0, ViolationKind::Short, "builtin::short", None)],
        DrcFilter::default(),
    );
    assert!(!texts(&ctx).iter().any(|t| t == "Waive" || t == "Show"));
    hover(&mut ctx, "k0");
    // The probe's fresh tree sees the pointer enter the row first; the
    // press itself sends only the action, not the row's own press.
    let pressed = messages_on_click(view(&ctx), window, "Waive");
    assert!(
        matches!(
            pressed.as_slice(),
            [
                PanelMsg::Drc(DrcPanelMsg::RowEntered(e)),
                PanelMsg::Drc(DrcPanelMsg::WaiveStarted(k)),
            ] if e == "k0" && k == "k0"
        ),
        "{pressed:?}"
    );
    let pressed = messages_on_click(view(&ctx), window, "Show");
    assert!(
        matches!(
            pressed.as_slice(),
            [
                PanelMsg::Drc(DrcPanelMsg::RowEntered(e)),
                PanelMsg::Drc(DrcPanelMsg::RowPicked(k)),
            ] if e == "k0" && k == "k0"
        ),
        "{pressed:?}"
    );
    let mut ctx = context(
        vec![row(0, ViolationKind::Short, "builtin::short", Some(""))],
        DrcFilter {
            show_waived: true,
            ..DrcFilter::default()
        },
    );
    hover(&mut ctx, "k0");
    let pressed = messages_on_click(view(&ctx), window, "Unwaive");
    assert!(
        matches!(
            pressed.as_slice(),
            [
                PanelMsg::Drc(DrcPanelMsg::RowEntered(e)),
                PanelMsg::Drc(DrcPanelMsg::Unwaived(k)),
            ] if e == "k0" && k == "k0"
        ),
        "{pressed:?}"
    );
}

/// #88: a tile lists only its category, and a group with no row left in it
/// is hidden, not shown as "(0)". No tile is on at first.
#[test]
fn a_tile_lists_its_category_and_hides_the_empty_groups() {
    let rows = vec![
        row(0, ViolationKind::Clearance, "user::HV_08", None),
        row(1, ViolationKind::Short, "builtin::short", None),
        row(2, ViolationKind::Clearance, "user::HV_08", None),
    ];
    let mut ctx = context(rows, DrcFilter::default());
    if let Some(drc) = ctx.pcb_drc.as_mut() {
        drc.clearance = 2;
        drc.short = 1;
    }
    let all = texts(&ctx);
    assert!(all.iter().any(|t| t == "HV_08 (2)"), "{all:?}");
    assert!(all.iter().any(|t| t == "Short (1)"), "{all:?}");
    assert!(
        all.iter()
            .any(|t| t == "Click a tile to list only that category."),
        "{all:?}"
    );
    let window = iced::Size::new(1400.0, 1200.0);
    let pressed = messages_on_click(view(&ctx), window, "short");
    assert!(
        matches!(
            pressed.as_slice(),
            [PanelMsg::Drc(DrcPanelMsg::TileToggled(DrcCategory::Short))]
        ),
        "{pressed:?}"
    );
    if let Some(drc) = ctx.pcb_drc.as_mut() {
        drc.view.category = Some(DrcCategory::Short);
    }
    let shorts = texts(&ctx);
    assert!(shorts.iter().any(|t| t == "Short (1)"), "{shorts:?}");
    assert!(!shorts.iter().any(|t| t.starts_with("HV_08")), "{shorts:?}");
    assert!(
        shorts.iter().any(|t| t == "Showing shorts only"),
        "{shorts:?}"
    );
    let drc = ctx.pcb_drc.as_ref().expect("panel");
    let kept: Vec<usize> = shown_rows(drc).iter().map(|r| r.index).collect();
    assert_eq!(kept, [1]);
}

/// #88: a filter that matches no row of a rule hides its whole group.
#[test]
fn a_severity_filter_hides_a_group_with_no_matching_row() {
    let mut warning = row(1, ViolationKind::Unconnected, "builtin::unconnected", None);
    warning.severity = Some(Severity::Warning);
    let ctx = context(
        vec![
            row(0, ViolationKind::Short, "builtin::short", None),
            warning,
        ],
        DrcFilter {
            severity: Some(Severity::Warning),
            ..DrcFilter::default()
        },
    );
    let shown = texts(&ctx);
    assert!(!shown.iter().any(|t| t.starts_with("Short (")), "{shown:?}");
    assert!(shown.iter().any(|t| t == "Unconnected (1)"), "{shown:?}");
}

/// #88: a row's second line names the layer through the shared layer-name
/// function, the position and the object count.
#[test]
fn a_row_names_its_layer_place_and_objects() {
    let ctx = context(
        vec![row(0, ViolationKind::Short, "builtin::short", None)],
        DrcFilter::default(),
    );
    let shown = texts(&ctx);
    let layer = crate::pcb_canvas::layer_style::copper_name(&ctx.i18n, CopperLayer::Top)
        .into_string()
        .replace(['\u{2068}', '\u{2069}'], "");
    let line = format!("{layer} · 1.000, 2.000 mm · 2 objects");
    assert!(shown.contains(&line), "{line}: {shown:?}");
}

/// Mockup row 88: the row whose form is open keeps Show and Waive, with
/// neither hover nor selection, and Waive waits for the git identity.
#[test]
fn the_open_row_keeps_its_actions_above_the_form() {
    let mut ctx = context(
        vec![row(0, ViolationKind::Short, "builtin::short", None)],
        DrcFilter::default(),
    );
    if let Some(drc) = ctx.pcb_drc.as_mut() {
        drc.waive_draft = Some(DrcWaiveDraft {
            key: "k0".into(),
            awaits_identity: true,
            ..DrcWaiveDraft::default()
        });
    }
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t == "Show"), "{shown:?}");
    // The row's Waive and the form's.
    assert_eq!(shown.iter().filter(|t| *t == "Waive").count(), 2);
    let window = iced::Size::new(1400.0, 1200.0);
    let pressed = messages_on_click(view(&ctx), window, "Cancel");
    assert!(
        pressed
            .iter()
            .any(|m| matches!(m, PanelMsg::Drc(DrcPanelMsg::WaiveCancelled))),
        "{pressed:?}"
    );
}

/// Mockup row 88: Waive opens one Reason field under the row, with Cancel
/// and Waive, and asks for the reason when it was left empty.
#[test]
fn waive_opens_the_reason_form_under_the_row() {
    let mut ctx = context(
        vec![row(0, ViolationKind::Short, "builtin::short", None)],
        DrcFilter::default(),
    );
    if let Some(drc) = ctx.pcb_drc.as_mut() {
        drc.waive_draft = Some(DrcWaiveDraft {
            key: "k0".into(),
            author: "C. Alp".into(),
            blank: true,
            ..DrcWaiveDraft::default()
        });
    }
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t == "Cancel"), "{shown:?}");
    // No Author field: the author comes from git config.
    let inputs: Vec<String> = lay_out_inputs(view(&ctx), iced::Size::new(1400.0, 1200.0))
        .into_iter()
        .map(|t| t.text)
        .collect();
    assert!(!inputs.iter().any(|t| t == "C. Alp"), "{inputs:?}");
    assert!(
        shown.iter().any(|t| t == "Write a reason to waive it."),
        "{shown:?}"
    );
}

/// Mockup row 88: a written rule's group has the Rule link; Short,
/// Unconnected and Keepout are set by no written rule, so theirs has none.
/// A part outside its room keeps its link to Placement.
#[test]
fn short_and_unconnected_groups_have_no_rule_link() {
    let mut unconnected = row(2, ViolationKind::Unconnected, "builtin::unconnected", None);
    unconnected.severity = Some(Severity::Warning);
    let ctx = context(
        vec![
            row(0, ViolationKind::Clearance, "user::HV_08", None),
            row(1, ViolationKind::Short, "builtin::short", None),
            unconnected,
        ],
        DrcFilter::default(),
    );
    let shown = texts(&ctx);
    assert!(shown.iter().any(|t| t == "Short (1)"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "Unconnected (1)"), "{shown:?}");
    assert_eq!(
        shown.iter().filter(|t| *t == "Rule").count(),
        1,
        "HV_08 only: {shown:?}"
    );
    use super::super::drc_rows::has_rule_link;
    assert!(has_rule_link(ViolationKind::Clearance));
    assert!(!has_rule_link(ViolationKind::Short));
    assert!(!has_rule_link(ViolationKind::Unconnected));
    assert!(!has_rule_link(ViolationKind::Keepout));
    assert!(has_rule_link(ViolationKind::PartOutsideRoom));
    assert!(has_rule_link(ViolationKind::TrackWidth));
}

/// Mockup row 88: in a 300 px dock the three filter dropdowns share one
/// row, and each label ("All severities" the longest) is clipped before
/// the arrow, not drawn under it: the label's room ends where the arrow
/// begins.
#[test]
fn a_filter_label_ends_before_its_arrow() {
    use iced::advanced::text::Renderer as _;
    let ctx = context(
        vec![row(0, ViolationKind::Short, "builtin::short", None)],
        DrcFilter::default(),
    );
    let laid: Vec<crate::layout_probe::LaidOutText> =
        lay_out_texts(view(&ctx), iced::Size::new(300.0, 1200.0))
            .into_iter()
            .map(|mut t| {
                t.text = t.text.replace(['\u{2068}', '\u{2069}'], "");
                t
            })
            .collect();
    let arrow = <iced::Renderer>::ARROW_DOWN_ICON.to_string();
    let arrows: Vec<iced::Rectangle> = laid
        .iter()
        .filter(|t| t.text == arrow)
        .map(|t| t.bounds)
        .collect();
    assert_eq!(arrows.len(), 3, "{laid:#?}");
    let pad = filter_field_padding();
    for label in ["All rules", "All layers", "All severities"] {
        let field = crate::layout_probe::find(&laid, label).bounds;
        let own = arrows
            .iter()
            .find(|a| a.x >= field.x && a.x + a.width <= field.x + field.width + 0.5)
            .unwrap_or_else(|| panic!("no arrow inside {label:?} at {field:?}: {arrows:?}"));
        let label_end = field.x + field.width - pad.right;
        assert!(
            label_end <= own.x + 0.5,
            "{label:?} runs to {label_end}, its arrow starts at {}",
            own.x
        );
    }
}
