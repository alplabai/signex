//! The hostile-operand test of ADR-0006 slice C: every catalog id, with each
//! operand in turn set to junk for its `OperandKind` (empty, out of range, an
//! unknown document, a path with `..`), runs through the test adapter as an
//! agent. Each call returns `Ran` or a typed refusal and never panics. It
//! catches validation a handler loses while slices 2–6 move it into the core.
//!
//! A panic found today is listed in [`KNOWN_PANICS`] with its issue; a hang
//! (the deliver limit reached) in [`KNOWN_HANGS`]. Both lists only shrink:
//! a listed pair that no longer fails fails the test, as does a new one.
//! Every document and path lives under the adapter's temporary root, and
//! the `..` paths resolve inside it.

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{Origin, Outcome, RefusalKind, TestAdapter};
use crate::keymap::{AppCommandId, OperandKind, all_command_ids, metadata_for};

/// `(id, operand)` pairs that panic on junk today. Do not fix them here:
/// each has an issue, and its fix removes the row.
const KNOWN_PANICS: &[(&str, &str)] = &[];

/// `(id, operand)` pairs whose work does not finish within the limit.
const KNOWN_HANGS: &[(&str, &str)] = &[];

const DELIVER_LIMIT: Duration = Duration::from_secs(5);

/// A floor under the calls whose junk got past operand parsing.
const MIN_PAST_PARSING: usize = 100;

/// The documents every call can name: a sheet and a board.
struct Fixture {
    adapter: TestAdapter,
    sheet: PathBuf,
    board: PathBuf,
}

fn fixture() -> Fixture {
    let mut adapter = TestAdapter::with_deliver_limit(DELIVER_LIMIT);
    let sheet = adapter.root().join("hostile.snxsch");
    let board = adapter.root().join("hostile.snxpcb");
    std::fs::write(&sheet, sheet_text()).expect("write the sheet");
    std::fs::write(&board, board_text()).expect("write the board");
    for path in [&sheet, &board] {
        let open = format!("path={}", path.display());
        let (_, pending) = adapter.execute("open_document", &[&open], Origin::Person);
        adapter.deliver(pending).expect("the fixture opens");
    }
    Fixture {
        adapter,
        sheet,
        board,
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

/// A value of `kind` a handler accepts the form of, for the operands that
/// are not the junk one.
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

/// The junk values of `kind`: empty, and out of range, an unknown document
/// or a path with `..` where the kind has one.
fn junk(kind: OperandKind, root: &Path) -> Vec<String> {
    let dotted = root.join("sandbox").join("a").join("..").join("..");
    let mut values = vec![String::new()];
    match kind {
        OperandKind::Point => values.push("1e300,-1e300".into()),
        OperandKind::Angle | OperandKind::Turn => values.push("721".into()),
        OperandKind::Length => values.push("-1".into()),
        OperandKind::Color => values.push("#gg0000".into()),
        OperandKind::Bool => values.push("maybe".into()),
        OperandKind::Text => {}
        OperandKind::Path => values.push(dotted.join("x.snxsch").display().to_string()),
        OperandKind::Document => {
            values.push("00000000-0000-7000-8000-000000000000".into());
            values.push(dotted.join("hostile.snxsch").display().to_string());
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

#[derive(Default)]
struct Found {
    panics: BTreeSet<(&'static str, &'static str)>,
    hangs: BTreeSet<(&'static str, &'static str)>,
    report: Vec<String>,
    /// Calls whose operands parsed, so a handler or a route saw the junk.
    past_parsing: usize,
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
}

/// Every operand of `id` set to each junk value, the others valid, against
/// the sheet and the board.
fn walk(id: &'static str, found: &mut Found) {
    let command = AppCommandId::new(id).expect("catalog id");
    let operands = metadata_for(&command).map_or(&[][..], |meta| meta.operands);
    if operands.is_empty() {
        return;
    }
    for target in [Target::Sheet, Target::Board] {
        let mut fixture = fixture();
        for bad in operands {
            for value in junk(bad.kind, fixture.adapter.root()) {
                let document = target.path(&fixture).to_path_buf();
                let args: Vec<String> = operands
                    .iter()
                    .filter(|op| op.required || op.name == bad.name)
                    .map(|op| {
                        let text = if op.name == bad.name {
                            value.clone()
                        } else {
                            valid(op.kind, &document)
                        };
                        format!("{}={text}", op.name)
                    })
                    .collect();
                match call(&mut fixture, id, &args) {
                    Ok(Outcome::Refused {
                        kind: RefusalKind::Operands,
                        ..
                    }) => {}
                    Ok(_) => found.past_parsing += 1,
                    Err(failure) => {
                        let what = format!("{id} {}={value:?} on the {}", bad.name, target.name());
                        found.record(id, bad.name, what, failure);
                        // The app may be broken past this point; start again.
                        fixture = self::fixture();
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Target {
    Sheet,
    Board,
}

impl Target {
    fn path(self, fixture: &Fixture) -> &Path {
        match self {
            Self::Sheet => &fixture.sheet,
            Self::Board => &fixture.board,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Sheet => "sheet",
            Self::Board => "board",
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
    eprintln!("hostile calls past operand parsing: {}", found.past_parsing);
    // Junk the parser refuses alone would make the walk vacuous.
    assert!(
        found.past_parsing >= MIN_PAST_PARSING,
        "only {} calls reached a route",
        found.past_parsing
    );
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
