//! The Clearance domain's class-to-class matrix (#2398, #2530): one
//! clearance per pair of net classes, the lower triangle live and the
//! mirror half inert. A written value is full ink, a fallback grey. A
//! clicked cell keeps the accent ring and opens its object pairs in a card
//! beside the matrix. The preview row under them follows the hovered cell
//! or row, else the pinned one.

use std::sync::LazyLock;

use iced::widget::{Column, Row, Space, container, mouse_area, scrollable, svg};
use iced::{Alignment, Background, Border, Element, Length, Point, Theme, Widget as _};
use signex_drc_dsl::table::matrix::{self, CellKey, OBJECT_PAIRS};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::coord::Unit;
use signex_widgets::constraint_grid::{
    GridMetrics, GridStyle, MatrixText, PairClearance, cell_text, matrix_view, mirror_swatch,
};
use signex_widgets::{Label, LengthFormat, button, text, text_input, tooltip};

use super::{Frame, SP, TS, up};
use crate::app::Message;
use crate::app::rules_tab::RulesTabMsg;
use crate::app::rules_tab::matrix::{
    ClassClearances, MatrixDefaults, MatrixPreview, Shown, default_key, matrix_preview, shown,
};
use crate::styles::{Radii, ti, tokens, tokens::resolve};

static UNDO: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../../../assets/icons/chrome/undo.svg").as_slice())
});

/// The fallbacks before the tab's first analysis: none.
static NO_DEFAULTS: MatrixDefaults = MatrixDefaults::new();

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

fn medium() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// What each cell falls back to, as the analysis keeps it.
fn defaults<'a>(frame: &Frame<'a>) -> &'a MatrixDefaults {
    frame
        .state
        .analysis
        .as_ref()
        .map_or(&NO_DEFAULTS, |analysis| &analysis.defaults)
}

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let caption = text(t!(i18n, ids::RULES_MATRIX_CAPTION))
        .size(TS.caption)
        .color(tokens::muted_text(tokens));
    if frame.classes.is_empty() {
        return Column::new()
            .spacing(SP.s)
            .push(caption.boxed())
            .push(
                text(t!(i18n, ids::RULES_MATRIX_NO_CLASSES))
                    .size(TS.body)
                    .color(tokens::muted_text(tokens))
                    .boxed(),
            )
            .height(Length::Fill)
            .boxed();
    }
    let left = Column::new()
        .spacing(SP.s)
        .push(caption.boxed())
        .push(grid(frame))
        .push(legend(frame));
    let mut top: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.xl)
        .align_y(Alignment::Start)
        .push(left.boxed());
    if let Some((a, b)) = frame.state.matrix_open.clone() {
        top = top.push(object_card(frame, a, b));
    }
    let body = Column::new()
        .spacing(SP.l)
        .push(top.boxed())
        .push(preview_row(frame));
    scrollable(body.padding([SP.m, SP.m]))
        .direction(crate::styles::thin_vertical())
        .style(crate::styles::scrollbar_style(tokens))
        .height(Length::Fill)
        .boxed()
}

/// The cell under `at` (relative to the matrix): the header row and the
/// name column are not cells, nor is the mirror half.
fn cell_at(classes: &[String], m: GridMetrics, at: Point) -> Option<CellKey> {
    let row = (at.y / m.row_height).floor() as isize - 1;
    let col = (at.x / m.cell_width).floor() as isize - 1;
    if row < 0 || col < 0 || col > row {
        return None;
    }
    let (row, col) = (row as usize, col as usize);
    Some(CellKey::new(
        classes.get(row)?.clone(),
        classes.get(col)?.clone(),
    ))
}

/// The matrix's words; a fallback cell reads as the legend's sample.
fn matrix_words(i18n: &I18n) -> MatrixText<'static> {
    MatrixText {
        clear: t!(i18n, ids::RULES_MATRIX_CLEAR_CELL).into(),
        fallback: t!(i18n, ids::RULES_MATRIX_FALLBACK_TIP).into(),
        fallback_value: ids::RULES_MATRIX_DEFAULT,
    }
}

