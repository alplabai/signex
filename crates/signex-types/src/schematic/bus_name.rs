//! Bus names — the one parser for what a bus label says a bus carries.
//!
//! A bus has no members of its own; its label names them. This module turns
//! label text into a [`BusName`] and expands it to the member net names
//! (`D[0..7]` carries `D0`..`D7`). Netlist and ERC both read it, so they can
//! never disagree on what a label means.
//!
//! # Syntax
//!
//! ```text
//! bus     = vector | group
//! vector  = prefix "[" first range-sep last "]"
//! group   = "{" item { blank item } "}"
//! item    = vector | scalar
//! range   = ".." | ":"
//! ```
//!
//! * `prefix` is non-empty and contains no whitespace or any of `[ ] { }`.
//! * `first` and `last` are unsigned decimal integers. They may run in either
//!   direction: `D[0..7]` carries `D0, D1, .. D7` and `D[7..0]` carries
//!   `D7, D6, .. D0`. The colon form `D[7:0]` means the same as `D[7..0]`.
//! * Nothing may follow the `]`: `D[0..3]A` is refused with
//!   [`BusNameError::TextAfterRange`].
//! * A group lists signals separated by blanks: `{SDA SCL}` carries `SDA` and
//!   `SCL`. An item may itself be a vector: `{A[0..1] CLK}` carries `A0`, `A1`
//!   and `CLK`. Groups do not nest.
//! * Whitespace is not allowed anywhere else (before or after the label,
//!   around the range separator, between prefix and bracket).
//!
//! A text that does not follow this is refused with a [`BusNameError`]; a bare
//! scalar such as `VCC` is a net name, not a bus name, and is refused as
//! [`BusNameError::NotABus`].

use thiserror::Error;

/// Most signals one bus name may expand to, summed over a whole group. A
/// hand-typed `D[0..4000000000]`, or a long group of wide ranges,
/// must not make the netlist allocate gigabytes or stall the checker.
pub const MAX_BUS_MEMBERS: u32 = 4096;

/// Why a label text is not a bus name.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BusNameError {
    #[error("bus name is empty")]
    Empty,
    #[error("bus name has leading or trailing whitespace")]
    SurroundingWhitespace,
    #[error("'{0}' is a plain net name, not a bus (expected 'NAME[a..b]' or '{{A B}}')")]
    NotABus(String),
    #[error("unbalanced or misplaced bracket in '{0}'")]
    BadBracket(String),
    #[error("unbalanced or misplaced brace in '{0}'")]
    BadBrace(String),
    #[error("vector '{0}' has no name before '['")]
    MissingPrefix(String),
    #[error("range in '{0}' must be two unsigned integers joined by '..' or ':'")]
    BadRange(String),
    #[error("whitespace inside '{0}'")]
    InternalWhitespace(String),
    #[error("nothing may follow the closing bracket: '{0}'")]
    TextAfterRange(String),
    #[error("a name may hold only one range: '{0}'")]
    MultipleRanges(String),
    #[error("group has no members")]
    EmptyGroup,
    #[error("groups cannot be nested: '{0}'")]
    NestedGroup(String),
    #[error("range in '{0}' expands to more than {MAX_BUS_MEMBERS} signals")]
    TooManyMembers(String),
}

impl BusNameError {
    /// True when the text was plausibly meant as a bus name and got it wrong
    /// (a stray space, a missing bracket, text after `]`). A plain net name
    /// (`VCC`) or empty text is not an attempt at one.
    pub fn looks_like_bus_name(&self) -> bool {
        !matches!(self, BusNameError::NotABus(_) | BusNameError::Empty)
    }
}

/// `prefix[first..last]`, in the direction it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusVector {
    pub prefix: String,
    pub first: u32,
    pub last: u32,
}

impl BusVector {
    /// Number of signals the vector carries.
    pub fn width(&self) -> u32 {
        self.first.abs_diff(self.last) + 1
    }

