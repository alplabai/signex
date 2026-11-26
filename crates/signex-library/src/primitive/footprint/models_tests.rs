//! Embedded STEP models (`[[models]]`, #3107) and the `snxfpt/1.4` token
//! (#3108).

use signex_types::format_version::{FormatVersion, MissingToken, NewerMinor};

use super::*;

fn stamp() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap()
}

fn footprint(name: &str, id: u128) -> Footprint {
    Footprint::with_identity(name, Uuid::from_u128(id), stamp())
}

/// A STEP-like text long enough to wrap over several base64 lines.
fn step_bytes(seed: &str) -> Vec<u8> {
    let mut text = String::from("ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n");
    for i in 0..400 {
        text.push_str(&format!(
            "#{i}=CARTESIAN_POINT('{seed}',({i}.,{}.,0.));\n",
            i * 7
        ));
    }
    text.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    text.into_bytes()
}

/// A file of `names`, each footprint attaching nothing yet.
fn file_of(names: &[&str]) -> FootprintFile {
    let mut file =
        FootprintFile::with_identity(footprint(names[0], 1), Uuid::from_u128(100), stamp());
    for (i, name) in names.iter().enumerate().skip(1) {
        file.footprints.push(footprint(name, 1 + i as u128));
    }
    file
}

fn uuid_of(file: &FootprintFile, index: usize) -> Uuid {
    file.footprints[index].uuid
}

#[test]
fn a_footprint_with_a_model_round_trips_byte_identical_as_1_4() {
    let mut file = file_of(&["SOIC-8"]);
    let step = step_bytes("a");
    let attachment = file
        .attach_step_model(uuid_of(&file, 0), "SOIC-8.step", &step)
        .expect("the footprint is there");
    assert_eq!(attachment.content_hash, step_model_hash(&step));
    let text = file.to_toml_string().expect("serialise");
    assert!(text.starts_with("format = \"snxfpt/1.4\"\n"), "{text}");
    let models_at = text.find("[[models]]").expect("a models section");
    assert!(
        models_at > text.rfind("[[footprints]]").unwrap(),
        "written last"
    );
    let entry = &text[models_at..];
    for key in [
        "hash = ",
        "filename = \"SOIC-8.step\"",
        "encoding = \"deflate+base64\"",
        "size = ",
    ] {
        assert!(entry.contains(key), "{key} missing: {entry}");
    }
    assert!(entry.contains("data = '''\n"), "{entry}");
    let data_lines: Vec<&str> = entry
        .split("data = '''\n")
        .nth(1)
        .unwrap()
        .lines()
        .take_while(|l| *l != "'''")
        .collect();
    assert!(data_lines.len() > 1, "wrapped over lines");
    assert!(data_lines.iter().all(|l| l.len() <= 76), "{data_lines:?}");
    assert!(
        data_lines[..data_lines.len() - 1]
            .iter()
            .all(|l| l.len() == 76)
    );

    let back = FootprintFile::from_toml_str(&text).expect("1.4 reads");
    assert_eq!(back.format, "snxfpt/1.4");
    assert_eq!(back.step_model(&back.footprints[0]).unwrap(), Some(step));
    assert_eq!(
        back.to_toml_string().unwrap(),
        text,
        "a re-save is byte-identical"
    );
}

#[test]
fn two_footprints_sharing_a_model_store_one_entry() {
    let mut file = file_of(&["SOIC-8", "SOIC-8-WIDE", "SOT-23"]);
    let shared = step_bytes("shared");
    let other = step_bytes("other");
    file.attach_step_model(uuid_of(&file, 0), "a.step", &shared);
    file.attach_step_model(uuid_of(&file, 1), "b.step", &shared);
    file.attach_step_model(uuid_of(&file, 2), "c.step", &other);
    let text = file.to_toml_string().unwrap();
    assert_eq!(text.matches("[[models]]").count(), 2, "{text}");
    let mut hashes = [step_model_hash(&shared), step_model_hash(&other)];
    hashes.sort();
    let first = text.find(&format!("hash = \"{}\"", hashes[0])).unwrap();
    let second = text.find(&format!("hash = \"{}\"", hashes[1])).unwrap();
    assert!(first < second, "sorted by hash");
    let back = FootprintFile::from_toml_str(&text).unwrap();
    assert_eq!(back.step_model(&back.footprints[1]).unwrap(), Some(shared));
}

