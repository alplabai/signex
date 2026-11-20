//! The Footprint preview column of Assign Footprints (Graphite #80): the
//! clicked row's footprint on the board ground with the layer legend, its
//! facts, and for a change the current and the new footprint side by side.
//! Read-only: drawn by the Components panel's footprint canvas.

use iced::widget::{Column, Row, Space, canvas, container};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, tooltip};

use super::{SP, TS, af, glyph, pill, semibold};
use crate::app::Message;
use crate::app::assign_footprints::{
    AssignFootprintsMsg, AssignFootprintsState, FootprintFacts, PadSize, PinMap, PreviewLoad,
    pin_map,
};
use crate::panels::components_panel::details::footprint_canvas::FootprintPreview;
use crate::styles::board_palette::{BoardPalette, BoardSwatch};
use crate::styles::{PillTone, Radii, ti, tokens};

/// Width of the column, and of the widened one.
const COLUMN_W: f32 = TS.body * 25.0;
/// Height of the board ground the footprints are drawn on.
const CANVAS_H: f32 = TS.body * 55.0 / 3.0;
/// How much the expand button widens the column and the drawing.
const WIDE: f32 = 2.0;
/// Width of a fact's label.
const FACT_W: f32 = TS.body * 7.5;
/// Side of a legend swatch.
const SWATCH: f32 = TS.body * 5.0 / 6.0;
/// Pin pairs written out before the list is cut.
const PIN_PAIRS: usize = 4;

const EXPAND_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M8.5 1.75h3.75V5.5M5.5 12.25H1.75V8.5M12.25 1.75L8 6M1.75 12.25L6 8" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

/// One footprint the preview draws: what is read of it, and its tag under a
/// change (NOW / NEW).
struct Side<'a> {
    load: Option<&'a PreviewLoad>,
    tag: Option<MessageId>,
}

impl Side<'_> {
    fn facts(&self) -> Option<&FootprintFacts> {
        match self.load? {
            PreviewLoad::Ready(facts) => Some(facts.as_ref()),
            PreviewLoad::Loading | PreviewLoad::Failed(_) => None,
        }
    }
}

/// The footprints of the previewed row: the current one, and the picked one
/// beside it when the row changes.
fn sides(state: &AssignFootprintsState) -> Vec<Side<'_>> {
    let Some(index) = state.preview_row else {
        return Vec::new();
    };
    let Some(part) = state.parts().get(index) else {
        return Vec::new();
    };
    let load = |binding: Option<&String>| binding.and_then(|b| state.previews.get(b));
    let current = load(part.current_binding.as_ref());
    match state.picks.get(index).and_then(Option::as_ref) {
        Some(pick) => vec![
            Side {
                load: current,
                tag: Some(ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_NOW),
            },
            Side {
                load: load(Some(pick)),
                tag: Some(ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_NEW),
            },
        ],
        None => vec![Side {
            load: current,
            tag: None,
        }],
    }
}

pub(super) fn column<'a>(
    state: &'a AssignFootprintsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let scale = if state.preview_wide { WIDE } else { 1.0 };
    let sides = sides(state);
    let mut col: Column<Element<'a, Message>> = Column::new()
        .spacing(SP.m)
        .push(heading(state.preview_wide, i18n, tokens))
        .push(caption(state, i18n, tokens))
        .push(drawing(&sides, CANVAS_H * scale, i18n, tokens))
        .push(legend(i18n, tokens))
        .push(facts(&sides, i18n, tokens))
        .push(
            text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_CAPTION))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    col = col.width(Length::Fill);
    let (fill, line) = (ti(tokens.panel_bg), r.line_soft);
    let edge = container(Space::new().width(Length::Fixed(1.0)).height(Length::Fill)).style(
        move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        },
    );
    let body =
        container(iced::widget::scrollable(col).style(crate::styles::scrollbar_style(tokens)))
            .padding([SP.l, SP.l + SP.xs])
            .width(Length::Fixed(COLUMN_W * scale))
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                ..container::Style::default()
            });
    Row::<Element<'a, Message>>::new()
        .push(edge.boxed())
        .push(body.boxed())
        .height(Length::Fill)
        .boxed()
}

