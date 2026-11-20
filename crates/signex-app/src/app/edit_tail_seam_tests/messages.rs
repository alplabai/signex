//! Block A test 12 (b): the Messages text every edit tail writes. The ring is
//! shared and capped, so each line is read through `logged_count` with a
//! needle holding a file name or designator unique to the test.

use std::path::{Path, PathBuf};

use signex_engine::{Command, UndoGroupId};
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, Wire,
};

use super::paper;
use crate::app::command::queries::tests::sheet;
use crate::app::handlers::editing_commands::net_color_driver as colours;
use crate::app::{Signex, UndoGroupKind, UndoGroupSheets};
use crate::diagnostics::logged_count;

/// A short name no other test uses: Messages keeps 160 bytes of a line, so
/// the unique part must sit early in it.
fn short_unique(stem: &str) -> String {
    format!("{stem}{:08x}", uuid::Uuid::now_v7().as_u128() as u32)
}

/// The line as the Messages ring keeps it: whitespace runs folded and cut at
/// 160 bytes, as `diagnostics::compact_message` does.
fn as_kept(line: &str) -> String {
    let compact = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() <= 160 {
        compact
    } else {
        crate::text_trunc::prefix_within_bytes(&compact, 157).to_string()
    }
}

/// Run `act` and assert it logged `line` exactly once.
fn says_once(line: &str, act: impl FnOnce()) {
    let needle = as_kept(line);
    let before = logged_count(&needle);
    act();
    assert_eq!(logged_count(&needle), before + 1, "expected once: {line}");
}

fn sheet_path(name: &str) -> PathBuf {
    std::env::temp_dir().join("edit-tail-seam").join(name)
}

/// An app with one sheet per name, the last one active.
fn app_with_sheets(names: &[&str]) -> (Signex, Vec<PathBuf>) {
    let (mut app, _task) = Signex::new();
    let paths: Vec<PathBuf> = names.iter().map(|name| sheet_path(name)).collect();
    for (n, path) in paths.iter().enumerate() {
        colours::add_sheet_with(&mut app, path, sheet(200 + n as u128));
        app.document_state.mark_document_clean(path);
    }
    (app, paths)
}

/// One structural edit on each of `stamped` under one new group, recorded as
/// a run of `kind` over `live`.
fn group_over(
    app: &mut Signex,
    kind: UndoGroupKind,
    live: &[PathBuf],
    stamped: &[PathBuf],
) -> UndoGroupId {
    let group = app.mint_undo_group();
    for path in stamped {
        let engine = app.document_state.engines.get_mut(path).unwrap();
        engine.begin_transaction().unwrap();
        engine.execute(paper("A3")).unwrap();
        engine.commit_transaction(Some(group)).unwrap();
    }
    let record = UndoGroupSheets {
        kind,
        live: live.to_vec(),
        ..Default::default()
    };
    app.document_state.undo_groups.insert(group, record);
    group
}

fn edit_directly(app: &mut Signex, path: &Path) {
    let engine = app.document_state.engines.get_mut(path).unwrap();
    engine.execute(paper("A2")).unwrap();
}

/// The words a run's messages use, as `UndoGroupSheets` words them.
struct Words {
    kind: UndoGroupKind,
    wide: &'static str,
    title: &'static str,
    noun: &'static str,
    kept: &'static str,
    holds: &'static str,
    done: &'static str,
}

const fn words(
    kind: UndoGroupKind,
    wide: &'static str,
    title: &'static str,
    noun: &'static str,
    kept: &'static str,
    holds: &'static str,
    done: &'static str,
) -> Words {
    Words {
        kind,
        wide,
        title,
        noun,
        kept,
        holds,
        done,
    }
}

