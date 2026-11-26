//! Graphite command-palette rows and footer: a source glyph, the label with
//! the typed characters highlighted, key caps for the binding, and a hint
//! footer. The card itself and where it sits are in `dropdown.rs`.

use iced::widget::{Row, Space, container, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, shortcut_ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, rich_text, span, text};

use crate::app::command_palette::{CommandAction, CommandEntry, CommandSource};
use crate::keymap::metadata_for;
use crate::styles::{self, Radii, Spacing, TypeScale, chrome_px, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

mod dropdown;
mod tile;

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// Path subtitle of a command row: its catalog category ("Place",
/// "Modify", ...). Rows of other sources carry their own detail line.
pub(super) fn path_subtitle(i18n: &I18n, entry: &CommandEntry) -> Option<String> {
    let category = match &entry.action {
        CommandAction::Command(id) => metadata_for(id)?.category,
        CommandAction::Panel(_) => "panels",
        _ => return None,
    };
    let first = category.chars().next()?;
    let rest = &category[first.len_utf8()..];
    Some(format!(
        "{}{rest}",
        i18n.upper_for_display(&first.to_string())
    ))
}

/// Group header: the group's title and how many of its rows are shown.
pub(super) fn group_header<'a, M: 'a>(
    i18n: &I18n,
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
                text(t!(i18n, source.group_title()))
                    .size(TS.section)
                    .style(styles::section_label(theme_tokens))
                    .boxed(),
            )
            .push(
                // raw: the count is already formatted for the locale by `I18n::count`.
                text(raw(i18n.count(count as i64)))
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

/// The card's own search row, for a palette with no title-bar field to type
/// in: one opened in its own window, or the main window's while the title
/// bar shows the search button. `input` is the real text input. A palette
/// typed into the title-bar field has no such row: the field is the input.
pub(super) fn search_row<'a, M: 'a>(
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
    input: Element<'a, M>,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.l).align_y(Alignment::Center);
    // raw: a search glyph, not language.
    line = line.push(text(raw("\u{2315}")).size(TS.body).color(r.ink4).boxed());
    line = line.push(input);
    line = line.push(cap(theme_tokens, t!(i18n, shortcut_ids::PALETTE_KEY_ESC)));
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
/// case-insensitive occurrence (which includes a word prefix), else none. A
/// scattered subsequence matches the row but is never highlighted.
pub(super) fn match_mask_in(i18n: &I18n, label: &str, query: &str) -> Vec<bool> {
    // The per-char fold keeps the length, so the mask lines up with `label`.
    let low = |c: char| i18n.fold_char_for_search(c);
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
    mask
}

/// [`match_mask_in`] under en-US.
#[cfg(test)]
pub(super) fn match_mask(label: &str, query: &str) -> Vec<bool> {
    match_mask_in(&I18n::en_us(), label, query)
}

/// [`label_runs_in`] under en-US.
#[cfg(test)]
pub(super) fn label_runs(label: &str, query: &str) -> Vec<(String, bool)> {
    label_runs_in(&I18n::en_us(), label, query)
}

/// The label as runs of plain and highlighted text.
pub(super) fn label_runs_in(i18n: &I18n, label: &str, query: &str) -> Vec<(String, bool)> {
    let mask = match_mask_in(i18n, label, query);
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
    // macOS labels (`⇧⌘P`) have no `+`: each symbol is its own cap.
    let mac: Option<Vec<Vec<String>>> = label
        .split_whitespace()
        .map(crate::keymap::mac_stroke_caps)
        .collect();
    if let Some(strokes) = mac.filter(|strokes| !strokes.is_empty()) {
        return strokes.into_iter().flatten().collect();
    }
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
        caps = caps.push(glyph_cap(theme_tokens, &key));
    }
    caps.boxed()
}

/// Characters of a result label that fit one row of the 640 px card beside the
/// key caps; longer labels end in an ellipsis.
const LABEL_BUDGET: usize = 58;
/// The label keeps at least this many characters beside a long context.
const LABEL_MIN_BUDGET: usize = 26;
/// Characters of a row's right-aligned context.
const CONTEXT_BUDGET: usize = 32;
/// Height of a result row (34 px at the 12 px base, mockup row 9).
const ROW_H: f32 = chrome_px(34.0 * 11.0 / 12.0);

/// `text` cut to `budget` characters, ending in an ellipsis when it was cut.
fn ellipsize(text: &str, budget: usize) -> String {
    if text.chars().count() <= budget {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(budget.saturating_sub(1)).collect();
    cut.push('\u{2026}');
    cut
}

/// One key cap: a key name from the catalog (Esc, Enter).
pub(super) fn cap<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    label: impl Into<signex_widgets::Label<'a>>,
) -> Element<'a, M> {
    container(text(label).size(TS.caption).font(mono()))
        .padding([1.0, SP.s + 1.0])
        .style(styles::key_cap(theme_tokens))
        .boxed()
}

/// A key cap showing a key as the keymap writes it, or an arrow glyph.
fn glyph_cap<'a, M: 'a>(theme_tokens: &ThemeTokens, key: &str) -> Element<'a, M> {
    // raw: a key as the keymap writes it, or an arrow glyph.
    cap(theme_tokens, raw(key.to_string()))
}

