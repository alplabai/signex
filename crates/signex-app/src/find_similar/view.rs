//! The Find Similar Objects card (#2526): the seed's properties with a Same /
//! Any choice each, the scope, the live counts and Select N.

use super::state::{shown_kind, shown_seed};
use super::{FindSimilarMsg, FindSimilarState, SimilarScope};
use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Font, Length, Theme, Widget};
use signex_engine::SimilarValue;
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, SegmentedStyle, raw, segmented, text, tooltip};

use crate::styles::{
    self, DialogButtonKind, PillTone, Radii, Spacing, TypeScale, chrome_px, dialog_body,
    dialog_button, ti, tokens,
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
/// Footer padding, vertical and horizontal (10 and 16 px at the default base).
const FOOTER_PAD_V: f32 = chrome_px(10.0 * 11.0 / 12.0);
const FOOTER_PAD_H: f32 = chrome_px(16.0 * 11.0 / 12.0);
/// The seed value column is cut to this many characters.
const VALUE_SHOWN_CHARS: usize = 28;

/// The catalog name of a property row.
fn property_label(property: signex_engine::SimilarProperty) -> signex_i18n::MessageId {
    use signex_engine::SimilarProperty as P;
    match property {
        P::Kind => ids::FIND_SIMILAR_PROP_KIND,
        P::Value => ids::PROPERTIES_VALUE,
        P::Footprint => ids::PROPERTIES_FOOTPRINT,
        P::LibraryPart => ids::FIND_SIMILAR_PROP_LIBRARY_PART,
        P::DesignatorPrefix => ids::FIND_SIMILAR_PROP_DESIGNATOR_PREFIX,
        P::NetName => ids::FIND_SIMILAR_PROP_NET_NAME,
        P::LabelType => ids::FIND_SIMILAR_PROP_LABEL_TYPE,
        P::Text => ids::PROPERTIES_TEXT,
        P::Name => ids::PROPERTIES_NAME,
        P::FileName => ids::FAB_OUTPUTS_NAME,
        P::Layer => ids::PCB_PRIM_LAYER,
        P::Width => ids::PCB_PRIM_WIDTH,
        P::Size => ids::PROPERTIES_SIZE,
        P::Drill => ids::BOARD_NET_CLASSES_COL_DRILL,
        P::Shape => ids::PCB_PRIM_SHAPE,
    }
}

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
fn row_value(v: &SimilarValue, i18n: &I18n) -> String {
    match v.property {
        signex_engine::SimilarProperty::Kind => shown_kind(&v.value, i18n),
        _ => v.value.clone(),
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
    r.segmented(size)
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
            container(text(UserText::new(full)).size(TS.body).font(mono()))
                .padding([SP.xs, SP.s])
                .style(styles::context_menu(tokens)),
            tooltip::Position::Top,
        )
        .boxed(),
    }
}

/// The corners a row fill takes: square between the table's rules, and the
/// table's own bottom corners on the last row, 1 px tighter inside its border.
fn row_corners(is_last: bool) -> iced::border::Radius {
    let radius = Radii::GRAPHITE.button - 1.0;
    let corners = iced::border::Radius::default();
    if is_last {
        corners.bottom_left(radius).bottom_right(radius)
    } else {
        corners
    }
}

/// One property row: name, seed value, Same / Any, laid out by
/// `styles::dialog_row`. A Same row gets the accent-soft fill.
fn property_row<'a>(
    v: &SimilarValue,
    same: bool,
    is_last: bool,
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
    let line = styles::dialog_row(vec![
        text(t!(i18n, property_label(v.property)))
            .size(TS.body)
            .color(ti(tokens.text))
            .width(PROPERTY_W)
            .boxed(),
        container(value_cell(&row_value(v, i18n), tokens, i18n))
            .width(Length::Fill)
            .boxed(),
        container(choice).width(MATCH_W).boxed(),
    ]);
    let fill = same.then_some(tokens::resolve(tokens).accent_soft);
    let radius = row_corners(is_last);
    container(line)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                radius,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The column heads on the list's head band (`styles::dialog_head_row`), in
