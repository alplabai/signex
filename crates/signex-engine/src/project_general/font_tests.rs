//! #1178: a Fonts edit is one undo step of the General history, a refused
//! one records nothing, and undo of another section leaves the fonts alone.

use signex_types::project::{
    FontEmbedding, FontFace, ProjectData, ProjectFontProblem, project_font_asset_path,
};

use super::*;

fn project() -> ProjectData {
    serde_json::from_str(r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#).unwrap()
}

fn add(family: &str, sha: &str) -> ProjectFontCommand {
    ProjectFontCommand::AddFile {
        path: project_font_asset_path(sha, "ttf"),
        faces: vec![FontFace {
            family: family.into(),
            style: "Regular".into(),
        }],
        embedding: FontEmbedding::Installable,
    }
}

fn families(data: &ProjectData) -> Vec<String> {
    data.project_fonts()
        .unwrap()
        .into_iter()
        .map(|entry| entry.family)
        .collect()
}

#[test]
fn a_font_edit_is_one_undo_step_and_a_refused_one_records_nothing() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    assert_eq!(history.apply_font(&mut data, &add("House", "aa")), Ok(true));
    let license = ProjectFontCommand::SetLicense {
        family: "house".into(),
        license: "OFL-1.1".into(),
    };
    assert_eq!(history.apply_font(&mut data, &license), Ok(true));
    let refused = ProjectFontCommand::Remove {
        family: "Other".into(),
    };
    assert_eq!(
        history.apply_font(&mut data, &refused),
        Err(ProjectFontProblem::NoSuchFamily {
            family: "Other".into()
        })
    );
    assert!(history.undo(&mut data), "the refused edit recorded no step");
    assert_eq!(data.project_font_family("House").unwrap().license, None);
    assert!(history.undo(&mut data));
    assert_eq!(data.project_fonts_raw(), None);
    assert!(history.redo(&mut data));
    assert!(history.redo(&mut data));
    assert_eq!(
        data.project_font_family("House")
            .unwrap()
            .license
            .as_deref(),
        Some("OFL-1.1")
    );
}

#[test]
fn listing_a_file_twice_changes_nothing_and_records_nothing() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    assert_eq!(history.apply_font(&mut data, &add("House", "aa")), Ok(true));
    assert_eq!(
        history.apply_font(&mut data, &add("House", "aa")),
        Ok(false)
    );
    assert!(history.undo(&mut data));
    assert!(!history.can_undo());
}

#[test]
fn undo_and_redo_walk_a_description_and_a_font_in_order() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    assert!(history.apply(
        &mut data,
        ProjectGeneralCommand::SetDescription("Rev B".into()),
    ));
    history.close_run();
    assert_eq!(history.apply_font(&mut data, &add("House", "aa")), Ok(true));
    assert!(history.undo(&mut data), "the font step");
    assert!(history.undo(&mut data), "the description step");
    assert_eq!(data.description, None);
    assert!(families(&data).is_empty());
    assert!(history.redo(&mut data));
    assert!(history.redo(&mut data));
    assert_eq!(families(&data), ["House"]);
}
