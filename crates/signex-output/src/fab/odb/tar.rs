//! An uncompressed POSIX ustar archive of the job's text files. Every
//! header byte is ASCII or NUL and the files are ASCII, so the archive is
//! text a `String` holds. No dates, owners or modes beyond the defaults,
//! so one board gives the same bytes.

use std::collections::BTreeSet;

const BLOCK: usize = 512;
const NAME_LEN: usize = 100;
const PREFIX_LEN: usize = 155;

/// The archive of `files` (paths relative to the job root, `/`
/// separated) under the folder `root`, each folder listed before what it
/// holds.
pub(crate) fn archive(root: &str, files: &[(String, String)]) -> String {
    let mut out = String::new();
    let mut folders = BTreeSet::new();
    for (path, contents) in files {
        let full = format!("{root}/{path}");
        let mut end = 0;
        while let Some(slash) = full[end..].find('/') {
            end += slash + 1;
            let folder = &full[..end];
            if folders.insert(folder.to_string()) {
                push_header(&mut out, folder, 0, b'5');
            }
        }
        push_header(&mut out, &full, contents.len(), b'0');
        out.push_str(contents);
        pad(&mut out);
    }
    out.extend(std::iter::repeat_n('\0', 2 * BLOCK));
    out
}

/// One header block. `kind` is `0` for a file, `5` for a folder.
fn push_header(out: &mut String, path: &str, size: usize, kind: u8) {
    let mut block = [0u8; BLOCK];
    let (prefix, name) = split_path(path);
    put(&mut block[0..100], name);
    let mode = if kind == b'5' { "0000755" } else { "0000644" };
    put(&mut block[100..108], mode);
    put(&mut block[108..116], "0000000");
    put(&mut block[116..124], "0000000");
    put(&mut block[124..136], &format!("{size:011o}"));
    put(&mut block[136..148], "00000000000");
    block[156] = kind;
    put(&mut block[257..263], "ustar");
    put(&mut block[263..265], "00");
    put(&mut block[345..500], prefix);
    block[148..156].fill(b' ');
    let sum: u32 = block.iter().map(|b| u32::from(*b)).sum();
    put(&mut block[148..156], &format!("{sum:06o}\0 "));
    out.extend(block.iter().map(|b| char::from(*b)));
}

/// The path as the header's prefix and name fields: the whole path as the
/// name when it fits, else split at the last `/` that fits both.
fn split_path(path: &str) -> (&str, &str) {
    if path.len() <= NAME_LEN {
        return ("", path);
    }
    let trimmed = path.trim_end_matches('/');
    let split = trimmed
        .match_indices('/')
        .map(|(i, _)| i)
        .rfind(|i| *i <= PREFIX_LEN && path.len() - i - 1 <= NAME_LEN);
    match split {
        Some(i) => (&path[..i], &path[i + 1..]),
        // Entity names are capped at 64 characters, so a job path always
        // splits; the tail is kept should one not.
        None => ("", &path[path.len().saturating_sub(NAME_LEN)..]),
    }
}

/// `text`'s bytes at the start of `field`, the rest left NUL.
fn put(field: &mut [u8], text: &str) {
    let bytes = text.as_bytes();
    let n = bytes.len().min(field.len());
    field[..n].copy_from_slice(&bytes[..n]);
}

/// NULs up to the next block boundary.
fn pad(out: &mut String) {
    let rest = out.len() % BLOCK;
    if rest != 0 {
        out.extend(std::iter::repeat_n('\0', BLOCK - rest));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The files of an archive: each file's path and contents, checking
    /// every header's checksum and the closing blocks.
    pub(crate) fn read_archive(archive: &str) -> Vec<(String, String)> {
        let bytes = archive.as_bytes();
        assert_eq!(bytes.len() % BLOCK, 0, "whole blocks");
        let field = |block: &[u8], from: usize, to: usize| {
            let raw = &block[from..to];
            let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
            String::from_utf8(raw[..end].to_vec()).expect("ascii")
        };
        let mut files = Vec::new();
        let mut at = 0;
        while bytes[at..at + BLOCK].iter().any(|b| *b != 0) {
            let block = &bytes[at..at + BLOCK];
            let mut blank = block.to_vec();
            blank[148..156].fill(b' ');
            let sum: u32 = blank.iter().map(|b| u32::from(*b)).sum();
            let stored = u32::from_str_radix(field(block, 148, 154).trim(), 8).expect("octal");
            assert_eq!(sum, stored, "checksum");
            assert_eq!(field(block, 257, 263), "ustar");
            let size = usize::from_str_radix(&field(block, 124, 135), 8).expect("size");
            let prefix = field(block, 345, 500);
            let name = field(block, 0, 100);
            let path = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            at += BLOCK;
            if block[156] == b'0' {
                let contents = String::from_utf8(bytes[at..at + size].to_vec()).expect("text");
                files.push((path, contents));
                at += size.div_ceil(BLOCK) * BLOCK;
            }
        }
        assert!(bytes[at..].iter().all(|b| *b == 0));
        assert_eq!(bytes.len() - at, 2 * BLOCK, "two closing blocks");
        files
    }

    #[test]
    fn files_round_trip_with_their_folders_first() {
        let long = format!("steps/pcb/layers/{}/features", "l".repeat(64));
        let files = vec![
            ("matrix/matrix".to_string(), "UNITS=MM\n".to_string()),
            (long.clone(), String::new()),
        ];
        let tar = archive("job", &files);
        let read = read_archive(&tar);
        assert_eq!(
            read,
            vec![
                ("job/matrix/matrix".to_string(), "UNITS=MM\n".to_string()),
                (format!("job/{long}"), String::new()),
            ]
        );
        assert!(tar.starts_with("job/\0"), "the root folder comes first");
        assert_eq!(tar, archive("job", &files), "the same bytes every time");
    }
}
