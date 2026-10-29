//! A child-sheet ERC finding carries child coordinates; it must be filed under
//! the child sheet, not drawn or panned to on the parent's canvas (#956).

use std::path::PathBuf;

use signex_types::format::SnxSchematic;
use signex_types::schematic::{HAlign, Label, LabelType, Point, VAlign};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};

const CHILD_AT: (f64, f64) = (150.0, 120.0);

/// `top.snxsch` is open and active with a sheet symbol for `child.snxsch`
/// (no pins); the on-disk child has a hierarchical label the parent does not
/// expose, at `CHILD_AT`.
fn parent_with_a_far_child_label() -> (Signex, PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-erc-child-space-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let mut child = sheet_with_net("R_CHILD", "CHILD_NET", &[]);
    child.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: Uuid::new_v4(),
        text: "ORPHAN".to_string(),
        position: Point::new(CHILD_AT.0, CHILD_AT.1),
        rotation: 0.0,
        label_type: LabelType::Hierarchical,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::default(),
        justify_v: VAlign::default(),
        font_family: None,
        locked: false,
    });
    let text = SnxSchematic::new(child).write_string().expect("serialize");
    std::fs::write(dir.join("child.snxsch"), text).expect("write child");

    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    let mut parent = sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]);
    parent.child_sheets[0].position = Point::new(10.0, 10.0);
    parent.child_sheets[0].size = (30.0, 30.0);
    open_with(&mut app.document_state, &top, parent);
    app.document_state.active_path = Some(top);
    (app, dir)
}

fn at_child_coordinate(x: f64, y: f64) -> bool {
    (x - CHILD_AT.0).abs() < 1e-6 && (y - CHILD_AT.1).abs() < 1e-6
}

#[test]
fn a_child_label_finding_draws_no_marker_on_the_parent_canvas() {
    let (mut app, dir) = parent_with_a_far_child_label();
    app.run_erc_blocking();

    let top = dir.join("top.snxsch");
    let parent_hit = app
        .ui_state
        .erc_violations_by_path
        .get(&top)
        .is_some_and(|v| {
            v.iter()
                .any(|v| at_child_coordinate(v.location.x, v.location.y))
        });
    let marker_hit = app
        .interaction_state
        .active_canvas()
        .erc_markers
        .iter()
        .any(|m| at_child_coordinate(m.x, m.y));
    std::fs::remove_dir_all(&dir).ok();
    assert!(!parent_hit, "child coordinate filed under the parent sheet");
    assert!(
        !marker_hit,
        "marker drawn on the parent canvas at child coordinates"
    );
}

#[test]
fn a_child_label_finding_is_filed_and_targeted_on_the_child_sheet() {
    let (mut app, dir) = parent_with_a_far_child_label();
    app.run_erc_blocking();

    let child = dir.join("child.snxsch");
    let filed = app.ui_state.erc_violations_by_path.get(&child).map(|v| {
        v.iter()
            .filter(|v| v.builtin_kind() == Some(signex_erc::RuleKind::BadHierSheetPin))
            .filter(|v| at_child_coordinate(v.location.x, v.location.y))
            .map(|v| v.location_space)
            .collect::<Vec<_>>()
    });
    let entries = app.build_erc_diagnostic_entries();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(filed, Some(vec![signex_erc::LocationSpace::ThisSheet]));
    let entry = entries
        .iter()
        .find(|e| {
            at_child_coordinate(e.world_x, e.world_y)
                && e.rule_kind == Some(signex_erc::RuleKind::BadHierSheetPin)
        })
        .expect("panel entry for the child label");
    assert_eq!(entry.sheet_path, child);
    assert!(entry.locatable);
}

#[test]
fn a_child_finding_whose_sheet_was_not_checked_is_reported_but_not_panned_to() {
    use signex_erc::{LocationSpace, RuleId, RuleKind, Severity, Violation};
    let top = PathBuf::from("/w/top.snxsch");
    let (mut app, _task) = Signex::new();
    let mut v = Violation::new(
        RuleId::builtin(RuleKind::BadHierSheetPin),
        RuleKind::BadHierSheetPin,
        Severity::Warning,
        "orphan",
        Point::new(CHILD_AT.0, CHILD_AT.1),
    );
    v.location_space = LocationSpace::ChildSheet;
    v.child_sheet = Some("gone.snxsch".to_string());
    app.ui_state
        .erc_violations_by_path
        .insert(top.clone(), vec![v]);

    app.refresh_active_erc_from_cache(Some(&top));
    let entries = app.build_erc_diagnostic_entries();

    assert!(app.interaction_state.active_canvas().erc_markers.is_empty());
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].locatable);
}

