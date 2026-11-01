//! Symbols in a `.snxsym` file outside every library (#1841), such as the
//! project's `SymbolLibrary.snxsym` from Add New ▸ Symbol Library. A row
//! binds only a symbol its library holds, so the picker offers these and
//! copies the chosen one into the target row's library before binding it.
//! The reads and the copy run on a worker, never in `update`.

use std::path::{Path, PathBuf};

use signex_library::{LibraryAdapter, PrimitiveRef, SymbolFile};
use uuid::Uuid;

/// One symbol of a file that is in no library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LooseSymbol {
    pub file: PathBuf,
    pub uuid: Uuid,
    pub name: String,
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
                read.symbols
                    .extend(container.symbols.iter().map(|symbol| LooseSymbol {
                        file: file.clone(),
                        uuid: symbol.uuid,
                        name: symbol.name.clone(),
                    }))
            }
            Err(why) => read.problems.push(why),
        }
    }
    read
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
