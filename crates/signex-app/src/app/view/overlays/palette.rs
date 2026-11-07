//! Graphite command-palette rows and footer: a source glyph, the label with
//! the typed characters highlighted, key caps for the binding, and a hint
//! footer. The legacy rows stay in `overlays/mod.rs`.

use iced::widget::{Row, Space, button, column, container, rich_text, scrollable, span, text};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

use crate::app::command_palette::{CommandAction, CommandEntry, CommandSource};
use crate::keymap::metadata_for;
use crate::styles::{self, Radii, Spacing, TypeScale, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// Path subtitle of a command row: its catalog category ("Place",
/// "Modify", ...). Rows of other sources carry their own detail line.
pub(super) fn path_subtitle(entry: &CommandEntry) -> Option<String> {
    let category = match &entry.action {
        CommandAction::Command(id) => metadata_for(id)?.category,
        CommandAction::Panel(_) => "panels",
        _ => return None,
    };
    let mut chars = category.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}

/// Group header: the group's title and how many of its rows are shown.
pub(super) fn group_header<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    source: CommandSource,
    count: usize,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    container(
        Row::<Element<'a, M>>::new()
            .spacing(SP.m)
            .align_y(Alignment::Center)
            .push(
                text(source.group_title())
                    .size(TS.section)
                    .style(styles::section_label(theme_tokens))
                    .boxed(),
            )
            .push(
                text(count.to_string())
                    .size(TS.section)
                    .font(mono())
                    .color(r.ink4)
                    .boxed(),
            ),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// The card's own search row. The main window types into the chrome strip,
/// so this echoes the live query (read only); a window palette passes its
/// real input as `input`.
pub(super) fn search_row<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    query: &str,
    input: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let ink = ti(theme_tokens.text);
    let body: Element<'a, M> = match input {
        Some(input) => input,
        None if query.is_empty() => text("Search commands, parts, nets")
            .size(TS.body)
            .color(r.ink4)
            .width(Length::Fill)
            .boxed(),
        None => text(query.to_string())
            .size(TS.body)
            .color(ink)
            .width(Length::Fill)
            .boxed(),
    };
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.l).align_y(Alignment::Center);
    line = line.push(text("\u{2315}").size(TS.body).color(r.ink4).boxed());
    line = line.push(body);
    line = line.push(cap(theme_tokens, "Esc"));
    let rule = r.line_soft;
    container(line)
        .padding([SP.l, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border::default().bottom(iced::border::Side::default().width(1.0).color(rule)),
            ..container::Style::default()
        })
        .boxed()
}

/// Themed scrollbar of the result list: a quiet rail, a thin rounded thumb.
pub(super) fn scrollbar_style(
    theme_tokens: &ThemeTokens,
) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style + 'static {
    let r = tokens::resolve(theme_tokens);
    let (thumb, thumb_hot) = (r.line_strong, r.ink4);
    move |theme: &Theme, status: scrollable::Status| {
        let mut style = scrollable::default(theme, status);
        let hot = !matches!(status, scrollable::Status::Active { .. });
        for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
            rail.background = None;
            rail.border = Border::default();
            rail.scroller.background = Background::Color(if hot { thumb_hot } else { thumb });
            rail.scroller.border = Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            };
        }
        style
    }
}

/// Which characters of `label` the `query` matched: the first contiguous
/// case-insensitive occurrence, else the in-order subsequence, else none.
pub(super) fn match_mask(label: &str, query: &str) -> Vec<bool> {
    let low = |c: char| c.to_lowercase().next().unwrap_or(c);
    let chars: Vec<char> = label.chars().map(low).collect();
    let wanted: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(low)
        .collect();
    let mut mask = vec![false; chars.len()];
    if wanted.is_empty() || wanted.len() > chars.len() {
        return mask;
    }
    let run = (0..=chars.len() - wanted.len()).find(|&i| chars[i..i + wanted.len()] == wanted[..]);
    if let Some(start) = run {
        mask[start..start + wanted.len()].fill(true);
        return mask;
    }
    let mut next = 0;
    for (i, c) in chars.iter().enumerate() {
        if next < wanted.len() && *c == wanted[next] {
            mask[i] = true;
            next += 1;
        }
    }
    if next < wanted.len() {
        mask.fill(false);
    }
    mask
}

/// The label as runs of plain and highlighted text.
pub(super) fn label_runs(label: &str, query: &str) -> Vec<(String, bool)> {
    let mask = match_mask(label, query);
    let mut runs: Vec<(String, bool)> = Vec::new();
    for (c, hit) in label.chars().zip(mask) {
        match runs.last_mut() {
            Some((s, h)) if *h == hit => s.push(c),
            _ => runs.push((c.to_string(), hit)),
        }
    }
    runs
}

/// The keys of a shortcut label, one per cap: `"Ctrl+Shift+P"` gives
/// `["Ctrl", "Shift", "P"]`; a trailing `+` key is kept.
pub(super) fn shortcut_keys(label: &str) -> Vec<String> {
    label
        .replace("++", "+\u{0}")
        .split(|c: char| c == '+' || c == ',' || c.is_whitespace())
        .filter(|k| !k.is_empty())
        .map(|k| k.replace('\u{0}', "+"))
        .collect()
}