/// "Footprint preview" and the expand button.
fn heading<'a>(wide: bool, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let ink = ti(tokens.text);
    let tip_id = if wide {
        ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_COLLAPSE
    } else {
        ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_EXPAND
    };
    let expand = button(glyph(EXPAND_GLYPH, r.ink3))
        .padding(SP.xs)
        .on_press(af(AssignFootprintsMsg::PreviewWidthToggled))
        .style(crate::styles::ghost_button(tokens));
    let tip = container(text(t!(i18n, tip_id)).size(TS.caption).color(ink))
        .padding([SP.xs, SP.m])
        .style(crate::styles::card(tokens));
    Row::<Element<'a, Message>>::new()
        .push(
            text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_TITLE))
                .size(TS.label)
                .color(ink)
                .font(semibold())
                .boxed(),
        )
        .push(Space::new().width(Length::Fill).boxed())
        .push(tooltip(expand, tip, tooltip::Position::Bottom).boxed())
        .align_y(Alignment::Center)
        .boxed()
}

/// The previewed part, and why it cannot change.
fn caption<'a>(
    state: &'a AssignFootprintsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let Some(part) = state.preview_row.and_then(|i| state.parts().get(i)) else {
        return Space::new().boxed();
    };
    let mut line = Row::<Element<'a, Message>>::new()
        .push(
            text(t!(
                i18n,
                ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_PART,
                "designator" => part.designator.as_str(),
                "value" => part.value.as_str()
            ))
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink3)
            .boxed(),
        )
        .spacing(SP.s)
        .align_y(Alignment::Center);
    if let Some(reason) = part.access.reason() {
        line = line.push(pill(t!(i18n, reason), PillTone::Neutral, tokens));
    }
    line.boxed()
}

/// Room each side gets, in proportion to its drawn width, so two footprints
/// side by side come out near one scale.
fn portion(side: &Side<'_>) -> u16 {
    side.facts().map_or(10, |facts| {
        (facts.width_mm * 10.0).round().clamp(1.0, 1000.0) as u16
    })
}

/// The footprints on the board ground, each with its name and tag under it.
fn drawing<'a>(
    sides: &[Side<'a>],
    height: f32,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let ground = BoardPalette::new().canvas;
    let mut canvases = Row::<Element<'a, Message>>::new()
        .spacing(SP.m)
        .height(Length::Fill);
    let mut labels = Row::<Element<'a, Message>>::new().spacing(SP.m);
    for side in sides {
        let share = Length::FillPortion(portion(side));
        canvases = canvases.push(
            container(side_drawing(side, i18n, tokens))
                .center_x(share)
                .center_y(Length::Fill)
                .boxed(),
        );
        let mut label = Column::<Element<'a, Message>>::new()
            .align_x(Alignment::Center)
            .width(share);
        if let Some(facts) = side.facts() {
            label = label.push(
                text(UserText::new(facts.name.clone()))
                    .size(TS.caption)
                    .color(r.ink3)
                    .font(super::super::graphite::mono())
                    .boxed(),
            );
        }
        if let Some(tag) = side.tag {
            let ink = if tag == ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_NEW {
                r.accent_text
            } else {
                r.ink4
            };
            label = label.push(
                text(t!(i18n, tag))
                    .size(TS.caption)
                    .color(ink)
                    .font(semibold())
                    .boxed(),
            );
        }
        labels = labels.push(label.boxed());
    }
    let board = container(canvases)
        .padding(SP.m)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(ground)),
            border: Border {
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    Column::<Element<'a, Message>>::new()
        .push(board.boxed())
        .push(labels.boxed())
        .spacing(SP.s)
        .boxed()
}