const KINDS: [Words; 10] = [
    words(
        UndoGroupKind::Annotate,
        "project-wide annotate",
        "Annotate",
        "annotate",
        "annotated designators",
        "designators",
        "annotated",
    ),
    words(
        UndoGroupKind::Reprefix,
        "project-wide re-prefix",
        "Re-prefix",
        "re-prefix",
        "re-prefixed designators",
        "designators",
        "re-prefixed",
    ),
    words(
        UndoGroupKind::Replace,
        "project-wide replace",
        "Replace",
        "replace",
        "replaced text",
        "text",
        "replaced",
    ),
    words(
        UndoGroupKind::Footprints,
        "project-wide footprint assignment",
        "Assign Footprints",
        "footprint assignment",
        "assigned footprints",
        "footprints",
        "assigned",
    ),
    words(
        UndoGroupKind::SheetNumbers,
        "project-wide sheet numbering",
        "Sheet numbering",
        "sheet numbering",
        "new sheet numbers",
        "sheet numbers",
        "renumbered",
    ),
    words(
        UndoGroupKind::BackAnnotate,
        "project-wide back annotate",
        "Back annotate",
        "back annotate",
        "back-annotated designators and values",
        "designators and values",
        "back-annotated",
    ),
    words(
        UndoGroupKind::AgentEdit,
        "cross-sheet agent edit",
        "Agent edit",
        "agent edit",
        "agent edits",
        "contents",
        "edited",
    ),
    words(
        UndoGroupKind::FootprintSwap,
        "project-wide footprint change",
        "Change Footprint",
        "footprint change",
        "changed footprints",
        "footprints",
        "changed",
    ),
    words(
        UndoGroupKind::PartSwap,
        "project-wide pin or gate swap",
        "Pin or gate swap",
        "pin or gate swap",
        "swapped pin connections",
        "pin connections",
        "swapped",
    ),
    words(
        UndoGroupKind::SheetEntrySync,
        "project-wide sheet entry sync",
        "Sheet entry sync",
        "sheet entry sync",
        "synced ports",
        "ports",
        "synced",
    ),
];

/// The lines of [`skipped_members_and_the_dropped_redo`], one test each so a
/// mutation of one log call shows in its own test.
#[derive(Clone, Copy, PartialEq)]
enum Skip {
    HistoryGone,
    Closed,
    DiskOnly,
    DroppedRedo,
}

/// History gone, closed and disk-only on one undo, then the dropped redo;
/// `check` is the line asserted.
fn skipped_members_and_the_dropped_redo(w: &Words, check: Skip) {
    let [a, b, c, d, e] = ["a", "b", "c", "d", "e"].map(short_unique);
    let (mut app, paths) = app_with_sheets(&[&b, &a]);
    let (b_path, a_path) = (paths[0].clone(), paths[1].clone());
    let live = [a_path.clone(), b_path, sheet_path(&c)];
    let group = group_over(&mut app, w.kind, &live, &[a_path]);
    app.document_state
        .undo_groups
        .get_mut(&group)
        .unwrap()
        .disk_only = vec![sheet_path(&d)];
    let (title, noun) = (w.title, w.noun);
    let gone = format!(
        "{title} undone on the other sheets but not on {b}: its undo history no longer holds \
         the {noun} (reopened, or trimmed past the history limit), so it keeps its {}.",
        w.kept
    );
    let closed = format!(
        "{title} undone on open sheets but not on {c}: closed since the {noun}, so its {} are \
         whatever was last saved.",
        w.holds
    );
    let disk = format!(
        "{title} undone on open sheets; not open, so still {} on disk: {d}",
        w.done
    );
    let undo_line = match check {
        Skip::HistoryGone => Some(gone),
        Skip::Closed => Some(closed),
        Skip::DiskOnly => Some(disk),
        Skip::DroppedRedo => None,
    };
    match &undo_line {
        Some(line) => says_once(line, || app.handle_undo_requested()),
        None => app.handle_undo_requested(),
    }

    app.document_state
        .undo_groups
        .get_mut(&group)
        .unwrap()
        .dropped = vec![sheet_path(&e)];
    let dropped = format!(
        "Cannot redo the {}: {e} went back to its saved state. Run {title} again.",
        w.wide
    );
    if check == Skip::DroppedRedo {
        says_once(&dropped, || app.handle_redo_requested());
    }
}

fn for_every_kind(check: Skip) {
    for w in &KINDS {
        skipped_members_and_the_dropped_redo(w, check);
    }
}

#[test]
fn a_member_whose_history_lost_the_group_is_named_for_every_kind() {
    for_every_kind(Skip::HistoryGone);
}

#[test]
fn a_member_closed_since_the_group_is_named_for_every_kind() {
    for_every_kind(Skip::Closed);
}

#[test]
fn a_member_only_on_disk_is_named_for_every_kind() {
    for_every_kind(Skip::DiskOnly);
}

#[test]
fn a_redo_after_a_member_went_back_to_disk_is_refused_for_every_kind() {
    for_every_kind(Skip::DroppedRedo);
}

