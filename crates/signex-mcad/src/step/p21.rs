//! The ISO 10303-21 clear-text encoding: instances numbered in the order
//! they are added, written one per line as `#n=RECORD;`, with the
//! exchange structure's real, string and list forms.

use std::fmt;

/// An entity instance name, `#n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Ref(u64);

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// The instances of the DATA section, in the order they were added.
#[derive(Debug, Default)]
pub(crate) struct Data {
    text: String,
    count: u64,
}

impl Data {
    /// Adds an instance whose record is `record` (what follows `=`, without
    /// the closing `;`) and returns its name.
    pub(crate) fn add(&mut self, record: impl fmt::Display) -> Ref {
        self.count += 1;
        let name = Ref(self.count);
        self.text.push_str(&format!("{name}={record};\n"));
        name
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

/// A REAL: always with a decimal point (`1.`, `0.25`, `1.5E-7`), so no
/// reader takes it for an INTEGER. Values within 1e-12 of a whole
/// multiple of 1e-9 snap to it, which drops the last-bit noise of the
/// kernel's arithmetic (`9.999999999999998` is written `10.`).
pub(crate) fn real(x: f64) -> String {
    let snapped = (x * 1e9).round() / 1e9;
    let x = if (snapped - x).abs() <= 1e-12 {
        snapped
    } else {
        x
    };
    let x = if x == 0.0 { 0.0 } else { x };
    let text = format!("{x:?}");
    match text.split_once('e') {
        Some((mantissa, exponent)) if mantissa.contains('.') => format!("{mantissa}E{exponent}"),
        Some((mantissa, exponent)) => format!("{mantissa}.E{exponent}"),
        None => text,
    }
}

/// A STRING: apostrophes and backslashes doubled, anything outside
/// printable ASCII as a `\X2\` UTF-16 run.
pub(crate) fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    let mut wide: Vec<u16> = Vec::new();
    let flush = |wide: &mut Vec<u16>, out: &mut String| {
        if wide.is_empty() {
            return;
        }
        out.push_str("\\X2\\");
        for unit in wide.drain(..) {
            out.push_str(&format!("{unit:04X}"));
        }
        out.push_str("\\X0\\");
    };
    for ch in text.chars() {
        if (' '..='~').contains(&ch) {
            flush(&mut wide, &mut out);
            match ch {
                '\'' => out.push_str("''"),
                '\\' => out.push_str("\\\\"),
                _ => out.push(ch),
            }
        } else {
            let mut units = [0u16; 2];
            wide.extend_from_slice(ch.encode_utf16(&mut units));
        }
    }
    flush(&mut wide, &mut out);
    out.push('\'');
    out
}

/// A LIST or SET of instance names: `(#1,#2)`.
pub(crate) fn refs(items: &[Ref]) -> String {
    let inner: Vec<String> = items.iter().map(Ref::to_string).collect();
    format!("({})", inner.join(","))
}

/// A list of three reals: `(x,y,z)`.
pub(crate) fn triple(v: [f64; 3]) -> String {
    format!("({},{},{})", real(v[0]), real(v[1]), real(v[2]))
}

/// A BOOLEAN or LOGICAL enumeration value.
pub(crate) fn flag(on: bool) -> &'static str {
    if on { ".T." } else { ".F." }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reals_always_carry_a_decimal_point() {
        assert_eq!(real(1.0), "1.0");
        assert_eq!(real(0.0), "0.0");
        assert_eq!(real(-0.0), "0.0");
        assert_eq!(real(1e-7), "1.E-7");
        assert_eq!(real(1.5e20), "1.5E20");
        assert_eq!(real(9.999_999_999_999_998), "10.0");
        assert_eq!(real(0.25), "0.25");
    }

    #[test]
    fn strings_escape_quotes_backslashes_and_wide_characters() {
        assert_eq!(string("R1"), "'R1'");
        assert_eq!(string("it's"), "'it''s'");
        assert_eq!(string("a\\b"), "'a\\\\b'");
        assert_eq!(string("Ω1"), "'\\X2\\03A9\\X0\\1'");
        assert_eq!(string("µΩ"), "'\\X2\\00B503A9\\X0\\'");
    }

    #[test]
    fn instances_are_numbered_in_order() {
        let mut data = Data::default();
        let a = data.add("CARTESIAN_POINT('',(0.,0.,0.))");
        let b = data.add(format!("VERTEX_POINT('',{a})"));
        assert_eq!(b.to_string(), "#2");
        assert_eq!(
            data.text(),
            "#1=CARTESIAN_POINT('',(0.,0.,0.));\n#2=VERTEX_POINT('',#1);\n"
        );
        assert_eq!(refs(&[a, b]), "(#1,#2)");
    }
}
