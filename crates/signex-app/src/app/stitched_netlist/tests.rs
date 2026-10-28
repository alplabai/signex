use std::collections::HashMap;
use std::path::PathBuf;

use uuid::Uuid;

use super::*;
use crate::app::state::scope::CaseFold;

/// The Messages panel keeps only this many leading bytes of a line.
const PANEL_CUT: usize = 157;

fn head(line: &str) -> &str {
    let mut end = PANEL_CUT.min(line.len());
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    &line[..end]
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("signex-stitched-{tag}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    dir
}

#[test]
fn an_unparsable_sheet_keeps_its_name_in_the_cut_even_with_a_long_error() {
    let path = PathBuf::from("/a/very/long/project/directory/for/a/board/child.snxsch");
    let why = format!("could not be parsed ({})", "x".repeat(400));

    let line = unreadable_message(&path, &why);

    assert!(
        head(&line).starts_with("Netlist: child.snxsch could not be parsed ("),
        "name right after the reason: {line}"
    );
    assert!(line.ends_with("'/a/very/long/project/directory/for/a/board/child.snxsch'"));
    assert!(
        !line.contains("could not be read"),
        "one reason per failure"
    );
}

#[test]
fn an_unparsable_root_sheet_says_so_and_names_the_file_first() {
    let dir = scratch("root");
    let root = dir.join("top.snxsch");
    std::fs::write(&root, "this is not a schematic").expect("write root");

    let stitched = stitched_project_netlist_from(
        &HashMap::new(),
        std::slice::from_ref(&root),
        &root,
        CaseFold::host_default(),
        Some(dir.clone()),
    );
    std::fs::remove_dir_all(&dir).ok();

    assert!(stitched.netlist.is_none());
    let line = stitched
        .warnings
        .iter()
        .find(|w| w.contains("not part of the assembled project"))
        .unwrap_or_else(|| panic!("root-sheet warning missing: {:?}", stitched.warnings));
    assert!(
        head(line).starts_with("Netlist: top.snxsch is not part of the assembled project"),
        "{line}"
    );
    assert!(line.ends_with(&format!("'{}'", root.display())), "{line}");
}
