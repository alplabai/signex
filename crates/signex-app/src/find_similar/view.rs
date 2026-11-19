//! The Find Similar Objects card (#2526): the seed's properties with a Same /
//! Any choice each, the scope, the live counts and Select N.

use super::state::shown_kind;
use super::{FindSimilarMsg, FindSimilarState, SimilarScope};
use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Font, Length, Theme, Widget};
use signex_engine::SimilarValue;
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, SegmentedStyle, raw, segmented, text, tooltip};

use crate::styles::{
    self, DialogButtonKind, PillTone, Radii, Spacing, TypeScale, chrome_px, dialog_body,
    dialog_button, dialog_footer, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Card width (520 px at the default base).
const DIALOG_W: f32 = chrome_px(520.0 * 11.0 / 12.0);
/// The Property column (150 px at the default base).
const PROPERTY_W: f32 = chrome_px(150.0 * 11.0 / 12.0);
/// The Match column (120 px at the default base).
const MATCH_W: f32 = chrome_px(120.0 * 11.0 / 12.0);
/// The label in front of the scope control (52 px at the default base).
const SCOPE_LABEL_W: f32 = chrome_px(52.0 * 11.0 / 12.0);
/// The seed value column is cut to this many characters.
const VALUE_SHOWN_CHARS: usize = 28;

/// How a seed value is drawn in its cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ShownValue {
    /// No value: the catalog's "(empty)", in italic.
    Empty,
    /// The whole value fits.
    Whole(String),
    /// Cut at [`VALUE_SHOWN_CHARS`] with an ellipsis; the hover holds `full`.
    Cut { shown: String, full: String },
}

pub(super) fn shown_value(value: &str) -> ShownValue {
    if value.is_empty() {
        return ShownValue::Empty;
    }
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(VALUE_SHOWN_CHARS).collect();
    if chars.next().is_some() {
        ShownValue::Cut {
            shown: format!("{head}…"),
            full: value.to_owned(),
        }
    } else {
        ShownValue::Whole(head)
    }
}

/// The value a row shows: a Kind reads with the dialog's kind names.
fn row_value(v: &SimilarValue) -> &str {
    match v.property {
        signex_engine::SimilarProperty::Kind => shown_kind(&v.value),
        _ => &v.value,
    }
}

/// The primary button's label: how many objects it selects.
pub(super) fn select_label(state: &FindSimilarState, i18n: &I18n) -> Localized {
    t!(i18n, ids::FIND_SIMILAR_SELECT, "count" => state.sheet_matches)
}

/// The primary button's message: none while a project count runs.
pub(super) fn select_message(state: &FindSimilarState) -> Option<FindSimilarMsg> {
    (!state.running).then_some(FindSimilarMsg::ApplyPressed)
}

fn segmented_look(tokens: &ThemeTokens, size: f32) -> SegmentedStyle {
    let r = tokens::resolve(tokens);
    SegmentedStyle::new(r.sunken, r.raised, r.shadow, r.ink3, ti(tokens.text), size)
}

fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