/// The class matrix, hover-tracked for the preview.
fn grid<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let style = GridStyle::from_tokens(tokens, TS.body).with_mono(mono());
    let values = ClassClearances {
        classes: &frame.classes,
        table: &frame.table,
        defaults: defaults(frame),
    };
    let words = matrix_words(i18n);
    let format = LengthFormat::new(i18n, Unit::Mm);
    let cells = matrix_view(&frame.state.matrix, &values, format, &words, style).map(move |msg| {
        up(RulesTabMsg::MatrixActed {
            project_id: id,
            msg,
        })
    });
    let classes = frame.classes.clone();
    let metrics = style.metrics();
    mouse_area(cells)
        .on_move(move |at| {
            up(RulesTabMsg::MatrixHovered {
                project_id: id,
                key: cell_at(&classes, metrics, at),
            })
        })
        .on_exit(up(RulesTabMsg::MatrixHovered {
            project_id: id,
            key: None,
        }))
        .boxed()
}

/// "0.8 written · (0.15 mm) fallback · mirror half".
fn legend<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let style = GridStyle::from_tokens(tokens, TS.body);
    let format = LengthFormat::new(i18n, Unit::Mm);
    let words = matrix_words(i18n);
    let muted = tokens::muted_text(tokens);
    let word = |id| text(t!(i18n, id)).size(TS.caption).color(muted).boxed();
    let item = |sample: Element<'a, Message>, name: Element<'a, Message>| {
        Row::new()
            .spacing(SP.xs)
            .align_y(Alignment::Center)
            .push(sample)
            .push(name)
            .boxed()
    };
    Row::new()
        .spacing(SP.l)
        .align_y(Alignment::Center)
        .push(item(
            text(cell_text(PairClearance::Written(0.8), format, &words))
                .size(TS.caption)
                .font(mono())
                .color(ti(tokens.text))
                .boxed(),
            word(ids::RULES_MATRIX_LEGEND_WRITTEN),
        ))
        .push(item(
            text(cell_text(PairClearance::Fallback(0.15), format, &words))
                .size(TS.caption)
                .font(mono())
                .color(style.faint_ink)
                .boxed(),
            word(ids::RULES_MATRIX_LEGEND_FALLBACK),
        ))
        .push(item(
            mirror_swatch(TS.body, style),
            word(ids::RULES_MATRIX_LEGEND_MIRROR),
        ))
        .boxed()
}

/// An object word of the rule text (`track`), as shown; a word this build
/// has no name for is shown as written.
fn object_word(i18n: &I18n, word: &str) -> String {
    let id = match word {
        "track" => ids::RULES_OBJECT_TRACK,
        "pad" => ids::RULES_OBJECT_PAD,
        "via" => ids::RULES_OBJECT_VIA,
        "pour" => ids::RULES_OBJECT_POUR,
        "any" => ids::RULES_OBJECT_ANY,
        other => return other.to_string(),
    };
    t!(i18n, id).into_string()
}

/// "Track – Pad".
fn pair_label(i18n: &I18n, [a, b]: &[String; 2]) -> Localized {
    t!(
        i18n,
        ids::RULES_MATRIX_PAIR,
        "a" => object_word(i18n, a),
        "b" => object_word(i18n, b)
    )
}

fn pair_width(m: GridMetrics) -> f32 {
    m.cell_width * 1.6
}

fn clearance_width(m: GridMetrics) -> f32 {
    m.cell_width
}