    /// Member names, in written order (`first` towards `last`).
    pub fn members(&self) -> Vec<String> {
        let name = |i: u32| format!("{}{}", self.prefix, i);
        if self.first <= self.last {
            (self.first..=self.last).map(name).collect()
        } else {
            (self.last..=self.first).rev().map(name).collect()
        }
    }
}

/// One entry of a bus name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusTerm {
    /// A single named signal (only inside a group).
    Scalar(String),
    Vector(BusVector),
}

/// A parsed bus label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusName {
    pub terms: Vec<BusTerm>,
    /// `true` for `{..}` group syntax, `false` for a lone vector.
    pub is_group: bool,
}

impl BusName {
    /// Every member net name, in written order. Duplicates are kept so a
    /// caller can see them; use [`BusName::contains`] for membership.
    pub fn members(&self) -> Vec<String> {
        self.terms
            .iter()
            .flat_map(|t| match t {
                BusTerm::Scalar(s) => vec![s.clone()],
                BusTerm::Vector(v) => v.members(),
            })
            .collect()
    }

    /// Number of signals the bus carries.
    pub fn width(&self) -> usize {
        self.terms
            .iter()
            .map(|t| match t {
                BusTerm::Scalar(_) => 1,
                BusTerm::Vector(v) => v.width() as usize,
            })
            .sum()
    }

    /// What two bus names must share to be the same bus on different sheets
    /// (#1049). A vector is identified by its prefix alone, so `D[0..7]` and
    /// `D[0..3]` are the same bus at different widths — which is what lets a
    /// width mismatch be reported against its counterpart rather than as two
    /// unrelated names. A group has no such stem, so it is identified by its
    /// whole written content: `{SDA SCL}` matches only `{SDA SCL}`.
    pub fn join_key(&self) -> String {
        match self.terms.as_slice() {
            [BusTerm::Vector(v)] if !self.is_group => format!("v:{}", v.prefix),
            terms => {
                let items: Vec<String> = terms
                    .iter()
                    .map(|t| match t {
                        BusTerm::Scalar(s) => s.clone(),
                        BusTerm::Vector(v) => format!("{}[{}..{}]", v.prefix, v.first, v.last),
                    })
                    .collect();
                format!("g:{}", items.join(" "))
            }
        }
    }

    /// True when `net_name` is one of the members (exact, case-sensitive).
    pub fn contains(&self, net_name: &str) -> bool {
        self.terms.iter().any(|t| match t {
            BusTerm::Scalar(s) => s == net_name,
            BusTerm::Vector(v) => vector_contains(v, net_name),
        })
    }
}

fn vector_contains(v: &BusVector, name: &str) -> bool {
    let Some(digits) = name.strip_prefix(&v.prefix) else {
        return false;
    };
    // `parse` alone would accept "+3" and drop leading zeros ("D03" != "D3").
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let Ok(n) = digits.parse::<u32>() else {
        return false;
    };
    digits == n.to_string() && n >= v.first.min(v.last) && n <= v.first.max(v.last)
}

/// Parse label text as a bus name. See the module docs for the syntax.
pub fn parse_bus_name(text: &str) -> Result<BusName, BusNameError> {
    if text.is_empty() {
        return Err(BusNameError::Empty);
    }
    if text.trim() != text {
        return Err(BusNameError::SurroundingWhitespace);
    }
    if let Some(inner) = text.strip_prefix('{') {
        let inner = inner
            .strip_suffix('}')
            .ok_or_else(|| BusNameError::BadBrace(text.to_string()))?;
        return parse_group(inner);
    }
    if text.contains(['{', '}']) {
        return Err(BusNameError::BadBrace(text.to_string()));
    }
    match parse_item(text)? {
        BusTerm::Vector(v) => Ok(BusName {
            terms: vec![BusTerm::Vector(v)],
            is_group: false,
        }),
        BusTerm::Scalar(s) => Err(BusNameError::NotABus(s)),
    }
}

