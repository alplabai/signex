//! The range-filter bar both surfaces draw. Pure: it reads the model and
//! turns interaction into [`RangeFilterMsg`], mapped by the caller.

use signex_widgets::dropdown_style::GraphiteDropdown as _;
use std::collections::BTreeMap;
use std::rc::Rc;

use iced::widget::{Widget, column, container, pick_list, row};
use iced::{Element, Length};
use signex_i18n::{I18n, UserText, ids, t};
use signex_library::RangeOp;
use signex_types::quantity::{SiPrefix, Unit};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{Label, PickListPlaceholder, button, raw, text, text_input};

use super::draft::{BoundDraft, RangeFilterDraft, RangeFilters};
use crate::library::messages::{RangeBound, RangeFilterMsg};
use crate::library::prefix_picker::prefix_picker;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
const NUMBER_W: f32 = 84.0;

/// A column a range filter can be put on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilterColumn {
    pub key: String,
    pub label: String,
    pub unit: Unit,
}

impl std::fmt::Display for FilterColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.label, self.unit.glyph())
    }
}

/// A parameter name as a column caption: its first letter in capitals
/// ("value" shows as "Value"), cased for the UI language.
pub(crate) fn parameter_caption(i18n: &signex_i18n::I18n, name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => i18n.upper_for_display(first.encode_utf8(&mut [0; 4])) + chars.as_str(),
        None => String::new(),
    }
}

/// The bar: one row per filtered column, and a picker to add another.
/// `None` when there is nothing to filter and no filter to show.
///
/// `excluded` is how many rows each column's filter dropped because their
/// cell is not a measurement in the column's unit.
pub(crate) fn range_filter_bar<'a, M: Clone + 'a>(
    filters: &'a RangeFilters,
    columns: &[FilterColumn],
    excluded: &BTreeMap<String, usize>,
    tokens: &'a ThemeTokens,
    i18n: &'a signex_i18n::I18n,
    wrap: impl Fn(RangeFilterMsg) -> M + 'a,
) -> Option<Element<'a, M>> {
    let free: Vec<FilterColumn> = columns
        .iter()
        .filter(|c| filters.get(&c.key).is_none())
        .cloned()
        .collect();
    if filters.is_empty() && free.is_empty() {
        return None;
    }
    let wrap: Rc<dyn Fn(RangeFilterMsg) -> M + 'a> = Rc::new(wrap);
    let mut bar = column![].spacing(SP.s);
    for (key, draft) in filters.iter() {
        let label = columns
            .iter()
            .find(|c| &c.key == key)
            .map_or(key.as_str(), |c| c.label.as_str());
        let label = parameter_caption(i18n, label);
        let dropped = excluded.get(key).copied().unwrap_or(0);
        bar = bar.push(filter_row(key, &label, draft, dropped, tokens, i18n, &wrap));
    }
    if !free.is_empty() {
        let w = Rc::clone(&wrap);
        bar = bar.push(
            pick_list(None::<FilterColumn>, free, move |c: &FilterColumn| {
                format!("{} ({})", parameter_caption(i18n, &c.label), c.unit.glyph())
            })
            .graphite()
            .on_select(move |c: FilterColumn| {
                w(RangeFilterMsg::ColumnPicked {
                    key: c.key,
                    unit: c.unit,
                })
            })
            .placeholder_label(t!(i18n, ids::LIBRARY_RANGE_FILTER_ADD))
            .padding(SP.s)
            .boxed(),
        );
    }
    Some(bar.boxed())
}

fn filter_row<'a, M: Clone + 'a>(
    key: &str,
    label: &str,
    draft: &'a RangeFilterDraft,
    dropped: usize,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    wrap: &Rc<dyn Fn(RangeFilterMsg) -> M + 'a>,
) -> Element<'a, M> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let error_c = theme_ext::error_color(tokens);

    let op = {
        let (w, key) = (Rc::clone(wrap), key.to_string());
        pick_list(Some(draft.op), RangeOp::ALL.to_vec(), |op: &RangeOp| {
            op.to_string()
        })
        .graphite()
        .on_select(move |op: RangeOp| {
            w(RangeFilterMsg::OperatorPicked {
                key: key.clone(),
                op,
            })
        })
        .padding([3, 6])
    };

    let mut line = row![
        text(raw(label.to_string())).size(TS.body).color(text_c),
        op,
        bound(
            key,
            RangeBound::Low,
            &draft.low,
            &draft.unit,
            tokens,
            i18n,
            wrap
        ),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    if draft.op.is_between() {
        line = line
            .push(
                text(t!(i18n, ids::LIBRARY_RANGE_FILTER_AND))
                    .size(TS.body)
                    .color(muted)
                    .boxed(),
            )
            .push(bound(
                key,
                RangeBound::High,
                &draft.high,
                &draft.unit,
                tokens,
                i18n,
                wrap,
            ));
    }
    let clear = {
        let (w, key) = (Rc::clone(wrap), key.to_string());
        button(text(raw("✕")).size(TS.body).color(muted))
            .padding([2, 6])
            .style(button::text)
            .on_press(w(RangeFilterMsg::Cleared { key }))
    };
    line = line.push(clear.boxed());

    let status: Option<(Label<'a>, iced::Color)> = [&draft.low, &draft.high]
        .into_iter()
        .find_map(|b| b.refusal.clone())
        .map(|why| (raw(why), error_c))
        .or_else(|| match draft.filter() {
            None => Some((
                t!(i18n, ids::LIBRARY_RANGE_FILTER_ENTER_NUMBER).into(),
                muted,
            )),
            Some(_) if dropped > 0 => Some((
                t!(
                    i18n,
                    ids::LIBRARY_RANGE_FILTER_EXCLUDED,
                    "count" => dropped,
                    "unit" => draft.unit.glyph()
                )
                .into(),
                muted,
            )),
            Some(_) => None,
        });
    let mut out = column![line].spacing(2);
    if let Some((msg, color)) = status {
        out = out.push(text(msg).size(TS.caption).color(color).boxed());
    }
    container(out).width(Length::Shrink).boxed()
}

/// A number field with its prefix picker, or the plain unit for a unit
/// that takes no prefix.
fn bound<'a, M: Clone + 'a>(
    key: &str,
    which: RangeBound,
    b: &'a BoundDraft,
    unit: &Unit,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    wrap: &Rc<dyn Fn(RangeFilterMsg) -> M + 'a>,
) -> Element<'a, M> {
    let number = {
        let (w, key) = (Rc::clone(wrap), key.to_string());
        text_input(
            t!(i18n, ids::LIBRARY_RANGE_FILTER_NUMBER_PLACEHOLDER),
            &b.text,
        )
        .on_input(move |text| {
            w(RangeFilterMsg::NumberEdited {
                key: key.clone(),
                bound: which,
                text,
            })
        })
        .padding(SP.s)
        .size(TS.body)
        .width(Length::Fixed(NUMBER_W))
    };
    let (w, pick_key) = (Rc::clone(wrap), key.to_string());
    let unit_part: Element<'a, M> = prefix_picker(unit, b.prefix, move |prefix: SiPrefix| {
        w(RangeFilterMsg::PrefixPicked {
            key: pick_key.clone(),
            bound: which,
            prefix,
        })
    })
    .unwrap_or_else(|| {
        text(UserText::new(unit.glyph()))
            .size(TS.body)
            .color(theme_ext::text_secondary(tokens))
            .boxed()
    });
    row![number, unit_part]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
}