/// capitals for the message language, over the row columns (mockup row 68).
fn table_head<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, FindSimilarMsg> {
    let faint = tokens::resolve(tokens).ink3;
    let head = |id| {
        let words = t!(i18n, id).into_string();
        // raw: catalog text already rendered, then upper-cased for the locale.
        text(raw(i18n.upper_for_display(&words)))
            .size(TS.section)
            .font(styles::medium_font())
            .color(faint)
            .wrapping(iced::widget::text::Wrapping::None)
    };
    styles::dialog_head_row(
        vec![
            head(ids::FIND_SIMILAR_COL_PROPERTY)
                .width(PROPERTY_W)
                .boxed(),
            head(ids::FIND_SIMILAR_COL_SEED).width(Length::Fill).boxed(),
            head(ids::FIND_SIMILAR_COL_MATCH).width(MATCH_W).boxed(),
        ],
        tokens,
    )
}

/// The property table: a bordered list, the head band, a rule under every
/// row (`styles::dialog_rows`, mockup row 68).
fn table<'a>(
    state: &FindSimilarState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindSimilarMsg> {
    let last = state.values.len().saturating_sub(1);
    let rows: Vec<Element<'a, FindSimilarMsg>> = state
        .values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let same = state.same.contains(&v.property);
            property_row(v, same, i == last, tokens, i18n)
        })
        .collect();
    styles::dialog_rows(Some(table_head(tokens, i18n)), rows, None, tokens)
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

/// Cancel and Select N on the chrome band under a rule (mockup row 68).
fn footer<'a>(
    state: &FindSimilarState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindSimilarMsg> {
    styles::dialog_footer_band(
        Vec::new(),
        vec![
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
        ],
        iced::Padding::from([FOOTER_PAD_V, FOOTER_PAD_H]),
        tokens,
    )
}

