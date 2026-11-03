//! Validation for re-naming / re-pointing a placed sheet symbol.
//!
//! Pure domain rules for `Command::ModifyChildSheet`: what a sheet symbol's
//! name and file reference must look like, and that they stay unique among
//! the sheet symbols of one document. Where the file lives (containment in
//! the project folder) is the caller's concern — the engine holds no
//! directory — so that check stays with the resolver in the app.

use std::path::Path;

use signex_types::schematic::ChildSheet;
use uuid::Uuid;

use crate::EngineError;

/// Extension every sheet file reference must keep.
const SHEET_EXTENSION: &str = "snxsch";

/// Comparison key for a file reference: separators and ASCII case folded,
/// since the project may live on a case-insensitive file system.
fn file_key(filename: &str) -> String {
    filename.replace('\\', "/").to_ascii_lowercase()
}

/// A trimmed, valid new name for sheet `id` among `children`.
pub(crate) fn validated_name(
    children: &[ChildSheet],
    id: Uuid,
    name: &str,
) -> Result<String, EngineError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(EngineError::EmptyChildSheetName);
    }
    let clash = children
        .iter()
        .any(|c| c.uuid != id && c.name.trim().eq_ignore_ascii_case(name));
    if clash {
        return Err(EngineError::DuplicateChildSheetName(name.to_string()));
    }
    Ok(name.to_string())
}

/// A trimmed, valid new file reference for sheet `id` among `children`.
pub(crate) fn validated_filename(
    children: &[ChildSheet],
    id: Uuid,
    filename: &str,
) -> Result<String, EngineError> {
    let filename = filename.trim();
    if filename.is_empty() {
        return Err(EngineError::EmptyChildSheetFilename);
    }
    let path = Path::new(filename);
    let keeps_extension = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(SHEET_EXTENSION))
        && path.file_stem().is_some_and(|stem| !stem.is_empty());
    if !keeps_extension {
        return Err(EngineError::InvalidChildSheetFilename(filename.to_string()));
    }
    if other_sheet_symbol_using_file(children, id, filename).is_some() {
        return Err(EngineError::DuplicateChildSheetFile(filename.to_string()));
    }
    Ok(filename.to_string())
}

/// The sheet symbol, other than `id`, that already points at `filename`: the
/// one-placement-per-file rule `Command::ModifyChildSheet` enforces. Lets a
/// dialog tell the user before the command refuses.
pub fn other_sheet_symbol_using_file<'a>(
    children: &'a [ChildSheet],
    id: Uuid,
    filename: &str,
) -> Option<&'a ChildSheet> {
    let key = file_key(filename.trim());
    children
        .iter()
        .find(|c| c.uuid != id && file_key(c.filename.trim()) == key)
}
