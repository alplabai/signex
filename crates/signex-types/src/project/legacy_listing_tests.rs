//! An empty or legacy `.snxprj` opens the same from an in-memory file
//! listing as from disk (#2431): the parser asks the caller, not the disk.

use super::*;

/// Writes an empty `Legacy.snxprj` plus `files` into a fresh folder, then
/// parses it from disk and from an in-memory listing of the same names.
fn both_ways(files: &[&str]) -> (ProjectData, ProjectData) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Legacy.snxprj");
    std::fs::write(&path, b"").unwrap();
    for name in files {
        std::fs::write(dir.path().join(name), b"x").unwrap();
    }
    let from_disk = parse_project(&path).unwrap();
    let in_memory = parse_project_bytes(&path, b"", |name| files.contains(&name)).unwrap();
    (from_disk, in_memory)
}

fn json(data: &ProjectData) -> serde_json::Value {
    serde_json::to_value(data).unwrap()
}

#[test]
fn legacy_project_with_sheet_and_board_matches_disk() {
    let (disk, memory) = both_ways(&["Legacy.snxsch", "Legacy.snxpcb"]);
    assert_eq!(memory.schematic_root.as_deref(), Some("Legacy.snxsch"));
    assert_eq!(memory.pcb_file.as_deref(), Some("Legacy.snxpcb"));
    assert_eq!(memory.sheets.len(), 1);
    assert_eq!(json(&memory), json(&disk));
}

#[test]
fn legacy_project_with_sheet_only_matches_disk() {
    let (disk, memory) = both_ways(&["Legacy.snxsch", "Other.snxpcb"]);
    assert_eq!(memory.pcb_file, None);
    assert_eq!(json(&memory), json(&disk));
}

#[test]
fn empty_folder_matches_disk() {
    let (disk, memory) = both_ways(&[]);
    assert!(memory.sheets.is_empty());
    assert_eq!(json(&memory), json(&disk));
}

#[test]
fn in_memory_listing_never_reads_the_disk() {
    // The folder holds nothing; the listing alone names the files.
    let path = Path::new("/nowhere/Legacy.snxprj");
    let data = parse_project_bytes(path, b"", |name| name == "Legacy.snxpcb").unwrap();
    assert_eq!(data.pcb_file.as_deref(), Some("Legacy.snxpcb"));
    assert_eq!(data.schematic_root, None);
}
