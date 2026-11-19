//! Key caps for the Graphite keymap page: "Ctrl+Shift+S" as three small
//! caps, a sequence as one group of caps per stroke with "then" between
//! them. A trigger that is not a plain `+`-joined stroke (a pointer
//! gesture, an unbound row, an invalid draft) keeps the old chip.

use super::{PrefMsg, TS};
use crate::styles::{ti, tokens::resolve};
use iced::alignment::Horizontal;
use iced::widget::{Row, container, row};
use iced::{Alignment, Background, Border, Element, Font, Length, Theme, Widget as _};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

/// Height of one cap: 18 px at the 12 px body.
const KEY_H: f32 = TS.body * 1.5;
/// Corner radius of a cap, as `styles::key_cap`.
const KEY_RADIUS: f32 = 4.0;
/// Gap between the caps of one stroke: 3 px at the 12 px body.
const KEY_GAP: f32 = TS.body / 4.0;

/// The keys of `label`, or `None` when it is not one plain stroke.
pub(super) fn stroke_keys(label: &str) -> Option<Vec<&str>> {
    let label = label.trim();
    if label.is_empty() || label.contains(char::is_whitespace) {
        return None;
    }
    let keys: Vec<&str> = label.split('+').collect();
    keys.iter().all(|k| !k.is_empty()).then_some(keys)
}

/// The caps of one stroke label, or `None` when it is not one stroke.
fn cap_keys(label: &str) -> Option<Vec<String>> {
    // macOS labels (`⇧⌘S`) have no `+`: each symbol is its own cap.
    match crate::keymap::mac_stroke_caps(label.trim()) {
        Some(caps) if !label.trim().contains(char::is_whitespace) => Some(caps),
        _ => Some(stroke_keys(label)?.into_iter().map(String::from).collect()),
    }
}

/// The mock's `.key`: raised fill, strong hairline, ink text.
fn cap_style(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let r = resolve(tokens);
    let (bg, line, ink) = (r.raised, r.line_strong, ti(tokens.text));
    move |_| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(ink),
        border: Border {
            width: 1.0,
            color: line,
            radius: KEY_RADIUS.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn cap<'a>(key: String, tokens: &ThemeTokens) -> Element<'a, PrefMsg> {
    let ink = ti(tokens.text);
    container(
        text(signex_i18n::UserText::new(key))
            .size(TS.caption)
            .font(Font::MONOSPACE)
            .color(ink)
            .align_x(Horizontal::Center),
    )
    .padding([0.0, crate::styles::Spacing::DEFAULT.s])
    .height(Length::Fixed(KEY_H))
    .center_y(Length::Fixed(KEY_H))
    .width(Length::Shrink)
    .style(cap_style(tokens))
    .boxed()
}

/// `label` as a row of key caps, or `None` when it needs the legacy chip.
pub(super) fn key_caps<'a>(label: &str, tokens: &ThemeTokens) -> Option<Element<'a, PrefMsg>> {
    let keys = cap_keys(label)?;
    let mut line = row![].spacing(KEY_GAP);
    for key in keys {
        line = line.push(cap(key, tokens));
    }
    Some(line.align_y(Alignment::Center).boxed())
}

/// A key sequence as caps: one group per stroke label, `then` in the
/// faint caption ink between groups. `None` when a stroke is not one plain
/// stroke, or there is none.
pub(super) fn sequence_caps<'a>(
    strokes: &[String],
    then: &Localized,
    tokens: &ThemeTokens,
) -> Option<Element<'a, PrefMsg>> {
    if strokes.is_empty() {
        return None;
    }
    let faint = resolve(tokens).ink4;
    let mut line: Row<Element<'a, PrefMsg>> =
        Row::new().spacing(KEY_GAP).align_y(Alignment::Center);
    for (index, stroke) in strokes.iter().enumerate() {
        if index > 0 {
            line = line.push(text(then.clone()).size(TS.caption).color(faint).boxed());
        }
        line = line.push(key_caps(stroke, tokens)?);
    }
    Some(line.boxed())
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
