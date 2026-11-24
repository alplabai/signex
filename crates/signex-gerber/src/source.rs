//! Where a thing was read from, what kind of file a name and its bytes
//! are, and the one entry point that reads any of them.

use std::path::Path;

use crate::excellon::read_excellon;
use crate::gerber::read_gerber;
use crate::image::Image;
use crate::job::{JobFile, read_job};
use crate::problem::{ProblemKind, ReadProblem};

/// A file name as the caller gave it, and a 1-based line in it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceLine {
    pub file: String,
    pub line: u32,
}

/// What a file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Gerber,
    Excellon,
    Job,
    /// Not fabrication data; listed as "Not read", never ignored.
    NotRead,
}

/// How many leading bytes the content sniff looks at.
const SNIFF_BYTES: usize = 8 * 1024;
/// How many leading non-blank lines may come before an Excellon `M48`.
const SNIFF_LINES: usize = 20;

const GERBER_EXTENSIONS: [&str; 13] = [
    "gbr", "gtl", "gbl", "gts", "gbs", "gto", "gbo", "gtp", "gbp", "gm1", "gko", "art", "pho",
];
const EXCELLON_EXTENSIONS: [&str; 2] = ["drl", "xln"];

/// The kind of `file` from its content first, then its extension.
pub fn sniff_kind(file: &str, bytes: &[u8]) -> FileKind {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(SNIFF_BYTES)]);
    kind_by_content(&head).unwrap_or_else(|| kind_by_extension(file))
}

fn kind_by_content(head: &str) -> Option<FileKind> {
    let body = head.trim_start_matches('\u{feff}').trim_start();
    if body.starts_with('{')
        && (body.contains("\"Header\"") || body.contains("\"FilesAttributes\""))
    {
        return Some(FileKind::Job);
    }
    let mut lines = body.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines
        .clone()
        .take(SNIFF_LINES)
        .any(|l| l.starts_with("M48"))
    {
        return Some(FileKind::Excellon);
    }
    let gerber = body.contains("%FS")
        || body.contains("%TF")
        || body.contains("%MO")
        || lines.any(|l| l.starts_with("G04"));
    gerber.then_some(FileKind::Gerber)
}

fn kind_by_extension(file: &str) -> FileKind {
    let extension = Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if GERBER_EXTENSIONS.contains(&extension.as_str()) {
        FileKind::Gerber
    } else if EXCELLON_EXTENSIONS.contains(&extension.as_str()) {
        FileKind::Excellon
    } else if extension == "gbrjob" {
        FileKind::Job
    } else {
        FileKind::NotRead
    }
}

/// One file as read.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadFile {
    /// A Gerber or Excellon image, with its problems.
    Image(Image),
    Job(JobFile),
    /// Nothing drawable: not text, not fabrication data, or a job file that
    /// cannot be read. The problem says which.
    NotRead {
        file: String,
        problem: ReadProblem,
    },
}

/// Read `file` from its bytes: an image, a job file, or why not.
pub fn read_file(file: &str, bytes: &[u8]) -> ReadFile {
    let text = match as_text(bytes) {
        Ok(text) => text,
        Err(line) => {
            let problem = ReadProblem {
                at: SourceLine {
                    file: file.to_string(),
                    line,
                },
                kind: ProblemKind::NotText,
                text: String::new(),
            };
            return not_read(file, problem);
        }
    };
    match sniff_kind(file, bytes) {
        FileKind::Gerber => ReadFile::Image(read_gerber(file, text)),
        FileKind::Excellon => ReadFile::Image(read_excellon(file, text)),
        FileKind::Job => match read_job(file, text) {
            Ok(job) => ReadFile::Job(job),
            Err(problem) => not_read(file, problem),
        },
        FileKind::NotRead => {
            let source = Source::new(file, text);
            not_read(file, source.problem(1, ProblemKind::NotAFabricationFile))
        }
    }
}

fn not_read(file: &str, problem: ReadProblem) -> ReadFile {
    ReadFile::NotRead {
        file: file.to_string(),
        problem,
    }
}

/// The bytes as text, or the 1-based line of the first byte that is not
/// UTF-8 text (a NUL counts as binary).
fn as_text(bytes: &[u8]) -> Result<&str, u32> {
    let line_of = |end: usize| {
        let breaks = bytes[..end].iter().filter(|b| **b == b'\n').count();
        u32::try_from(breaks).map_or(u32::MAX, |n| n.saturating_add(1))
    };
    match std::str::from_utf8(bytes) {
        Ok(text) => match text.find('\0') {
            Some(at) => Err(line_of(at)),
            None => Ok(text),
        },
        Err(error) => Err(line_of(error.valid_up_to())),
    }
}

/// A file's name and lines, for problems that quote their line.
pub(crate) struct Source<'a> {
    file: &'a str,
    lines: Vec<&'a str>,
}

impl<'a> Source<'a> {
    pub(crate) fn new(file: &'a str, text: &'a str) -> Self {
        Source {
            file,
            lines: text.lines().collect(),
        }
    }

    /// The number of the last line, at least 1.
    pub(crate) fn last_line(&self) -> u32 {
        u32::try_from(self.lines.len()).map_or(u32::MAX, |n| n.max(1))
    }

    pub(crate) fn at(&self, line: u32) -> SourceLine {
        SourceLine {
            file: self.file.to_string(),
            line,
        }
    }

    pub(crate) fn problem(&self, line: u32, kind: ProblemKind) -> ReadProblem {
        let text = usize::try_from(line)
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|i| self.lines.get(i))
            .map(|l| l.trim().to_string())
            .unwrap_or_default();
        ReadProblem {
            at: self.at(line),
            kind,
            text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_decides_before_the_extension() {
        assert_eq!(sniff_kind("a.txt", b"M48\nMETRIC\n"), FileKind::Excellon);
        assert_eq!(sniff_kind("a.dat", b"%FSLAX46Y46*%\n"), FileKind::Gerber);
        assert_eq!(sniff_kind("a.dat", b"G04 hello*\n"), FileKind::Gerber);
        assert_eq!(
            sniff_kind("a.json", b"{\n \"Header\": {}\n}"),
            FileKind::Job
        );
        assert_eq!(sniff_kind("a.GTL", b""), FileKind::Gerber);
        assert_eq!(sniff_kind("a.drl", b""), FileKind::Excellon);
        assert_eq!(sniff_kind("a.gbrjob", b""), FileKind::Job);
        assert_eq!(sniff_kind("readme.txt", b"hello"), FileKind::NotRead);
        assert_eq!(sniff_kind("noext", b"hello"), FileKind::NotRead);
    }

    #[test]
    fn bytes_that_are_not_text_name_their_line() {
        assert_eq!(as_text(b"ok\nok\n\xff"), Err(3));
        assert_eq!(as_text(b"a\0b"), Err(1));
        assert_eq!(as_text(b"fine"), Ok("fine"));
    }

    #[test]
    fn a_problem_quotes_its_line_trimmed() {
        let source = Source::new("f.gbr", "one\n  two  \r\nthree");
        let problem = source.problem(2, ProblemKind::BadCoordinate);
        assert_eq!(problem.text, "two");
        assert_eq!(problem.at.line, 2);
        assert_eq!(source.problem(9, ProblemKind::BadCoordinate).text, "");
        assert_eq!(source.last_line(), 3);
    }
}