/// One footprint drawn read-only, or why it is not.
fn side_drawing<'a>(side: &Side<'a>, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let line = |words: Localized| -> Element<'a, Message> {
        text(words)
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink4)
            .boxed()
    };
    match side.load {
        None => line(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_UNLINKED)),
        Some(PreviewLoad::Loading) => line(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_LOADING)),
        Some(PreviewLoad::Failed(why)) => line(t!(
            i18n,
            ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_FAILED,
            "why" => why.as_str()
        )),
        Some(PreviewLoad::Ready(facts)) => {
            canvas(FootprintPreview::new(&facts.preview).with_theme_tokens(tokens))
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed()
        }
    }
}

/// Top copper, Silkscreen and Courtyard chips in the board colours.
fn legend<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let palette = BoardPalette::new();
    let ink = tokens::resolve(tokens).ink3;
    let entries = [
        (
            ids::DIALOG_ASSIGN_FOOTPRINTS_LEGEND_TOP_COPPER,
            palette.layer(PcbLayer::Copper(CopperLayer::Top)),
            true,
        ),
        (
            ids::DIALOG_ASSIGN_FOOTPRINTS_LEGEND_SILKSCREEN,
            palette.layer(PcbLayer::Technical(TechnicalLayer::SilkscreenTop)),
            true,
        ),
        (
            ids::DIALOG_ASSIGN_FOOTPRINTS_LEGEND_COURTYARD,
            palette.swatch(BoardSwatch::Courtyard),
            false,
        ),
    ];
    let mut row = Row::<Element<'a, Message>>::new()
        .spacing(SP.m)
        .align_y(Alignment::Center);
    for (label, colour, filled) in entries {
        let swatch =
            container(Space::new().width(SWATCH).height(SWATCH)).style(move |_: &Theme| {
                container::Style {
                    background: filled.then_some(Background::Color(colour)),
                    border: Border {
                        width: 1.0,
                        color: colour,
                        radius: Radii::GRAPHITE.small.min(SWATCH / 4.0).into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }
            });
        row = row.push(
            Row::<Element<'a, Message>>::new()
                .push(swatch.boxed())
                .push(text(t!(i18n, label)).size(TS.caption).color(ink).boxed())
                .spacing(SP.xs)
                .align_y(Alignment::Center)
                .boxed(),
        );
    }
    row.wrap().boxed()
}

/// Pads, pad 1, courtyard and, for a change, the pin map: the current value
/// struck through to the new one.
fn facts<'a>(sides: &[Side<'a>], i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let (now, new) = match sides {
        [one] => (None, one.facts()),
        [current, picked] => (current.facts(), picked.facts()),
        _ => return Space::new().boxed(),
    };
    let Some(new) = new else {
        return Space::new().boxed();
    };
    let changed = sides.len() > 1;
    let none = || t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_NONE).into_string();
    let size = |pad: Option<PadSize>| match pad {
        Some(PadSize::Round(d)) => {
            t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_SIZE_ROUND, "d" => mm(d)).into_string()
        }
        Some(PadSize::Rect(w, h)) => t!(
            i18n,
            ids::DIALOG_ASSIGN_FOOTPRINTS_SIZE_RECT,
            "w" => mm(w),
            "h" => mm(h)
        )
        .into_string(),
        None => none(),
    };
    let courtyard = |extent: Option<(f64, f64)>| match extent {
        Some((w, h)) => t!(
            i18n,
            ids::DIALOG_ASSIGN_FOOTPRINTS_SIZE_RECT,
            "w" => mm(w),
            "h" => mm(h)
        )
        .into_string(),
        None => none(),
    };
    let pair = |f: &dyn Fn(&FootprintFacts) -> String| -> (Option<String>, String) {
        let old = changed.then(|| now.map_or_else(none, f));
        (old, f(new))
    };
    let mut col = Column::<Element<'a, Message>>::new().spacing(SP.xs);
    col = col.push(fact(
        ids::DIALOG_ASSIGN_FOOTPRINTS_FACT_PADS,
        pair(&|facts| facts.pads.len().to_string()),
        i18n,
        tokens,
    ));
    col = col.push(fact(
        ids::DIALOG_ASSIGN_FOOTPRINTS_FACT_PAD_ONE,
        pair(&|facts| size(facts.pad_one)),
        i18n,
        tokens,
    ));
    col = col.push(fact(
        ids::DIALOG_ASSIGN_FOOTPRINTS_FACT_COURTYARD,
        pair(&|facts| courtyard(facts.courtyard_mm)),
        i18n,
        tokens,
    ));
    if let (true, Some(now)) = (changed, now) {
        col = col.push(pin_map_fact(now, new, i18n, tokens));
    }
    col.boxed()
}