/// The open cell's object pairs: All objects, then one row per object
/// pair of the rule text, each typed in place, with ↺ on a written row.
fn object_card<'a>(frame: &Frame<'a>, a: String, b: String) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let m = GridStyle::from_tokens(tokens, TS.body).metrics();
    let title = text(t!(
        i18n,
        ids::RULES_MATRIX_OBJECTS_TITLE,
        "a" => a.as_str(),
        "b" => b.as_str()
    ))
    .size(TS.body)
    .font(medium())
    .width(Length::Fill);
    let close = button(text(t!(i18n, ids::RULES_MATRIX_COLLAPSE)).size(TS.caption))
        .padding([SP.xs, SP.m])
        .style(crate::styles::outline_button(tokens))
        .on_press(up(RulesTabMsg::MatrixCollapsed { project_id: id }));
    let muted = tokens::muted_text(tokens);
    let head = Row::new()
        .spacing(SP.s)
        .push(
            text(t!(i18n, ids::RULES_MATRIX_COL_PAIR))
                .size(TS.caption)
                .color(muted)
                .width(pair_width(m))
                .boxed(),
        )
        .push(
            text(t!(i18n, ids::RULES_MATRIX_COL_CLEARANCE))
                .size(TS.caption)
                .color(muted)
                .width(clearance_width(m))
                .align_x(iced::alignment::Horizontal::Right)
                .boxed(),
        )
        .push(Space::new().width(m.row_height).boxed());
    let whole = CellKey::new(a, b);
    let mut rows: Column<Element<'a, Message>> = Column::new()
        .spacing(SP.xs)
        .push(head.boxed())
        .push(pair_row(frame, whole.clone(), None));
    for [p, q] in OBJECT_PAIRS {
        let key = CellKey {
            pair: Some([p.to_string(), q.to_string()]),
            ..whole.clone()
        };
        rows = rows.push(pair_row(frame, key, Some(&whole)));
    }
    let rows = mouse_area(rows).on_exit(up(RulesTabMsg::MatrixHovered {
        project_id: id,
        key: None,
    }));
    let (raised, accent) = (resolve(tokens).raised, ti(tokens.accent));
    let header = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(title.boxed())
        .push(close.boxed());
    container(
        Column::new()
            .spacing(SP.s)
            .push(header.boxed())
            .push(rows.boxed()),
    )
    .padding([SP.m, SP.l])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(raised)),
        border: Border {
            width: 1.0,
            color: accent,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// One row of the object card. `whole` is the All objects key of an object
/// pair row; `None` for the All objects row itself.
fn pair_row<'a>(frame: &Frame<'a>, key: CellKey, whole: Option<&CellKey>) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let m = GridStyle::from_tokens(tokens, TS.body).metrics();
    let format = LengthFormat::new(i18n, Unit::Mm);
    let defaults = defaults(frame);
    let label: Element<'a, Message> = match &key.pair {
        None => text(t!(i18n, ids::RULES_MATRIX_ALL_OBJECTS))
            .size(TS.body)
            .font(medium())
            .width(pair_width(m))
            .boxed(),
        Some(pair) => text(pair_label(i18n, pair))
            .size(TS.body)
            .width(pair_width(m))
            .boxed(),
    };
    let written = matrix::cell(&frame.table, &key)
        .and_then(|row| row.limit(signex_drc_dsl::table::LimitField::MinClearance))
        .map(|limit| limit.value.clone());
    // An empty object pair shows the All objects value; an empty All
    // objects row its fallback.
    let placeholder_mm = match whole {
        Some(whole) => shown(&frame.table, defaults, whole).map(Shown::mm),
        None => defaults
            .get(&default_key(&key))
            .map(|nm| crate::app::rules_tab::json::mm(*nm)),
    };
    let placeholder: Label<'static> = match placeholder_mm {
        Some(mm) => cell_text(PairClearance::Fallback(mm), format, &matrix_words(i18n)),
        None => t!(i18n, ids::RULES_MATRIX_NO_DEFAULT).into(),
    };
    let value = match frame
        .state
        .matrix_edit
        .as_ref()
        .filter(|edit| edit.key.same_as(&key))
    {
        Some(edit) => edit.text.clone(),
        None => written.clone().unwrap_or_default(),
    };
    let typed_key = key.clone();
    let input = text_input(placeholder, value)
        .on_input(move |text| {
            up(RulesTabMsg::MatrixPairTyped {
                project_id: id,
                key: typed_key.clone(),
                text,
            })
        })
        .on_submit(up(RulesTabMsg::MatrixCellCommitted { project_id: id }))
        .size(TS.body)
        .font(mono())
        .padding(SP.xs)
        .align_x(iced::alignment::Horizontal::Right)
        .width(clearance_width(m))
        .style(crate::panels::form_look::text_input_style);
    let clear: Element<'a, Message> = if written.is_some() {
        let tip = if whole.is_some() {
            ids::RULES_MATRIX_CLEAR_PAIR
        } else {
            ids::RULES_MATRIX_CLEAR_CELL
        };
        let ink = tokens::muted_text(tokens);
        let glyph = svg(UNDO.clone())
            .width(TS.body)
            .height(TS.body)
            .style(move |_: &Theme, _| svg::Style {
                color: Some(ink),
                ..svg::Style::default()
            });
        tooltip(
            button(glyph)
                .padding(SP.xs)
                .style(crate::styles::ghost_button(tokens))
                .on_press(up(RulesTabMsg::MatrixCellCleared {
                    project_id: id,
                    key: key.clone(),
                })),
            text(t!(i18n, tip)).size(TS.caption),
            tooltip::Position::Bottom,
        )
        .boxed()
    } else {
        Space::new().width(m.row_height).boxed()
    };
    let is_pinned = frame
        .state
        .matrix_pin
        .as_ref()
        .is_some_and(|pin| pin.same_as(&key));
    let wash = resolve(tokens).accent_soft;
    let line = container(
        Row::new()
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .push(label)
            .push(input.boxed())
            .push(clear),
    )
    .style(move |_: &Theme| container::Style {
        background: is_pinned.then_some(Background::Color(wash)),
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    mouse_area(line)
        .on_enter(up(RulesTabMsg::MatrixHovered {
            project_id: id,
            key: Some(key.clone()),
        }))
        .on_press(up(RulesTabMsg::MatrixPairPinned {
            project_id: id,
            key,
        }))
        .boxed()
}

/// What the preview caption says about `preview`.
fn preview_caption(i18n: &I18n, preview: &MatrixPreview) -> Localized {
    let format = LengthFormat::new(i18n, Unit::Mm);
    let (a, b) = (preview.key.a.as_str(), preview.key.b.as_str());
    let Some(value) = preview.shown else {
        return t!(i18n, ids::RULES_MATRIX_PREVIEW_BARE, "a" => a, "b" => b);
    };
    let number = format.number(value.mm());
    match (&preview.key.pair, value) {
        (None, _) => t!(
            i18n,
            ids::RULES_MATRIX_PREVIEW_CLASSES,
            "a" => a,
            "b" => b,
            "value" => number
        ),
        (Some(pair), Shown::Written(_)) => t!(
            i18n,
            ids::RULES_MATRIX_PREVIEW_OBJECTS_WRITTEN,
            "a" => a,
            "b" => b,
            "pair" => pair_label(i18n, pair).into_string(),
            "value" => number
        ),
        (Some(pair), Shown::Fallback(_)) => t!(
            i18n,
            ids::RULES_MATRIX_PREVIEW_OBJECTS_FALLBACK,
            "a" => a,
            "b" => b,
            "pair" => pair_label(i18n, pair).into_string(),
            "value" => number
        ),
    }
}

/// The preview row: what is drawn and at which value, and what the
/// drawing shows.
fn preview_row<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let faint = resolve(tokens).ink4;
    let muted = tokens::muted_text(tokens);
    let caption = match matrix_preview(frame.state, &frame.table, defaults(frame)) {
        Some(preview) => preview_caption(i18n, &preview),
        None => t!(i18n, ids::RULES_MATRIX_PREVIEW_NONE),
    };
    let drawn = super::grid_preview::drawn(frame);
    // The drawing brings its own heading; show the row's only without it.
    let mut head: Row<Element<'a, Message>> = Row::new().spacing(SP.m).align_y(Alignment::Center);
    if drawn.is_none() {
        head = head.push(
            text(t!(i18n, ids::RULES_MATRIX_PREVIEW))
                .size(TS.section)
                .style(crate::styles::section_label(tokens))
                .boxed(),
        );
    }
    let head = head
        .push(Space::new().width(Length::Fill).boxed())
        .push(text(caption).size(TS.caption).color(faint).boxed());
    let mut left = Column::new()
        .spacing(SP.s)
        .push(head.width(Length::Fill).boxed());
    if let Some(drawn) = drawn {
        left = left.push(drawn);
    }
    Row::new()
        .spacing(SP.xl)
        .align_y(Alignment::Start)
        .push(left.width(Length::FillPortion(3)).boxed())
        .push(
            text(t!(i18n, ids::RULES_MATRIX_PREVIEW_ABOUT))
                .size(TS.caption)
                .color(muted)
                .width(Length::FillPortion(2))
                .boxed(),
        )
        .boxed()
}
