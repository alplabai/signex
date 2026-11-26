//! The Variant Manager's summary cards (#1293): per variant the parts it
//! fits, the cost (no price source yet: "—") and its changes against Base;
//! the active variant's card is highlighted. At most four cards to a row,
//! each wide enough for its lines; a narrow pane wraps them.

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_widgets::text;

use super::super::table::{SP, TS};
use super::{Manager, mono, pill, semibold, tint_over_panel};
use crate::app::Message;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, ti};

/// Cards in one row.
const PER_ROW: usize = 4;
/// Width of a card in body ems (mockup row 32).
const CARD_EMS: f32 = 13.5;
/// Strength of the accent fill of the active card.
const ACTIVE_FILL: f32 = 0.35;

/// Every variant's card, at most four to a row and wrapping when the pane
/// cannot hold them at their width.
pub(super) fn view<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let variants: Vec<Option<&str>> = m.variants().collect();
    let mut rows: Column<Element<'a, Message>> = Column::new().spacing(SP.m);
    for chunk in variants.chunks(PER_ROW) {
        let mut line: Row<Element<'a, Message>> = Row::new().spacing(SP.m).width(Length::Fill);
        for &variant in chunk {
            line = line.push(card(m, variant, Length::Fixed(TS.body * CARD_EMS)));
        }
        rows = rows.push(line.wrap().boxed());
    }
    rows.boxed()
}

/// One variant's card, `width` wide.
pub(super) fn card<'a>(
    m: &Manager<'_>,
    variant: Option<&str>,
    width: Length,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let is_active = m.is_active(variant);
    // A name too long for the card ends in an ellipsis before the pill
    // (mockup row 32); a short one shows whole.
    let mut head: Row<Element<'a, Message>> = row![
        container(
            text(signex_i18n::UserText::new(m.name(variant)))
                .size(TS.body)
                .font(semibold())
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
        )
        .width(Length::Fill)
        .clip(true),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if m.is_default(variant) {
        head = head.push(pill(
            t!(m.i18n, ids::VARIANT_MANAGER_DEFAULT),
            PillTone::Neutral,
            m.tokens,
        ));
    }
    if is_active {
        head = head.push(pill(
            t!(m.i18n, ids::PROJECT_VARIANTS_ACTIVE),
            PillTone::Accent,
            m.tokens,
        ));
    }
    let column = m.column(variant);
    let fitted = m.matrix().zip(column).map(|(matrix, column)| {
        t!(
            m.i18n,
            ids::PROJECT_VARIANTS_CARD_FITTED,
            "fitted" => matrix.fitted_count(column),
            "total" => matrix.rows.len(),
        )
    });
    let vs_base = match (variant, m.matrix().zip(column)) {
        (None, _) => Some((t!(m.i18n, ids::PROJECT_VARIANTS_REFERENCE), true)),
        (Some(_), Some((matrix, column))) => Some((
            t!(m.i18n, ids::PROJECT_VARIANTS_CHANGES, "changes" => matrix.changes(column)),
            false,
        )),
        (Some(_), None) => None,
    };
    let none = || t!(m.i18n, ids::PROJECT_VARIANTS_NO_VALUE);
    let facts = column![
        fact(
            m,
            t!(m.i18n, ids::PROJECT_VARIANTS_CARD_PARTS),
            fitted.unwrap_or_else(none),
            false
        ),
        fact(m, t!(m.i18n, ids::PROJECT_VARIANTS_CARD_COST), none(), true),
        match vs_base {
            Some((words, quiet)) => fact(
                m,
                t!(m.i18n, ids::PROJECT_VARIANTS_CARD_VS_BASE),
                words,
                quiet
            ),
            None => fact(
                m,
                t!(m.i18n, ids::PROJECT_VARIANTS_CARD_VS_BASE),
                none(),
                true
            ),
        },
    ]
    .spacing(SP.xs / 2.0);
    let (fill, edge, edge_width) = if is_active {
        (
            tint_over_panel(r.accent_soft, ACTIVE_FILL, m.tokens),
            r.accent_text,
            1.5,
        )
    } else {
        (ti(m.tokens.panel_bg), ti(m.tokens.border), 1.0)
    };
    container(column![head, facts].spacing(SP.s))
        .width(width)
        .padding([SP.s * 1.5, SP.m])
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: edge_width,
                color: edge,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// One caption / value line of a card; a `quiet` value is muted, not mono.