/// `top -> (p, q)`, `p -> (a, b)`, `q -> (a, b)`; `a` and `b` each hold
/// `R_DUP`. `top`, `p` and `q` all walk the `a`/`b` clash (#998).
fn two_descendants_sharing_a_designator() -> (Signex, PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-erc-desc-clash-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let write = |name: &str, sheet| {
        let text = SnxSchematic::new(sheet).write_string().expect("serialize");
        std::fs::write(dir.join(name), text).expect("write sheet");
    };
    write("a.snxsch", sheet_with_net("R_DUP", "NET_A", &[]));
    write("b.snxsch", sheet_with_net("R_DUP", "NET_B", &[]));
    for (name, reference) in [("p.snxsch", "R_P"), ("q.snxsch", "R_Q")] {
        write(
            name,
            sheet_with_net(reference, "NET_M", &["a.snxsch", "b.snxsch"]),
        );
    }
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R_TOP", "NET_T", &["p.snxsch", "q.snxsch"]),
    );
    app.document_state.active_path = Some(top);
    (app, dir)
}

#[test]
fn a_clash_between_two_descendant_sheets_is_reported_once_per_run() {
    let (mut app, dir) = two_descendants_sharing_a_designator();
    app.run_erc_blocking();

    let mut owners: Vec<String> = app
        .ui_state
        .erc_violations_by_path
        .iter()
        .flat_map(|(path, list)| {
            list.iter()
                .filter(|v| v.builtin_kind() == Some(signex_erc::RuleKind::DuplicateRefDesignator))
                .filter(|v| v.message.contains("R_DUP"))
                .map(move |_| path.file_name().unwrap().to_string_lossy().to_string())
        })
        .collect();
    std::fs::remove_dir_all(&dir).ok();
    owners.sort();
    // One finding per clashing symbol (a's and b's), not one per parent.
    assert_eq!(owners.len(), 2, "{owners:?}");
}

/// Two copies of one channel: `ch{1,2}/top -> (a, b)`, `a` and `b` both hold
/// `R_DUP`. Four distinct files, so two independent clashes whose
/// diagnostics read identically (#998).
#[test]
fn identical_clashes_in_copied_channel_directories_are_each_reported() {
    let dir = std::env::temp_dir().join(format!("signex-erc-copied-{}", Uuid::new_v4()));
    for ch in ["ch1", "ch2"] {
        std::fs::create_dir_all(dir.join(ch)).expect("tempdir");
    }
    let write = |name: &str, sheet| {
        let text = SnxSchematic::new(sheet).write_string().expect("serialize");
        std::fs::write(dir.join(name), text).expect("write sheet");
    };
    for ch in ["ch1", "ch2"] {
        write(
            &format!("{ch}/a.snxsch"),
            sheet_with_net("R_DUP", "NET_A", &[]),
        );
        write(
            &format!("{ch}/b.snxsch"),
            sheet_with_net("R_DUP", "NET_B", &[]),
        );
        write(
            &format!("{ch}/top.snxsch"),
            sheet_with_net("R_TOP", "NET_T", &["a.snxsch", "b.snxsch"]),
        );
    }
    let mut app = app_workspace(
        &dir.to_string_lossy(),
        &["ch1/top.snxsch", "ch2/top.snxsch"],
    );
    let top = dir.join("ch1/top.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R_TOP", "NET_T", &["a.snxsch", "b.snxsch"]),
    );
    app.document_state.active_path = Some(top);
    app.run_erc_blocking();

    let count: usize = app
        .ui_state
        .erc_violations_by_path
        .values()
        .flatten()
        .filter(|v| v.builtin_kind() == Some(signex_erc::RuleKind::DuplicateRefDesignator))
        .filter(|v| v.message.contains("R_DUP"))
        .count();
    std::fs::remove_dir_all(&dir).ok();
    // a and b per channel, two channels.
    assert_eq!(count, 4);
}

