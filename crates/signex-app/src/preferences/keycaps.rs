//! Key caps for the Graphite keymap table: "Ctrl+Shift+S" as three small
//! caps. A trigger that is not a plain `+`-joined stroke (a sequence, a
//! pointer gesture, an unbound row) keeps the old chip.

use super::{PrefMsg, TS};
use crate::styles::key_cap;
use iced::widget::{container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

/// The keys of `label`, or `None` when it is not one plain stroke.
pub(super) fn stroke_keys(label: &str) -> Option<Vec<&str>> {
    let label = label.trim();
    if label.is_empty() || label.contains(char::is_whitespace) {
        return None;
    }
    let keys: Vec<&str> = label.split('+').collect();
    keys.iter().all(|k| !k.is_empty()).then_some(keys)
}

/// `label` as a row of key caps, or `None` when it needs the legacy chip.
pub(super) fn key_caps<'a>(label: &str, tokens: &ThemeTokens) -> Option<Element<'a, PrefMsg>> {
    let keys = stroke_keys(label)?;
    let mut line = row![].spacing(crate::styles::Spacing::DEFAULT.s);
    for key in keys {
        line = line.push(
            container(text(raw(key.to_string())).size(TS.dense))
                .padding([2.0, crate::styles::Spacing::DEFAULT.m])
                .width(Length::Shrink)
                .style(key_cap(tokens))
                .boxed(),
        );
    }
    Some(line.align_y(Alignment::Center).boxed())
}

#[cfg(test)]
mod tests {
    use super::stroke_keys;

    #[test]
    fn a_plain_stroke_splits_into_keys() {
        assert_eq!(
            stroke_keys("Ctrl+Shift+S"),
            Some(vec!["Ctrl", "Shift", "S"])
        );
        assert_eq!(stroke_keys("F5"), Some(vec!["F5"]));
    }

    #[test]
    fn sequences_gestures_and_empty_labels_keep_the_chip() {
        assert_eq!(stroke_keys(""), None);
        assert_eq!(stroke_keys("Middle Drag"), None);
        assert_eq!(stroke_keys("Ctrl++"), None);
    }
}