fn parse_group(inner: &str) -> Result<BusName, BusNameError> {
    if inner.contains(['{', '}']) {
        return Err(BusNameError::NestedGroup(inner.to_string()));
    }
    let terms = inner
        .split_whitespace()
        .map(parse_item)
        .collect::<Result<Vec<_>, _>>()?;
    if terms.is_empty() {
        return Err(BusNameError::EmptyGroup);
    }
    let total: usize = terms
        .iter()
        .map(|t| match t {
            BusTerm::Scalar(_) => 1,
            BusTerm::Vector(v) => v.width() as usize,
        })
        .sum();
    if total > MAX_BUS_MEMBERS as usize {
        return Err(BusNameError::TooManyMembers(inner.to_string()));
    }
    Ok(BusName {
        terms,
        is_group: true,
    })
}

/// One whitespace-free token: a scalar or a vector.
fn parse_item(token: &str) -> Result<BusTerm, BusNameError> {
    if token.contains(char::is_whitespace) {
        return Err(BusNameError::InternalWhitespace(token.to_string()));
    }
    let Some(open) = token.find('[') else {
        if token.contains(']') {
            return Err(BusNameError::BadBracket(token.to_string()));
        }
        return Ok(BusTerm::Scalar(token.to_string()));
    };
    let Some(close) = token.find(']') else {
        return Err(BusNameError::BadBracket(token.to_string()));
    };
    if close < open {
        return Err(BusNameError::BadBracket(token.to_string()));
    }
    let (prefix, range, after) = (&token[..open], &token[open + 1..close], &token[close + 1..]);
    if prefix.is_empty() {
        return Err(BusNameError::MissingPrefix(token.to_string()));
    }
    if after.contains(['[', ']']) || range.contains('[') {
        return Err(BusNameError::MultipleRanges(token.to_string()));
    }
    if !after.is_empty() {
        return Err(BusNameError::TextAfterRange(token.to_string()));
    }
    let (first, last) = parse_range(range).ok_or_else(|| BusNameError::BadRange(token.into()))?;
    let vector = BusVector {
        prefix: prefix.to_string(),
        first,
        last,
    };
    if vector.width() > MAX_BUS_MEMBERS {
        return Err(BusNameError::TooManyMembers(token.to_string()));
    }
    Ok(BusTerm::Vector(vector))
}

