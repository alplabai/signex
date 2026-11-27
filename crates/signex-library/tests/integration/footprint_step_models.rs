//! #3107: saving a footprint whose model a pre-#3107 library keeps only in
//! `step/<hash>.step` embeds it in the footprint file, and the `step/`
//! file stays where it was.

#![cfg(feature = "local-git")]

use signex_library::adapter::LibraryAdapter;
use signex_library::primitive::{Footprint, StepAttachment};
use signex_library::step_model_hash;

use super::local_git_rollback::init;

#[test]
fn a_footprint_save_embeds_a_step_dir_model_and_keeps_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _lib_id, adapter) = init(&dir);
    let step = b"ISO-10303-21;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n".to_vec();
    let hash = step_model_hash(&step);
    let legacy = file
        .parent()
        .unwrap()
        .join("step")
        .join(format!("{hash}.step"));
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(&legacy, &step).unwrap();

    let mut fp = Footprint::empty("OLD");
    fp.step_attachment = Some(StepAttachment {
        content_hash: hash,
        filename: "old.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    });
    adapter.save_footprint(fp.clone(), "add").unwrap();

    let envelope = adapter.read_footprint_file(fp.uuid).unwrap();
    assert_eq!(envelope.format_token(), "snxfpt/1.4");
    assert_eq!(
        envelope.step_model(&envelope.footprints[0]).unwrap(),
        Some(step.clone())
    );
    assert_eq!(
        std::fs::read(&legacy).unwrap(),
        step,
        "the step/ file stays"
    );
    assert_eq!(adapter.get_footprint(fp.uuid).unwrap(), fp);
}