fn blocked_undo(w: &Words) {
    let [a, b] = ["a", "b"].map(short_unique);
    let (mut app, paths) = app_with_sheets(&[&b, &a]);
    group_over(&mut app, w.kind, &paths, &paths);
    edit_directly(&mut app, &paths[0]);
    let line = format!(
        "Cannot undo the {}: {b} has a later step (an edit or a net-class pick). Undo the newer \
         edit on {b} first.",
        w.wide
    );
    says_once(&line, || app.handle_undo_requested());
}

fn blocked_redo(w: &Words) {
    let [a, b] = ["a", "b"].map(short_unique);
    let (mut app, paths) = app_with_sheets(&[&b, &a]);
    group_over(&mut app, w.kind, &paths, &paths);
    app.handle_undo_requested();
    edit_directly(&mut app, &paths[0]);
    let line = format!(
        "Cannot redo the {}: {b} was edited after the undo, so the {} can't be redone across \
         sheets.",
        w.wide, w.noun
    );
    says_once(&line, || app.handle_redo_requested());
}

#[test]
fn a_blocked_grouped_undo_is_refused_with_its_reason_for_every_kind() {
    KINDS.iter().for_each(blocked_undo);
}

#[test]
fn a_blocked_grouped_redo_is_refused_with_its_reason_for_every_kind() {
    KINDS.iter().for_each(blocked_redo);
}

/// `Undone: <label>` / `Redone: <label>` after an agent's labelled step.
#[test]
fn an_agents_step_says_undone_and_redone_with_its_label() {
    let (mut app, paths) = app_with_sheets(&["agent.snxsch"]);
    let summary = short_unique("tie off spare ");
    let command = crate::keymap::AppCommandId::new("apply_edits").unwrap();
    let revision = app.document_state.engines[&paths[0]].revision();
    let operands = [
        format!("document={}", paths[0].display()),
        format!("expect_revision={revision}"),
        r#"edits=[{"id":"place_junction","args":{"at":[50.8,50.8]}}]"#.to_string(),
        format!("summary={summary}"),
    ];
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = crate::app::command::CommandArgs::parse_operands(&command, &refs).unwrap();
    let ran = app.run_operands_settled(&command, &args).0;
    assert!(
        matches!(ran, crate::app::command::CommandOutcome::Answered(_)),
        "{ran:?}"
    );
    let label = format!("Agent: {summary}");
    says_once(&format!("Undone: {label}"), || app.handle_undo_requested());
    says_once(&format!("Redone: {label}"), || app.handle_redo_requested());
}

/// Run `name` from `operands` as the operand route runs it.
fn run_operands(app: &mut Signex, name: &str, operands: &[String]) -> serde_json::Value {
    let command = crate::keymap::AppCommandId::new(name).unwrap();
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = crate::app::command::CommandArgs::parse_operands(&command, &refs).unwrap();
    match app.run_operands_settled(&command, &args).0 {
        crate::app::command::CommandOutcome::Answered(value) => value,
        crate::app::command::CommandOutcome::Ran => serde_json::Value::Null,
        other => panic!("{name} did not run: {other:?}"),
    }
}

/// An agent's edit list on `sheet` labelled `summary`; its answer.
fn summarised_edits(app: &mut Signex, sheet: &Path, summary: &str) -> serde_json::Value {
    let revision = app.document_state.engines[sheet].revision();
    run_operands(
        app,
        "apply_edits",
        &[
            format!("document={}", sheet.display()),
            format!("expect_revision={revision}"),
            r#"edits=[{"id":"place_junction","args":{"at":[50.8,50.8]}}]"#.to_string(),
            format!("summary={summary}"),
        ],
    )
}

#[test]
fn an_agents_single_command_says_its_label_and_undo_group() {
    let dir = tempfile::tempdir().unwrap();
    let name = format!("{}.snxsch", short_unique("one"));
    let sheet_at = dir.path().join(&name);
    // An agent acts on the documents of an open project.
    let mut app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir.path().to_string_lossy(),
        &[&name],
    );
    colours::add_sheet_with(&mut app, &sheet_at, sheet(200));
    app.document_state.mark_document_clean(&sheet_at);
    // The agent socket serving: a command naming its document is one step.
    app.agent.serving = true;
    let line = format!("Agent: place_junction on {name} (undo group ");
    says_once(&line, || {
        run_operands(
            &mut app,
            "place_junction",
            &[
                format!("document={}", sheet_at.display()),
                "at=50.8,50.8".to_string(),
            ],
        );
    });
}

