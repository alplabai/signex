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
//! The documents are a sheet and a board in an open project. An id runs
//! against the document its all-valid call reaches a handler for, with that
//! document's tab active and `document=` given wherever the id takes it.
//! A call reached a handler when it ran, or when its refusal is neither an
//! enablement gate's (`Needs::reason` over the whole catalog) nor one the
//! document resolver gives for that id (probed with no document, the other
//! kind and an unknown id); a junk `document` counts only when it ran. A
//! per-kind floor of such calls keeps the walk from going vacuous.
//!
//! Control: with `assert!(value.as_deref() != Some(".."))` before
//! `app.set_sheet_variable(..)` in `placed_operands.rs`, this test fails.
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
/// of what trunk reached when this landed (point 141, angle 12, turn 3,
/// text 752, path 32, bool 29, color 4, document 65, length 2). The only
/// length operand, `stitch_vias pitch`, reaches its handler through the
/// fixture board's net `GND`.
const MIN_REACHED: &[(OperandKind, usize)] = &[
    (OperandKind::Point, 70),
    (OperandKind::Angle, 6),
    (OperandKind::Turn, 1),
    (OperandKind::Text, 370),
    (OperandKind::Path, 16),
    (OperandKind::Bool, 14),
    (OperandKind::Color, 2),
    (OperandKind::Document, 32),
    (OperandKind::Length, 1),
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
        sheets: vec![signex_types::project::SheetEntry {
            name: "hostile".into(),
            filename: "hostile.snxsch".into(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        }],
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
        schematic_design: Default::default(),
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
        // A wire through the valid point, for the commands that need one.
        wires: vec![signex_types::schematic::Wire {
            uuid: uuid::Uuid::now_v7(),
            start: signex_types::schematic::Point::new(10.16, 10.16),
            end: signex_types::schematic::Point::new(30.48, 10.16),
            stroke_width: 0.0,
            color: None,
            locked: false,
            unknown_columns: Default::default(),
        }],
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
        // A net the area tools (stitch_vias) can name.
        nets: vec![signex_types::pcb::NetDef {
            number: 1,
            name: "GND".into(),
        }],
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

/// Optional operands given a valid value on every call: the document, the
/// area `start` / `end` a board tool needs without a zone `id`, and the net
/// it acts on.
const ALWAYS_GIVEN: &[&str] = &["document", "start", "end", "net"];

/// A valid value for `op` in the fixture: what its name needs (a board
/// layer, an area corner, the board's net), else the form of its kind.
fn valid_for(op: &Operand, document: &Path) -> String {
    match op.name {
        "layer" => "TopCopper".into(),
        "net" => "GND".into(),
        "start" => "0,0".into(),
        "end" => "20.32,20.32".into(),
        _ => valid(op.kind, document),
    }
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

/// The refusals that say a call never reached a handler: every enablement
/// gate's reason in the catalog (`Needs::reason`), and what the document
/// resolution says for the id at hand, taken from the resolver itself by
/// probing it (no document, the other kind, an unknown id).
struct NotReached {
    gates: BTreeSet<&'static str>,
    resolution: BTreeSet<String>,
}

impl NotReached {
    /// Whether `outcome`, of a call whose junk sat in `bad`, reached a
    /// handler: it ran, or a handler refused it. A junk `document` is the
    /// resolver's to refuse, so only a run counts there.
    fn reached(&self, outcome: &Outcome, bad: &Operand) -> bool {
        match outcome {
            Outcome::Ran => true,
            _ if bad.kind == OperandKind::Document => false,
            _ => self.handler_refused(outcome),
        }
    }

    /// A refusal no gate and no resolution gives.
    fn handler_refused(&self, outcome: &Outcome) -> bool {
        match outcome {
            Outcome::Ran => false,
            Outcome::Refused {
                kind: RefusalKind::Disabled,
                reason_en,
            } => {
                !self.gates.iter().any(|gate| reason_en.contains(gate))
                    && !self.resolution.contains(reason_en)
            }
            Outcome::Refused { .. } => false,
        }
    }
}

/// Every enablement gate's reason, over the whole catalog.
fn gate_reasons() -> BTreeSet<&'static str> {
    all_command_ids()
        .filter_map(|id| AppCommandId::new(id).ok())
        .filter_map(|command| crate::app::command::enablement_gate(&command))
        .map(|gate| gate.reason())
        .collect()
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

    fn count(&mut self, bad: &Operand, outcome: &Outcome, not: &NotReached) {
        let entry = self.counts.entry(bad.kind.name()).or_default();
        entry.0 += 1;
        entry.1 += usize::from(not.reached(outcome, bad));
        entry.2 += usize::from(*outcome == Outcome::Ran);
    }
}

/// The arguments of `id`: every required operand and `document` valid,
/// then `bad` set to `value` (`None`: left out).
fn arguments(operands: &[Operand], bad: Option<(&Operand, &str)>, document: &Path) -> Vec<String> {
    operands
        .iter()
        .filter_map(|op| match bad {
            Some((bad, value)) if bad.name == op.name => Some(format!("{}={value}", op.name)),
            _ if op.required || ALWAYS_GIVEN.contains(&op.name) => {
                Some(format!("{}={}", op.name, valid_for(op, document)))
            }
            _ => None,
        })
        .collect()
}

/// Run `args` on `target`, its tab active as a person would have it.
fn call_on(
    fixture: &mut Fixture,
    target: Target,
    id: &str,
    args: &[String],
) -> Result<Outcome, Failure> {
    let document = target.path(fixture);
    fixture
        .adapter
        .activate(&document)
        .expect("the target's tab is open");
    call(fixture, id, args)
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

    fn other(self) -> Self {
        match self {
            Self::Sheet => Self::Board,
            Self::Board => Self::Sheet,
        }
    }
}

/// What the resolver says for `id` on `target` when the document is left
/// out, of the other kind, or unknown: refusals before any handler.
fn resolution_refusals(
    id: &'static str,
    operands: &[Operand],
    target: Target,
    found: &mut Found,
) -> BTreeSet<String> {
    let mut fixture = fixture();
    let valid_args = arguments(operands, None, &target.path(&fixture));
    let without: Vec<String> = valid_args
        .iter()
        .filter(|arg| !arg.starts_with("document="))
        .cloned()
        .collect();
    let mut probes = vec![without.clone()];
    if operands.iter().any(|op| op.name == "document") {
        let other = target.other().path(&fixture).display().to_string();
        for document in [other.as_str(), "00000000-0000-7000-8000-000000000000"] {
            let mut args = without.clone();
            args.push(format!("document={document}"));
            probes.push(args);
        }
    }
    let mut refusals = BTreeSet::new();
    for args in probes {
        match call_on(&mut fixture, target, id, &args) {
            Ok(Outcome::Refused { reason_en, .. }) => {
                refusals.insert(reason_en);
            }
            Ok(Outcome::Ran) => {}
            Err(failure) => {
                found.record(id, "document", format!("{id} probe {args:?}"), failure);
                fixture = self::fixture();
            }
        }
    }
    refusals
}

/// The documents `id` accepts: the ones its all-valid call reaches a
/// handler for, each with its not-reached set. Both when it reaches one
/// for neither. A probe that panics or hangs is recorded, not accepted.
fn targets(
    id: &'static str,
    operands: &[Operand],
    gates: &BTreeSet<&'static str>,
    found: &mut Found,
) -> Vec<(Target, NotReached)> {
    let mut all = Vec::new();
    let mut accepted = Vec::new();
    for target in [Target::Sheet, Target::Board] {
        let mut not = NotReached {
            gates: gates.clone(),
            resolution: resolution_refusals(id, operands, target, found),
        };
        let mut fixture = fixture();
        let args = arguments(operands, None, &target.path(&fixture));
        match call_on(&mut fixture, target, id, &args) {
            Ok(Outcome::Ran) => accepted.push((target, not)),
            Ok(Outcome::Refused { reason_en, .. }) => {
                // What the all-valid call is refused for, the junk did not
                // cause: on this target it never counts as reached.
                let handler = not.handler_refused(&Outcome::Refused {
                    kind: RefusalKind::Disabled,
                    reason_en: reason_en.clone(),
                });
                not.resolution.insert(reason_en);
                if handler {
                    accepted.push((target, not));
                } else {
                    all.push((target, not));
                }
            }
            Err(failure) => found.record(id, "", format!("{id} valid call {args:?}"), failure),
        }
    }
    if accepted.is_empty() { all } else { accepted }
}

/// Every operand of `id` set to each junk value against the documents it
/// accepts.
fn walk(id: &'static str, gates: &BTreeSet<&'static str>, found: &mut Found) {
    let command = AppCommandId::new(id).expect("catalog id");
    let operands = metadata_for(&command).map_or(&[][..], |meta| meta.operands);
    if operands.is_empty() {
        return;
    }
    for (target, not) in targets(id, operands, gates, found) {
        let mut fixture = fixture();
        for bad in operands {
            for value in junk(bad.kind, fixture.adapter.root()) {
                let document = target.path(&fixture);
                let args = arguments(operands, Some((bad, &value)), &document);
                match call_on(&mut fixture, target, id, &args) {
                    Ok(outcome) => found.count(bad, &outcome, &not),
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
    let gates = gate_reasons();
    for id in all_command_ids() {
        walk(id, &gates, &mut found);
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
