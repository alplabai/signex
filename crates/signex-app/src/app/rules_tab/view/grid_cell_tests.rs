//! Mockup row 89: with no rule written, the clearance column shows, muted,
//! the clearance Run DRC checks with anyway, instead of empty cells. The
//! Physical All row is empty until a fab tier is picked, then shows the
//! tier's floors; Short-circuit and Un-routed net show the fixed No and On
//! on All (maintainer decision 2026-11-24).

use iced::Size;
use signex_drc_dsl::table::catalog::Domain;
use signex_drc_dsl::table::grid::GridScope;
use signex_types::net_class::NetClass;
use signex_types::pcb::{NetDef, PcbBoard};
use signex_types::project::{DrcRuleSet, FabTemplate, SetupProfile};

use super::BuiltinClearance;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::rules_tab::grid::{RowRef, mm_text};
use crate::app::rules_tab::matrix_tests::send;
use crate::app::rules_tab::{RulesPanel, RulesTabMsg};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::layout_probe::{LaidOutText, lay_out_texts};

/// A two-layer board whose setup asks `clearance` mm, with `HV_A` and
/// `LV_A`.
fn board(clearance: f64) -> PcbBoard {
    let mut board = SetupProfile::Standard.pcb(2).new_board();
    board
        .setup
        .as_mut()
        .expect("a new board has a setup")
        .clearance = clearance;
    board.nets = [(1, "HV_A"), (2, "LV_A")]
        .into_iter()
        .map(|(number, name)| NetDef {
            number,
            name: name.into(),
        })
        .collect();
    board
}

/// `HV` (0.5 mm, `HV_A`) and `LV` (no clearance of its own, `LV_A`).
fn classes() -> Vec<NetClass> {
    let mut hv = NetClass::named("HV");
    hv.nets = vec!["HV_A".into()];
    hv.clearance = 0.5;
    let mut lv = NetClass::named("LV");
    lv.nets = vec!["LV_A".into()];
    vec![hv, lv]
}

/// A project that stores no rules, on a machine with none, its board open
/// and its Rules tab on Clearance.
fn machine_rules_tab(dir: &tempfile::TempDir) -> (Signex, ProjectId) {
    rules_tab_with(dir, Some(board(0.3)))
}

/// As [`machine_rules_tab`], with `open` as the open board, or none.
fn rules_tab_with(dir: &tempfile::TempDir, open: Option<PcbBoard>) -> (Signex, ProjectId) {
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    app.ui_state.machine_drc_rules = DrcRuleSet::default();
    let project = &mut app.document_state.projects[0].data;
    project.pcb_file = Some("board.snxpcb".into());
    project.net_classes = classes();
    project.rules = None;
    if let Some(open) = open {
        app.open_pcb_tab(dir.path().join("board.snxpcb"), "board".into(), open);
    }
    let id = app.document_state.projects[0].id;
    let _ = app.update(Message::Menu(crate::menu_bar::MenuMessage::command(
        "open_rules",
    )));
    send(
        &mut app,
        RulesTabMsg::DomainPicked {
            project_id: id,
            domain: Domain::Clearance,
        },
    );
    (app, id)
}

fn laid_out(app: &Signex, id: ProjectId) -> Vec<LaidOutText> {
    lay_out_texts(app.view_rules_tab(id), Size::new(1600.0, 1000.0))
}

/// The texts drawn on the line of the text `row`.
fn row_line<'t>(texts: &'t [LaidOutText], row: &str) -> Vec<&'t LaidOutText> {
    let lines: Vec<f32> = texts
        .iter()
        .filter(|t| t.text == row)
        .map(|t| t.bounds.center_y())
        .collect();
    assert!(!lines.is_empty(), "{row} is not drawn");
    texts
        .iter()
        .filter(|t| {
            lines
                .iter()
                .any(|y| (t.bounds.center_y() - y).abs() < t.bounds.height / 2.0)
        })
        .collect()
}

/// The texts reading `value` on the line of the text `row`.
fn on_row<'t>(texts: &'t [LaidOutText], row: &str, value: &str) -> Vec<&'t LaidOutText> {
    row_line(texts, row)
        .into_iter()
        .filter(|t| t.text == value)
        .collect()
}

fn pick(app: &mut Signex, id: ProjectId, domain: Domain) {
    send(
        app,
        RulesTabMsg::DomainPicked {
            project_id: id,
            domain,
        },
    );
}

/// Pick and apply the Standard fab tier in the Presets panel, then close
/// the panel.
fn apply_standard_tier(app: &mut Signex, id: ProjectId) {
    let panel = |panel| RulesTabMsg::PanelPicked {
        project_id: id,
        panel,
    };
    send(app, panel(Some(RulesPanel::Presets)));
    send(
        app,
        RulesTabMsg::PresetPicked {
            project_id: id,
            preset: "tier:standard".into(),
        },
    );
    send(app, RulesTabMsg::PresetApplied { project_id: id });
    send(app, panel(None));
    let dsl = app.document_state.projects[0]
        .data
        .rules
        .as_ref()
        .and_then(|rules| rules.drc.dsl.clone())
        .unwrap_or_default();
    assert!(dsl.contains("rule fab_track"), "the tier is stored: {dsl}");
}

/// The Standard tier's floors the Physical All row shows: track width,
/// via diameter and via hole.
fn standard_physical() -> [String; 3] {
    let c = FabTemplate::Standard.capabilities();
    [c.track_width, c.via_diameter, c.via_drill].map(mm_text)
}

