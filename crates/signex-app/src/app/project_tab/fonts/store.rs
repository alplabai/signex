//! Copying a font file into the project (#1178): read it, check it is a font
//! the project may carry, and write it to `assets/fonts/<sha256>.<ext>`.
//! Blocking file IO: it only ever runs on a worker, never in `update`. The
//! project lists the file only after this returns, so a saved project never
//! names a font file that is not on disk.

use std::path::Path;

use sha2::{Digest, Sha256};
use signex_i18n::{Translatable, ids};
use signex_output::font_file::{FontFileProblem, inspect_font_file};
use signex_types::atomic_io::atomic_write;
use signex_types::project::{
    FontFileInfo, MAX_PROJECT_FONT_BYTES, project_font_asset_path, project_font_extension,
};

/// A font file copied into the project, and what it said about itself.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredFont {
    /// The file name it was added from, for the Messages line.
    pub file_name: String,
    /// Where it is now, relative to the project folder.
    pub path: String,
    pub info: FontFileInfo,
    /// False when the same bytes were already in the folder.
    pub copied: bool,
}

/// Copy `source` into `project_dir`'s font folder, or say why not. Nothing
/// is written for a refused file.
pub(super) fn store_font_file(
    source: &Path,
    project_dir: &Path,
) -> Result<StoredFont, Translatable> {
    let file_name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| source.display().to_string());
    let io = |error: std::io::Error| {
        Translatable::new(ids::PROJECT_FONTS_IO)
            .arg("file", file_name.clone())
            .arg("why", error.to_string())
    };
    let Some(extension) = project_font_extension(&file_name) else {
        return Err(
            Translatable::new(ids::PROJECT_FONTS_NOT_A_FONT_FILE).arg("file", file_name.clone())
        );
    };
    let size = std::fs::metadata(source).map_err(io)?.len();
    if size > MAX_PROJECT_FONT_BYTES {
        return Err(too_large(&file_name));
    }
    let bytes = std::fs::read(source).map_err(io)?;
    let info = inspect_font_file(&bytes).map_err(|problem| match problem {
        FontFileProblem::TooLarge { .. } => too_large(&file_name),
        FontFileProblem::NotAFont => {
            Translatable::new(ids::PROJECT_FONTS_UNREADABLE_FILE).arg("file", file_name.clone())
        }
        FontFileProblem::NoFamily => Translatable::new(ids::PROJECT_FONTS_NO_FAMILY),
    })?;
    if !info.embedding.allows_project_copy() {
        let family = info
            .faces
            .first()
            .map(|face| face.family.clone())
            .unwrap_or_default();
        return Err(Translatable::new(ids::PROJECT_FONTS_RESTRICTED).arg("family", family));
    }
    let path = project_font_asset_path(&format!("{:x}", Sha256::digest(&bytes)), extension);
    let target = project_dir.join(&path);
    // Content-addressed: the same bytes under this name are this font.
    let present = std::fs::read(&target).is_ok_and(|on_disk| on_disk == bytes);
    if !present {
        atomic_write(&target, &bytes).map_err(io)?;
    }
    Ok(StoredFont {
        file_name,
        path,
        info,
        copied: !present,
    })
}

fn too_large(file_name: &str) -> Translatable {
    Translatable::new(ids::PROJECT_FONTS_TOO_LARGE)
        .arg("file", file_name.to_string())
        .arg("mib", MAX_PROJECT_FONT_BYTES / (1024 * 1024))
}
