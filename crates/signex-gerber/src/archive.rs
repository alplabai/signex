use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::{GerberLoadBatch, GerberLoadFailure, load_autodetected_reader};

const MAX_ARCHIVE_MEMBER_BYTES: u64 = 16 * 1024 * 1024;

pub fn load_zip_archive(path: impl AsRef<Path>) -> GerberLoadBatch {
    let path = path.as_ref();
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            return GerberLoadBatch {
                layers: Vec::new(),
                failures: vec![GerberLoadFailure {
                    path: path.to_path_buf(),
                    message: format!("could not open ZIP archive: {error}"),
                }],
            };
        }
    };
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("archive.zip");
    load_zip_reader(name, file)
}

/// Reads supported fabrication members directly from an archive without
/// extracting any member to the filesystem.
pub fn load_zip_reader<R>(archive_name: &str, reader: R) -> GerberLoadBatch
where
    R: Read + Seek,
{
    let mut archive = match ZipArchive::new(reader) {
        Ok(archive) => archive,
        Err(error) => {
            return GerberLoadBatch {
                layers: Vec::new(),
                failures: vec![GerberLoadFailure {
                    path: PathBuf::from(archive_name),
                    message: format!("invalid ZIP archive: {error}"),
                }],
            };
        }
    };
    let mut batch = GerberLoadBatch::default();
    let mut duplicate_counts = HashMap::<String, usize>::new();

    for index in 0..archive.len() {
        let mut member = match archive.by_index(index) {
            Ok(member) => member,
            Err(error) => {
                batch.failures.push(GerberLoadFailure {
                    path: PathBuf::from(archive_name),
                    message: format!("could not read ZIP member {index}: {error}"),
                });
                continue;
            }
        };
        if member.is_dir() {
            continue;
        }
        let raw_name = member.name().to_owned();
        let member_path = PathBuf::from(format!("{archive_name}::{raw_name}"));
        if member.enclosed_name().is_none() {
            batch.failures.push(GerberLoadFailure {
                path: member_path,
                message: "unsafe archive member path was rejected".to_owned(),
            });
            continue;
        }
        if !is_fabrication_member(&raw_name) {
            continue;
        }
        if member.size() > MAX_ARCHIVE_MEMBER_BYTES {
            batch.failures.push(GerberLoadFailure {
                path: member_path,
                message: format!(
                    "archive member exceeds the {} MiB safety limit",
                    MAX_ARCHIVE_MEMBER_BYTES / (1024 * 1024),
                ),
            });
            continue;
        }

        let mut source = Vec::with_capacity(member.size() as usize);
        if let Err(error) = member.read_to_end(&mut source) {
            batch.failures.push(GerberLoadFailure {
                path: member_path,
                message: format!("could not read archive member: {error}"),
            });
            continue;
        }
        match load_autodetected_reader(&raw_name, source.as_slice()) {
            Ok(mut layer) => {
                let display_name = Path::new(&raw_name)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or(&raw_name)
                    .to_owned();
                let count = duplicate_counts.entry(display_name.clone()).or_default();
                *count += 1;
                layer.name = if *count > 1 {
                    format!("{display_name} ({count})")
                } else {
                    display_name
                };
                batch.layers.push(layer);
            }
            Err(failure) => batch.failures.push(GerberLoadFailure {
                path: member_path,
                message: failure.message,
            }),
        }
    }
    batch
}

fn is_fabrication_member(name: &str) -> bool {
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "gbr"
            | "ger"
            | "pho"
            | "art"
            | "gbx"
            | "gtl"
            | "gbl"
            | "gto"
            | "gbo"
            | "gts"
            | "gbs"
            | "gtp"
            | "gbp"
            | "gm1"
            | "gm2"
            | "gko"
            | "gvc"
            | "gsp"
            | "drl"
            | "drd"
    ) || (extension.starts_with("gl")
        && extension[2..]
            .chars()
            .all(|character| character.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::*;

    #[test]
    fn archive_loads_supported_members_ignores_irrelevant_and_names_duplicates() {
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        let gerber = b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nM02*\n";
        archive.start_file("top/board.gtl", options).unwrap();
        archive.write_all(gerber).unwrap();
        archive.start_file("backup/board.gtl", options).unwrap();
        archive.write_all(gerber).unwrap();
        archive.start_file("holes.drl", options).unwrap();
        archive
            .write_all(b"M48\nMETRIC\nT01C0.8\n%\nG05\nT01\nX1.0Y1.0\nM30\n")
            .unwrap();
        archive.start_file("README.txt", options).unwrap();
        archive.write_all(b"ignored documentation").unwrap();
        let archive = archive.finish().unwrap();

        let batch = load_zip_reader("board.zip", archive);

        assert!(batch.failures.is_empty());
        assert_eq!(batch.layers.len(), 3);
        assert_eq!(batch.layers[0].name, "board.gtl");
        assert_eq!(batch.layers[1].name, "board.gtl (2)");
        assert_eq!(batch.layers[2].name, "holes.drl");
    }

    #[test]
    fn archive_rejects_unsafe_member_paths_without_extraction() {
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file("../outside.gbr", SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.5*%\nD10*\nX0Y0D03*\nM02*\n")
            .unwrap();
        let archive = archive.finish().unwrap();

        let batch = load_zip_reader("unsafe.zip", archive);

        assert!(batch.layers.is_empty());
        assert_eq!(batch.failures.len(), 1);
        assert_eq!(
            batch.failures[0].message,
            "unsafe archive member path was rejected",
        );
    }
}
