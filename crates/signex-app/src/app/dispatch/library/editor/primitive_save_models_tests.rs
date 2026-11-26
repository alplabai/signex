//! #3107: saving a footprint file whose model its library keeps only in
//! `step/<hash>.step` embeds the model; the `step/` file stays.

use crate::app::FootprintDocument;
use crate::app::core::new_library_defaults::NewLibraryDefaults;
use crate::app::handlers::menu::export::tests::app_workspace;
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