#[test]
fn an_agents_edit_list_says_how_many_edits_landed_where() {
    let (mut app, paths) = app_with_sheets(&["agent.snxsch"]);
    let summary = short_unique("list ");
    let line = format!("Agent: {summary}: 1 edit(s) applied to agent.snxsch (undo group ");
    says_once(&line, || {
        summarised_edits(&mut app, &paths[0], &summary);
    });
}

#[test]
fn an_agents_revert_says_what_it_took_back() {
    let (mut app, paths) = app_with_sheets(&["agent.snxsch"]);
    let summary = short_unique("revert ");
    let group = summarised_edits(&mut app, &paths[0], &summary)["group"]
        .as_u64()
        .expect("a group");
    let line = format!("Reverted Agent: {summary} on agent.snxsch (undo group {group})");
    says_once(&line, || {
        run_operands(
            &mut app,
            "revert",
            &[
                format!("document={}", paths[0].display()),
                format!("group={group}"),
            ],
        );
    });
}

/// A sheet with the symbol `reference` locked at the origin and an unlocked
/// wire, active. The symbol's and the wire's selection items.
fn locked_symbol(reference: &str) -> (Signex, PathBuf, SelectedItem, SelectedItem) {
    let name = format!("{}.snxsch", short_unique("lk"));
    let (mut app, path) = colours::app_with_sheet(&name);
    let engine = app.document_state.engines.get_mut(&path).unwrap();
    let mut doc = engine.document().clone();
    doc.symbols[0].reference = reference.to_string();
    doc.symbols[0].locked = true;
    let (symbol, wire) = (doc.symbols[0].uuid, doc.wires[0].uuid);
    *engine = signex_engine::Engine::new(doc).unwrap();
    (
        app,
        path,
        SelectedItem::new(symbol, SelectedKind::Symbol),
        SelectedItem::new(wire, SelectedKind::Wire),
    )
}

fn move_items(items: Vec<SelectedItem>) -> Command {
    Command::MoveSelection {
        items,
        dx: 5.0,
        dy: 0.0,
        keep_wires: false,
        wiring: signex_types::corner::DragWiringMode::Auto,
    }
}

#[test]
fn a_locked_item_skipped_or_refused_by_the_gateway_says_so() {
    let reference = short_unique("LK");
    let skipped =
        format!("Skipped 1 locked object(s): {reference}; unlock in Properties to change them");
    let (mut app, _path, symbol, wire) = locked_symbol(&reference);
    says_once(&skipped, || {
        app.apply_engine_commands(vec![move_items(vec![symbol, wire])], true, true);
    });
    let (mut app, _path, symbol, wire) = locked_symbol(&reference);
    says_once(&skipped, || {
        app.apply_engine_command(move_items(vec![symbol, wire]), true, true);
    });
    let (mut app, _path, symbol, _wire) = locked_symbol(&reference);
    let refused =
        format!("{reference} is locked and cannot be moved; unlock it in Properties first");
    says_once(&refused, || {
        app.apply_engine_command(move_items(vec![symbol]), true, true);
    });
}

/// A junction whose id the sheet already holds: the engine refuses it with
/// `IdInUse`, whose text carries that (fresh) id.
fn taken_junction(app: &Signex, path: &Path) -> (Command, uuid::Uuid) {
    let junction = app.document_state.engines[path].document().junctions[0].clone();
    let id = junction.uuid;
    (Command::PlaceJunction { junction }, id)
}

#[test]
fn an_engine_refusal_names_its_context_in_every_tail() {
    let in_use = |id: uuid::Uuid| {
        format!("an object with id {id} is already in the sheet; the new object needs a fresh id")
    };

    let (mut app, path) = colours::app_with_sheet(&format!("{}.snxsch", short_unique("er")));
    let (command, id) = taken_junction(&app, &path);
    let batch = format!("Engine command execution failed: {}", in_use(id));
    says_once(&batch, || {
        app.apply_engine_commands(vec![command], true, true);
    });

    let (command, id) = taken_junction(&app, &path);
    let single = format!("Engine command failed: {}", in_use(id));
    says_once(&single, || {
        app.apply_engine_command(command, true, true);
    });

    let (mut app, path) = colours::app_with_sheet(&format!("{}.snxsch", short_unique("er")));
    let (command, id) = taken_junction(&app, &path);
    let by_path = format!("Engine command failed: {}", in_use(id));
    says_once(&by_path, || {
        assert!(app.execute_on_sheet(&path, command).is_err());
    });
}

