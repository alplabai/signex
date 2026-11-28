//! #3107: saving a footprint file whose model its library keeps only in
//! `step/<hash>.step` embeds the model; the `step/` file stays.

use crate::app::FootprintDocument;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::library::NewLibraryDefaults;
use signex_library::{Footprint, FootprintFile, StepAttachment, step_model_hash};

#[test]
fn a_footprint_save_embeds_a_step_folder_model_and_keeps_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let library = dir.path().join("L.snxlib");
    crate::library::commands::create_library_file(&library, &NewLibraryDefaults::default())
        .expect("library");
    let step = b"ISO-10303-21;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n".to_vec();
    let hash = step_model_hash(&step);
    let legacy = dir.path().join("step").join(format!("{hash}.step"));
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(&legacy, &step).unwrap();

    let mut footprint = Footprint::empty("OLD");
    footprint.step_attachment = Some(StepAttachment {
        content_hash: hash,
        filename: "old.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    });
    let file = FootprintFile::from_footprint(footprint);
    let path = dir.path().join("footprints").join("OLD.snxfpt");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, file.to_toml_string().unwrap()).unwrap();

    let mut editor = FootprintDocument::new(path.clone(), file);
    editor.file.footprints[0].description = "edited".into();
    editor.mark_dirty();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.save_primitive_tab_blocking(&path).expect("save");

    let saved = FootprintFile::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved.format_token(), "snxfpt/1.4");
    assert_eq!(saved.footprints[0].description, "edited");
    assert_eq!(
        saved.step_model(&saved.footprints[0]).unwrap(),
        Some(step.clone())
    );
    assert_eq!(
        std::fs::read(&legacy).unwrap(),
        step,
        "the step/ file stays"
    );
}

/// Library `name` under `root` whose `step/` holds `step`, and a footprint
/// file in it attaching that model by hash only (written before #3107).
fn legacy_library(root: &std::path::Path, name: &str, step: &[u8]) -> std::path::PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(dir.join("step")).unwrap();
    crate::library::commands::create_library_file(
        &dir.join(format!("{name}.snxlib")),
        &NewLibraryDefaults::default(),
    )
    .expect("library");
    let hash = step_model_hash(step);
    std::fs::write(dir.join("step").join(format!("{hash}.step")), step).unwrap();
    let mut footprint = Footprint::empty("OLD");
    footprint.step_attachment = Some(StepAttachment {
        content_hash: hash,
        filename: "old.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    });
    let path = dir.join("footprints").join("OLD.snxfpt");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = FootprintFile::from_footprint(footprint);
    std::fs::write(&path, file.to_toml_string().unwrap()).unwrap();
    path
}

/// An app with a dirty editor on the footprint file at `path`.
fn app_editing(root: &std::path::Path, path: &std::path::Path) -> crate::app::Signex {
    let mut app = app_workspace(&root.to_string_lossy(), &[]);
    let file = FootprintFile::from_bytes(&std::fs::read(path).unwrap()).unwrap();
    let mut editor = FootprintDocument::new(path.to_path_buf(), file);
    editor.file.footprints[0].description = "edited".into();
    editor.mark_dirty();
    app.document_state
        .footprint_editors
        .insert(path.to_path_buf(), editor);
    // An open tab keeps the editor once its save lands clean.
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "OLD".into(),
        path: path.to_path_buf(),
        project_id: None,
        kind: crate::app::TabKind::FootprintEditor(path.to_path_buf()),
    });
    app
}

/// Save As `from` to `to` in another library, run to completion; the file
/// written there.
fn save_as(
    app: &mut crate::app::Signex,
    from: &std::path::Path,
    to: &std::path::Path,
) -> FootprintFile {
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    let task = app
        .handle_save_primitive_as(from, to)
        .expect("save as starts");
    for message in crate::persist::run_task::outputs(task) {
        let _ = app.update(message);
    }
    FootprintFile::from_bytes(&std::fs::read(to).expect("written")).unwrap()
}

