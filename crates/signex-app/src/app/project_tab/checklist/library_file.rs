//! The user's checklist template library on disk (#1291): the host side of
//! `TemplateLibrary`, which takes the file's contents and returns the bytes
//! to write and does no IO (#2431). Blocking IO: run it off the UI thread.

use std::path::Path;

use signex_types::project::{SavedTemplate, TemplateLibrary};

/// The file's bytes, `None` when there is no file.
fn contents(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// The library file at `path`; an empty library when there is no file.
pub(super) fn read_library_file(path: &Path) -> Result<TemplateLibrary, String> {
    TemplateLibrary::from_file(path, contents(path)?.as_deref())
}

/// Add `template` to the library file at `path` and write it back
/// atomically; the library as written. A file that does not read as a
/// library is refused, never written over.
pub(super) fn save_into_library_file(
    path: &Path,
    template: SavedTemplate,
) -> Result<TemplateLibrary, String> {
    let (library, bytes) =
        TemplateLibrary::with_template(path, contents(path)?.as_deref(), template)?;
    signex_types::atomic_io::atomic_write(path, &bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(library)
}

#[cfg(test)]
#[path = "library_file_tests.rs"]
mod tests;