/// A sheet whose tab was closed with unsaved edits keeps its engine parked
/// (#1089). ERC navigation to that sheet must reattach the engine, not
/// replace it with the on-disk copy (#1135).
#[test]
fn erc_navigation_keeps_the_dirty_engine_of_a_tab_less_sheet() {
    let dir = std::env::temp_dir().join(format!("signex-erc-nav-dirty-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let path = dir.join("child.snxsch");
    let on_disk = sheet_with_net("R_DISK", "DISK_NET", &[]);
    let text = SnxSchematic::new(on_disk)
        .write_string()
        .expect("serialize");
    std::fs::write(&path, text).expect("write sheet");

    let mut app = app_workspace(&dir.to_string_lossy(), &["child.snxsch"]);
    open_with(
        &mut app.document_state,
        &path,
        sheet_with_net("R_EDIT", "EDITED_NET", &[]),
    );
    app.document_state.dirty_paths.insert(path.clone());
    assert!(app.document_state.tabs.is_empty());

    app.ensure_sheet_open_and_active(&path);

    let held: Vec<String> = app.document_state.engines[&path]
        .document()
        .labels
        .iter()
        .map(|l| l.text.clone())
        .collect();
    let tabs = app.document_state.tabs.len();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(held, vec!["EDITED_NET".to_string()]);
    assert_eq!(tabs, 1);
}

/// Quick Fix on a finding whose sheet cannot be opened must not place the
/// NoConnect on whichever sheet happens to be active (#1367).
#[test]
fn quick_fix_refuses_when_the_target_sheet_fails_to_open() {
    let dir = std::env::temp_dir().join(format!("signex-erc-qf-missing-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &[]),
    );
    app.document_state.active_path = Some(top.clone());
    let missing = dir.join("missing.snxsch");
    app.ui_state.erc_violations_by_path.insert(
        missing,
        vec![signex_erc::Violation::new(
            signex_erc::RuleId::builtin(signex_erc::RuleKind::UnusedPin),
            signex_erc::RuleKind::UnusedPin,
            signex_erc::Severity::Warning,
            "unused pin",
            Point::new(5.0, 5.0),
        )],
    );

    let _ = app.handle_erc_quick_fix(0);

    let placed = app.document_state.engines[&top]
        .document()
        .no_connects
        .len();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(placed, 0, "NoConnect landed on the active sheet");
}

// ---- net-level findings (#1364) ----

mod net_level {
    use super::*;
    use signex_erc::{LocationSpace, NetFinding, NetFindingPin, RuleId, RuleKind, Severity};
    use signex_types::schematic::{SelectedItem, SelectedKind};

    fn net_violation(pin_at: Point) -> signex_erc::Violation {
        let mut v = signex_erc::Violation::new(
            RuleId::user("net_rule"),
            RuleKind::NetLabelConflict,
            Severity::Warning,
            "net-level",
            pin_at,
        );
        v.location_space = LocationSpace::Net;
        v.net = Some(NetFinding {
            name: "NET".to_string(),
            pins: vec![NetFindingPin {
                symbol: Uuid::from_u128(7),
                reference: "R1".to_string(),
                number: "1".to_string(),
                position: pin_at,
            }],
        });
        v
    }

    #[test]
    fn a_net_finding_draws_one_net_marker_at_its_pin_and_is_locatable() {
        let top = PathBuf::from("/w/top.snxsch");
        let (mut app, _task) = Signex::new();
        app.ui_state
            .erc_violations_by_path
            .insert(top.clone(), vec![net_violation(Point::new(3.0, 4.0))]);
        app.refresh_active_erc_from_cache(Some(&top));

        let markers = &app.interaction_state.active_canvas().erc_markers;
        assert_eq!(markers.len(), 1);
        assert!(markers[0].net_level);
        assert_eq!((markers[0].x, markers[0].y), (3.0, 4.0));
        let entries = app.build_erc_diagnostic_entries();
        assert!(entries[0].locatable);
        assert_eq!(
            entries[0].net.as_ref().map(|n| n.name.as_str()),
            Some("NET")
        );
    }

    #[test]
    fn clicking_a_net_finding_selects_the_whole_net_and_fits_its_box() {
        let dir = std::env::temp_dir().join(format!("signex-erc-net-focus-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("tempdir");
        let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
        let top = dir.join("top.snxsch");
        let sheet = sheet_with_net("R1", "NET", &[]);
        let (wire, label) = (sheet.wires[0].uuid, sheet.labels[0].uuid);
        app.open_schematic_tab(top.clone(), "top".to_string(), sheet);

        let mut v = net_violation(Point::new(0.0, 0.0));
        v.net.as_mut().expect("net").pins[0].symbol =
            app.active_render_snapshot().expect("sheet is open").symbols[0].uuid;
        let symbol = v.net.as_ref().expect("net").pins[0].symbol;
        app.ui_state.erc_violations_by_path.insert(top, vec![v]);
        let _ = app.handle_focus_erc_diagnostic_index(0);
        std::fs::remove_dir_all(&dir).ok();

        let selected = &app.interaction_state.active_canvas().selected;
        for item in [
            SelectedItem::new(wire, SelectedKind::Wire),
            SelectedItem::new(label, SelectedKind::Label),
            SelectedItem::new(symbol, SelectedKind::Symbol),
        ] {
            assert!(selected.contains(&item), "{item:?} not highlighted");
        }
        let fit = app
            .interaction_state
            .canvas
            .pending_fit
            .get()
            .expect("viewport fit staged");
        // The wire runs (0,0)-(10,0): the fit box must frame all of it.
        assert!(fit.x <= 0.0 && fit.x + fit.width >= 10.0, "{fit:?}");
    }

    #[test]
    fn fit_rect_frames_the_box_with_margin_and_a_minimum_size() {
        let r = crate::app::handlers::canvas::fit_view::fit_rect([0.0, 0.0, 100.0, 50.0]);
        assert_eq!((r.x, r.y, r.width, r.height), (-2.0, -2.0, 104.0, 54.0));
        let dot = crate::app::handlers::canvas::fit_view::fit_rect([5.0, 5.0, 5.0, 5.0]);
        assert_eq!((dot.width, dot.height), (16.0, 16.0));
        assert_eq!((dot.x, dot.y), (-3.0, -3.0));
    }
}