/// The dialog as a centred card on a scrim in the main window.
pub fn view<'a>(
    state: &'a FindSimilarState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindSimilarMsg> {
    // The rest of the seed is named by the selection handler, in English (#2061).
    let seed_text = shown_seed(&state.seed_label, i18n);
    let seed = t!(i18n, ids::FIND_SIMILAR_SEED, "seed" => seed_text.as_str());
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

    let footer = footer(state, tokens, i18n);

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
        let en = I18n::en_us();
        let seed = shown_seed(&state.seed_label, &en);
        assert_eq!(seed, "Part R1");
        assert_eq!(
            en.message_plain(
                ids::FIND_SIMILAR_SEED,
                &[("seed", Arg::from(seed.as_str()))]
            ),
            "Seed: Part R1"
        );
        // The Kind cell reads Part; matching keeps the engine's value.
        assert_eq!(row_value(&state.values[0], &en), "Part");
        assert_eq!(state.criteria()[0].value, "Symbol");
    }

    /// #2060: every property name reads as the engine's `label` and
    /// follows the language.
    #[test]
    fn the_property_names_keep_their_english() {
        let en = I18n::en_us();
        let xa = I18n::new(&signex_i18n::LanguagePref::Locale("en-XA".to_owned()));
        for property in SimilarProperty::ALL {
            let id = property_label(property);
            assert_eq!(en.message_plain(id, &[]), property.label());
            assert_ne!(xa.message_plain(id, &[]), property.label());
        }
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

    fn four_values() -> Vec<SimilarValue> {
        let mut values = part_values();
        values.push(SimilarValue {
            property: SimilarProperty::Footprint,
            value: "RES_0603".into(),
        });
        values.push(SimilarValue {
            property: SimilarProperty::LibraryPart,
            value: "Stock:STOCK-R".into(),
        });
        values
    }

    /// RGBA of the pixel at `(x, y)` in a `width`-wide capture.
    fn pixel(pixels: &[u8], width: f32, x: f32, y: f32) -> [u8; 4] {
        let i = ((y as usize) * (width as usize) + x as usize) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    }

    /// Mockup row 68: the heads read in capitals on their band and sit over
    /// the columns of the rows.
    #[test]
    fn the_table_heads_are_capitals_over_their_columns() {
        let en = I18n::en_us();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let state = FindSimilarState::new("Symbol R3".into(), four_values(), None);
        let window = iced::Size::new(DIALOG_W, 600.0);
        let texts = crate::layout_probe::lay_out_texts(table(&state, &tokens, &en), window);
        let property = crate::layout_probe::find(&texts, "PROPERTY");
        crate::layout_probe::find(&texts, "SEED VALUE");
        crate::layout_probe::find(&texts, "MATCH");
        let kind = crate::layout_probe::find(
            &texts,
            &en.message_plain(property_label(SimilarProperty::Kind), &[]),
        );
        assert!(
            (property.bounds.x - kind.bounds.x).abs() < 0.5,
            "{texts:#?}"
        );
        assert!(property.bounds.y + property.bounds.height <= kind.bounds.y);
    }

    /// Mockup row 68: a rule runs under every row, so between two unfilled
    /// rows the row padding changes colour.
    #[test]
    fn a_rule_separates_the_table_rows() {
        let en = I18n::en_us();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let state = FindSimilarState::new("Symbol R3".into(), four_values(), None);
        let window = iced::Size::new(DIALOG_W, 600.0);
        let texts = crate::layout_probe::lay_out_texts(table(&state, &tokens, &en), window);
        let label = |p| en.message_plain(property_label(p), &[]);
        let top = crate::layout_probe::find(&texts, &label(SimilarProperty::Value)).bounds;
        let bottom = crate::layout_probe::find(&texts, &label(SimilarProperty::LibraryPart)).bounds;
        let pixels = crate::layout_probe::draw_pixels(table(&state, &tokens, &en), window);
        // In the left row padding, clear of the border and of any text.
        let x = SP.m / 2.0 + 1.0;
        let (from, to) = (top.y + top.height / 2.0, bottom.y + bottom.height / 2.0);
        let mut changes = 0;
        let mut y = from;
        while y + 1.0 < to {
            if pixel(&pixels, window.width, x, y) != pixel(&pixels, window.width, x, y + 1.0) {
                changes += 1;
            }
            y += 1.0;
        }
        assert!(
            changes >= 4,
            "two rules, each an edge in and out: {changes}"
        );
    }

    /// Mockup row 68: a Same fill on the last row keeps the table's rounded
    /// bottom corner, so the corner pixel is not the row fill.
    #[test]
    fn a_same_last_row_keeps_the_table_corner_round() {
        let en = I18n::en_us();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let mut state = FindSimilarState::new("Symbol R3".into(), four_values(), None);
        state.same.insert(SimilarProperty::LibraryPart);
        let window = iced::Size::new(DIALOG_W, 600.0);
        let height = crate::layout_probe::root_size(table(&state, &tokens, &en), window).height;
        let texts = crate::layout_probe::lay_out_texts(table(&state, &tokens, &en), window);
        let last = en.message_plain(property_label(SimilarProperty::LibraryPart), &[]);
        let last = crate::layout_probe::find(&texts, &last).bounds;
        let pixels = crate::layout_probe::draw_pixels(table(&state, &tokens, &en), window);
        // In the row's left padding, level with its label: the Same fill.
        let fill = pixel(&pixels, window.width, SP.m / 2.0 + 1.0, last.center_y());
        // The row's own bottom-left pixel, just inside the 1 px border.
        let corner = pixel(&pixels, window.width, 1.5, height - 2.5);
        assert_ne!(fill, [0, 0, 0, 0], "the last row is filled");
        assert_ne!(corner, fill, "the fill is cut at the rounded corner");
    }

    /// Mockup row 68: the footer is a band under a rule, so its top line is
    /// drawn in another colour than the band below it.
    #[test]
    fn a_rule_sits_above_the_footer() {
        let en = I18n::en_us();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let state = FindSimilarState::new("Symbol R3".into(), part_values(), None);
        let window = iced::Size::new(DIALOG_W, 100.0);
        let pixels = crate::layout_probe::draw_pixels(footer(&state, &tokens, &en), window);
        // At the left end: the buttons sit on the right.
        let x = FOOTER_PAD_H / 2.0;
        let band = pixel(&pixels, window.width, x, FOOTER_PAD_V / 2.0 + 1.0);
        assert_ne!(pixel(&pixels, window.width, x, 0.0), band);
        assert_ne!(band, [0, 0, 0, 255], "the band is filled");
    }
}
