//! The diff text of a footprint file (#3109).

use super::*;

fn file_with_model(step: &[u8]) -> (String, String) {
    let stamp = DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap();
    let fp = Footprint::with_identity("SOIC-8", Uuid::from_u128(1), stamp);
    let mut file = FootprintFile::with_identity(fp, Uuid::from_u128(2), stamp);
    let uuid = file.footprints[0].uuid;
    let hash = file
        .attach_step_model(uuid, "SOIC-8.step", step)
        .unwrap()
        .content_hash;
    (file.to_toml_string().unwrap(), hash)
}

#[test]
fn each_model_data_block_becomes_one_summary_line() {
    let step: Vec<u8> = (0..20_000u32).flat_map(|i| i.to_le_bytes()).collect();
    let (text, hash) = file_with_model(&step);
    let shown = textconv_footprint_text(&text);
    let expected = format!(
        "data = <deflate+base64, {} bytes, sha256 {}…>\n",
        step.len(),
        &hash[..12]
    );
    assert!(shown.contains(&expected), "{shown}");
    assert!(shown.contains(&format!("hash = \"{hash}\"\n")), "{shown}");
    assert!(!shown.contains("'''\n'''"), "{shown}");
    assert!(shown.ends_with(&expected), "the block was last: {shown}");
    // Everything before the data block is kept byte for byte.
    let head = text.split("data = '''").next().unwrap();
    assert!(shown.starts_with(head));
    assert!(shown.len() < text.len());
}

#[test]
fn a_changed_model_differs_in_its_hash_size_and_summary_lines_only() {
    let (a, _) = file_with_model(b"first model");
    let (b, _) = file_with_model(b"second model, longer");
    let (a, b) = (textconv_footprint_text(&a), textconv_footprint_text(&b));
    let differing: Vec<(&str, &str)> = a.lines().zip(b.lines()).filter(|(x, y)| x != y).collect();
    assert_eq!(a.lines().count(), b.lines().count());
    assert_eq!(
        differing.len(),
        4,
        "attachment hash, entry hash, size and summary: {differing:?}"
    );
}

#[test]
fn text_without_models_is_unchanged() {
    let stamp = DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap();
    let mut fp = Footprint::with_identity("R0603", Uuid::from_u128(1), stamp);
    fp.pads.push(Pad::default());
    let text = FootprintFile::with_identity(fp, Uuid::from_u128(2), stamp)
        .to_toml_string()
        .unwrap();
    assert_eq!(textconv_footprint_text(&text), text);
    assert_eq!(
        textconv_footprint_text("data = '''\nx\n'''\n"),
        "data = '''\nx\n'''\n"
    );
    assert_eq!(textconv_footprint_text(""), "");
}
