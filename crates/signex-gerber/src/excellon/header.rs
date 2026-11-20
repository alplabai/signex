//! The `M48` header: units with their zero mode and digits, tools, and the
//! machine settings a drawing does not need.

use super::{Part, Reader};
use crate::image::{ApertureDef, Units};
use crate::number::{Omitted, decimal_to_nm};
use crate::problem::ProblemKind;

/// Header words that set the machine, not the drawing: accepted as they
/// are. `FMAT,2` and `ICI,OFF` are the only values of theirs read.
const MACHINE_WORDS: [&str; 20] = [
    "VER", "ATC", "DETECT", "OM48", "G90", "G05", "AFS", "BLKD", "CCW", "DTMDIST", "FSB", "HPCK",
    "NCSL", "OSTOP", "OTCLMP", "PCKPARAM", "PVS", "SBK", "SIXM", "TCST",
];

/// `T01C0.300F200S65` as (1, Some(`0.300`)); `T02C` as (2, Some(``));
/// `T3` as (3, None). `None` for text that is not a tool word.
pub(super) fn split_tool(content: &str) -> Option<(u32, Option<&str>)> {
    let rest = content.strip_prefix('T')?;
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let tool = rest[..digits].parse::<u32>().ok()?;
    let tail = &rest[digits..];
    let plain = tail.chars().all(|c| {
        c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'C' | 'F' | 'S' | 'B' | 'H' | 'Z')
    });
    if !plain {
        return None;
    }
    let size = tail.find('C').map(|at| {
        let value = &tail[at + 1..];
        let end = value
            .find(|c: char| c.is_ascii_alphabetic())
            .unwrap_or(value.len());
        &value[..end]
    });
    Some((tool, size))
}

/// `000.000` as (3, 3).
fn digits_pattern(field: &str) -> Option<(u32, u32)> {
    let (integer, decimal) = field.split_once('.')?;
    let zeros = |s: &str| s.bytes().all(|b| b == b'0');
    if !(zeros(integer) && zeros(decimal)) {
        return None;
    }
    Some((
        u32::try_from(integer.len()).ok()?,
        u32::try_from(decimal.len()).ok()?,
    ))
}

impl Reader<'_> {
    pub(super) fn header_line(&mut self, content: &str, line: u32) {
        match content {
            "%" | "M95" => self.part = Part::Body,
            "M71" => self.image.units = Some(Units::Millimetres),
            "M72" => self.image.units = Some(Units::Inches),
            "FMAT,2" | "ICI,OFF" => {}
            _ if content.starts_with("METRIC") || content.starts_with("INCH") => {
                self.units_line(content, line);
            }
            _ if split_tool(content).is_some() => self.tool_definition(content, line),
            _ if content.starts_with("FMAT") || content.starts_with("ICI") => {
                self.unsupported(line, content);
            }
            _ if MACHINE_WORDS.iter().any(|w| content.starts_with(w)) => {}
            _ => self.bad_header(content, line),
        }
    }

    fn bad_header(&mut self, content: &str, line: u32) {
        self.report(
            line,
            ProblemKind::BadExcellonHeader {
                text: content.to_string(),
            },
        );
    }

    /// `METRIC,LZ,000.000`: the units, then the zeros a coordinate keeps
    /// (`LZ` keeps leading zeros, so trailing ones are left out) and the
    /// digits, both optional.
    fn units_line(&mut self, content: &str, line: u32) {
        let mut fields = content.split(',').map(str::trim);
        self.image.units = match fields.next() {
            Some("METRIC") => Some(Units::Millimetres),
            Some("INCH") => Some(Units::Inches),
            _ => {
                self.bad_header(content, line);
                return;
            }
        };
        for field in fields {
            match field {
                "LZ" => self.omitted = Some(Omitted::Trailing),
                "TZ" => self.omitted = Some(Omitted::Leading),
                _ => match digits_pattern(field) {
                    Some(digits) => self.digits = Some(digits),
                    None => self.bad_header(content, line),
                },
            }
        }
    }

    /// A tool and its size. One whose size is missing, not a number or
    /// not above zero has no size; its holes are kept and it is reported
    /// with their count when the file ends.
    pub(super) fn tool_definition(&mut self, content: &str, line: u32) {
        let Some((tool, size)) = split_tool(content) else {
            self.bad_header(content, line);
            return;
        };
        let units = self.units(line);
        let diameter = size
            .filter(|s| !s.is_empty())
            .and_then(|s| decimal_to_nm(s, units))
            .filter(|d| *d > 0);
        self.tools.insert(tool, (diameter, line));
        self.image.apertures.insert(
            tool,
            ApertureDef {
                d_code: tool,
                definition: content.to_string(),
                function: self.function.take(),
                at: self.source.at(line),
            },
        );
    }
}
