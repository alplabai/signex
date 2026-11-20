//! X2 attributes: file (`%TF`), aperture (`%TA`) and object (`%TO`)
//! attributes, and `%TD` deleting them. Values use `\uXXXX` UTF-16 escapes
//! for the characters a field cannot hold, the inverse of the Signex
//! exporter's `fab::gerber::escape`. Attributes the viewer has no use for
//! (`.Part`, `.MD5`, user attributes, ...) are valid X2 and change no
//! drawing, so they are accepted without a problem.

use super::state::Reader;
use crate::file_function::FileFunction;
use crate::image::{FilePolarity, Image};

/// `TF.FileFunction,Copper,L1,Top` as (`TF`, `.FileFunction`,
/// `Copper,L1,Top`); `None` for a statement too short to name its kind.
pub(crate) fn split_attribute(statement: &str) -> Option<(&str, &str, &str)> {
    let kind = statement.get(..2)?;
    let rest = statement.get(2..)?;
    let (name, value) = rest.split_once(',').unwrap_or((rest, ""));
    Some((kind, name.trim(), value))
}

/// Apply file attribute `name` to `image`; false when its value is not
/// understood.
pub(crate) fn file_attribute(image: &mut Image, name: &str, value: &str) -> bool {
    match name {
        ".FileFunction" => image.function = Some(FileFunction::parse(value)),
        ".FilePolarity" => match value.trim() {
            "Positive" => image.polarity = FilePolarity::Positive,
            "Negative" => image.polarity = FilePolarity::Negative,
            _ => return false,
        },
        ".GenerationSoftware" => image.generation_software = Some(value.to_string()),
        ".ProjectId" => image.project_id = Some(value.to_string()),
        _ => {}
    }
    true
}

/// A field with its `\uXXXX` escapes decoded (UTF-16, surrogate pairs
/// joined). An escape that names no character is kept as written.
pub(crate) fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find("\\u") {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(high) = unit(rest) else {
            out.push_str("\\u");
            rest = &rest[2..];
            continue;
        };
        let pair = (0xD800..0xDC00)
            .contains(&high)
            .then(|| unit(&rest[6..]))
            .flatten()
            .filter(|low| (0xDC00..0xE000).contains(low));
        let (decoded, used) = match pair {
            Some(low) => (char::decode_utf16([high, low]).next(), 12),
            None => (char::decode_utf16([high]).next(), 6),
        };
        match decoded.and_then(Result::ok) {
            Some(c) => out.push(c),
            None => out.push_str(&rest[..used]),
        }
        rest = &rest[used..];
    }
    out.push_str(rest);
    out
}

/// The UTF-16 unit of a `\uXXXX` at the start of `text`.
fn unit(text: &str) -> Option<u16> {
    let hex = text.strip_prefix("\\u")?.get(..4)?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u16::from_str_radix(hex, 16).ok()
}

impl Reader<'_> {
    pub(super) fn attribute(&mut self, statement: &str, line: u32) {
        let Some((kind, name, value)) = split_attribute(statement) else {
            self.unsupported(line, statement);
            return;
        };
        match kind {
            "TF" => {
                if !file_attribute(&mut self.image, name, value) {
                    self.unsupported(line, statement);
                }
            }
            "TA" => {
                if name == ".AperFunction" {
                    self.aperture_function = Some(value.to_string());
                }
            }
            "TO" => self.object_attribute(name, value),
            _ => self.delete_attribute(name),
        }
    }

    fn object_attribute(&mut self, name: &str, value: &str) {
        let fields: Vec<String> = value.split(',').map(unescape).collect();
        let first = fields.first().cloned().filter(|f| !f.is_empty());
        match name {
            ".N" => self.attributes.net = first,
            ".P" => {
                self.attributes.pin = first.map(|designator| {
                    let pin = fields.get(1).cloned().unwrap_or_default();
                    (designator, pin)
                });
            }
            ".C" => self.attributes.component = first,
            _ => {}
        }
    }

    /// `%TD*%` deletes every aperture and object attribute; `%TD.N*%` one.
    fn delete_attribute(&mut self, name: &str) {
        match name {
            "" => {
                self.aperture_function = None;
                self.attributes = Default::default();
            }
            ".AperFunction" => self.aperture_function = None,
            ".N" => self.attributes.net = None,
            ".P" => self.attributes.pin = None,
            ".C" => self.attributes.component = None,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_decode_to_the_exporters_input() {
        assert_eq!(unescape("GND"), "GND");
        assert_eq!(unescape("A\\u002CB\\u002AC\\u0025D\\u005C"), "A,B*C%D\\");
        assert_eq!(unescape("\\u03A9"), "Ω");
        // A character outside the basic plane is a surrogate pair.
        assert_eq!(unescape("\\uD83D\\uDE00"), "😀");
        // Not an escape, or a lone surrogate: kept as written.
        assert_eq!(unescape("\\uZZZZ\\u12"), "\\uZZZZ\\u12");
        assert_eq!(unescape("x\\uD800y"), "x\\uD800y");
    }

    #[test]
    fn attribute_statements_split_at_the_first_comma() {
        assert_eq!(
            split_attribute("TF.GenerationSoftware,Alp Lab,Signex"),
            Some(("TF", ".GenerationSoftware", "Alp Lab,Signex"))
        );
        assert_eq!(split_attribute("TD"), Some(("TD", "", "")));
        assert_eq!(split_attribute("TD.N"), Some(("TD", ".N", "")));
        assert_eq!(split_attribute("T"), None);
    }
}