/// The text a row's label cell draws: the label, then a component's value
/// (mockup row 9: "D3 · ESD5Z5.0").
pub(super) fn row_label_text(entry: &CommandEntry) -> String {
    if entry.value.is_empty() {
        entry.label.clone()
    } else {
        format!("{} \u{b7} {}", entry.label, entry.value)
    }
}

/// The right-aligned context of a row: a command's category, else the row's
/// own detail (terminal count, sheet, project). A command's detail is its
/// binding, drawn as key caps instead.
pub(super) fn row_context_text(i18n: &I18n, entry: &CommandEntry) -> Option<String> {
    let context = path_subtitle(i18n, entry).or_else(|| {
        (entry.source != CommandSource::Command && !entry.detail.is_empty())
            .then(|| entry.detail.clone())
    })?;
    Some(ellipsize(&context, CONTEXT_BUDGET))
}

/// How many characters of the label fit beside `context_chars` of context.
fn label_budget(context_chars: usize) -> usize {
    LABEL_BUDGET
        .saturating_sub(context_chars)
        .max(LABEL_MIN_BUDGET)
}

/// The label cell: the row's text with the typed characters highlighted,
/// clipped so it never runs under the context.
fn row_label<'a, M: Clone + 'a>(
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
    shown: &str,
    query: &str,
) -> Element<'a, M> {
    let (ink, accent) = (
        ti(theme_tokens.text),
        tokens::resolve(theme_tokens).accent_text,
    );
    let spans: Vec<_> = label_runs_in(i18n, shown, query)
        .into_iter()
        .map(|(run, hit)| {
            // raw: runs of the row's label, already rendered, split at the matches.
            let s = span::<()>(raw(run)).color(if hit { accent } else { ink });
            if hit { s.font(bold()) } else { s }
        })
        .collect();
    container(
        rich_text::<(), M, Theme>(spans)
            .size(TS.label)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    )
    .width(Length::Fill)
    .clip(true)
    .boxed()
}

/// The context cell, in the quietest ink.
fn row_context<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    entry: &CommandEntry,
    context: String,
) -> Element<'a, M> {
    let label: signex_widgets::Label<'a> = if entry.source == CommandSource::Command {
        signex_i18n::UserText::new(context).into()
    } else {
        // raw: the detail is design data or worded by the palette index in
        // English (a net's terminal count, #2061).
        raw(context)
    };
    text(label)
        .size(TS.caption)
        .color(tokens::resolve(theme_tokens).ink4)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// One result row, on one line (mockup row 9): kind tile, label, context, and
