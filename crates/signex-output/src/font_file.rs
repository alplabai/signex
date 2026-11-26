//! Reading a font file before it is copied into a project (#1178): its
//! families and styles, its embedding permission (`OS/2.fsType`) and the
//! license it names. A project font arrives with someone else's project, so
//! the bytes are untrusted: only the pure-Rust reader the PDF export already
//! uses opens them, the size is capped, and a file it cannot read is refused,
//! never half-read. Nothing here touches the disk.

use signex_types::project::{FontEmbedding, FontFace, FontFileInfo, MAX_PROJECT_FONT_BYTES};
use ttf_parser::{Face, Language, Permissions, name_id};

/// Most faces a collection is read for; a header that claims more is refused.
const MAX_FACES: u32 = 64;

/// Longest family or style name kept, in characters.
const MAX_NAME_CHARS: usize = 128;

/// Longest license hint kept, in characters.
const MAX_LICENSE_HINT_CHARS: usize = 200;

/// Why a font file cannot be taken.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FontFileProblem {
    #[error(
        "the file is {bytes} bytes, more than the {MAX_PROJECT_FONT_BYTES} a project font may be"
    )]
    TooLarge { bytes: u64 },
    #[error("the file is not a TrueType or OpenType font this build can read")]
    NotAFont,
    #[error("the font names no family")]
    NoFamily,
}

/// What `bytes` say about the font: every face's family and style, the
/// strictest embedding permission of the faces and the license it names.
pub fn inspect_font_file(bytes: &[u8]) -> Result<FontFileInfo, FontFileProblem> {
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size > MAX_PROJECT_FONT_BYTES {
        return Err(FontFileProblem::TooLarge { bytes: size });
    }
    let count = ttf_parser::fonts_in_collection(bytes).unwrap_or(1);
    if count == 0 || count > MAX_FACES {
        return Err(FontFileProblem::NotAFont);
    }
    let mut faces = Vec::new();
    let mut embedding: Option<FontEmbedding> = None;
    let mut license_hint = None;
    for index in 0..count {
        let face = Face::parse(bytes, index).map_err(|_| FontFileProblem::NotAFont)?;
        let family = best_name(&face, &[name_id::TYPOGRAPHIC_FAMILY, name_id::FAMILY])
            .map(|text| capped(&text, MAX_NAME_CHARS))
            .ok_or(FontFileProblem::NoFamily)?;
        let style = best_name(&face, &[name_id::TYPOGRAPHIC_SUBFAMILY, name_id::SUBFAMILY])
            .map(|text| capped(&text, MAX_NAME_CHARS))
            .unwrap_or_default();
        let this = face_embedding(&face);
        embedding = Some(embedding.map_or(this, |seen| seen.strictest(this)));
        if license_hint.is_none() {
            license_hint = best_name(&face, &[name_id::LICENSE_URL, name_id::LICENSE]);
        }
        faces.push(FontFace { family, style });
    }
    Ok(FontFileInfo {
        faces,
        embedding: embedding.unwrap_or(FontEmbedding::Restricted),
        license_hint,
    })
}

/// The permission of one face. A face without an `OS/2` table states no
/// restriction; a permission value the specification does not allow is read
/// as restricted, so an unclear font is never copied.
fn face_embedding(face: &Face) -> FontEmbedding {
    if face.tables().os2.is_none() {
        return FontEmbedding::Installable;
    }
    match face.permissions() {
        Some(Permissions::Installable) => FontEmbedding::Installable,
        Some(Permissions::Editable) => FontEmbedding::Editable,
        Some(Permissions::PreviewAndPrint) => FontEmbedding::PreviewAndPrint,
        Some(Permissions::Restricted) | None => FontEmbedding::Restricted,
    }
}

/// The first of `ids` the font names, in US English when it has it, as the
/// font database names families; control characters dropped and the length
/// capped, since the text is shown.
fn best_name(face: &Face, ids: &[u16]) -> Option<String> {
    for id in ids {
        let mut other_language = None;
        for name in face.names() {
            if name.name_id != *id {
                continue;
            }
            let Some(text) = name
                .to_string()
                .map(|text| capped(&text, MAX_LICENSE_HINT_CHARS))
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            if name.language() == Language::English_UnitedStates {
                return Some(text);
            }
            other_language.get_or_insert(text);
        }
        if other_language.is_some() {
            return other_language;
        }
    }
    None
}

/// `text` without control characters, trimmed, at most `max` characters.
fn capped(text: &str, max: usize) -> String {
    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
    clean
        .trim()
        .chars()
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
#[path = "font_file_tests.rs"]
mod tests;
