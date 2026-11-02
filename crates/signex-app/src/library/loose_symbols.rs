//! Symbols in a `.snxsym` file outside every library (#1841), such as the
//! project's `SymbolLibrary.snxsym` from Add New ▸ Symbol Library. A row
//! binds only a symbol its library holds, so the picker offers these and
//! copies the chosen one into the target row's library before binding it.
//! The reads and the copy run on a worker, never in `update`.

use std::path::{Path, PathBuf};

use crate::app::file_extension::has_extension;
use signex_library::{LibraryAdapter, PrimitiveRef, SymbolFile};
use uuid::Uuid;

/// One symbol of a file that is in no library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LooseSymbol {
    pub file: PathBuf,
    pub uuid: Uuid,
    pub name: String,
    /// The `.snxlib` that holds `file` but is not open, when there is one:
    /// the symbol belongs to that library, so it is opened, not copied.
    pub library: Option<PathBuf>,
}

/// What reading some loose files found.
#[derive(Debug, Default)]
pub struct LooseRead {
    pub symbols: Vec<LooseSymbol>,
    /// One line per file that could not be read, for the Messages panel.
    pub problems: Vec<String>,
}

/// Read every symbol of each of `files`.
pub fn read_loose_symbols(files: &[PathBuf]) -> LooseRead {
    let mut read = LooseRead::default();
    for file in files {
        match read_container(file) {
            Ok(container) => {
                let library = enclosing_snxlib(file);
                read.symbols
                    .extend(container.symbols.iter().map(|symbol| LooseSymbol {
                        file: file.clone(),
                        uuid: symbol.uuid,
                        name: symbol.name.clone(),
                        library: library.clone(),
                    }))
            }
            Err(why) => read.problems.push(why),
        }
    }
    read
}

/// The `.snxlib` whose primitive directory holds `file`. A library keeps its
/// symbols directly in `<dir of the .snxlib>/symbols/` (what the adapter
/// scans), so `file` belongs to the one `*.snxlib` in the directory above its
/// `symbols/` folder. `None` when the file is elsewhere, or when that
/// directory holds several `.snxlib` files (the library is then unknown).
/// Reads a directory, so call it off the UI thread.
pub fn enclosing_snxlib(file: &Path) -> Option<PathBuf> {
    let symbols = file.parent()?;
    if !symbols
        .file_name()?
        .to_str()?
        .eq_ignore_ascii_case("symbols")
    {
        return None;
    }
    let mut found = std::fs::read_dir(symbols.parent()?)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| has_extension(path, "snxlib") && path.is_file());
    let only = found.next()?;
    found.next().is_none().then_some(only)
}

fn read_container(file: &Path) -> Result<SymbolFile, String> {
    let bytes =
        std::fs::read(file).map_err(|e| format!("Could not read {}: {e}", file.display()))?;
    SymbolFile::from_bytes(&bytes).map_err(|e| format!("Could not read {}: {e}", file.display()))
}

/// Copy `symbol` of `file` into the library of `adapter` through its save
/// path (container write inside the library, commit). The symbol keeps its
/// uuid unless the library already has one with it. The source is left
/// untouched. Returns the reference the row binds.
pub fn copy_into_library(
    adapter: &dyn LibraryAdapter,
    file: &Path,
    symbol: Uuid,
) -> Result<PrimitiveRef, String> {
    let container = read_container(file)?;
    let mut copy = container
        .get_symbol(symbol)
        .cloned()
        .ok_or_else(|| format!("{} no longer holds that symbol.", file.display()))?;
    if adapter.get_symbol(copy.uuid).is_ok() {
        copy.uuid = Uuid::now_v7();
    }
    let uuid = copy.uuid;
    let from = file.file_name().map_or_else(
        || file.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let message = format!("copy symbol {} from {from}", copy.name);
    adapter
        .save_symbol(copy, &message)
        .map_err(|e| format!("Could not copy the symbol into the library: {e}"))?;
    Ok(PrimitiveRef::new(adapter.library_id(), uuid))
}

#[cfg(test)]
mod tests {
    use super::enclosing_snxlib;
    use std::path::{Path, PathBuf};

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }

    fn held(dir: &Path) -> PathBuf {
        let file = dir.join("symbols").join("Foo.snxsym");
        touch(&file);
        file
    }

    #[test]
    fn a_file_in_the_primitive_dir_belongs_to_the_one_snxlib_beside_it() {
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("x.snxlib"));
        assert_eq!(
            enclosing_snxlib(&held(tmp.path())),
            Some(tmp.path().join("x.snxlib"))
        );
    }

    #[test]
    fn a_stray_snxlib_higher_up_does_not_claim_a_file_that_is_not_in_its_symbols_dir() {
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("stray.snxlib"));
        let shared = tmp.path().join("shared").join("sym").join("Foo.snxsym");
        touch(&shared);
        assert_eq!(enclosing_snxlib(&shared), None);
        // A root `.snxlib` whose `symbols/` does not hold the file.
        let other = tmp.path().join("other").join("Foo.snxsym");
        touch(&other);
        assert_eq!(enclosing_snxlib(&other), None);
        // Nested deeper than the primitive dir is not a library symbol either.
        let deep = tmp.path().join("symbols").join("sub").join("Foo.snxsym");
        touch(&deep);
        assert_eq!(enclosing_snxlib(&deep), None);
    }

    #[test]
    fn the_symbols_folder_and_snxlib_names_match_ignoring_case() {
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("X.SNXLIB"));
        let file = tmp.path().join("Symbols").join("Foo.snxsym");
        touch(&file);
        assert_eq!(enclosing_snxlib(&file), Some(tmp.path().join("X.SNXLIB")));
    }

    #[test]
    fn two_snxlib_files_in_one_directory_leave_the_library_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("a.snxlib"));
        touch(&tmp.path().join("b.snxlib"));
        assert_eq!(enclosing_snxlib(&held(tmp.path())), None);
    }
}