/// the key caps of a bound command.
pub(super) fn result_row<'a, M: Clone + 'a>(
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
    entry: &CommandEntry,
    query: &str,
    is_active: bool,
    on_press: M,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let ink = ti(theme_tokens.text);
    let context = row_context_text(i18n, entry);
    let budget = label_budget(context.as_ref().map_or(0, |c| c.chars().count()));
    let shown = ellipsize(&row_label_text(entry), budget);
    let mut line: Row<Element<'a, M>> = Row::new()
        .spacing(SP.l)
        .height(Length::Fill)
        .align_y(Alignment::Center);
    line = line.push(tile::kind_tile(theme_tokens, entry.source));
    line = line.push(row_label(i18n, theme_tokens, &shown, query));
    if let Some(context) = context {
        line = line.push(row_context(theme_tokens, entry, context));
    }
    if entry.source == CommandSource::Command && !entry.detail.is_empty() {
        line = line.push(caps_row(theme_tokens, &entry.detail));
    }
    let (active_fill, hover_fill) = (
        r.accent_soft,
        styles::tokens::overlay(theme_tokens, styles::tokens::Overlay::Hover),
    );
    button(line)
        .width(Length::Fill)
        .height(Length::Fixed(ROW_H))
        .padding([0.0, SP.l])
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

/// The footer's fill: the chrome colour (mockup row 9), so its sunken key caps
/// read as caps.
fn footer_fill(theme_tokens: &ThemeTokens) -> iced::Color {
    ti(theme_tokens.toolbar_bg)
}

/// Hint footer: navigation caps, the Run cap and what Enter will run.
pub(super) fn hint_footer<'a, M: 'a>(
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
    _selected: Option<&CommandEntry>,
    extra: Option<Localized>,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let hint = |s: Localized| text(s).size(TS.caption).color(r.ink3).boxed();
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.m).align_y(Alignment::Center);
    line = line.push(glyph_cap(theme_tokens, "\u{2191}"));
    line = line.push(glyph_cap(theme_tokens, "\u{2193}"));
    line = line.push(hint(t!(i18n, ids::OVERLAY_PALETTE_NAVIGATE)));
    line = line.push(cap(theme_tokens, t!(i18n, shortcut_ids::PALETTE_KEY_ENTER)));
    line = line.push(hint(t!(i18n, ids::OVERLAY_PALETTE_RUN)));
    line = line.push(Space::new().width(Length::Fill).boxed());
    // "Enter Run" already says what Enter does; no sentence repeats it.
    if let Some(more) = extra {
        line = line.push(
            text(more)
                .size(TS.caption)
                .color(r.ink3)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    }
    let (rule, fill) = (r.line_soft, footer_fill(theme_tokens));
    container(line)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
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
    fn only_a_contiguous_match_is_highlighted() {
        // A scattered subsequence still matches the row but is not marked.
        assert!(match_mask("Annotate Quietly", "aq").iter().all(|m| !m));
        assert!(match_mask("abc", "cba").iter().all(|m| !m));
        let mask = match_mask("Open Project Options", "proj");
        assert_eq!(mask.iter().filter(|m| **m).count(), 4);
        assert!(mask[5] && mask[8]);
    }

    #[test]
    fn a_turkish_query_highlights_dotted_and_dotless_i() {
        let tr = I18n::with_message_locale_for_test("tr");
        let mask = match_mask_in(&tr, "İstanbul ILIK", "istanbul");
        assert_eq!(mask.iter().filter(|m| **m).count(), 8);
        assert!(mask[0] && mask[7] && !mask[8]);
        let mask = match_mask_in(&tr, "İstanbul ILIK", "ılık");
        assert_eq!(mask.len(), "İstanbul ILIK".chars().count());
        assert!(mask[9..].iter().all(|m| *m));
        // Under en-US a plain `i` still matches and highlights `İ`.
        assert!(match_mask("İstanbul", "istanbul").iter().all(|m| *m));
    }

    fn row(source: CommandSource, label: &str, value: &str, detail: &str) -> CommandEntry {
        CommandEntry {
            source,
            label: label.into(),
            value: value.into(),
            detail: detail.into(),
            action: CommandAction::OpenFile(std::path::PathBuf::new()),
        }
    }

    /// Mockup row 9: a component row reads designator and value on the left,
    /// its sheet on the right, and the query highlights inside the value.
    #[test]
    fn a_component_row_has_its_value_left_and_its_sheet_right() {
        let i18n = I18n::en_us();
        let entry = row(CommandSource::Symbol, "D3", "VBUS clamp", "usb-c.snxsch");
        let shown = row_label_text(&entry);
        assert_eq!(shown, "D3 \u{b7} VBUS clamp");
        assert_eq!(
            row_context_text(&i18n, &entry).as_deref(),
            Some("usb-c.snxsch")
        );
        let hits: String = label_runs(&shown, "vbu")
            .into_iter()
            .filter(|(_, hit)| *hit)
            .map(|(run, _)| run)
            .collect();
        assert_eq!(hits, "VBU");
    }

    /// A command's detail is its binding: key caps, never context text.
    #[test]
    fn a_command_binding_is_not_context() {
        let i18n = I18n::en_us();
        let entry = row(CommandSource::Command, "Save", "", "Ctrl+S");
        assert_eq!(row_context_text(&i18n, &entry), None);
    }

    /// A long context is cut and leaves the label its minimum.
    #[test]
    fn a_long_context_is_cut_and_the_label_keeps_room() {
        let i18n = I18n::en_us();
        let long = "x".repeat(CONTEXT_BUDGET * 2);
        let entry = row(CommandSource::File, "a.snxsch", "", &long);
        let context = row_context_text(&i18n, &entry).unwrap();
        assert_eq!(context.chars().count(), CONTEXT_BUDGET);
        assert!(context.ends_with('\u{2026}'));
        assert_eq!(label_budget(0), LABEL_BUDGET);
        assert_eq!(label_budget(LABEL_BUDGET), LABEL_MIN_BUDGET);
    }

    /// Mockup row 9: the footer's key caps stand out from it in both themes.
    #[test]
    fn the_footer_fill_differs_from_its_key_caps() {
        use signex_types::theme::{ThemeId, theme_tokens};
        for id in ThemeId::BUILTINS {
            let t = theme_tokens(*id);
            assert_ne!(footer_fill(&t), tokens::resolve(&t).sunken, "{id:?}");
        }
    }

    #[test]
    fn empty_query_highlights_nothing() {
        assert_eq!(
            label_runs("Place wire", ""),
            vec![("Place wire".into(), false)]
        );
    }
}
