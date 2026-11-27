//! The board 3D view reads a footprint's model from its footprint file
//! (#3107), else from `step/<hash>.step` beside a library written before.

use std::path::{Path, PathBuf};

use signex_library::{Footprint, FootprintFile, StepAttachment, step_model_hash};
use signex_renderer::board3d::ComponentModel;
use signex_types::part::FootprintSourceRef;
use uuid::Uuid;

use super::board_3d::read_models;
use crate::app::core::new_library_defaults::NewLibraryDefaults;

/// One triangle: the smallest STEP the importer tessellates.
const TRIANGLE_STEP: &str = "ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('simple triangle'),'2;1');
ENDSEC;
DATA;
#1 = CARTESIAN_POINT('',(0.,0.,0.));
#2 = CARTESIAN_POINT('',(1.,0.,0.));
#3 = CARTESIAN_POINT('',(0.,1.,0.));
#10 = VERTEX_POINT('',#1);
#11 = VERTEX_POINT('',#2);
#12 = VERTEX_POINT('',#3);
#20 = POLY_LOOP('',(#10,#11,#12));
#21 = FACE_OUTER_BOUND('',#20,.T.);
#30 = ADVANCED_FACE('',(#21),#999,.T.);
ENDSEC;
END-ISO-10303-21;
";

/// A library at `dir/L.snxlib` holding `file` as its footprint's envelope.
fn library_with(dir: &Path, file: &FootprintFile) -> (PathBuf, FootprintSourceRef) {
    let library = dir.join("L.snxlib");
    crate::library::commands::create_library_file(&library, &NewLibraryDefaults::default())
        .expect("library");
    let uuid = file.footprints[0].uuid;
    std::fs::create_dir_all(dir.join("footprints")).unwrap();
    std::fs::write(
        dir.join("footprints").join(format!("{uuid}.snxfpt")),
        file.to_toml_string().unwrap(),
    )
    .unwrap();
    let source = FootprintSourceRef {
        library: Uuid::from_u128(7),
        footprint: uuid,
        version: "1.0.0".into(),
    };
    (library, source)
}

fn read_one(library: PathBuf, source: FootprintSourceRef) -> Result<ComponentModel, String> {
    let mut read = read_models(std::slice::from_ref(&source), &[(source.library, library)]);
    match (read.models.pop(), read.failures.pop()) {
        (Some((_, model)), None) => Ok(model),
        (None, Some((_, why))) => Err(why),
        other => panic!("one result expected: {other:?}"),
    }
}

fn is_attached_model(model: &ComponentModel) -> bool {
    matches!(model, ComponentModel::Model(m) if !m.meshes.is_empty())
}

#[test]
fn an_embedded_model_is_read_without_a_step_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = FootprintFile::from_footprint(Footprint::empty("TRI"));
    let uuid = file.footprints[0].uuid;
    file.attach_step_model(uuid, "tri.step", TRIANGLE_STEP.as_bytes());
    let (library, source) = library_with(dir.path(), &file);
    assert!(!dir.path().join("step").exists());
    let model = read_one(library, source).expect("read");
    assert!(is_attached_model(&model), "{model:?}");
}

#[test]
fn a_model_in_the_step_folder_is_read_as_a_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let hash = step_model_hash(TRIANGLE_STEP.as_bytes());
    let mut footprint = Footprint::empty("TRI");
    footprint.step_attachment = Some(StepAttachment {
        content_hash: hash.clone(),
        filename: "tri.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    });
    let file = FootprintFile::from_footprint(footprint);
    let (library, source) = library_with(dir.path(), &file);
    std::fs::create_dir_all(dir.path().join("step")).unwrap();
    std::fs::write(
        dir.path().join("step").join(format!("{hash}.step")),
        TRIANGLE_STEP,
    )
    .unwrap();
    let model = read_one(library, source).expect("read");
    assert!(is_attached_model(&model), "{model:?}");
}

#[test]
fn a_corrupted_embedded_model_fails_naming_the_footprint() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = FootprintFile::from_footprint(Footprint::empty("TRI-BAD"));
    let uuid = file.footprints[0].uuid;
    file.attach_step_model(uuid, "tri.step", TRIANGLE_STEP.as_bytes());
    let size = TRIANGLE_STEP.len();
    let text = file.to_toml_string().unwrap().replace(
        &format!("size = {size}\n"),
        &format!("size = {}\n", size + 1),
    );
    let (library, source) = library_with(dir.path(), &file);
    std::fs::write(
        dir.path().join("footprints").join(format!("{uuid}.snxfpt")),
        text,
    )
    .unwrap();
    let why = read_one(library, source).expect_err("the model does not decode");
    assert!(why.contains("\"TRI-BAD\""), "{why}");
}
