//! Reading a package off the UI thread (mockup row 193f): a folder's files,
//! a zip's entries read in place (never unpacked to disk), or the files
//! picked or dropped. Every file goes through `signex_gerber::read_file`,
//! so a file that is not fabrication data lands in `Package::not_read`.
//! A file whose bytes cannot be had (a permission, a lock, its size) is a
//! Not read row too; only a top-level path that is not there, a zip that
//! is not one, or a zip whose entries come to more than the total cap,
//! fails the whole read.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use signex_gerber::{
    Package, ProblemKind, ReadFile, ReadProblem, SourceLine, assemble, read_file, sniff_kind,
};

/// A file larger than this is not read: fabrication files are far smaller,
/// and a stray disk image dropped on the tab must not fill memory.
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
/// All the entries of one zip together may not decompress to more than
/// this, counted as they are read: a zip of many entries each under
/// `MAX_FILE_BYTES` must not fill memory either.
const MAX_ZIP_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const BYTES_PER_MIB: u64 = 1024 * 1024;

/// The decompressed sizes a zip read allows: per entry, and for all the
/// entries of one zip together.
#[derive(Debug, Clone, Copy)]
pub struct ZipLimits {
    pub entry_bytes: u64,
    pub total_bytes: u64,
}

impl ZipLimits {
    pub const DEFAULT: Self = Self {
        entry_bytes: MAX_FILE_BYTES,
        total_bytes: MAX_ZIP_TOTAL_BYTES,
    };
}

/// What a read gives back: the package and the paths it was opened from.
#[derive(Debug, Clone)]
pub struct PackageRead {
    pub paths: Vec<PathBuf>,
    pub package: Package,
}

/// Why a whole read failed; Messages words it from the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadFailure {
    /// A path that is not there, a zip that is not one, a worker that
    /// failed: the reason as the system gave it.
    Unreadable(String),
    /// The entries of the zip `file` decompress to more than `limit_mib`
    /// together; none of the package is opened.
    ZipTooLarge { file: String, limit_mib: u64 },
}

impl From<String> for ReadFailure {
    fn from(why: String) -> Self {
        Self::Unreadable(why)
    }
}

impl From<&str> for ReadFailure {
    fn from(why: &str) -> Self {
        Self::Unreadable(why.to_string())
    }
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
pub fn read_paths(paths: Vec<PathBuf>) -> Result<PackageRead, ReadFailure> {
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
        return too_large(&name, MAX_FILE_BYTES);
    }
    match std::fs::read(path) {
        Ok(bytes) => read_file(&name, &bytes),
        Err(error) => unopenable(&name, &error),
    }
}

/// The entries of the zip at `path`, read in memory.
fn read_zip(path: &Path) -> Result<Vec<ReadFile>, ReadFailure> {
    let file = std::fs::File::open(path).map_err(|e| describe(path, &e))?;
    read_zip_from(file, path)
}

/// The file entries of a zip archive within the default limits.
pub fn read_zip_from<R: std::io::Read + std::io::Seek>(
    reader: R,
    path: &Path,
) -> Result<Vec<ReadFile>, ReadFailure> {
    read_zip_within(reader, path, ZipLimits::DEFAULT)
}

/// The file entries of a zip archive, by their names in the archive. An
/// entry that cannot be read, or reads past `limits.entry_bytes` whatever
/// its header says, is a Not read row. The bytes actually decompressed are
/// counted across entries; past `limits.total_bytes` the whole zip is
/// refused and none of it is returned.
pub fn read_zip_within<R: std::io::Read + std::io::Seek>(
    reader: R,
    path: &Path,
    limits: ZipLimits,
) -> Result<Vec<ReadFile>, ReadFailure> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| describe(path, &e))?;
    let archive_name = file_name(path);
    let zip_too_large = || ReadFailure::ZipTooLarge {
        file: archive_name.clone(),
        limit_mib: limits.total_bytes / BYTES_PER_MIB,
    };
    let mut decompressed: u64 = 0;
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
        // One byte past whichever cap is nearer tells that cap is passed.
        let remaining = limits.total_bytes.saturating_sub(decompressed);
        let mut bytes = Vec::new();
        let read = entry
            .by_ref()
            .take(limits.entry_bytes.min(remaining).saturating_add(1))
            .read_to_end(&mut bytes);
        let read_bytes = bytes.len() as u64;
        if read_bytes > remaining {
            return Err(zip_too_large());
        }
        decompressed += read_bytes;
        files.push(match read {
            Err(error) => unopenable(&name, &error),
            Ok(_) if read_bytes > limits.entry_bytes => too_large(&name, limits.entry_bytes),
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

fn too_large(file: &str, limit_bytes: u64) -> ReadFile {
    let limit_mib = limit_bytes / BYTES_PER_MIB;
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
            too_large("huge.gbr", MAX_FILE_BYTES),
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

    /// A zip holding `sizes` bytes of text in entries `f0.gbr`, `f1.gbr`….
    fn zip_of(sizes: &[usize]) -> std::io::Cursor<Vec<u8>> {
        use std::io::Write as _;
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (index, size) in sizes.iter().enumerate() {
            writer.start_file(format!("f{index}.gbr"), options).unwrap();
            writer.write_all(&vec![b'G'; *size]).unwrap();
        }
        writer.finish().unwrap()
    }

    /// Entries each under the per-entry cap that together decompress past
    /// the total cap refuse the whole zip; nothing of it is returned.
    #[test]
    fn a_zip_past_the_total_cap_is_refused_whole() {
        let limits = ZipLimits {
            entry_bytes: 1000,
            total_bytes: 1000,
        };
        let read = read_zip_within(zip_of(&[600, 600]), Path::new("p.zip"), limits);
        assert_eq!(
            read.unwrap_err(),
            ReadFailure::ZipTooLarge {
                file: "p.zip".into(),
                limit_mib: 0,
            }
        );
    }

    /// Under both caps every entry is read; one past the per-entry cap
    /// alone is a Not read row and the rest still read.
    #[test]
    fn a_zip_under_both_caps_reads_every_entry() {
        let limits = ZipLimits {
            entry_bytes: 1000,
            total_bytes: 2000,
        };
        let files = read_zip_within(zip_of(&[600, 600]), Path::new("p.zip"), limits).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|file| !matches!(
            file,
            ReadFile::NotRead {
                problem: ReadProblem {
                    kind: ProblemKind::TooLarge { .. },
                    ..
                },
                ..
            }
        )));
        let files = read_zip_within(zip_of(&[1200, 600]), Path::new("p.zip"), limits).unwrap();
        assert_eq!(files.len(), 2);
        assert!(matches!(
            &files[0],
            ReadFile::NotRead {
                problem: ReadProblem {
                    kind: ProblemKind::TooLarge { .. },
                    ..
                },
                ..
            }
        ));
    }
}