fn parse_range(range: &str) -> Option<(u32, u32)> {
    let (a, b) = range.split_once("..").or_else(|| range.split_once(':'))?;
    let number = |s: &str| {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        s.parse::<u32>().ok()
    };
    Some((number(a)?, number(b)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn members(text: &str) -> Vec<String> {
        parse_bus_name(text).expect("parses").members()
    }

    #[test]
    fn ascending_and_descending_ranges() {
        assert_eq!(members("D[0..3]"), ["D0", "D1", "D2", "D3"]);
        assert_eq!(members("D[3..0]"), ["D3", "D2", "D1", "D0"]);
        assert_eq!(members("D[2..2]"), ["D2"]);
    }

    #[test]
    fn colon_form_matches_dotted_form() {
        assert_eq!(parse_bus_name("D[7:0]"), parse_bus_name("D[7..0]"));
        assert_eq!(members("D[0:2]"), ["D0", "D1", "D2"]);
    }

    #[test]
    fn text_after_the_range_is_refused() {
        for text in ["D[0..2]A", "D[0..2]_N", "D[0..2] "] {
            assert!(parse_bus_name(text).is_err(), "{text:?}");
        }
        assert_eq!(
            parse_bus_name("D[0..2]A"),
            Err(BusNameError::TextAfterRange("D[0..2]A".into()))
        );
        assert_eq!(
            parse_bus_name("{A[0..1]x CLK}"),
            Err(BusNameError::TextAfterRange("A[0..1]x".into()))
        );
    }

    #[test]
    fn a_long_group_is_refused_by_its_total_width_without_expanding() {
        let group = format!("{{{}}}", ["A[0..4095]"; 40].join(" "));
        assert!(group.len() < 500);
        assert!(matches!(
            parse_bus_name(&group),
            Err(BusNameError::TooManyMembers(_))
        ));
        assert!(parse_bus_name("{A[0..4094] CLK}").is_ok());
    }

    #[test]
    fn named_groups_and_vectors_inside_groups() {
        let g = parse_bus_name("{SDA SCL}").unwrap();
        assert!(g.is_group);
        assert_eq!(g.members(), ["SDA", "SCL"]);
        assert_eq!(members("{A[0..1]  CLK}"), ["A0", "A1", "CLK"]);
        assert_eq!(parse_bus_name("{A[0..1] CLK}").unwrap().width(), 3);
    }

    #[test]
    fn contains_is_exact() {
        let n = parse_bus_name("D[0..7]").unwrap();
        assert!(n.contains("D0") && n.contains("D7"));
        assert!(!n.contains("D8"));
        assert!(!n.contains("D33"), "typo is not a member");
        assert!(!n.contains("D03"), "leading zero is a different net");
        assert!(!n.contains("D+3"));
        assert!(!n.contains("D"));
        assert!(!n.contains("D3 "));
        let g = parse_bus_name("{SDA SCL}").unwrap();
        assert!(g.contains("SDA") && !g.contains("SDAx"));
    }

    #[test]
    fn join_key_ignores_a_vectors_range_but_not_a_groups_content() {
        let key = |t: &str| parse_bus_name(t).unwrap().join_key();
        assert_eq!(key("D[0..7]"), key("D[3:0]"), "one stem, any range");
        assert_ne!(key("D[0..7]"), key("A[0..7]"));
        assert_eq!(key("{SDA SCL}"), key("{SDA SCL}"));
        assert_ne!(key("{SDA SCL}"), key("{SDA}"));
        assert_ne!(key("{D[0..1]}"), key("D[0..1]"), "a group is not a vector");
    }

    #[test]
    fn width_counts_signals() {
        assert_eq!(parse_bus_name("D[7..0]").unwrap().width(), 8);
        assert_eq!(parse_bus_name("D[0:0]").unwrap().width(), 1);
    }

    #[test]
    fn malformed_text_is_refused_with_a_typed_error() {
        use BusNameError::*;
        let cases = [
            ("", Empty),
            (" D[0..7]", SurroundingWhitespace),
            ("D[0..7] ", SurroundingWhitespace),
            ("VCC", NotABus("VCC".into())),
            ("D[0..7", BadBracket("D[0..7".into())),
            ("D0..7]", BadBracket("D0..7]".into())),
            ("D]0..7[", BadBracket("D]0..7[".into())),
            ("[0..7]", MissingPrefix("[0..7]".into())),
            ("D[a..7]", BadRange("D[a..7]".into())),
            ("D[0..]", BadRange("D[0..]".into())),
            ("D[-1..3]", BadRange("D[-1..3]".into())),
            ("D[0-7]", BadRange("D[0-7]".into())),
            ("D [0..7]", InternalWhitespace("D [0..7]".into())),
            ("D[0 ..7]", InternalWhitespace("D[0 ..7]".into())),
            ("D[0..1]B[0..1]", MultipleRanges("D[0..1]B[0..1]".into())),
            ("{}", EmptyGroup),
            ("{ }", EmptyGroup),
            ("{A {B}}", NestedGroup("A {B}".into())),
            ("{A", BadBrace("{A".into())),
            ("A}", BadBrace("A}".into())),
            ("D[0..3]{", BadBrace("D[0..3]{".into())),
            ("D[0..99999]", TooManyMembers("D[0..99999]".into())),
        ];
        for (text, want) in cases {
            assert_eq!(parse_bus_name(text), Err(want), "{text:?}");
        }
    }
}
