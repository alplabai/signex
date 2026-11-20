//! How a Find needle matches text: the match options and the compiled query
//! that scan and replace both take (#2525).

use std::ops::Range;

use super::fold;

/// How the needle matches. Scan and replace take the same value, so a hit
/// the scan lists is always one the replace rewrites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchOptions {
    /// Upper and lower case differ. Off, case folds under the rule in
    /// `fold`.
    pub match_case: bool,
    /// A hit may not touch a letter, digit or `_` on either side, so `VOUT`
    /// does not match inside `VOUT_3V3`.
    pub whole_word: bool,
    /// The needle is a regular expression.
    pub regex: bool,
}

impl MatchOptions {
    /// A case-insensitive substring: the matching Find had before #2525.
    pub const SUBSTRING: Self = Self {
        match_case: false,
        whole_word: false,
        regex: false,
    };

    /// The dialog's default: whole words, so a net name never matches inside
    /// a longer one.
    pub const WHOLE_WORD: Self = Self {
        whole_word: true,
        ..Self::SUBSTRING
    };
}

impl Default for MatchOptions {
    fn default() -> Self {
        Self::WHOLE_WORD
    }
}

/// A regex needle that does not compile. Carries the parser's message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid pattern: {0}")]
pub struct PatternError(pub String);

#[derive(Debug, Clone, Default)]
enum Matcher {
    /// An empty needle: matches nothing.
    #[default]
    Nothing,
    Literal {
        /// The needle's chars, folded unless `match_case`.
        needle: Vec<char>,
        match_case: bool,
    },
    Pattern(regex::Regex),
}

/// A needle compiled under its [`MatchOptions`]. Built once per search and
/// handed to [`super::scan_sheet`] and [`super::replace_commands`].
#[derive(Debug, Clone, Default)]
pub struct FindQuery {
    matcher: Matcher,
    options: MatchOptions,
}

/// A char that makes a hit next to it part of a longer word.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether the byte range `at` of `text` stands alone: no word char touches
/// it on either side.
fn is_whole_word(text: &str, at: &Range<usize>) -> bool {
    let before = text[..at.start].chars().next_back();
    let after = text[at.end..].chars().next();
    !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
}

impl FindQuery {
    /// Compile `needle` under `options`. A literal needle is trimmed; a
    /// regex is kept as typed, since its spaces are part of the pattern. An
    /// empty needle compiles to a query that matches nothing; only a regex
    /// can fail.
    pub fn new(needle: &str, options: MatchOptions) -> Result<Self, PatternError> {
        let needle = if options.regex { needle } else { needle.trim() };
        let matcher = if needle.is_empty() {
            Matcher::Nothing
        } else if options.regex {
            let pattern = regex::RegexBuilder::new(needle)
                .case_insensitive(!options.match_case)
                .build()
                .map_err(|e| PatternError(e.to_string()))?;
            Matcher::Pattern(pattern)
        } else {
            let needle = if options.match_case {
                needle.chars().collect()
            } else {
                fold::folded(needle)
            };
            Matcher::Literal {
                needle,
                match_case: options.match_case,
            }
        };
        Ok(Self { matcher, options })
    }

    /// The options it was compiled with.
    pub fn options(&self) -> MatchOptions {
        self.options
    }

    /// The needle was empty, so nothing matches.
    pub fn is_empty(&self) -> bool {
        matches!(self.matcher, Matcher::Nothing)
    }

    /// Whether `text` holds at least one hit.
    pub fn is_match(&self, text: &str) -> bool {
        !self.ranges(text).is_empty()
    }

    /// The byte ranges of every hit in `text`, left to right, never
    /// overlapping and never empty.
    pub fn ranges(&self, text: &str) -> Vec<Range<usize>> {
        let whole = |at: &Range<usize>| !self.options.whole_word || is_whole_word(text, at);
        match &self.matcher {
            Matcher::Nothing => Vec::new(),
            Matcher::Pattern(pattern) => pattern
                .find_iter(text)
                .map(|m| m.range())
                .filter(|at| !at.is_empty() && whole(at))
                .collect(),
            Matcher::Literal { needle, match_case } => {
                literal_ranges(text, needle, *match_case, whole)
            }
        }
    }

    /// `text` with every hit replaced by `replacement`. A regex replacement
    /// expands `$1` / `${name}` from the hit's groups; a literal one is
    /// written as typed.
    pub fn replace_all(&self, text: &str, replacement: &str) -> String {
        let ranges = self.ranges(text);
        if ranges.is_empty() {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for at in ranges {
            out.push_str(&text[last..at.start]);
            match &self.matcher {
                Matcher::Pattern(pattern) => {
                    // The hit is a whole match of the pattern, so it captures
                    // again at the same start.
                    match pattern.captures_at(text, at.start) {
                        Some(caps) if caps.get(0).map(|m| m.range()) == Some(at.clone()) => {
                            caps.expand(replacement, &mut out);
                        }
                        _ => out.push_str(replacement),
                    }
                }
                _ => out.push_str(replacement),
            }
            last = at.end;
        }
        out.push_str(&text[last..]);
        out
    }
}

/// The literal hits of `needle` in `text`. Positions are found on the folded
/// chars (same count as the original, see `fold`) and returned as byte
/// ranges of `text`.
fn literal_ranges(
    text: &str,
    needle: &[char],
    match_case: bool,
    whole: impl Fn(&Range<usize>) -> bool,
) -> Vec<Range<usize>> {
    let hay: Vec<char> = if match_case {
        text.chars().collect()
    } else {
        fold::folded(text)
    };
    // Byte offset of every char, plus the end of the text.
    let offsets: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < hay.len() {
        if fold::matches_at(&hay, i, needle) {
            let at = offsets[i]..offsets[i + needle.len()];
            if whole(&at) {
                out.push(at);
                i += needle.len();
                continue;
            }
        }
        i += 1;
    }
    out
}