/// Review finding: a save embeds the model on disk; the editor must hold it
/// too, or a later Save As into another library (whose `step/` lacks it)
/// writes the footprint without its model.
#[test]
fn a_save_as_after_a_migrating_save_keeps_the_model() {
    let dir = tempfile::tempdir().expect("tempdir");
    let step = b"ISO-10303-21;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n".to_vec();
    let origin = legacy_library(dir.path(), "A", &step);
    let mut app = app_editing(dir.path(), &origin);
    app.save_primitive_tab_blocking(&origin).expect("save");
    let hash = step_model_hash(&step);
    assert!(
        app.document_state.footprint_editors[&origin]
            .file
            .models
            .get(&hash)
            .is_some(),
        "the editor holds the model the save embedded"
    );
    // Only the editor's copy can carry it now.
    std::fs::remove_file(dir.path().join("A/step").join(format!("{hash}.step"))).unwrap();
    let mut editor = app
        .document_state
        .footprint_editors
        .remove(&origin)
        .unwrap();
    editor.file.footprints[0].description = "again".into();
    editor.mark_dirty();
    app.document_state
        .footprint_editors
        .insert(origin.clone(), editor);

    let target = dir.path().join("B/footprints/NEW.snxfpt");
    let _ = legacy_library(dir.path(), "B", b"unrelated");
    let saved = save_as(&mut app, &origin, &target);
    assert_eq!(saved.step_model(&saved.footprints[0]).unwrap(), Some(step));
}

/// Save As into another library embeds a model the origin library keeps
/// only in `step/`.
#[test]
fn a_save_as_into_another_library_embeds_the_origin_step_model() {
    let dir = tempfile::tempdir().expect("tempdir");
    let step = b"ISO-10303-21;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n".to_vec();
    let origin = legacy_library(dir.path(), "A", &step);
    let _ = legacy_library(dir.path(), "B", b"unrelated");
    let mut app = app_editing(dir.path(), &origin);
    let target = dir.path().join("B/footprints/NEW.snxfpt");
    let saved = save_as(&mut app, &origin, &target);
    assert_eq!(saved.format_token(), "snxfpt/1.4");
    assert_eq!(
        saved.step_model(&saved.footprints[0]).unwrap(),
        Some(step.clone())
    );
    let hash = step_model_hash(&step);
    assert!(
        dir.path()
            .join("A/step")
            .join(format!("{hash}.step"))
            .exists()
    );
}

/// Review finding: undoing past a save that embedded a model keeps the
/// model (entries are content-addressed), so a later save still has it.
#[test]
fn undo_past_a_migrating_save_keeps_the_model() {
    let dir = tempfile::tempdir().expect("tempdir");
    let step = b"ISO-10303-21;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n".to_vec();
    let hash = step_model_hash(&step);
    let origin = legacy_library(dir.path(), "A", &step);
    let mut app = app_editing(dir.path(), &origin);
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&origin)
            .unwrap();
        editor.push_history();
        editor.file.footprints[0].description = "before undo".into();
        editor.mark_dirty();
    }
    app.save_primitive_tab_blocking(&origin).expect("save");
    let editor = app
        .document_state
        .footprint_editors
        .get_mut(&origin)
        .unwrap();
    assert!(editor.undo(), "an undo step past the save");
    assert!(
        editor.file.models.get(&hash).is_some(),
        "undo keeps the model"
    );
    editor.mark_dirty();

    // Only the editor's copy can carry the model into the other library.
    std::fs::remove_file(dir.path().join("A/step").join(format!("{hash}.step"))).unwrap();
    let _ = legacy_library(dir.path(), "B", b"unrelated");
    let target = dir.path().join("B/footprints/NEW.snxfpt");
    let saved = save_as(&mut app, &origin, &target);
    assert_eq!(saved.step_model(&saved.footprints[0]).unwrap(), Some(step));
}
