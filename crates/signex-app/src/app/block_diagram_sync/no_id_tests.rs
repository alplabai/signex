//! #2968: a sheet that would be a block but has no document id in its file
//! refuses generate, regenerate and the tab's Re-sync, named, and nothing is
//! written. An open sheet whose id was given when it opened and is not
//! saved counts as having none (DECISIONS, "Block diagram from a hierarchy
//! whose sheets hold no document id").

use std::path::Path;

use signex_types::block_diagram::read_block_diagram;

use super::job::SyncMode;
use super::tests::{job, plain, save, write_hierarchy};
use crate::app::project_sheets::load_sheet_file;

/// Replace the sheet at `path` with the corpus leaf, whose `schematic_id` is
/// the nil uuid, as a file no build has saved since ids came in.
fn without_id(path: &Path) {
    let leaf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/schematic/hierarchy-child-leaf.snxsch");
    std::fs::copy(leaf, path).expect("the nil-id sheet");
}

#[test]
fn an_open_sheet_whose_id_is_not_saved_refuses_and_once_saved_gives_its_block() {
    let dir = tempfile::tempdir().expect("dir");
    write_hierarchy(dir.path(), &[("Alpha", "a.snxsch")]);
    let alpha = dir.path().join("a.snxsch");
    without_id(&alpha);
    // Open, it holds an id given in memory that its file does not have.
    let mut open = load_sheet_file(&alpha).expect("reads").expect("there");
    assert!(open.uuid.is_nil());
    open.uuid = uuid::Uuid::now_v7();
    let run = || {
        let mut sync = job(dir.path(), SyncMode::Generate { overwrite: true });
        sync.open.insert(alpha.clone(), open.clone());
        sync.run()
    };
    let refused = run().expect_err("refused while the id is not saved");
    assert_eq!(
        plain(&refused),
        "the sheet a.snxsch has no document id yet; open it and save it, then run again"
    );
    let file = dir.path().join("demo.snxblk");
    assert!(!file.exists(), "nothing written");

    std::fs::remove_file(&alpha).expect("the old file");
    save(open.clone(), &alpha);
    let answer = run().expect("generated once saved");
    let diagram = read_block_diagram(&file).expect("written").diagram;
    let linked: Vec<_> = (diagram.blocks.iter())
        .filter_map(|b| b.sheet.as_ref().map(|l| l.sheet.as_uuid()))
        .collect();
    assert_eq!(linked, [open.uuid], "{answer}");
}

#[test]
fn regenerate_with_apply_refuses_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    write_hierarchy(dir.path(), &[("Alpha", "a.snxsch"), ("Beta", "b.snxsch")]);
    job(dir.path(), SyncMode::Generate { overwrite: false })
        .run()
        .expect("generated");
    let file = dir.path().join("demo.snxblk");
    let bytes = std::fs::read(&file).expect("bytes");
    without_id(&dir.path().join("b.snxsch"));
    let refused = job(dir.path(), SyncMode::Regenerate { apply: true })
        .run()
        .expect_err("refused");
    assert_eq!(
        plain(&refused),
        "the sheet b.snxsch has no document id yet; open it and save it, then run again"
    );
    assert_eq!(
        std::fs::read(&file).expect("bytes"),
        bytes,
        "nothing written"
    );
}

/// Two sheets without an id: the `[other]` branch, with the count and the
/// first sheet in walk order.
#[test]
fn two_sheets_without_an_id_are_counted() {
    let dir = tempfile::tempdir().expect("dir");
    write_hierarchy(dir.path(), &[("Alpha", "a.snxsch"), ("Beta", "b.snxsch")]);
    without_id(&dir.path().join("a.snxsch"));
    without_id(&dir.path().join("b.snxsch"));
    let refused = job(dir.path(), SyncMode::Generate { overwrite: false })
        .run()
        .expect_err("refused");
    assert_eq!(
        plain(&refused),
        "2 sheets have no document id yet (the first is a.snxsch); \
         open and save them, then run again"
    );
    assert!(!dir.path().join("demo.snxblk").exists(), "nothing written");
}