#[test]
fn removing_the_last_user_drops_the_entry_and_the_token() {
    let plain = file_of(&["SOIC-8"]);
    let model_free = plain.to_toml_string().unwrap();
    assert!(model_free.starts_with("format = \"snxfpt/1.1\"\n"));
    assert!(!model_free.contains("models"), "{model_free}");

    let mut file = plain.clone();
    file.attach_step_model(uuid_of(&file, 0), "a.step", &step_bytes("a"));
    let with_model = FootprintFile::from_toml_str(&file.to_toml_string().unwrap()).unwrap();
    let mut removed = with_model.clone();
    removed.footprints[0].step_attachment = None;
    assert_eq!(
        removed.to_toml_string().unwrap(),
        model_free,
        "the same bytes as a file that never had the model"
    );
}

#[test]
fn removing_the_last_model_restores_a_1_3_token() {
    let mut file = file_of(&["GEN"]);
    file.footprints[0].generated.silk_outline = true;
    let before = file.to_toml_string().unwrap();
    assert!(before.starts_with("format = \"snxfpt/1.3\"\n"), "{before}");
    file.attach_step_model(uuid_of(&file, 0), "a.step", &step_bytes("a"));
    assert_eq!(file.format_token(), "snxfpt/1.4");
    file.footprints[0].step_attachment = None;
    assert_eq!(file.to_toml_string().unwrap(), before);
}

