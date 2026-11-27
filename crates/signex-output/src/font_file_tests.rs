//! #1178: a font file is read for its family, style, embedding permission
//! and license before it is copied into a project; a restricted, malformed
//! or oversized file is refused without a panic.

use signex_types::project::{FontEmbedding, FontFace, MAX_PROJECT_FONT_BYTES};

use super::{FontFileProblem, inspect_font_file};

const ROBOTO: &[u8] = include_bytes!("../../signex-app/assets/fonts/Roboto-Regular.ttf");

/// `font` with its `OS/2.fsType` set to `fs_type`. ttf-parser does not check
/// table checksums, so the patched copy still reads.
fn with_fs_type(font: &[u8], fs_type: u16) -> Vec<u8> {
    let mut bytes = font.to_vec();
    let tables = usize::from(u16::from_be_bytes([bytes[4], bytes[5]]));
    let os2 = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|&record| &bytes[record..record + 4] == b"OS/2")
        .map(|record| {
            let at = &bytes[record + 8..record + 12];
            u32::from_be_bytes([at[0], at[1], at[2], at[3]]) as usize
        })
        .expect("the font has an OS/2 table");
    bytes[os2 + 8..os2 + 10].copy_from_slice(&fs_type.to_be_bytes());
    bytes
}

#[test]
fn a_bundled_face_reads_its_family_style_permission_and_license() {
    let info = inspect_font_file(ROBOTO).expect("Roboto reads");
    assert_eq!(
        info.faces,
        [FontFace {
            family: "Roboto".into(),
            style: "Regular".into()
        }]
    );
    assert_eq!(info.embedding, FontEmbedding::Installable);
    assert_eq!(
        info.license_hint.as_deref(),
        Some("https://openfontlicense.org")
    );
}

#[test]
fn each_fs_type_permission_is_read() {
    for (fs_type, expected) in [
        (0x0000, FontEmbedding::Installable),
        (0x0002, FontEmbedding::Restricted),
        (0x0004, FontEmbedding::PreviewAndPrint),
        (0x0008, FontEmbedding::Editable),
        // Two exclusive bits at once (OS/2 version 4): unclear, so restricted.
        (0x0006, FontEmbedding::Restricted),
    ] {
        let info = inspect_font_file(&with_fs_type(ROBOTO, fs_type)).expect("still reads");
        assert_eq!(info.embedding, expected, "fsType {fs_type:#06x}");
    }
}

#[test]
fn a_malformed_or_truncated_file_is_refused_without_a_panic() {
    assert_eq!(inspect_font_file(b""), Err(FontFileProblem::NotAFont));
    assert_eq!(
        inspect_font_file(b"this is not a font at all, only text"),
        Err(FontFileProblem::NotAFont)
    );
    for cut in [12, 64, 1024, ROBOTO.len() / 2] {
        // A cut file may still parse its header; it must never panic.
        let _ = inspect_font_file(&ROBOTO[..cut]);
    }
    // A collection header that claims more faces than any real one.
    let mut collection = b"ttcf\x00\x01\x00\x00".to_vec();
    collection.extend_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        inspect_font_file(&collection),
        Err(FontFileProblem::NotAFont)
    );
}

#[test]
fn a_file_over_the_cap_is_refused_before_it_is_read() {
    let size = usize::try_from(MAX_PROJECT_FONT_BYTES + 1).expect("fits");
    let mut big = vec![0u8; size];
    big[..ROBOTO.len()].copy_from_slice(ROBOTO);
    assert_eq!(
        inspect_font_file(&big),
        Err(FontFileProblem::TooLarge {
            bytes: MAX_PROJECT_FONT_BYTES + 1
        })
    );
}