#[test]
fn an_edit_on_a_read_only_sheet_is_refused_once_with_its_name() {
    let name = format!("{}.snxsch", short_unique("ro"));
    let mut doc = colours::fixture_sheet();
    doc.unknown.newer_minor_source = Some("snxsch/1.7".into());
    let (mut app, _task) = Signex::new();
    colours::add_sheet_with(&mut app, &sheet_path(&name), doc);
    let line = format!(
        "Not changed: {name} is read-only, written by a newer Signex (snxsch/1.7). Edit it in \
         that version."
    );
    says_once(&line, || {
        assert!(!app.apply_engine_command(paper("A3"), true, true));
        assert!(!app.apply_engine_command(paper("A2"), true, true));
    });
}

// ---- Engine warnings and notices, through every tail.

fn tp_symbol(sheet: &mut SchematicSheet, reference: &str, at: (f64, f64)) -> uuid::Uuid {
    let uuid = uuid::Uuid::now_v7();
    sheet.symbols.push(Symbol {
        uuid,
        lib_id: "TP".to_string(),
        reference: reference.to_string(),
        position: Point::new(at.0, at.1),
        unit: 1,
        ..Default::default()
    });
    uuid
}

fn wire(sheet: &mut SchematicSheet, a: (f64, f64), b: (f64, f64)) {
    sheet.wires.push(Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        ..Default::default()
    });
}

fn one_pin_part() -> LibSymbol {
    LibSymbol {
        id: "TP".to_string(),
        reference: String::new(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// The engine's own "kept pin" case: wires of two nets overlap at (40..60,
/// 40); `reference`'s pin, wired to J3, moved onto that overlap with "keep
/// wires" raises one legacy warning and one typed notice, both naming it.
fn crossing_sheet(reference: &str) -> (SchematicSheet, uuid::Uuid) {
    let mut sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.lib_symbols.insert("TP".to_string(), one_pin_part());
    tp_symbol(&mut sheet, "J1", (0.0, 40.0));
    tp_symbol(&mut sheet, "J2", (100.0, 40.0));
    wire(&mut sheet, (0.0, 40.0), (60.0, 40.0));
    wire(&mut sheet, (40.0, 40.0), (100.0, 40.0));
    let part = tp_symbol(&mut sheet, reference, (50.0, 10.0));
    tp_symbol(&mut sheet, "J3", (20.0, 10.0));
    wire(&mut sheet, (20.0, 10.0), (50.0, 10.0));
    (sheet, part)
}

fn kept_move(part: uuid::Uuid) -> Command {
    Command::MoveSelection {
        items: vec![SelectedItem::new(part, SelectedKind::Symbol)],
        dx: 0.0,
        dy: 30.0,
        keep_wires: true,
        wiring: signex_types::corner::DragWiringMode::Auto,
    }
}

#[test]
fn an_engine_warning_and_notice_reach_messages_through_every_tail() {
    type Tail = fn(&mut Signex, &Path, Command);
    let tails: [(&str, Tail); 3] = [
        ("batch", |app, _, command| {
            assert!(app.apply_engine_commands(vec![command], true, true));
        }),
        ("single", |app, _, command| {
            assert!(app.apply_engine_command(command, true, true));
        }),
        ("by path", |app, path, command| {
            assert_eq!(app.execute_on_sheet(path, command), Ok(true));
        }),
    ];
    for (tail, run) in tails {
        let reference = short_unique("TP");
        let (doc, part) = crossing_sheet(&reference);
        let (mut app, _task) = Signex::new();
        let path = sheet_path(&format!("{}.snxsch", short_unique("nt")));
        colours::add_sheet_with(&mut app, &path, doc);
        let warning =
            format!("could not keep {reference} pin 1 connected without crossing another net");
        let (warned, named) = (logged_count(&warning), logged_count(&reference));
        run(&mut app, &path, kept_move(part));
        assert_eq!(
            logged_count(&warning),
            warned + 1,
            "{tail}: the legacy warning"
        );
        assert_eq!(
            logged_count(&reference),
            named + 2,
            "{tail}: the warning and the typed notice both name the pin"
        );
    }
}
