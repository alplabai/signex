//! Rendered text, the one kind of text only this crate makes.

use std::fmt;

/// A rendered message, for display only. It has no `Serialize`, and no
/// equality with `&str`: state, files and undo labels keep the
/// [`MessageId`](crate::MessageId) and its arguments, never the rendered
/// text.
#[derive(Debug, Clone)]
pub struct Localized(String);

impl Localized {
    pub(crate) fn new(text: String) -> Self {
        Self(text)
    }

    /// The text before the argument value `value`, the value itself and the
    /// text after it, for drawing the value as its own widget (a key cap
    /// inside a sentence). The isolated placeable is looked for first, so a
    /// value that also occurs in the words is found where the argument is;
    /// the isolation marks are dropped with the split. `None` when the
    /// translation leaves the argument out or `value` is empty.
    pub fn split_around(&self, value: &str) -> Option<(Localized, Localized, Localized)> {
        if value.is_empty() {
            return None;
        }
        let isolated = format!("\u{2068}{value}\u{2069}");
        let (before, after) = self
            .0
            .split_once(&isolated)
            .or_else(|| self.0.split_once(value))?;
        Some((
            Self(before.to_owned()),
            Self(value.to_owned()),
            Self(after.to_owned()),
        ))
    }

    /// Hand the text to a widget.
    pub fn into_string(self) -> String {
        self.0
    }

    /// The text cut into runs at each verbatim occurrence of one of `marks`,
    /// as `(run, is_mark)` in reading order; the runs join back to the text.
    /// The sentence stays one catalog message: a translation that words a
    /// mark differently keeps that part plain. Where two marks start at the
    /// same place, the longer one wins.
    pub fn runs_marking(self, marks: &[&Localized]) -> Vec<(Localized, bool)> {
        let mut runs = Vec::new();
        let mut rest = self.0.as_str();
        loop {
            let next = marks
                .iter()
                .map(|mark| mark.0.as_str())
                .filter(|mark| !mark.is_empty())
                .filter_map(|mark| rest.find(mark).map(|at| (at, mark.len())))
                .min_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
            let Some((at, len)) = next else {
                break;
            };
            if at > 0 {
                runs.push((Self(rest[..at].to_owned()), false));
            }
            runs.push((Self(rest[at..at + len].to_owned()), true));
            rest = &rest[at + len..];
        }
        if !rest.is_empty() {
            runs.push((Self(rest.to_owned()), false));
        }
        runs
    }
}

impl fmt::Display for Localized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Localized;

    fn l(text: &str) -> Localized {
        Localized::new(text.to_owned())
    }

    fn plain(runs: Vec<(Localized, bool)>) -> Vec<(String, bool)> {
        runs.into_iter()
            .map(|(r, m)| (r.into_string(), m))
            .collect()
    }

    #[test]
    fn the_marks_are_cut_out_in_reading_order_and_the_runs_join_back() {
        let runs = l("Use A or B, then A.").runs_marking(&[&l("B"), &l("A")]);
        let runs = plain(runs);
        let want = [
            ("Use ", false),
            ("A", true),
            (" or ", false),
            ("B", true),
            (", then ", false),
            ("A", true),
            (".", false),
        ];
        let want: Vec<(String, bool)> = want.iter().map(|(s, m)| (s.to_string(), *m)).collect();
        assert_eq!(runs, want);
    }

    #[test]
    fn a_mark_the_text_does_not_contain_leaves_it_one_plain_run() {
        let runs = plain(l("Nothing here.").runs_marking(&[&l("absent"), &l("")]));
        assert_eq!(runs, vec![("Nothing here.".to_owned(), false)]);
    }

    #[test]
    fn the_longer_mark_wins_where_two_start_together() {
        let runs = plain(l("Per sheet undo").runs_marking(&[&l("Per"), &l("Per sheet")]));
        assert_eq!(
            runs,
            vec![("Per sheet".to_owned(), true), (" undo".to_owned(), false)]
        );
    }
}