#[test]
fn an_unwritten_clearance_shows_the_boards_clearance_raised_by_its_class() {
    let dir = tempfile::tempdir().unwrap();
    let (app, id) = machine_rules_tab(&dir);
    let texts = laid_out(&app, id);
    assert!(
        !on_row(&texts, "All", "0.3mm").is_empty(),
        "All shows the board's setup clearance"
    );
    assert!(
        !on_row(&texts, "HV", "0.5mm").is_empty(),
        "HV's own 0.5 mm raises it"
    );
    assert!(
        !on_row(&texts, "LV", "0.3mm").is_empty(),
        "LV asks nothing more"
    );
}

#[test]
fn with_no_board_open_an_unwritten_clearance_shows_the_checks_default() {
    let dir = tempfile::tempdir().unwrap();
    let (app, id) = rules_tab_with(&dir, None);
    assert!(
        app.rules_board(id).is_none(),
        "only the schematic and Rules tabs are open"
    );
    let texts = laid_out(&app, id);
    let default = format!("{}mm", signex_drc::DEFAULT_CLEARANCE_MM);
    assert!(
        !on_row(&texts, "All", &default).is_empty(),
        "All shows the check's default"
    );
    assert!(
        !on_row(&texts, "HV", "0.5mm").is_empty(),
        "HV's own 0.5 mm still raises it"
    );
    assert!(
        !on_row(&texts, "LV", &default).is_empty(),
        "LV asks nothing more"
    );
}

#[test]
fn hiding_inherited_values_keeps_the_base_clearance_on_all_only() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = machine_rules_tab(&dir);
    send(&mut app, RulesTabMsg::InheritedToggled { project_id: id });
    let texts = laid_out(&app, id);
    let shown: Vec<&LaidOutText> = texts
        .iter()
        .filter(|t| t.text == "0.3mm" || t.text == "0.5mm")
        .collect();
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert_eq!(on_row(&texts, "All", "0.3mm").len(), 1);
}

#[test]
fn a_board_with_no_setup_clearance_shows_the_checks_default() {
    let builtin = BuiltinClearance::of(&board(0.0), &classes()).expect("a usable board");
    assert_eq!(
        builtin.for_row(&RowRef::All, &[]),
        Some(signex_drc::DEFAULT_CLEARANCE_MM)
    );
}

#[test]
fn a_net_takes_its_class_and_a_scoped_rule_takes_nothing() {
    let builtin = BuiltinClearance::of(&board(0.3), &classes()).expect("a usable board");
    let in_hv = [GridScope::Class("HV".into()), GridScope::All];
    assert_eq!(
        builtin.for_row(&RowRef::Net("HV_A".into()), &in_hv),
        Some(0.5)
    );
    assert_eq!(
        builtin.for_row(&RowRef::Net("LOOSE".into()), &[GridScope::All]),
        Some(0.3)
    );
    assert_eq!(builtin.for_row(&RowRef::Scoped("tight".into()), &[]), None);
}

#[test]
fn the_physical_all_row_is_empty_until_a_fab_tier_is_picked() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = machine_rules_tab(&dir);
    pick(&mut app, id, Domain::Physical);
    let texts = laid_out(&app, id);
    let lengths: Vec<&str> = row_line(&texts, "All")
        .into_iter()
        .map(|t| t.text.as_str())
        .filter(|text| text.ends_with("mm"))
        .collect();
    assert!(lengths.is_empty(), "All shows no length: {lengths:?}");
}

#[test]
fn the_physical_all_row_shows_the_picked_tiers_floors() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = machine_rules_tab(&dir);
    apply_standard_tier(&mut app, id);
    pick(&mut app, id, Domain::Physical);
    let texts = laid_out(&app, id);
    for floor in standard_physical() {
        assert!(
            !on_row(&texts, "All", &floor).is_empty(),
            "All shows the tier's {floor}"
        );
    }
}

#[test]
fn hiding_inherited_values_keeps_the_tiers_floors_on_all() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = machine_rules_tab(&dir);
    apply_standard_tier(&mut app, id);
    pick(&mut app, id, Domain::Physical);
    send(&mut app, RulesTabMsg::InheritedToggled { project_id: id });
    let texts = laid_out(&app, id);
    let [track, ..] = standard_physical();
    assert_eq!(on_row(&texts, "All", &track).len(), 1, "All keeps {track}");
    let shown = texts.iter().filter(|t| t.text == track).count();
    assert_eq!(shown, 1, "the rows below hide the inherited {track}");
}

#[test]
fn short_circuit_and_unrouted_net_show_no_and_on_on_all_with_or_without_a_tier() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = machine_rules_tab(&dir);
    for tier in ["none", "standard"] {
        if tier == "standard" {
            apply_standard_tier(&mut app, id);
            pick(&mut app, id, Domain::Clearance);
        }
        let texts = laid_out(&app, id);
        assert_eq!(
            on_row(&texts, "All", "No").len(),
            1,
            "Short-circuit, {tier}"
        );
        assert_eq!(
            on_row(&texts, "All", "On").len(),
            1,
            "Un-routed net, {tier}"
        );
        assert!(
            on_row(&texts, "HV", "No").is_empty() && on_row(&texts, "HV", "On").is_empty(),
            "the fixed values are All's alone, {tier}"
        );
    }
}
