//! The netlist-incomplete prompt's list (#431, mockup row #28): one row per
//! page the netlist leaves out, built from the same issues as the warning
//! "Export anyway" logs.

use std::path::PathBuf;

use uuid::Uuid;

use super::{app_with_missing_child, open_with, sheet_with_net, workspace};
use crate::app::handlers::menu::export::ExportIssues;
use crate::app::state::{OmittedPage, OmittedReason};

fn missing(file: &str, placed_by: Option<&str>) -> OmittedPage {
    OmittedPage {
        file: file.to_owned(),
        reason: OmittedReason::FileMissing {
            placed_by: placed_by.map(str::to_owned),
        },
    }
}

#[test]
fn every_hole_is_one_row_and_a_complete_netlist_problem_is_none() {
    let issues = ExportIssues {
        stitch: vec![
            signex_net::StitchIssue::MissingChild {
                parent_path: "top.snxsch".into(),
                sheet_name: "U_usb".into(),
                filename: "usb.snxsch".into(),
            },
            // The same parent placed twice reports its hole twice.
            signex_net::StitchIssue::MissingChild {
                parent_path: "top.snxsch".into(),
                sheet_name: "U_usb".into(),
                filename: "usb.snxsch".into(),
            },
            signex_net::StitchIssue::SheetCycle {
                parent_path: "a.snxsch".into(),
                filename: "loop.snxsch".into(),
            },
            signex_net::StitchIssue::SheetKeyCollision {
                key: "x.snxsch".into(),
                kept: "/p/x.snxsch".into(),
                dropped: "/p/X.snxsch".into(),
            },
            // Loud, but the netlist is whole: not a row.
            signex_net::StitchIssue::NameCollision { name: "GND".into() },
        ],
        missing_pages: vec![PathBuf::from("/p/notes.snxsch")],
        unreadable: vec![(
            PathBuf::from("/p/sensors.snxsch"),
            "could not be read (newer format)".to_owned(),
        )],
        ..ExportIssues::default()
    };

    let rows = issues.omitted_pages();

    assert_eq!(
        rows.iter().map(|r| r.file.as_str()).collect::<Vec<_>>(),
        [
            "sensors.snxsch",
            "notes.snxsch",
            "usb.snxsch",
            "loop.snxsch",
            "X.snxsch"
        ],
        "{rows:#?}"
    );
    assert_eq!(
        rows[0].reason,
        OmittedReason::Unreadable("could not be read (newer format)".to_owned())
    );
    assert_eq!(rows[1], missing("notes.snxsch", None));
    assert_eq!(rows[2], missing("usb.snxsch", Some("U_usb")));
    match (&rows[3].reason, &rows[4].reason) {
        (OmittedReason::Stitch(cycle), OmittedReason::Stitch(collision)) => {
            assert!(
                cycle.starts_with("sheet cycle") && cycle.contains("loop.snxsch"),
                "the stitch line, without its Messages lead: {cycle}"
            );
            assert!(
                collision.contains("/p/X.snxsch") && !collision.starts_with("Netlist:"),
                "{collision}"
            );
        }
        other => panic!("cycle and collision are stitch reasons: {other:?}"),
    }
}

#[test]
fn a_symbol_without_a_name_leaves_placed_by_unknown() {
    let issues = ExportIssues {
        stitch: vec![signex_net::StitchIssue::MissingChild {
            parent_path: "top.snxsch".into(),
            sheet_name: "  ".into(),
            filename: "gone.snxsch".into(),
        }],
        ..ExportIssues::default()
    };
    assert_eq!(issues.omitted_pages(), [missing("gone.snxsch", None)]);
}

#[test]
fn the_prompt_lists_the_missing_child_it_refuses_over() {
    let mut app = app_with_missing_child("gone-rows.snxsch");
    let out = std::env::temp_dir().join(format!("signex-rows-{}.net", Uuid::now_v7()));

    let task = app.handle_export_netlist_finished(
        app.document_state.active_path.clone().expect("active"),
        Ok(out.clone()),
    );
    crate::app::handlers::menu::export::settle(&mut app, task);

    let prompt = app
        .document_state
        .netlist_incomplete_prompt
        .clone()
        .expect("the incomplete-export prompt");
    // The fixture's sheet symbol has no name, so who places it is unknown.
    assert_eq!(prompt.omitted, [missing("gone-rows.snxsch", None)]);
    assert!(
        prompt
            .messages
            .iter()
            .any(|m| m.contains("gone-rows.snxsch")),
        "the logged warning names the same page: {:?}",
        prompt.messages
    );
    assert!(!out.exists(), "nothing is written while the prompt is up");
}

#[test]
fn a_child_that_will_not_parse_is_one_unreadable_row_not_also_missing() {
    let dir = std::env::temp_dir().join(format!("signex-export-rows-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    std::fs::write(dir.join("child.snxsch"), "(this is not a schematic").expect("write child");
    let dir_str = dir.to_string_lossy().to_string();
    let mut ds = workspace(&dir_str, &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    open_with(
        &mut ds,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]),
    );
    ds.active_path = Some(top);

    let (_ctx, issues) = crate::app::handlers::menu::export::build_export_scope(&ds).expect("ctx");
    let rows = issues.omitted_pages();
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(rows.len(), 1, "{rows:#?}");
    assert_eq!(rows[0].file, "child.snxsch");
    assert!(
        matches!(&rows[0].reason, OmittedReason::Unreadable(why) if why.contains("could not be parsed")),
        "{rows:#?}"
    );
}