fn fact<'a>(
    m: &Manager<'_>,
    caption: Localized,
    value: Localized,
    quiet: bool,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let value = text(value)
        .size(TS.caption)
        .wrapping(iced::widget::text::Wrapping::None);
    let value = if quiet {
        value.color(r.ink4)
    } else {
        value.font(mono()).color(ti(m.tokens.text))
    };
    row![
        text(caption).size(TS.caption).color(r.ink4),
        Space::new().width(Length::Fill),
        value,
    ]
    .spacing(SP.m)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mockup row 32: the longest line of a card, its caption beside a
    /// four-digit change count in the mono face, fits inside the padding.
    #[test]
    fn a_card_holds_its_longest_line() {
        const MONO_ADVANCE_EM: f32 = 0.6;
        let chars = "vs Base".len() + "9999 changes".len();
        let line = chars as f32 * TS.caption * MONO_ADVANCE_EM + SP.m;
        assert!(line + 2.0 * SP.m <= TS.body * CARD_EMS);
    }

    use super::super::Parts;
    use crate::app::state::ProjectId;
    use crate::layout_probe::{LaidOutText, lay_out_texts};
    use iced::{Rectangle, Size};
    use signex_output::{VariantMatrix, VariantRow};
    use signex_types::project::VariantSettings;
    use signex_types::theme::{ThemeId, theme_tokens};

    const LONG_NAME: &str = "Prototype build for the field trial with the long name";

    fn variant_matrix(variants: &[String]) -> VariantMatrix {
        let row = |designator: &str, fitted: Vec<bool>| VariantRow {
            designator: designator.into(),
            link: None,
            value: "10k".into(),
            source_value: "10k".into(),
            footprint: "R_0603".into(),
            overridden: vec![false; fitted.len()],
            fitted,
            parameters: Vec::new(),
            channel: None,
        };
        VariantMatrix {
            variants: variants.to_vec(),
            rows: vec![
                row("R1", vec![true, false, true, true]),
                row("R2", vec![true, true, false, true]),
                row("U1", vec![true, true, true, true]),
            ],
        }
    }

    /// The summary laid out `width` wide in `tid` with `active` the active
    /// variant (`None` is Base).
    fn laid_out(tid: ThemeId, active: Option<&str>, width: f32) -> Vec<LaidOutText> {
        let definitions: Vec<String> = Vec::from(["Lite", LONG_NAME, "Pro"].map(String::from));
        let matrix = variant_matrix(&definitions);
        let tokens = theme_tokens(tid);
        let active = active.and_then(|name| definitions.iter().find(|d| *d == name));
        let settings = VariantSettings::default();
        let m = Manager {
            project_id: ProjectId::test_id(32),
            definitions: &definitions,
            active: active.map(String::as_str),
            selected: active.map(String::as_str),
            tab: None,
            parts: Parts::Read(&matrix),
            default: None,
            shown: Some(matrix.with_settings(&settings)),
            cell: None,
            settings: &settings,
            settings_locked: false,
            stale: false,
            i18n: signex_i18n::I18n::en_us_ref(),
            tokens: &tokens,
        };
        lay_out_texts(view(&m), Size::new(width, TS.body * 100.0))
    }

    /// The left edges of the cards a summary `width` wide lays out: as many
    /// as fit side by side, at most four.
    fn card_lefts(width: f32) -> Vec<f32> {
        let card = TS.body * CARD_EMS;
        let mut lefts = vec![0.0];
        while lefts.len() < PER_ROW {
            let next = lefts.len() as f32 * (card + SP.m);
            if next + card > width {
                break;
            }
            lefts.push(next);
        }
        lefts
    }

    /// Mockup row 32: in Graphite Light and Dark, at the default and at a
    /// narrow tab width, with Base or the long-named variant active, every
    /// line of every card stays inside its card's padding and no text
    /// overlaps another (the name gives way to the active pill).
    #[test]
    fn no_card_text_overlaps_another_or_leaves_its_card() {
        let card = TS.body * CARD_EMS;
        let default_width = PER_ROW as f32 * (card + SP.m);
        let narrow_width = card * 1.5;
        for tid in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            for active in [None, Some(LONG_NAME)] {
                for width in [default_width, narrow_width] {
                    let texts = laid_out(tid, active, width);
                    let case = format!("{tid:?}, active {active:?}, {width} wide");
                    // Base, Lite, the long name and Pro, each with its
                    // three captions and values, one active pill, and the
                    // default pill (Base here; Set as default, mockup row 32).
                    assert_eq!(texts.len(), 4 * 7 + 2, "{case}: {texts:#?}");
                    let lefts = card_lefts(width);
                    for t in &texts {
                        let b = t.bounds;
                        let inside = lefts.iter().any(|&left| {
                            b.x >= left + SP.m - 0.5 && b.x + b.width <= left + card - SP.m + 0.5
                        });
                        assert!(inside, "{case}: {:?} leaves its card: {b:?}", t.text);
                    }
                    for (i, a) in texts.iter().enumerate() {
                        for b in &texts[i + 1..] {
                            let overlap = a
                                .bounds
                                .intersection(&b.bounds)
                                .map_or(0.0, |r: Rectangle| r.width * r.height);
                            assert!(
                                overlap < 0.5,
                                "{case}: {:?} {:?} overlaps {:?} {:?}",
                                a.text,
                                a.bounds,
                                b.text,
                                b.bounds
                            );
                        }
                    }
                }
            }
        }
    }

    /// Mockup row 32: a long variant name gives way to the card, ending
    /// before the active pill, while a short name shows at its full width.
    #[test]
    fn a_long_name_is_cut_to_its_card_and_a_short_one_shows_whole() {
        let card = TS.body * CARD_EMS;
        let texts = laid_out(ThemeId::GraphiteLight, Some(LONG_NAME), 1000.0);
        let long = crate::layout_probe::find(&texts, LONG_NAME).bounds;
        let pill = texts
            .iter()
            .find(|t| t.text.contains("active"))
            .expect("the active pill")
            .bounds;
        assert!(long.x + long.width <= pill.x, "{long:?} runs into {pill:?}");
        assert!(long.width < card - 2.0 * SP.m, "{long:?}");
        let natural = crate::layout_probe::natural::<Message>(
            text(signex_i18n::UserText::new("Lite".to_string()))
                .size(TS.body)
                .font(semibold())
                .boxed(),
            "Lite",
        );
        let lite = crate::layout_probe::find(&texts, "Lite").bounds;
        assert!(lite.width + 0.5 >= natural.width, "{lite:?} vs {natural:?}");
    }
}
