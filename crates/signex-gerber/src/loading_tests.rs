use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;

use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use lib_gerber_edit::layer::LayerData;

use super::{
    GerberPrimitive, LayerType, load_autodetected_reader, load_excellon_reader,
    load_fabrication_files, load_gerber_file, load_gerber_reader, reload_layers,
};

fn generated_fixture_member(member: &str) -> Vec<u8> {
    let archive_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/gerber/generated/gerbolyze-protoboards/tht_pitch100mil_20x30.zip");
    let archive_file = File::open(archive_path).expect("generated fixture archive must exist");
    let mut archive = ZipArchive::new(archive_file).expect("fixture must be a ZIP archive");
    let mut file = archive
        .by_name(member)
        .expect("requested Gerber member must exist");
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .expect("fixture member must be readable");
    data
}

#[test]
fn loads_generated_rs274x_layer_and_extracts_geometry() {
    let bytes = generated_fixture_member("proto-F_Cu.gbr");
    let layer = load_gerber_reader("proto-F_Cu.gbr", Cursor::new(bytes))
        .expect("generated copper layer must parse");

    assert_ne!(layer.layer_type, LayerType::Drill);
    assert!(!layer.geometry.primitives.is_empty());
    assert!(
        layer
            .geometry
            .bounds
            .is_some_and(|bounds| bounds.is_finite())
    );
    assert!(
        layer
            .geometry
            .primitives
            .iter()
            .any(|primitive| matches!(primitive, GerberPrimitive::Flash { .. }))
    );
}

