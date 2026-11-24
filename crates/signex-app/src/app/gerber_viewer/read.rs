//! Reading a package off the UI thread (mockup row 193f): a folder's files,
//! a zip's entries read in place (never unpacked to disk), or the files
//! picked or dropped. Every file goes through `signex_gerber::read_file`,
//! so a file that is not fabrication data lands in `Package::not_read`.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use signex_gerber::{Package, ReadFile, assemble, read_file};

/// A file larger than this is not read: fabrication files are far smaller,
/// and a stray disk image dropped on the tab must not fill memory.
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// What a read gives back: the package and the paths it was opened from.
#[derive(Debug, Clone)]
pub struct PackageRead {
    pub paths: Vec<PathBuf>,
    pub package: Package,
}

/// Whether `name` is a zip package, by its extension.
pub fn is_zip(name: &Path) -> bool {
    name.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

/// Read `paths` into one package: a folder contributes its files (not its
/// subfolders), a zip its entries, any other path itself. Blocking.
pub fn read_paths(paths: Vec<PathBuf>) -> Result<PackageRead, String> {
    let mut files = Vec::new();
    for path in &paths {
        if path.is_dir() {
            files.extend(read_folder(path)?);
        } else if is_zip(path) {
            files.extend(read_zip(path)?);
        } else {
            files.push(read_one(path)?);
        }
    }
    Ok(PackageRead {
        paths,
        package: assemble(files),
    })
}

/// The files of `folder`, by name.
fn read_folder(folder: &Path) -> Result<Vec<ReadFile>, String> {
    let entries = std::fs::read_dir(folder).map_err(|e| describe(folder, &e))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    files.iter().map(|path| read_one(path)).collect()
}

fn read_one(path: &Path) -> Result<ReadFile, String> {
    let size = std::fs::metadata(path)
        .map_err(|e| describe(path, &e))?
        .len();
    if size > MAX_FILE_BYTES {
        return Err(too_large(path));
    }
    let bytes = std::fs::read(path).map_err(|e| describe(path, &e))?;
    Ok(read_file(&file_name(path), &bytes))
}

/// The entries of the zip at `path`, read in memory.
fn read_zip(path: &Path) -> Result<Vec<ReadFile>, String> {
    let file = std::fs::File::open(path).map_err(|e| describe(path, &e))?;
    read_zip_from(file, path)
}

/// The file entries of a zip archive, by their names in the archive.
pub fn read_zip_from<R: std::io::Read + std::io::Seek>(
    reader: R,
    path: &Path,
) -> Result<Vec<ReadFile>, String> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| describe(path, &e))?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| describe(path, &e))?;
        // A folder entry, or a name that escapes the archive: not a file.
        let Some(name) = entry.enclosed_name().filter(|_| !entry.is_dir()) else {
            continue;
        };
        if entry.size() > MAX_FILE_BYTES {
            return Err(too_large(&name));
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|e| describe(path, &e))?;
        files.push(read_file(&file_name(&name), &bytes));
    }
    Ok(files)
}

/// A file's name as the layer list and Messages show it.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn describe(path: &Path, error: &dyn std::fmt::Display) -> String {
    format!("{}: {error}", path.display())
}

fn too_large(path: &Path) -> String {
    format!("{}: larger than {MAX_FILE_BYTES} bytes", path.display())
}