fn italic() -> Font {
    Font {
        style: iced::font::Style::Italic,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// The seed value cell.
fn value_cell<'a>(value: &str, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, FindSimilarMsg> {
    let ink = ti(tokens.text);
    let faint = tokens::resolve(tokens).ink3;
    match shown_value(value) {
        ShownValue::Empty => text(t!(i18n, ids::FIND_SIMILAR_EMPTY))
            .size(TS.body)
            .font(italic())
            .color(faint)
            .boxed(),
        ShownValue::Whole(whole) => text(UserText::new(whole))
            .size(TS.body)
            .font(mono())
            .color(ink)
            .boxed(),
        ShownValue::Cut { shown, full } => tooltip(
            text(UserText::new(shown))
                .size(TS.body)
                .font(mono())
                .color(ink),
            container(text(UserText::new(full)).size(TS.body).font(mono())).padding([SP.xs, SP.s]),
            tooltip::Position::Top,
        )
        .style(styles::context_menu(tokens))
        .boxed(),
    }
}

/// One property row: name, seed value, Same / Any. A Same row gets the
/// accent-soft fill.
fn property_row<'a>(
    v: &SimilarValue,
    same: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindSimilarMsg> {
    let property = v.property;
    let choice = segmented(
        [
            (true, Label::from(t!(i18n, ids::FIND_SIMILAR_SAME))),
            (false, Label::from(t!(i18n, ids::FIND_SIMILAR_ANY))),
        ],
        same,
        segmented_look(tokens, TS.caption),
        move |same| FindSimilarMsg::PropertySet(property, same),
    );
    let line = row![
        // raw: the property's name comes from signex-engine (#2060).
        text(raw(v.property.label()))
            .size(TS.body)
            .color(ti(tokens.text))
            .width(PROPERTY_W),
        container(value_cell(row_value(v), tokens, i18n)).width(Length::Fill),
        container(choice).width(MATCH_W),
    ]
    .align_y(Alignment::Center)
    .spacing(SP.m);
    let fill = same.then_some(tokens::resolve(tokens).accent_soft);
    container(line)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn table<'a>(
    state: &FindSimilarState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindSimilarMsg> {
    let faint = tokens::resolve(tokens).ink3;
    let head = |id| text(t!(i18n, id)).size(TS.caption).color(faint);
    let heading = row![
        head(ids::FIND_SIMILAR_COL_PROPERTY).width(PROPERTY_W),
        head(ids::FIND_SIMILAR_COL_SEED).width(Length::Fill),
        head(ids::FIND_SIMILAR_COL_MATCH).width(MATCH_W),
    ]
    .spacing(SP.m)
    .padding([0.0, SP.m]);
    let mut rows: Column<Element<'a, FindSimilarMsg>> = column![heading].spacing(SP.xs);
    for v in &state.values {
        rows = rows.push(property_row(
            v,
            state.same.contains(&v.property),
            tokens,
            i18n,
        ));
    }
    rows.boxed()
}

fn scope_row<'a>(
    state: &FindSimilarState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindSimilarMsg> {
    row![
        text(t!(i18n, ids::FIND_SCOPE))
            .size(TS.body)
            .color(tokens::resolve(tokens).ink3)
            .width(SCOPE_LABEL_W),
        segmented(
            [
                (
                    SimilarScope::Sheet,
                    Label::from(t!(i18n, ids::FIND_SIMILAR_SCOPE_SHEET)),
                ),
                (
                    SimilarScope::Project,
                    Label::from(t!(i18n, ids::FIND_SIMILAR_SCOPE_PROJECT)),
                ),
            ],
            state.scope,
            segmented_look(tokens, TS.body),
            FindSimilarMsg::ScopeChosen,
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(SP.m)
    .boxed()
}

fn pill<'a>(label: Localized, tone: PillTone, tokens: &ThemeTokens) -> Element<'a, FindSimilarMsg> {
    container(text(label).size(TS.caption))
        .padding([SP.xs, SP.m])
        .style(styles::pill(tokens, tone))
        .boxed()
}

/// The live counts: matches here, and on a project scope the matches on the
/// other sheets, or "Searching..." while they are counted.
fn counts<'a>(
    state: &FindSimilarState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindSimilarMsg> {
    let here = if state.board {
        ids::FIND_SIMILAR_COUNT_BOARD
    } else {
        ids::FIND_SIMILAR_COUNT_SHEET
    };
    let mut line: Row<Element<'a, FindSimilarMsg>> = row![pill(
        t!(i18n, here, "count" => state.sheet_matches),
        PillTone::Ok,
        tokens
    )]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if !state.board && state.scope == SimilarScope::Project {
        if state.running {
            line = line.push(pill(
                t!(i18n, ids::FIND_SEARCHING),
                PillTone::Neutral,
                tokens,
            ));
        } else if let Some(more) = state.other_sheet_total() {
            line = line.push(pill(
                t!(i18n, ids::FIND_SIMILAR_COUNT_ELSEWHERE, "count" => more),
                PillTone::Neutral,
                tokens,
            ));
        }
    }
    line.boxed()
}

/// The dialog as a centred card on a scrim in the main window.
pub fn view<'a>(
    state: &'a FindSimilarState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindSimilarMsg> {
    // The seed is named by the selection handler, in English (#2061).
    let seed = t!(i18n, ids::FIND_SIMILAR_SEED, "seed" => state.seed_label.as_str());
    let header = crate::app::view::dialogs::graphite_header(
        t!(i18n, ids::FIND_SIMILAR_TITLE).into_string(),
        Some(seed.into_string()),
        None,
        FindSimilarMsg::Closed,
        crate::app::view::dialogs::DIALOG_HEADER_H,
        tokens,
    );

    let mut body: Column<Element<'a, FindSimilarMsg>> =
        column![table(state, tokens, i18n)].spacing(SP.m);
    // A board search reaches its own board only (#2403): no scope to pick.
    if !state.board {
        body = body.push(scope_row(state, tokens, i18n));
    }
    body = body.push(counts(state, tokens, i18n));
    if !state.board {
        body = body.push_boxed(
            text(t!(i18n, ids::FIND_SIMILAR_FOOTER))
                .size(TS.caption)
                .color(tokens::resolve(tokens).ink3),
        );
    }

    let footer = dialog_footer(vec![
        dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
            DialogButtonKind::Cancel,
            Some(FindSimilarMsg::Closed),
            tokens,
        ),
        dialog_button(
            select_label(state, i18n),
            DialogButtonKind::Primary,
            select_message(state),
            tokens,
        ),
    ]);

    let dialog =
        container(column![header, dialog_body(body), footer].width(Length::Fixed(DIALOG_W)))
            .style(styles::modal_card(tokens))
            .clip(true);

    container(
        column![
            Space::new().height(Length::Fill),
            row![
                Space::new().width(Length::Fill),
                dialog,
                Space::new().width(Length::Fill),
            ],
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(crate::app::view::dialogs::scrim_style(tokens))
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_engine::{SimilarProperty, SimilarValue};
    use signex_i18n::Arg;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn part_values() -> Vec<SimilarValue> {
        vec![
            SimilarValue {
                property: SimilarProperty::Kind,
                value: "Symbol".into(),
            },
            SimilarValue {
                property: SimilarProperty::Value,
                value: "10k".into(),
            },
        ]
    }

    #[test]
    fn long_and_empty_values_are_shortened_for_display() {
        assert_eq!(shown_value(""), ShownValue::Empty);
        assert_eq!(shown_value("10k"), ShownValue::Whole("10k".into()));
        let fits = "x".repeat(VALUE_SHOWN_CHARS);
        assert_eq!(shown_value(&fits), ShownValue::Whole(fits.clone()));
    }

    #[test]
    fn a_29_character_value_is_cut_and_its_hover_holds_the_whole_value() {
        let long = "Stock:STOCK-RESISTOR-0603-1PCT";
        let long = &long[..29];
        assert_eq!(long.chars().count(), 29);
        let ShownValue::Cut { shown, full } = shown_value(long) else {
            panic!("a 29-character value is cut");
        };
        assert_eq!(shown.chars().count(), VALUE_SHOWN_CHARS + 1);
        assert!(shown.ends_with('…'));
        assert!(long.starts_with(shown.trim_end_matches('…')));
        assert_eq!(full, long, "the hover holds the whole value");
    }

    #[test]
    fn a_new_dialog_starts_with_kind_same_and_names_the_seed_a_part() {
        let state = FindSimilarState::new("Symbol R1".into(), part_values(), None);
        assert_eq!(state.criteria().len(), 1);
        assert_eq!(state.criteria()[0].property, SimilarProperty::Kind);
        assert_eq!(state.scope, SimilarScope::Sheet);
        assert_eq!(state.seed_label, "Part R1");
        assert_eq!(
            I18n::en_us().message_plain(
                ids::FIND_SIMILAR_SEED,
                &[("seed", Arg::from(state.seed_label.as_str()))]
            ),
            "Seed: Part R1"
        );
        // The Kind cell reads Part; matching keeps the engine's value.
        assert_eq!(row_value(&state.values[0]), "Part");
        assert_eq!(state.criteria()[0].value, "Symbol");
    }

    #[test]
    fn the_select_button_counts_the_sheet_matches_and_waits_for_a_count() {
        let mut state = FindSimilarState::new("Symbol R3".into(), part_values(), None);
        state.sheet_matches = 7;
        let en = I18n::en_us();
        assert_eq!(
            select_label(&state, &en)
                .into_string()
                .replace(['\u{2068}', '\u{2069}'], ""),
            "Select 7"
        );
        assert!(matches!(
            select_message(&state),
            Some(FindSimilarMsg::ApplyPressed)
        ));
        state.running = true;
        assert!(
            select_message(&state).is_none(),
            "Select waits for the count"
        );
    }

    #[test]
    fn the_card_builds_in_both_graphite_themes() {
        let en = I18n::en_us();
        let mut state = FindSimilarState::new("Symbol R3".into(), part_values(), None);
        state.scope = SimilarScope::Project;
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let _ = view(&state, &tokens, &en);
            let board = state.clone().on_board();
            let _ = view(&board, &tokens, &en);
        }
    }
}