#[test]
fn an_attachment_without_an_entry_keeps_the_lower_token() {
    let mut file = file_of(&["SOIC-8"]);
    file.footprints[0].step_attachment = Some(StepAttachment {
        content_hash: step_model_hash(b"elsewhere"),
        filename: "a.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    });
    let text = file.to_toml_string().unwrap();
    assert!(text.starts_with("format = \"snxfpt/1.1\"\n"), "{text}");
    let back = FootprintFile::from_toml_str(&text).unwrap();
    assert_eq!(back.step_model(&back.footprints[0]).unwrap(), None);
}

#[test]
fn a_1_3_build_refuses_a_file_with_models_by_name() {
    const SNXFPT_1_3_ONLY: FormatVersion = FormatVersion {
        current: "snxfpt/1.3",
        readable: &["snxfpt/1", "snxfpt/1.1", "snxfpt/1.2", "snxfpt/1.3"],
        newer_minor: NewerMinor::Refused,
        missing: MissingToken::Refused,
    };
    let mut file = file_of(&["SOIC-8"]);
    file.attach_step_model(uuid_of(&file, 0), "a.step", &step_bytes("a"));
    let refused = SNXFPT_1_3_ONLY
        .read(Some(file.format_token()))
        .expect_err("a 1.3 build refuses 1.4");
    assert_eq!(refused.found(), Some("snxfpt/1.4"));
    assert!(refused.to_string().contains("snxfpt/1.4"), "{refused}");
    assert_eq!(
        FOOTPRINT_FILE_VERSION.read(Some("snxfpt/1.4")).as_deref(),
        Ok("snxfpt/1.4")
    );
}

/// A saved file with one model, and the model's hash.
fn saved_with_model(name: &str) -> (String, String) {
    let mut file = file_of(&[name]);
    let attachment = file
        .attach_step_model(uuid_of(&file, 0), "a.step", &step_bytes("a"))
        .unwrap();
    (file.to_toml_string().unwrap(), attachment.content_hash)
}

fn decode_error(text: &str) -> ModelError {
    let file = FootprintFile::from_toml_str(text).expect("the shape still reads");
    let error = file.verify_models().expect_err("the model does not decode");
    assert_eq!(
        file.step_model(&file.footprints[0]).unwrap_err(),
        error,
        "the 3D read sees the same error"
    );
    error
}

#[test]
fn corrupted_data_fails_naming_the_footprint() {
    let (text, _) = saved_with_model("QFN-16");
    let data_at = text.find("data = '''\n").unwrap() + "data = '''\n".len() + 10;
    let flipped = if &text[data_at..=data_at] == "A" {
        "B"
    } else {
        "A"
    };
    let corrupted = format!("{}{flipped}{}", &text[..data_at], &text[data_at + 1..]);
    let error = decode_error(&corrupted);
    assert_eq!(error.footprint, "QFN-16");
    assert!(error.to_string().contains("\"QFN-16\""), "{error}");
}

#[test]
fn a_wrong_size_fails_naming_the_footprint() {
    let (text, _) = saved_with_model("QFN-16");
    let size = step_bytes("a").len();
    let corrupted = text.replacen(
        &format!("size = {size}\n"),
        &format!("size = {}\n", size + 1),
        1,
    );
    assert_ne!(corrupted, text);
    let error = decode_error(&corrupted);
    assert_eq!(error.footprint, "QFN-16");
    assert!(matches!(error.fault, ModelFault::Size { .. }), "{error}");
}

#[test]
fn a_wrong_hash_fails_naming_the_footprint() {
    let (text, hash) = saved_with_model("QFN-16");
    let other = step_model_hash(b"something else");
    let corrupted = text.replace(&hash, &other);
    let error = decode_error(&corrupted);
    assert_eq!(error.footprint, "QFN-16");
    assert!(matches!(error.fault, ModelFault::Hash { .. }), "{error}");
}

#[test]
fn an_unknown_encoding_is_refused_on_load_naming_the_footprint() {
    let (text, _) = saved_with_model("QFN-16");
    let corrupted = text.replace(
        "encoding = \"deflate+base64\"",
        "encoding = \"zstd+base64\"",
    );
    match FootprintFile::from_toml_str(&corrupted) {
        Err(error @ FootprintFileError::InvalidModelEntry { .. }) => {
            assert!(error.to_string().contains("\"QFN-16\""), "{error}");
        }
        other => panic!("expected InvalidModelEntry, got {other:?}"),
    }
}

#[test]
fn a_model_in_step_dir_is_embedded_on_save_and_the_file_stays() {
    let dir = tempfile::tempdir().unwrap();
    let step_dir = dir.path().join("step");
    std::fs::create_dir_all(&step_dir).unwrap();
    let step = step_bytes("legacy");
    let hash = step_model_hash(&step);
    let legacy = step_dir.join(format!("{hash}.step"));
    std::fs::write(&legacy, &step).unwrap();

    let mut file = file_of(&["OLD"]);
    file.footprints[0].step_attachment = Some(StepAttachment {
        content_hash: hash.clone(),
        filename: "old.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    });
    assert_eq!(file.embed_models_from_step_dir(&step_dir), 1);
    assert_eq!(file.embed_models_from_step_dir(&step_dir), 0, "once");
    assert!(legacy.exists(), "the step/ file is never removed");
    let text = file.to_toml_string().unwrap();
    assert!(text.starts_with("format = \"snxfpt/1.4\"\n"));
    let back = FootprintFile::from_toml_str(&text).unwrap();
    assert_eq!(back.step_model(&back.footprints[0]).unwrap(), Some(step));
}

#[test]
fn a_step_dir_file_that_does_not_match_its_hash_is_not_embedded() {
    let dir = tempfile::tempdir().unwrap();
    let hash = step_model_hash(b"the real model");
    std::fs::write(dir.path().join(format!("{hash}.step")), b"other bytes").unwrap();
    let mut file = file_of(&["OLD"]);
    file.footprints[0].step_attachment = Some(StepAttachment {
        content_hash: hash,
        filename: "old.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    });
    assert_eq!(file.embed_models_from_step_dir(dir.path()), 0);
    assert_eq!(file.embed_models_from_step_dir(&dir.path().join("none")), 0);
    assert!(file.models.is_empty());
}

#[test]
fn a_container_merge_carries_the_editors_models() {
    let baseline = file_of(&["SOIC-8"]);
    let mut editor = baseline.clone();
    let step = step_bytes("merged");
    editor.attach_step_model(uuid_of(&editor, 0), "a.step", &step);
    let (merged, _) = editor
        .merge_into(&baseline, baseline.clone(), stamp())
        .expect("merges");
    assert_eq!(
        merged.step_model(&merged.footprints[0]).unwrap(),
        Some(step)
    );
}

#[test]
fn a_copy_for_another_library_keeps_its_models() {
    let mut file = file_of(&["SOIC-8"]);
    let step = step_bytes("copied");
    file.attach_step_model(uuid_of(&file, 0), "a.step", &step);
    let copy = file.copy_for_library(&Default::default(), stamp());
    let back = FootprintFile::from_toml_str(&copy.to_toml_string().unwrap()).unwrap();
    assert_eq!(back.step_model(&back.footprints[0]).unwrap(), Some(step));
}

#[test]
fn the_step_hash_is_lowercase_sha256_hex() {
    assert_eq!(
        step_model_hash(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

/// Two footprints, two models, the given file names.
fn two_models(names: [&str; 2]) -> FootprintFile {
    let mut file = file_of(&["A", "B"]);
    file.attach_step_model(uuid_of(&file, 0), names[0], &step_bytes("one"));
    file.attach_step_model(uuid_of(&file, 1), names[1], &step_bytes("two"));
    file
}

#[test]
fn a_file_name_spelling_a_placeholder_is_kept_as_a_name() {
    let placeholder = |i: usize| format!("__SIGNEX_MODEL_DATA_a1b2c3d4_{i}__");
    for names in [
        [placeholder(1), placeholder(0)],
        [
            format!("data = \"{}\"", placeholder(1)),
            format!("data = \"{}\"", placeholder(0)),
        ],
    ] {
        let file = two_models([&names[0], &names[1]]);
        let text = file.to_toml_string().expect("serialise");
        let back = FootprintFile::from_toml_str(&text).expect("reads back");
        back.verify_models().expect("both models decode");
        let mut stored: Vec<&str> = back
            .models
            .iter()
            .map(|(_, m)| m.filename.as_str())
            .collect();
        stored.sort_unstable();
        let mut wanted: Vec<&str> = names.iter().map(String::as_str).collect();
        wanted.sort_unstable();
        assert_eq!(stored, wanted);
        assert_eq!(back.to_toml_string().unwrap(), text);
    }
}

#[test]
fn a_file_name_spelling_a_whole_data_line_is_refused_or_kept() {
    let hostile = "x\ndata = \"__SIGNEX_MODEL_DATA_a1b2c3d4_0__\"\ny";
    let file = two_models([hostile, hostile]);
    match file.to_toml_string() {
        Ok(text) => {
            let back = FootprintFile::from_toml_str(&text).expect("reads back");
            back.verify_models().expect("both models decode");
            assert!(
                back.models
                    .iter()
                    .map(|(_, m)| m)
                    .all(|m| m.filename == hostile)
            );
        }
        Err(error) => assert!(
            matches!(error, FootprintFileError::ModelDataPlacement { .. }),
            "{error}"
        ),
    }
}

#[test]
fn an_inflate_stops_one_byte_past_the_declared_size() {
    let mut file = file_of(&["BIG"]);
    let step = vec![b'0'; 4_000_000];
    file.attach_step_model(uuid_of(&file, 0), "big.step", &step);
    let text = file
        .to_toml_string()
        .unwrap()
        .replacen("size = 4000000\n", "size = 10\n", 1);
    let error = decode_error(&text);
    assert_eq!(
        error.fault,
        ModelFault::Size {
            declared: 10,
            actual: 11
        }
    );
}

/// `text` with its one `data` block replaced by `block`.
fn with_data_block(text: &str, block: &str) -> String {
    let open = text.find("data = '''\n").unwrap() + "data = '''\n".len();
    let close = open + text[open..].find("'''").unwrap();
    format!("{}{block}{}", &text[..open], &text[close..])
}

#[test]
fn data_written_at_another_level_and_width_is_kept_as_stored() {
    use base64::Engine as _;
    use std::io::Write as _;
    let (text, _) = saved_with_model("QFN-16");
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(9));
    encoder.write_all(&step_bytes("a")).unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(encoder.finish().unwrap());
    let block: String = encoded
        .as_bytes()
        .chunks(60)
        .map(|line| format!("{}\n", std::str::from_utf8(line).unwrap()))
        .collect();
    let other = with_data_block(&text, &block);
    assert_ne!(other, text);
    let back = FootprintFile::from_toml_str(&other).unwrap();
    assert_eq!(
        back.step_model(&back.footprints[0]).unwrap(),
        Some(step_bytes("a"))
    );
    assert_eq!(
        back.to_toml_string().unwrap(),
        other,
        "the data is kept as stored"
    );
}

#[test]
fn a_model_that_does_not_decode_is_saved_back_unchanged() {
    let (text, _) = saved_with_model("QFN-16");
    let corrupted = with_data_block(&text, "AAAA\nBBBB\n");
    let file = FootprintFile::from_toml_str(&corrupted).unwrap();
    assert!(file.verify_models().is_err());
    assert_eq!(file.to_toml_string().unwrap(), corrupted);
}

#[test]
fn data_with_characters_base64_does_not_use_is_refused_on_load() {
    let (text, _) = saved_with_model("QFN-16");
    let corrupted = with_data_block(&text, "AAAA'' BBBB\n");
    assert!(matches!(
        FootprintFile::from_toml_str(&corrupted),
        Err(FootprintFileError::InvalidModelEntry { .. })
    ));
}
