//! The template library file on disk (#1291): no file reads as an empty
//! library, a save writes it and keeps a newer build's keys, and a file that
//! does not read as a library is refused and left as it is.

use serde_json::json;
use signex_types::project::{
    ChecklistStage, SavedTemplate, SavedTemplateItem, TEMPLATE_LIBRARY_FILE, TemplateLibrary,
};

use super::{read_library_file, save_into_library_file};

fn template(name: &str, title: &str) -> SavedTemplate {
    SavedTemplate {
        name: name.into(),
        saved_from: None,
        items: vec![SavedTemplateItem {
            stage: ChecklistStage::Fabrication,
            title: title.into(),
            link: None,
            unknown: Default::default(),
        }],
        unknown: Default::default(),
    }
}

#[test]
fn the_library_file_is_read_saved_and_never_written_over() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config").join(TEMPLATE_LIBRARY_FILE);
    assert_eq!(read_library_file(&path), Ok(TemplateLibrary::default()));
    save_into_library_file(&path, template("A", "one")).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["synced_at"] = json!("2026-11-01");
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let library = save_into_library_file(&path, template("A", "two")).unwrap();
    assert_eq!(library.templates.len(), 1, "same name: replaced");
    assert_eq!(read_library_file(&path).unwrap(), library);
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(written["synced_at"], "2026-11-01");

    std::fs::write(&path, b"[not a library").unwrap();
    assert!(save_into_library_file(&path, template("C", "four")).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"[not a library");
}
