//! Netlist stitching under the PROBED case rule of a project's volume (#694).
//!
//! The fold is injected on the loaded project (`LoadedProject::case_fold`)
//! rather than read from the CI host's filesystem, which is case-sensitive.

use std::path::PathBuf;

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::Signex;
use crate::app::handlers::menu::export::build_export_scope;
use crate::app::state::scope::CaseFold;

fn set_fold(app: &mut Signex, fold: CaseFold) {
    app.document_state.projects[0].case_fold = fold;
}

fn is_shared_reference(issue: &signex_net::StitchIssue) -> bool {
    matches!(
        issue,
        signex_net::StitchIssue::SharedReferenceAcrossInstances { .. }
    )
}

fn is_key_collision(issue: &signex_net::StitchIssue) -> bool {
    matches!(issue, signex_net::StitchIssue::SheetKeyCollision { .. })
}

/// One file the user has open under two spellings, as on APFS: `Top.snxsch`
/// (the root) and `top.snxsch` (the same sheet, reached by a differently
/// cased reference).
fn one_file_two_spellings(fold: CaseFold) -> Signex {
    let mut app = app_workspace("/w/ci", &["Top.snxsch", "top.snxsch"]);
    set_fold(&mut app, fold);
    let sheet = sheet_with_net("R1", "NET", &[]);
    let upper = PathBuf::from("/w/ci/Top.snxsch");
    let lower = PathBuf::from("/w/ci/top.snxsch");
    open_with(&mut app.document_state, &upper, sheet.clone());
    open_with(&mut app.document_state, &lower, sheet);
    app.document_state.active_path = Some(upper);
    app
}

#[test]
fn a_case_insensitive_volume_stitches_one_sheet_for_one_file() {
    let app = one_file_two_spellings(CaseFold::from_probe(true));

    let (ctx, issues) = build_export_scope(&app.document_state).expect("scope");

    assert!(
        !issues.stitch.iter().any(is_shared_reference),
        "one file must not be instanced twice: {:?}",
        issues.stitch
    );
    assert!(
        !issues.stitch.iter().any(is_key_collision),
        "the second spelling is never loaded, so nothing collides: {:?}",
        issues.stitch
    );
    assert!(issues.messages().is_empty(), "{:?}", issues.messages());
    let netlist = ctx.netlist.expect("netlist");
    let terminals: usize = netlist.nets.iter().map(|n| n.terminals.len()).sum();
    assert_eq!(terminals, 1, "R1 pin 1 appears once, not once per spelling");
}

/// The fold is built by hand (what `from_probe(false)` yields on a Windows
/// host) so the NTFS per-directory case-sensitivity case can fail on Linux CI
/// too: it must differ from `CaseFold::FOLD`, which merges the two files.
#[test]
fn a_case_sensitive_directory_keeps_both_case_variants_as_two_sheets() {
    // Two real files on an NTFS directory with per-directory case
    // sensitivity: `A.snxsch` and `a.snxsch`, even on a Windows host.
    let mut app = app_workspace("/w/cs", &["A.snxsch", "a.snxsch"]);
    let fold = CaseFold {
        fold_case: false,
        backslash_is_separator: true,
        normalize_unicode: false,
    };
    assert_ne!(fold, CaseFold::FOLD);
    set_fold(&mut app, fold);
    open_with(
        &mut app.document_state,
        &PathBuf::from("/w/cs/A.snxsch"),
        sheet_with_net("R_UPPER", "NET_U", &[]),
    );
    open_with(
        &mut app.document_state,
        &PathBuf::from("/w/cs/a.snxsch"),
        sheet_with_net("R_LOWER", "NET_L", &[]),
    );
    app.document_state.active_path = Some(PathBuf::from("/w/cs/A.snxsch"));

    let (ctx, issues) = build_export_scope(&app.document_state).expect("scope");

    assert!(
        !issues.stitch.iter().any(is_key_collision),
        "two files, two keys: {:?}",
        issues.stitch
    );
    let netlist = ctx.netlist.expect("netlist");
    let mut names: Vec<&str> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["NET_L", "NET_U"], "neither sheet dropped out");
}

#[test]
fn a_root_named_only_in_schematic_root_assembles_and_keys_under_the_projects_fold() {
    // `Top.snxsch` is the declared root but not a listed page, and no open
    // sheet references it, so nothing but the project's own fold can say how
    // it is keyed. The assembly and the graph must still agree.
    let mut app = app_workspace("/w/rt", &["a.snxsch", "sub.snxsch"]);
    app.document_state.projects[0].data.schematic_root = Some("Top.snxsch".to_string());
    set_fold(&mut app, CaseFold::from_probe(true));
    let child = sheet_with_net("R1", "NET", &[]);
    let open = |app: &mut Signex, name: &str, sheet| {
        let path = PathBuf::from("/w/rt").join(name);
        open_with(&mut app.document_state, &path, sheet);
    };
    open(
        &mut app,
        "Top.snxsch",
        sheet_with_net("R0", "N0", &["Sub.snxsch"]),
    );
    open(&mut app, "a.snxsch", sheet_with_net("RA", "NA", &[]));
    open(&mut app, "Sub.snxsch", child.clone());
    open(&mut app, "sub.snxsch", child);
    app.document_state.active_path = Some(PathBuf::from("/w/rt/a.snxsch"));

    let (_ctx, issues) = build_export_scope(&app.document_state).expect("scope");

    assert!(
        issues.stitch.is_empty(),
        "assembled and keyed under the project's fold, one file loads once: {:?}",
        issues.stitch
    );
}