/// A shortcut label as a row of key caps.
fn caps_row<'a, M: 'a>(theme_tokens: &ThemeTokens, label: &str) -> Element<'a, M> {
    let mut caps: Row<Element<'a, M>> = Row::new().spacing(SP.xs + 1.0);
    for key in shortcut_keys(label) {
        caps = caps.push(cap(theme_tokens, &key));
    }
    caps.boxed()
}

/// One key cap.
pub(super) fn cap<'a, M: 'a>(theme_tokens: &ThemeTokens, label: &str) -> Element<'a, M> {
    container(text(label.to_string()).size(TS.caption).font(mono()))
        .padding([1.0, SP.s + 1.0])
        .style(styles::key_cap(theme_tokens))
        .boxed()
}

/// One result row.
pub(super) fn result_row<'a, M: Clone + 'a>(
    theme_tokens: &ThemeTokens,
    entry: &CommandEntry,
    query: &str,
    is_active: bool,
    on_press: M,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let (ink, accent) = (ti(theme_tokens.text), r.accent_text);
    let spans: Vec<_> = label_runs(&entry.label, query)
        .into_iter()
        .map(|(run, hit)| {
            let s = span::<()>(run).color(if hit { accent } else { ink });
            if hit { s.font(bold()) } else { s }
        })
        .collect();
    let mut label_col: iced::widget::Column<Element<'a, M>> =
        column![].spacing(SP.xs).width(Length::Fill);
    label_col = label_col.push(rich_text::<(), M, Theme>(spans).size(TS.label).boxed());
    let is_command = entry.source == CommandSource::Command;
    if let Some(path) = path_subtitle(entry) {
        label_col = label_col.push(text(path).size(TS.caption).color(r.ink3).boxed());
    } else if !is_command && !entry.detail.is_empty() {
        label_col = label_col.push(
            text(entry.detail.clone())
                .size(TS.caption)
                .font(mono())
                .color(r.ink3)
                .boxed(),
        );
    }
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.l).align_y(Alignment::Center);
    line = line.push(label_col.boxed());
    if is_command && !entry.detail.is_empty() {
        line = line.push(caps_row(theme_tokens, &entry.detail));
    }
    let (active_fill, hover_fill) = (
        r.sunken,
        styles::tokens::overlay(theme_tokens, styles::tokens::Overlay::Hover),
    );
    button(line)
        .width(Length::Fill)
        .padding([SP.m, SP.l])
        .on_press(on_press)
        .style(move |_: &Theme, status: button::Status| {
            let fill = match status {
                button::Status::Hovered | button::Status::Pressed => Some(hover_fill),
                _ if is_active => Some(active_fill),
                _ => None,
            };
            button::Style {
                background: fill.map(Background::Color),
                text_color: ink,
                border: Border {
                    radius: Radii::GRAPHITE.button.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        })
        .boxed()
}

fn bold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// Hint footer: navigation caps, the Run cap and what Enter will run.
pub(super) fn hint_footer<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    selected: Option<&CommandEntry>,
    extra: Option<String>,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let hint = |s: &'static str| text(s).size(TS.caption).color(r.ink3).boxed();
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.m).align_y(Alignment::Center);
    line = line.push(cap(theme_tokens, "\u{2191}"));
    line = line.push(cap(theme_tokens, "\u{2193}"));
    line = line.push(hint("Navigate"));
    line = line.push(cap(theme_tokens, "Enter"));
    line = line.push(hint("Open"));
    line = line.push(cap(theme_tokens, "Esc"));
    line = line.push(hint("Close"));
    line = line.push(Space::new().width(Length::Fill).boxed());
    if let Some(entry) = selected {
        line = line.push(
            text(format!("Enter runs {}", entry.label))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    } else if let Some(more) = extra {
        line = line.push(text(more).size(TS.caption).color(r.ink3).boxed());
    }
    let rule = r.line_soft;
    container(line)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(r.sunken)),
            border: Border::default().top(iced::border::Side::default().width(1.0).color(rule)),
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_label_splits_into_keys() {
        assert_eq!(shortcut_keys("Ctrl+Shift+P"), ["Ctrl", "Shift", "P"]);
        assert_eq!(shortcut_keys("Ctrl++"), ["Ctrl", "+"]);
        assert_eq!(shortcut_keys("T A"), ["T", "A"]);
    }

    #[test]
    fn contiguous_match_is_preferred_and_case_insensitive() {
        let mask = match_mask("Annotate Schematics", "ann");
        assert_eq!(&mask[..4], &[true, true, true, false]);
        assert!(mask[4..].iter().all(|m| !m));
    }

    #[test]
    fn subsequence_matches_in_order_or_not_at_all() {
        let mask = match_mask("Annotate Quietly", "aq");
        assert_eq!(mask.iter().filter(|m| **m).count(), 2);
        assert!(match_mask("abc", "cba").iter().all(|m| !m));
    }

    #[test]
    fn empty_query_highlights_nothing() {
        assert_eq!(
            label_runs("Place wire", ""),
            vec![("Place wire".into(), false)]
        );
    }
}
