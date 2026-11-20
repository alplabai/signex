//! The hostile-operand test of ADR-0006 slice C: every catalog id with
//! operands runs through the test adapter as an agent, with each operand in
//! turn set to junk for its `OperandKind`, the other required ones valid.
//! Each call returns `Ran` or a typed refusal and never panics. It catches
//! validation a handler loses while slices 2–6 move it into the core.
//!
//! The junk is of two sorts. Some the parser refuses (empty, out of range,
//! a malformed colour). The rest parses and so reaches the route: points at
//! the edge of the storable range, an unknown pin reference, full turns,
//! a tiny length, text with `..`, other scripts or 10 000 characters, odd
//! but valid colours, an unknown document id, paths with `..`, a folder.
//! The documents are a sheet and a board in an open project; an id runs
//! against the document its valid call accepts. A per-kind floor of calls
//! that reach a handler (past parsing and past finding the document) keeps
//! the walk from going vacuous.
//!
//! A panic found today is listed in [`KNOWN_PANICS`] with its issue; a hang
//! (the deliver limit reached) in [`KNOWN_HANGS`]. Both lists only shrink:
//! a listed pair that no longer fails fails the test, as does a new one.
//! Every document and path lives under the adapter's temporary root, and
//! the `..` paths resolve inside it.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{Origin, Outcome, RefusalKind, TestAdapter};
use crate::keymap::{AppCommandId, Operand, OperandKind, all_command_ids, metadata_for};

/// `(id, operand)` pairs that panic on junk today. Do not fix them here:
/// each has an issue, and its fix removes the row.
const KNOWN_PANICS: &[(&str, &str)] = &[];

/// `(id, operand)` pairs whose work does not finish within the limit.
const KNOWN_HANGS: &[(&str, &str)] = &[];

const DELIVER_LIMIT: Duration = Duration::from_secs(5);

/// Per kind, a floor under the junk calls that reached a handler: about half
/// of what trunk reached when this landed (point 485, angle 36, turn 9,
/// text 1591, path 66, bool 75, color 12, document 198, length 4).
const MIN_REACHED: &[(OperandKind, usize)] = &[
    (OperandKind::Point, 240),
    (OperandKind::Angle, 18),
    (OperandKind::Turn, 4),
    (OperandKind::Text, 800),
    (OperandKind::Path, 33),
    (OperandKind::Bool, 37),
    (OperandKind::Color, 6),
    (OperandKind::Document, 99),
    (OperandKind::Length, 2),
];

/// Refusals given before any handler reads the operands: the document was
/// not found, is not open, is of the other kind, or is in no project.
const RESOLUTION_REFUSALS: &[&str] = &[
    "no document has the id",
    "is not open",
    "not part of an open project",
    "is a schematic, not a board",
    "is a board, not a schematic",
];

/// The open project's sheet and board.
struct Fixture {
    adapter: TestAdapter,
    sheet: PathBuf,
    board: PathBuf,
}

fn fixture() -> Fixture {
    let mut adapter = TestAdapter::with_deliver_limit(DELIVER_LIMIT);
    let dir = adapter.root().join("P");
    std::fs::create_dir_all(&dir).expect("the project folder");
    let sheet = dir.join("hostile.snxsch");
    let board = dir.join("hostile.snxpcb");
    std::fs::write(&sheet, sheet_text()).expect("write the sheet");
    std::fs::write(&board, board_text()).expect("write the board");
    let project = dir.join("P.snxprj");
    signex_types::project::write_project(&project, &project_data(&dir)).expect("write the project");
    open(&mut adapter, &project);
    for path in [&sheet, &board] {
        let read = adapter.open_from_tree(path).expect("the tree opens it");
        adapter.deliver(read).expect("the read finishes");
    }
    Fixture {
        adapter,
        sheet,
        board,
    }
}

fn open(adapter: &mut TestAdapter, path: &Path) {
    let open = format!("path={}", path.display());
    let (outcome, pending) = adapter.execute("open_document", &[&open], Origin::Person);
    assert_eq!(
        outcome,
        Outcome::Ran,
        "the fixture opens {}",
        path.display()
    );
    adapter.deliver(pending).expect("the open finishes");
}

