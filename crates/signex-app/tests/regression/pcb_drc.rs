//! #2222 - the design-rule check driven through `Message`s on a real board
//! opened from disk: the report, the Messages panel rows, a click on a row
//! selecting its items, and a board that is never edited.

use pcb_geom::ItemRef;
use signex_app::app::{FileMsg, Message, PcbMsg, Signex};
use signex_app::dock::DockMessage;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::PanelMsg;
use signex_drc::ViolationKind;
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point, Segment};
use uuid::Uuid;

fn track(n: u128, y: f64, net: u32) -> Segment {
    Segment {
        uuid: Uuid::from_u128(n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// Two tracks 0.4 mm apart centre to centre: a 0.15 mm gap, under the
/// default 0.2 mm; a third 5 mm away.
fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: Vec::new(),
        segments: vec![track(1, 0.0, 1), track(2, 0.4, 2), track(3, 5.0, 3)],
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn open_board() -> (Signex, tempfile::TempDir, std::path::PathBuf) {
    open(board())
}

fn open(board: PcbBoard) -> (Signex, tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    crate::drive::drive(&mut app, task);
    (app, tmp, path)
}

/// The open board as the tab holds it, for a before/after comparison.
fn board_dump(app: &Signex) -> String {
    let docs = &app.document_state;
    format!("{:?}", docs.tabs[docs.active_tab].cached_document)
}

fn run_drc(app: &mut Signex) {
    let task = app.update(Message::Pcb(PcbMsg::DrcRequested));
    crate::drive::drive(app, task);
}

#[test]
fn the_check_lists_the_close_pair_and_clicking_it_selects_both_tracks() {
    let (mut app, _tmp, path) = open_board();
    run_drc(&mut app);

    let done = app
        .interaction_state
        .pcb_drc
        .done_for(&path)
        .expect("a report for the open board");
    assert_eq!(done.report.violations.len(), 1, "{:?}", done.report);
    let v = &done.report.violations[0];
    assert_eq!(v.kind, ViolationKind::Clearance);
    assert_eq!((v.measured_nm, v.required_nm), (150_000, 200_000));

    let panel = app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("panel");
    assert!(panel.ran && !panel.running && !panel.stale);
    assert_eq!((panel.clearance, panel.short, panel.unconnected), (1, 0, 0));
    assert_eq!(panel.rows.len(), 1);

    let task = app.update(Message::Pcb(PcbMsg::DrcViolationPicked(0)));
    crate::drive::drive(&mut app, task);
    let selected: Vec<ItemRef> = app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(
        selected,
        vec![
            ItemRef::Track(Uuid::from_u128(1)),
            ItemRef::Track(Uuid::from_u128(2))
        ]
    );
}

#[test]
fn the_check_does_not_edit_the_board() {
    let (mut app, _tmp, path) = open_board();
    let board_before = board_dump(&app);
    let file_before = std::fs::read(&path).unwrap();
    run_drc(&mut app);
    assert!(app.interaction_state.pcb_drc.done_for(&path).is_some());
    assert_eq!(board_dump(&app), board_before);
    assert_eq!(std::fs::read(&path).unwrap(), file_before);
    assert_eq!(app.document_state.dirty_document_count(), 0);
}

#[test]
fn the_tools_menu_row_runs_the_check_and_is_greyed_without_a_board() {
    let run_row = |app: &mut Signex| {
        let id = AppCommandId::new("run_drc").expect("id");
        let task = app.update(Message::Menu(MenuMessage::Command(id)));
        crate::drive::drive(app, task);
    };
    let (mut app, _tmp, path) = open_board();
    run_row(&mut app);
    let done = app
        .interaction_state
        .pcb_drc
        .done_for(&path)
        .expect("report");
    assert_eq!(done.report.violations.len(), 1);

    let (mut none, _boot) = Signex::new();
    none.ui_state.first_run_tour_open = false;
    run_row(&mut none);
    assert!(none.document_state.panel_ctx.pcb_drc.is_none());
}

#[test]
fn the_panel_button_message_runs_the_check() {
    let (mut app, _tmp, path) = open_board();
    let task = app.update(Message::Dock(DockMessage::Panel(PanelMsg::RunDrc)));
    crate::drive::drive(&mut app, task);
    assert!(app.interaction_state.pcb_drc.done_for(&path).is_some());
}

#[test]
fn run_drc_with_no_board_open_runs_nothing() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    run_drc(&mut app);
    assert!(app.document_state.panel_ctx.pcb_drc.is_none());
}

/// #2307 step 2a: a run carries the resolved severity of each rule, taken
/// from the machine's `User Default` profile for a board without a project,
/// and the severity never changes what is found.
#[test]
fn a_run_carries_the_severities_of_the_machine_profile_and_finds_the_same() {
    use signex_app::app::ErcMsg;
    use signex_types::violation::Severity;
    let (mut app, _tmp, path) = open_board();
    run_drc(&mut app);
    let plain = app.interaction_state.pcb_drc.done_for(&path).unwrap();
    let plain = plain.report.clone();
    assert_eq!(plain.rules.severities, signex_drc::DrcSeverities::builtin());
    assert_eq!(
        plain.severity_of(&plain.violations[0]),
        Some(Severity::Error)
    );

    let mut profile = signex_types::project::RuleProfile::new("User Default", "1", <_>::default());
    profile
        .drc
        .severity
        .insert("builtin::clearance".into(), "warning".into());
    let task = app.update(Message::Erc(ErcMsg::UserDefaultLoaded(
        std::sync::Arc::new(Ok(Some(profile))),
    )));
    crate::drive::drive(&mut app, task);
    run_drc(&mut app);
    let done = app.interaction_state.pcb_drc.done_for(&path).unwrap();
    assert_eq!(done.report.violations, plain.violations);
    assert_eq!(
        done.report.severity_of(&done.report.violations[0]),
        Some(Severity::Warning)
    );
}

#[test]
fn the_default_clearance_is_said_and_a_set_rule_is_not() {
    let (mut app, _tmp, _path) = open_board();
    run_drc(&mut app);
    let panel = app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("panel");
    assert_eq!(panel.default_clearance_mm.as_deref(), Some("0.2"));

    let mut set = board();
    set.setup = Some(signex_types::pcb::PcbSetup {
        grid_size: 0.0,
        trace_width: 0.0,
        via_diameter: 0.0,
        via_drill: 0.0,
        clearance: 0.2,
        track_min_width: 0.0,
        via_min_diameter: 0.0,
        via_min_drill: 0.0,
    });
    let (mut app, _tmp, _path) = open(set);
    run_drc(&mut app);
    let panel = app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("panel");
    assert!(panel.ran);
    assert_eq!(panel.default_clearance_mm, None);
}

/// Load a `User Default` profile whose `drc` layer holds `dsl`.
fn load_profile_dsl(app: &mut Signex, dsl: Option<&str>) {
    use signex_app::app::ErcMsg;
    let mut profile = signex_types::project::RuleProfile::new("User Default", "1", <_>::default());
    profile.drc.dsl = dsl.map(str::to_string);
    let task = app.update(Message::Erc(ErcMsg::UserDefaultLoaded(
        std::sync::Arc::new(Ok(Some(profile))),
    )));
    crate::drive::drive(app, task);
}

/// The Messages lines of one DRC run, in order.
fn drc_lines_of_a_run(app: &mut Signex) -> Vec<String> {
    let _ = signex_app::diagnostics::init_logging();
    let since = signex_app::diagnostics::newest_entry_id().unwrap_or(0);
    run_drc(app);
    signex_app::diagnostics::recent_entries()
        .into_iter()
        .filter(|e| e.id > since && e.message.contains("DRC"))
        .map(|e| e.message)
        .collect()
}

/// #2307 step 2b: with no rule text (none, or an empty one) a run reports
/// and says exactly what it did before the rule language.
#[test]
fn without_rule_text_the_report_and_its_messages_are_unchanged() {
    // As trunk writes them, float formatting included.
    let pinned = [
        "DRC: No clearance rule set on this board; checked with 0.19999999999999998 mm. Set it in Board Setup.",
        "DRC: 1 clearance, 0 short, 0 unconnected, 0 keepout (minimum clearance 0.19999999999999998 mm)",
        "DRC: clearance 0.15 mm, needs 0.19999999999999998 mm, net 1 / net 2 at (0, 0.19999999999999998) mm",
    ];
    for dsl in [None, Some(""), Some("  \n")] {
        let (mut app, _tmp, path) = open_board();
        load_profile_dsl(&mut app, dsl);
        assert_eq!(drc_lines_of_a_run(&mut app), pinned, "{dsl:?}");
        let done = app.interaction_state.pcb_drc.done_for(&path).unwrap();
        assert_eq!(done.report.violations.len(), 1);
        assert_eq!(done.report.violations[0].rule, None);
        assert_eq!(done.report.rules.user_rules().count(), 0);
    }
}

/// A rule in the machine profile's text loosens the clearance and adds a
/// width check; both are said in Messages, with each finding's rule.
#[test]
fn rule_text_in_the_profile_is_applied_by_the_run() {
    use signex_types::violation::Severity;
    let (mut app, _tmp, path) = open_board();
    load_profile_dsl(
        &mut app,
        Some(
            "rule tight on clearance require clearance >= 0.1mm then error\n\
             rule wide on track require width >= 0.3mm then warning \"too thin\"",
        ),
    );
    let lines = drc_lines_of_a_run(&mut app);
    let done = app.interaction_state.pcb_drc.done_for(&path).unwrap();
    let kinds: Vec<_> = done.report.violations.iter().map(|v| v.kind).collect();
    assert_eq!(kinds, [ViolationKind::TrackWidth; 3], "{lines:?}");
    let v = &done.report.violations[0];
    assert_eq!((v.measured_nm, v.required_nm), (250_000, 300_000));
    assert_eq!(v.rule.as_deref(), Some("user::wide"));
    assert_eq!(done.report.severity_of(v), Some(Severity::Warning));
    assert!(
        lines.contains(&"DRC DSL: applying 2 rule(s)".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"DRC: 3 track width, 0 via diameter, 0 via drill".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with(
            "DRC: track width 0.25 mm, needs at least 0.3 mm (user::wide), net 1 / net 1"
        )),
        "{lines:?}"
    );
    // With written rules the board value holds only where none applies.
    assert!(
        lines.contains(
            &"DRC: No clearance rule set on this board; 0.19999999999999998 mm applies \
              where no written rule does. Set it in Board Setup."
                .to_string()
        ),
        "{lines:?}"
    );
    assert!(
        lines.contains(
            &"DRC: 0 clearance, 0 short, 0 unconnected, 0 keepout (minimum clearance \
              0.19999999999999998 mm where no written rule applies)"
                .to_string()
        ),
        "{lines:?}"
    );
    let panel = app.document_state.panel_ctx.pcb_drc.as_ref().unwrap();
    assert_eq!(panel.rows.len(), 3);
    assert!(!panel.rows[0].over_maximum);
    assert!(panel.written_rules);
    assert_eq!(
        (panel.rules_not_applied, panel.rule_text_unparsed),
        (0, false)
    );
}

/// A syntax error drops the whole text: the results say so, not only the
/// Messages line, so a typo never passes as a clean board.
#[test]
fn rule_text_with_a_syntax_error_is_said_beside_the_results() {
    let (mut app, _tmp, path) = open_board();
    load_profile_dsl(
        &mut app,
        Some("rule tight on clearance require clearance 0.1mm then error"),
    );
    let lines = drc_lines_of_a_run(&mut app);
    // Its own line, so the Messages length cap never cuts it off.
    assert!(
        lines.contains(&"DRC rules: the rule text was not applied (syntax error)".to_string()),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("DRC: 1 clearance") && l.ends_with("mm)")),
        "{lines:?}"
    );
    let done = app.interaction_state.pcb_drc.done_for(&path).unwrap();
    assert_eq!(done.report.violations.len(), 1);
    let panel = app.document_state.panel_ctx.pcb_drc.as_ref().unwrap();
    assert!(panel.rule_text_unparsed && !panel.written_rules);
}

/// A rule that does not compile is named in Messages and skipped; the rest
/// of the text still applies.
#[test]
fn a_rule_that_does_not_compile_is_reported_and_skipped() {
    let (mut app, _tmp, path) = open_board();
    load_profile_dsl(
        &mut app,
        Some(
            "rule bad on track require width >= 1in then error\n\
             rule tight on clearance require clearance >= 0.1mm then error",
        ),
    );
    let lines = drc_lines_of_a_run(&mut app);
    assert!(
        lines.contains(
            &"DRC DSL (profile): line 1, column 37: rule 'bad': unknown unit 'in' \
              (mm, mil, um or nm); the rule is skipped"
                .to_string()
        ),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"DRC DSL: applying 1 rule(s)".to_string()),
        "{lines:?}"
    );
    let done = app.interaction_state.pcb_drc.done_for(&path).unwrap();
    // `tight` loosens the 0.2 mm default to 0.1 mm: the 0.15 mm gap passes.
    assert!(done.report.violations.is_empty(), "{:?}", done.report);
    // A clean board with a skipped rule still says so, on its own line.
    assert!(
        lines.contains(&"DRC rules: 1 written rule(s) not applied".to_string()),
        "{lines:?}"
    );
    let panel = app.document_state.panel_ctx.pcb_drc.as_ref().unwrap();
    assert_eq!(panel.rules_not_applied, 1);
}