/// A length in mm with two decimals, as the preview states it.
fn mm(value: f64) -> String {
    format!("{value:.2}")
}

/// One fact: its label, then the value (or the old value, an arrow and the
/// new one).
fn fact<'a>(
    label: MessageId,
    (old, new): (Option<String>, String),
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let mono = super::super::graphite::mono();
    let mut value = Row::<Element<'a, Message>>::new()
        .spacing(SP.xs)
        .align_y(Alignment::Center);
    let new_ink = if old.is_some() {
        r.accent_text
    } else {
        ti(tokens.text)
    };
    if let Some(old) = old {
        value = value
            .push(
                text(UserText::new(old))
                    .size(TS.caption)
                    .color(r.ink4)
                    .font(mono)
                    .boxed(),
            )
            .push(
                text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_ARROW))
                    .size(TS.caption)
                    .color(r.ink4)
                    .boxed(),
            );
    }
    value = value.push(
        text(UserText::new(new))
            .size(TS.caption)
            .color(new_ink)
            .font(mono)
            .boxed(),
    );
    fact_row(label, value.boxed(), i18n, tokens)
}

/// The label column and a value.
fn fact_row<'a>(
    label: MessageId,
    value: Element<'a, Message>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    Row::<Element<'a, Message>>::new()
        .push(
            text(t!(i18n, label))
                .size(TS.caption)
                .color(tokens::resolve(tokens).ink4)
                .width(Length::Fixed(FACT_W))
                .boxed(),
        )
        .push(value)
        .align_y(Alignment::Center)
        .boxed()
}

/// The pin map of a change: every pin keeps its pad (ok), or the pads the
/// new footprint lacks (error).
fn pin_map_fact<'a>(
    now: &FootprintFacts,
    new: &FootprintFacts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (words, colour): (Localized, Color) = match pin_map(&now.pads, &new.pads) {
        PinMap::Matches(numbers) => {
            let arrow = t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_ARROW).into_string();
            let mut pairs: Vec<String> = numbers
                .iter()
                .take(PIN_PAIRS)
                .map(|n| format!("{n} {arrow} {n}"))
                .collect();
            if numbers.len() > PIN_PAIRS {
                pairs.push(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_PREVIEW_NONE).into_string());
            }
            (
                t!(
                    i18n,
                    ids::DIALOG_ASSIGN_FOOTPRINTS_PIN_MAP_MATCHES,
                    "pairs" => pairs.join(", ")
                ),
                crate::styles::pill_colors(tokens, PillTone::Ok).1,
            )
        }
        PinMap::Missing(numbers) => (
            t!(
                i18n,
                ids::DIALOG_ASSIGN_FOOTPRINTS_PIN_MAP_MISSING,
                "numbers" => numbers.join(", ")
            ),
            tokens::danger(tokens),
        ),
    };
    fact_row(
        ids::DIALOG_ASSIGN_FOOTPRINTS_FACT_PIN_MAP,
        text(words).size(TS.caption).color(colour).boxed(),
        i18n,
        tokens,
    )
}