fn project_data(dir: &Path) -> signex_types::project::ProjectData {
    signex_types::project::ProjectData {
        document_id: None,
        description: None,
        name: "P".into(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("hostile.snxsch".into()),
        pcb_file: Some("hostile.snxpcb".into()),
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        fab_outputs: None,
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

fn sheet_text() -> String {
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode the sheet")
}

fn board_text() -> String {
    let board = signex_types::pcb::PcbBoard {
        sketch: None,
        stackup: None,
        options: Default::default(),
        unknown: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
        free: Default::default(),
    };
    signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("encode the board")
}

/// A value of `kind` a handler accepts the form of.
fn valid(kind: OperandKind, document: &Path) -> String {
    match kind {
        OperandKind::Point => "10.16,10.16".into(),
        OperandKind::Angle | OperandKind::Turn => "90".into(),
        OperandKind::Text => "x".into(),
        OperandKind::Path | OperandKind::Document => document.display().to_string(),
        OperandKind::Bool => "true".into(),
        OperandKind::Color => "#ff0000".into(),
        OperandKind::Length => "1.27".into(),
    }
}

/// The junk values of `kind`: first what the parser refuses, then what it
/// lets through to the route.
fn junk(kind: OperandKind, root: &Path) -> Vec<String> {
    let dotted = root.join("P").join("a").join("..").join("..");
    let mut values = vec![String::new()];
    let mut add = |more: &[&str]| values.extend(more.iter().map(|value| value.to_string()));
    match kind {
        OperandKind::Point => add(&[
            "1e300,-1e300",
            "9000000000,-9000000000",
            "-9000000000,9000000000",
            "0,0",
            "Z9.99",
            "Nowhere.SDA",
        ]),
        OperandKind::Angle => add(&["45", "360", "-360", "0"]),
        OperandKind::Turn => add(&["721", "360", "-360", "0.001"]),
        OperandKind::Length => add(&["-1", "0.000001", "1e9"]),
        OperandKind::Color => add(&["#gg0000", "#000000", "ffffff"]),
        OperandKind::Bool => add(&["maybe", "false"]),
        OperandKind::Text => {
            values.extend(["..", "../../x", "Ω☃𝔘 ﷽"].map(String::from));
            values.push("x".repeat(10_000));
        }
        OperandKind::Path => {
            values.push(dotted.join("x.snxsch").display().to_string());
            values.push(dotted.join("P").join("Ω.snxsch").display().to_string());
            values.push(root.display().to_string());
        }
        OperandKind::Document => {
            values.push("00000000-0000-7000-8000-000000000000".into());
            values.push(
                dotted
                    .join("P")
                    .join("hostile.snxsch")
                    .display()
                    .to_string(),
            );
        }
    }
    values
}

/// What one call did, when it was not `Ran` or a typed refusal.
enum Failure {
    Panic(String),
    Hang,
}

/// Run `id` once with `args`: what it did, or a panic or a hang.
fn call(fixture: &mut Fixture, id: &str, args: &[String]) -> Result<Outcome, Failure> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let ran = catch_unwind(AssertUnwindSafe(|| {
        let (outcome, pending) = fixture.adapter.execute(id, &args, Origin::Agent);
        fixture.adapter.deliver(pending).map(|()| outcome)
    }));
    match ran {
        Ok(Ok(outcome)) => Ok(outcome),
        Ok(Err(_)) => Err(Failure::Hang),
        Err(payload) => Err(Failure::Panic(panic_text(payload.as_ref()))),
    }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic without a message".into())
}

/// Whether a call got to a handler: past parsing and past finding its
/// document.
fn reached_a_handler(outcome: &Outcome) -> bool {
    match outcome {
        Outcome::Ran => true,
        Outcome::Refused {
            kind: RefusalKind::Disabled,
            reason_en,
        } => !RESOLUTION_REFUSALS
            .iter()
            .any(|refusal| reason_en.contains(refusal)),
        Outcome::Refused { .. } => false,
    }
}

#[derive(Default)]
struct Found {
    panics: BTreeSet<(&'static str, &'static str)>,
    hangs: BTreeSet<(&'static str, &'static str)>,
    report: Vec<String>,
    /// Per kind: calls, calls that reached a handler, calls that ran.
    counts: BTreeMap<&'static str, (usize, usize, usize)>,
}

impl Found {
    fn record(&mut self, id: &'static str, operand: &'static str, what: String, failure: Failure) {
        match failure {
            Failure::Panic(message) => {
                self.panics.insert((id, operand));
                self.report.push(format!("PANIC {what}: {message}"));
            }
            Failure::Hang => {
                self.hangs.insert((id, operand));
                self.report.push(format!("HANG {what}"));
            }
        }
    }

    fn count(&mut self, kind: OperandKind, outcome: &Outcome) {
        let entry = self.counts.entry(kind.name()).or_default();
        entry.0 += 1;
        entry.1 += usize::from(reached_a_handler(outcome));
        entry.2 += usize::from(*outcome == Outcome::Ran);
    }
}

/// The arguments of `id`: every required operand valid, and `bad` set to
/// `value`.
fn arguments(operands: &[Operand], bad: &Operand, value: &str, document: &Path) -> Vec<String> {
    operands
        .iter()
        .filter(|op| op.required || op.name == bad.name)
        .map(|op| {
            let text = if op.name == bad.name {
                value.to_string()
            } else {
                valid(op.kind, document)
            };
            format!("{}={text}", op.name)
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Sheet,
    Board,
}

impl Target {
    fn path(self, fixture: &Fixture) -> PathBuf {
        match self {
            Self::Sheet => fixture.sheet.clone(),
            Self::Board => fixture.board.clone(),
        }
    }
}

/// The documents `id` accepts: the ones its all-valid call reaches a
/// handler for. Both when it reaches one for neither or both.
fn targets(id: &str, operands: &[Operand]) -> Vec<Target> {
    let both = [Target::Sheet, Target::Board];
    let accepted: Vec<Target> = both
        .into_iter()
        .filter(|target| {
            let mut fixture = fixture();
            let document = target.path(&fixture);
            let args = operands
                .iter()
                .filter(|op| op.required)
                .map(|op| format!("{}={}", op.name, valid(op.kind, &document)))
                .collect::<Vec<_>>();
            call(&mut fixture, id, &args).map_or(true, |outcome| reached_a_handler(&outcome))
        })
        .collect();
    if accepted.is_empty() {
        both.to_vec()
    } else {
        accepted
    }
}

/// Every operand of `id` set to each junk value against the documents it
/// accepts.
fn walk(id: &'static str, found: &mut Found) {
    let command = AppCommandId::new(id).expect("catalog id");
    let operands = metadata_for(&command).map_or(&[][..], |meta| meta.operands);
    if operands.is_empty() {
        return;
    }
    for target in targets(id, operands) {
        let mut fixture = fixture();
        for bad in operands {
            for value in junk(bad.kind, fixture.adapter.root()) {
                let document = target.path(&fixture);
                let args = arguments(operands, bad, &value, &document);
                match call(&mut fixture, id, &args) {
                    Ok(outcome) => found.count(bad.kind, &outcome),
                    Err(failure) => {
                        let shown: String = value.chars().take(40).collect();
                        let what = format!("{id} {}={shown:?} on {}", bad.name, document.display());
                        found.record(id, bad.name, what, failure);
                        // The app may be broken past this point; start again.
                        fixture = self::fixture();
                    }
                }
            }
        }
    }
}

#[test]
fn every_id_survives_junk_in_each_operand() {
    let _ = crate::diagnostics::init_logging();
    let mut found = Found::default();
    for id in all_command_ids() {
        walk(id, &mut found);
    }
    for line in &found.report {
        eprintln!("{line}");
    }
    for (kind, (calls, reached, ran)) in &found.counts {
        eprintln!("hostile {kind}: {calls} calls, {reached} reached a handler, {ran} ran");
    }
    for (kind, floor) in MIN_REACHED {
        let reached = found.counts.get(kind.name()).map_or(0, |count| count.1);
        assert!(
            reached >= *floor,
            "only {reached} `{}` junk calls reached a handler (floor {floor})",
            kind.name()
        );
    }
    let known_panics: BTreeSet<_> = KNOWN_PANICS.iter().copied().collect();
    let known_hangs: BTreeSet<_> = KNOWN_HANGS.iter().copied().collect();
    assert_eq!(
        found.panics,
        known_panics,
        "the panics changed (a fixed one leaves KNOWN_PANICS; a new one is a regression):\n{}",
        found.report.join("\n")
    );
    assert_eq!(
        found.hangs,
        known_hangs,
        "the hangs changed:\n{}",
        found.report.join("\n")
    );
}
