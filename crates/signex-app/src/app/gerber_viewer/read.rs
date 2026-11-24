//! Reading a package off the UI thread (mockup row 193f): a folder's files,
//! a zip's entries read in place (never unpacked to disk), or the files
//! picked or dropped. Every file goes through `signex_gerber::read_file`,
//! so a file that is not fabrication data lands in `Package::not_read`.
//! A file whose bytes cannot be had (a permission, a lock, its size) is a
//! Not read row too; only a top-level path that is not there, or a zip
//! that is not one, fails the whole read.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use signex_gerber::{
    Package, ProblemKind, ReadFile, ReadProblem, SourceLine, assemble, read_file, sniff_kind,
};

/// A file larger than this is not read: fabrication files are far smaller,
/// and a stray disk image dropped on the tab must not fill memory.
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const BYTES_PER_MIB: u64 = 1024 * 1024;

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

/// The one name rule for opening in the viewer from elsewhere (a Projects
/// row, a drop on another tab): a zip package, or a name the reader takes
/// for a Gerber, drill or job file.
pub fn is_fabrication_name(path: &Path) -> bool {
    is_zip(path) || sniff_kind(&file_name(path), b"") != signex_gerber::FileKind::NotRead
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
            files.push(read_top_file(path)?);
        }
    }
    Ok(PackageRead {
        paths,
        package: assemble(files),
    })
}

/// The files of `folder`, by name. An entry the folder listing cannot
/// give is a Not read row named by the folder.
fn read_folder(folder: &Path) -> Result<Vec<ReadFile>, String> {
    let entries = std::fs::read_dir(folder).map_err(|e| describe(folder, &e))?;
    let mut files = Vec::new();
    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(error) => files.push(unopenable(&file_name(folder), &error)),
        }
    }
    paths.retain(|path| path.is_file());
    paths.sort();
    files.extend(paths.iter().map(|path| read_one(path)));
    Ok(files)
}

/// A path given at the top: one that is not there fails the read, one that
/// cannot be read is a Not read row.
fn read_top_file(path: &Path) -> Result<ReadFile, String> {
    std::fs::metadata(path).map_err(|e| describe(path, &e))?;
    Ok(read_one(path))
}

fn read_one(path: &Path) -> ReadFile {
    let name = file_name(path);
    let size = match std::fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(error) => return unopenable(&name, &error),
    };
    if size > MAX_FILE_BYTES {
        return too_large(&name);
    }
    match std::fs::read(path) {
        Ok(bytes) => read_file(&name, &bytes),
        Err(error) => unopenable(&name, &error),
    }
}

/// The entries of the zip at `path`, read in memory.
fn read_zip(path: &Path) -> Result<Vec<ReadFile>, String> {
    let file = std::fs::File::open(path).map_err(|e| describe(path, &e))?;
    read_zip_from(file, path)
}

/// The file entries of a zip archive, by their names in the archive. An
/// entry that cannot be read, or reads past the size cap whatever its
/// header says, is a Not read row.
pub fn read_zip_from<R: std::io::Read + std::io::Seek>(
    reader: R,
    path: &Path,
) -> Result<Vec<ReadFile>, String> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| describe(path, &e))?;
    let archive_name = file_name(path);
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                files.push(unopenable(&archive_name, &error));
                continue;
            }
        };
        // A folder entry, or a name that escapes the archive: not a file.
        let Some(name) = entry.enclosed_name().filter(|_| !entry.is_dir()) else {
            continue;
        };
        let name = file_name(&name);
        let mut bytes = Vec::new();
        let read = entry
            .by_ref()
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes);
        files.push(match read {
            Err(error) => unopenable(&name, &error),
            Ok(_) if bytes.len() as u64 > MAX_FILE_BYTES => too_large(&name),
            Ok(_) => read_file(&name, &bytes),
        });
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

/// A Not read row for a whole file, at its first line as the reader's own
/// file-level problems are.
fn not_read(file: &str, kind: ProblemKind) -> ReadFile {
    ReadFile::NotRead {
        file: file.to_string(),
        problem: ReadProblem {
            at: SourceLine {
                file: file.to_string(),
                line: 1,
            },
            kind,
            text: String::new(),
        },
    }
}

fn unopenable(file: &str, error: &dyn std::fmt::Display) -> ReadFile {
    let why = error.to_string();
    not_read(file, ProblemKind::Unopenable { why })
}

fn too_large(file: &str) -> ReadFile {
    let limit_mib = MAX_FILE_BYTES / BYTES_PER_MIB;
    not_read(file, ProblemKind::TooLarge { limit_mib })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// mockup row 193f: a file that cannot be read is listed, and the rest
    /// of the package still opens.
    #[test]
    fn a_file_that_cannot_be_read_is_a_not_read_row() {
        let package = assemble(vec![
            too_large("huge.gbr"),
            unopenable("locked.drl", &"in use"),
        ]);
        let kinds: Vec<_> = package.not_read.iter().map(|p| &p.kind).collect();
        assert_eq!(
            kinds,
            vec![
                &ProblemKind::TooLarge { limit_mib: 256 },
                &ProblemKind::Unopenable {
                    why: "in use".into()
                },
            ]
        );
        assert_eq!(package.not_read[0].at.file, "huge.gbr");
        assert_eq!(package.not_read[0].at.line, 1);
    }

    /// A folder whose file is gone between the listing and the read still
    /// opens; a top-level file that is not there fails.
    #[test]
    fn a_missing_top_level_file_fails_and_a_listed_one_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("gone.gbr");
        assert!(read_paths(vec![gone.clone()]).is_err());
        match read_one(&gone) {
            ReadFile::NotRead { problem, .. } => {
                assert!(matches!(problem.kind, ProblemKind::Unopenable { .. }));
            }
            other => panic!("expected a Not read row, got {other:?}"),
        }
    }
}