#[test]
fn reports_the_file_name_for_invalid_gerber_data() {
    let error = load_gerber_reader("broken.gbr", Cursor::new(b"not gerber"))
        .expect_err("invalid Gerber data must fail");

    assert_eq!(error.path, PathBuf::from("broken.gbr"));
    assert!(
        error.message.contains("parse") || error.message.contains("format"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_excellon_input_from_the_gerber_only_loader() {
    let error = load_gerber_reader("holes.drl", Cursor::new(b"M48\nM30\n"))
        .expect_err("the RS-274X loader must not accept drill input");

    assert!(error.message.contains("Excellon"));
}

#[test]
fn accepts_common_generic_gerber_extension() {
    let data = b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n";
    let layer = load_gerber_reader("generic.ger", Cursor::new(data))
        .expect("generic .ger extension must parse as RS-274X");

    assert!(!layer.geometry.primitives.is_empty());
}

#[test]
fn preserves_original_gerber_source_text_exactly() {
    let source = "%FSLAX46Y46*%\r\n%MOMM*%\r\nG04 Original spacing stays here *\r\nM02*\r\n";

    let layer = load_gerber_reader("source.gbr", Cursor::new(source.as_bytes()))
        .expect("test Gerber must parse");

    assert_eq!(layer.gerber_source(), Ok(source));
}

#[test]
fn drill_layer_reports_that_gerber_source_is_not_available() {
    let layer = load_excellon_reader("holes.drl", Cursor::new(b"M48\nMETRIC\nT01C0.8\n%\nM30\n"))
        .expect("test Excellon must parse");

    assert_eq!(
        layer.gerber_source(),
        Err("Source view is available only for Gerber layers."),
    );
}

#[test]
fn reload_reparses_changed_files_and_reports_missing_sources() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("reload.gbr");
    std::fs::write(
        &path,
        b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nM02*\n",
    )
    .expect("write initial Gerber");
    let original = load_gerber_file(&path).expect("load initial Gerber");
    let in_memory = load_gerber_reader(
        "memory.gbr",
        Cursor::new(b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nM02*\n"),
    )
    .expect("load in-memory Gerber");
    std::fs::write(
        &path,
        b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nX1000000Y0D03*\nM02*\n",
    )
    .expect("replace Gerber");

    let batch = reload_layers(vec![original, in_memory]);

    assert_eq!(batch.layers.len(), 1);
    assert_eq!(batch.layers[0].0, 0);
    assert_eq!(batch.layers[0].1.geometry.primitives.len(), 2);
    assert_eq!(batch.failures.len(), 1);
    assert_eq!(
        batch.failures[0].message,
        "layer has no disk source to reload",
    );
}

#[test]
fn autodetection_prefers_content_over_misleading_extensions() {
    let gerber = load_autodetected_reader(
        "unknown.txt",
        Cursor::new(b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nM02*\n"),
    )
    .expect("Gerber content must be detected");
    let drill = load_autodetected_reader(
        "misnamed.gbr",
        Cursor::new(b"M48\nMETRIC\nT01C0.8\n%\nG05\nT01\nX1.0Y1.0\nM30\n"),
    )
    .expect("Excellon content must override extension");

    assert!(matches!(gerber.data, LayerData::Gerber(_)));
    assert_eq!(gerber.name, "unknown.txt");
    assert!(matches!(drill.data, LayerData::Excellon(_)));
    assert_eq!(drill.name, "misnamed.gbr");
}

#[test]
fn autodetection_reports_ambiguous_and_unsupported_content() {
    let ambiguous = load_autodetected_reader(
        "ambiguous.txt",
        Cursor::new(b"M48\n%FSLAX46Y46*%\n%MOMM*%\n"),
    )
    .expect_err("mixed signatures must be ambiguous");
    let unsupported =
        load_autodetected_reader("notes.txt", Cursor::new(b"This is not fabrication data.\n"))
            .expect_err("unknown content must be unsupported");

    assert!(ambiguous.message.contains("ambiguous fabrication file"));
    assert!(unsupported.message.contains("unsupported fabrication file"));
}

#[test]
fn unified_loader_routes_job_archive_gerber_and_excellon_files() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let gerber_path = directory.path().join("layer.gbr");
    std::fs::write(
        &gerber_path,
        b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nM02*\n",
    )
    .expect("write Gerber layer");
    let job_path = directory.path().join("job-without-extension");
    std::fs::write(&job_path, br#"{"FilesAttributes":[{"Path":"layer.gbr"}]}"#)
        .expect("write Gerber job");

    let archive_path = directory.path().join("archive-without-extension");
    let archive_file = File::create(&archive_path).expect("create ZIP archive");
    let mut archive = ZipWriter::new(archive_file);
    archive
        .start_file("holes.drl", SimpleFileOptions::default())
        .expect("start Excellon member");
    archive
        .write_all(b"M48\nMETRIC\nT01C0.8\n%\nG05\nT01\nX1.0Y1.0\nM30\n")
        .expect("write Excellon member");
    archive.finish().expect("finish ZIP archive");

    let batch = load_fabrication_files([job_path, archive_path]);

    assert!(batch.failures.is_empty());
    assert_eq!(batch.layers.len(), 2);
    assert!(matches!(batch.layers[0].data, LayerData::Gerber(_)));
    assert!(matches!(batch.layers[1].data, LayerData::Excellon(_)));
}

#[test]
fn unified_loader_rejects_mismatched_container_extensions() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let zip_path = directory.path().join("not-an-archive.zip");
    std::fs::write(&zip_path, b"plain text").expect("write false ZIP");
    let job_path = directory.path().join("not-a-job.gbrjob");
    std::fs::write(&job_path, br#"{"Unrelated":[]}"#).expect("write false Gerber job");

    let batch = load_fabrication_files([zip_path, job_path]);

    assert!(batch.layers.is_empty());
    assert_eq!(batch.failures.len(), 2);
    assert!(
        batch.failures[0]
            .message
            .contains("ZIP signature is missing")
    );
    assert!(batch.failures[1].message.contains("FilesAttributes"));
}

#[test]
fn loads_generated_excellon_layer_with_tools_and_hits() {
    let bytes = generated_fixture_member("proto-PTH.drl");
    let layer = load_excellon_reader("proto-PTH.drl", Cursor::new(bytes))
        .expect("generated plated drill layer must parse");

    assert_eq!(layer.layer_type, LayerType::Drill);
    assert!(matches!(
        &layer.data,
        LayerData::Excellon(data) if !data.tools.is_empty()
    ));
    assert!(layer.geometry.primitives.iter().any(|primitive| matches!(
        primitive,
        GerberPrimitive::DrillHit {
            diameter,
            tool: Some(_),
            ..
        } if *diameter > 0.0
    )));
    assert!(
        layer
            .geometry
            .bounds
            .is_some_and(|bounds| bounds.is_finite())
    );
}
