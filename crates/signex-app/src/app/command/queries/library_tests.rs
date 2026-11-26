//! #2944: `query_library` answers an open library's rows, one row by id or
//! part number, or the rows holding a text, the same in the window and in a
//! headless run; a library that is not open is refused.

use std::path::Path;

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{InternalPn, LibraryAdapter, PrimitiveRef};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;
use crate::library::component_preview::test_row;

/// A library at `path` with a resistor `R-1` and a capacitor `C-1`; its id.
fn library_on_disk(path: &Path) -> Uuid {
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "parts".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let options = LibraryInitOptions {
        enable_git: false,
        use_lfs: false,
    };
    let adapter = LocalGitAdapter::init(path, manifest, options).expect("init library");
    for (table, pn, mpn) in [("R", "R-1", "RC0603-10K"), ("C", "C-1", "CL10-100N")] {
        let mut row = test_row();
        row.row_id = Uuid::now_v7();
        row.internal_pn = InternalPn::new(pn);
        row.primary_mpn.mpn = mpn.into();
        row.symbol_ref = PrimitiveRef::new(adapter.library_id(), Uuid::now_v7());
        row.symbol_version = "0.0.1".into();
        adapter.insert_row(table, row, "seed").expect("insert row");
    }
    adapter.library_id()
}

/// An app with the library mounted, and the library's id.
fn mounted(dir: &tempfile::TempDir) -> (Signex, Uuid) {
    let root = dir.path().join("parts.snxlib");
    let id = library_on_disk(&root);
    let (mut app, _boot) = Signex::new();
    app.library.pending_mounts.clear();
    app.library.open_library(root).expect("mount");
    (app, id)
}

fn args(operands: &[&str]) -> (AppCommandId, CommandArgs) {
    let command = AppCommandId::new("query_library").expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    (command, args)
}

/// The answer through the window's query path.
fn in_window(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let (command, args) = args(operands);
    app.run_query(&command, &args).expect("a query").0
}

/// The answer through a headless run's dispatch.
fn headless(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let (command, args) = args(operands);
    app.run_headless_command_in_test(&command, args).0
}

fn answer(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn part_numbers(answer: &Value) -> Vec<String> {
    answer["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["internal_pn"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn every_row_one_row_and_a_search_answer_the_same_in_both_paths() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = mounted(&dir);
    let library = format!("library={id}");
    for operands in [
        vec![library.as_str()],
        vec![library.as_str(), "part=R-1"],
        vec![library.as_str(), "text=cl10"],
    ] {
        let window = answer(in_window(&mut app, &operands));
        let run = answer(headless(&mut app, &operands));
        assert_eq!(window, run, "{operands:?}");
    }
    let all = answer(in_window(&mut app, &[library.as_str()]));
    // Tables in name order: C before R.
    assert_eq!(part_numbers(&all), ["C-1", "R-1"]);
    assert_eq!(all["count"], 2);
    assert_eq!(all["library"]["id"], id.to_string());
    let one = answer(in_window(&mut app, &[library.as_str(), "part=R-1"]));
    assert_eq!(part_numbers(&one), ["R-1"]);
    let row = &one["rows"][0];
    assert_eq!(row["table"], "R");
    assert_eq!(row["mpn"], "RC0603-10K");
    assert!(row["state"].is_string());
    let found = answer(in_window(&mut app, &[library.as_str(), "text=cl10"]));
    assert_eq!(part_numbers(&found), ["C-1"]);
}

#[test]
fn a_library_that_is_not_open_or_a_part_it_has_not_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = mounted(&dir);
    let other = format!("library={}", Uuid::now_v7());
    let CommandOutcome::Disabled { reason } = in_window(&mut app, &[other.as_str()]) else {
        panic!("not refused");
    };
    assert!(reason.render_en_us().contains("is not open"));
    let library = format!("library={id}");
    let CommandOutcome::Disabled { reason } = in_window(&mut app, &[library.as_str(), "part=X-9"])
    else {
        panic!("not refused");
    };
    assert!(reason.render_en_us().contains("has no part `X-9`"));
}
