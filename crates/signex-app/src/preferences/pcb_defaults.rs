//! PCB ▸ PCB Defaults (#2868): what each new board dimension starts from —
//! line width, text height, decimals, arrows, text, extension lines and
//! number format — on the `setting_row` grid. Drafts; persisted on Save.

use iced::alignment::Horizontal;
use iced::widget::container;
use iced::{Background, Border, Color, Element, Font, Length, Padding, Theme, Widget as _};
use signex_engine::pcb::primitive_edit::{DIMENSION_MAX_PRECISION, DimensionStyleEdit as E};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::pcb_dimension_style::{
    ArrowStyle, BoardFont, DimensionTextPosition, DimensionUnits,
};
use signex_types::theme::ThemeTokens;
use signex_widgets::text_input;

use super::PrefMsg;
use super::pages_view::controls::{segmented_choice, toggle};
use super::setting_row::{SettingRow, help_text, page, section_header, setting_row};
use crate::panels::pcb_primitive_properties::{PrimitiveField as F, format_number};
use crate::pcb_defaults_prefs::PcbDefaultsDraft;
use crate::styles::{self, ti, tokens};

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;
const SP: styles::Spacing = styles::Spacing::DEFAULT;

/// Width of a number or text field.
const FIELD_W: f32 = TS.body * 8.0;

/// What a PCB Defaults row changed.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbDefaultsMsg {
    /// Text typed in a number, Prefix or Suffix field.
    Typed(F, String),
    /// A choice or switch picked.
    Picked(E),
    /// A number of decimals picked.
    PrecisionPicked(u8),
}

fn msg(m: PcbDefaultsMsg) -> PrefMsg {
    PrefMsg::PcbDefaults(m)
}

/// The page.
pub(super) fn content_pcb_defaults<'a>(
    draft: &'a PcbDefaultsDraft,
    t: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, PrefMsg> {
    let d = &draft.defaults;
    let s = &d.style;
    let number = |label: MessageId, field: F, value: f64| {
        let text = draft
            .texts
            .get(&field)
            .cloned()
            .unwrap_or_else(|| format_number(value));
        let error = draft.refused.get(&field).map(|why| {
            let what = t!(i18n, label).into_string();
            t!(i18n, ids::PCB_PRIM_REFUSED, "what" => what, "why" => why.as_str())
        });
        row(
            label,
            entry(field, text, error.is_some(), t),
            error,
            t,
            i18n,
        )
    };
    let words = |label: MessageId, field: F, value: &str| {
        let text = draft
            .texts
            .get(&field)
            .cloned()
            .unwrap_or_else(|| value.to_string());
        row(label, entry(field, text, false, t), None, t, i18n)
    };
    let choice =
        |label: MessageId, control: Element<'a, PrefMsg>| row(label, control, None, t, i18n);
    let precision = (0..=DIMENSION_MAX_PRECISION)
        .map(|p| {
            let step = t!(i18n, ids::PCB_PRIM_DIM_PRECISION_STEP, "decimals" => u32::from(p));
            (p, step)
        })
        .collect();
    let items = vec![
        section_header(t!(i18n, ids::PREFERENCES_SECTION_PCB_DIMENSIONS), None, t),
        help_text(t!(i18n, ids::PREFERENCES_PCB_DIMENSIONS_HELP), t),
        number(ids::PCB_PRIM_LINE_WIDTH, F::LineWidth, d.line_width),
        number(ids::PCB_PRIM_TEXT_HEIGHT, F::TextHeight, d.text_height),
        choice(
            ids::PCB_PRIM_PRECISION,
            segmented_choice(precision, d.precision, t, |p| {
                msg(PcbDefaultsMsg::PrecisionPicked(p))
            }),
        ),
        section_header(t!(i18n, ids::PCB_PRIM_DIM_ARROWS), None, t),
        choice(
            ids::PCB_PRIM_DIM_ARROW_STYLE,
            segmented_choice(
                named(
                    i18n,
                    &[
                        (ids::PCB_PRIM_DIM_ARROW_FILLED, ArrowStyle::Filled),
                        (ids::PCB_PRIM_DIM_ARROW_OPEN, ArrowStyle::Open),
                        (ids::PCB_PRIM_DIM_ARROW_NONE, ArrowStyle::None),
                    ],
                ),
                s.arrow_style,
                t,
                |v| msg(PcbDefaultsMsg::Picked(E::ArrowStyle(v))),
            ),
        ),
        number(
            ids::PCB_PRIM_DIM_ARROW_LENGTH,
            F::ArrowLength,
            s.arrow_length,
        ),
        number(ids::PCB_PRIM_DIM_ARROW_WIDTH, F::ArrowWidth, s.arrow_width),
        section_header(t!(i18n, ids::PCB_PRIM_DIM_TEXT), None, t),
        choice(
            ids::PCB_PRIM_DIM_FONT,
            segmented_choice(
                named(
                    i18n,
                    &[
                        (ids::PCB_PRIM_DIM_FONT_MONO, BoardFont::Mono),
                        (ids::PCB_PRIM_DIM_FONT_SANS, BoardFont::Sans),
                    ],
                ),
                s.font,
                t,
                |v| msg(PcbDefaultsMsg::Picked(E::Font(v))),
            ),
        ),
        choice(
            ids::PCB_PRIM_DIM_BOLD,
            toggle(s.bold, t, |on| msg(PcbDefaultsMsg::Picked(E::Bold(on)))),
        ),
        choice(
            ids::PCB_PRIM_DIM_ITALIC,
            toggle(s.italic, t, |on| msg(PcbDefaultsMsg::Picked(E::Italic(on)))),
        ),
        number(ids::PCB_PRIM_DIM_TEXT_WIDTH, F::TextWidth, s.text_width),
        choice(
            ids::PCB_PRIM_DIM_TEXT_POSITION,
            segmented_choice(
                named(
                    i18n,
                    &[
                        (ids::PCB_PRIM_DIM_TEXT_ABOVE, DimensionTextPosition::Above),
                        (ids::PCB_PRIM_DIM_TEXT_INLINE, DimensionTextPosition::Inline),
                        (ids::PCB_PRIM_DIM_TEXT_MANUAL, DimensionTextPosition::Manual),
                    ],
                ),
                s.text_position,
                t,
                |v| msg(PcbDefaultsMsg::Picked(E::TextPosition(v))),
            ),
        ),
        number(ids::PCB_PRIM_DIM_TEXT_GAP, F::TextGap, s.text_gap),
        section_header(t!(i18n, ids::PCB_PRIM_DIM_EXTENSIONS), None, t),
        number(
            ids::PCB_PRIM_DIM_EXT_OFFSET,
            F::ExtensionOffset,
            s.extension_offset,
        ),
        number(
            ids::PCB_PRIM_DIM_EXT_OVERSHOOT,
            F::ExtensionOvershoot,
            s.extension_overshoot,
        ),
        number(
            ids::PCB_PRIM_DIM_EXT_WIDTH,
            F::ExtensionWidth,
            s.extension_width,
        ),
        section_header(t!(i18n, ids::PCB_PRIM_DIM_UNITS), None, t),
        choice(
            ids::PCB_PRIM_DIM_UNIT,
            segmented_choice(
                named(
                    i18n,
                    &[
                        (ids::PCB_PRIM_DIM_UNIT_MM, DimensionUnits::Mm),
                        (ids::PCB_PRIM_DIM_UNIT_MIL, DimensionUnits::Mil),
                        (ids::PCB_PRIM_DIM_UNIT_IN, DimensionUnits::In),
                    ],
                ),
                s.unit,
                t,
                |v| msg(PcbDefaultsMsg::Picked(E::Unit(v))),
            ),
        ),
        choice(
            ids::PCB_PRIM_DIM_SHOW_UNIT,
            toggle(s.show_unit, t, |on| {
                msg(PcbDefaultsMsg::Picked(E::ShowUnit(on)))
            }),
        ),
        words(ids::PCB_PRIM_DIM_PREFIX, F::Prefix, &s.prefix),
        words(ids::PCB_PRIM_DIM_SUFFIX, F::Suffix, &s.suffix),
        choice(
            ids::PCB_PRIM_DIM_TRAILING_ZEROS,
            toggle(s.trailing_zeros, t, |on| {
                msg(PcbDefaultsMsg::Picked(E::TrailingZeros(on)))
            }),
        ),
    ];
    page(items)
}

/// The choices of a segmented row, each with its catalog label.
fn named<V: Copy>(i18n: &I18n, pairs: &[(MessageId, V)]) -> Vec<(V, Localized)> {
    pairs.iter().map(|&(id, v)| (v, t!(i18n, id))).collect()
}

/// A setting row named `label` around `control`, with `error` under it.
fn row<'a>(
    label: MessageId,
    control: Element<'a, PrefMsg>,
    error: Option<Localized>,
    t: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    setting_row(
        SettingRow {
            label: t!(i18n, label),
            pills: Vec::new(),
            help: None,
            control,
            error,
        },
        t,
    )
}

/// A Graphite field: a bordered frame around a transparent mono input,
/// red-edged while Save would refuse its text.
fn entry<'a>(field: F, text: String, refused: bool, t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let (ink4, ink, err) = (r.ink4, ti(t.text), ti(t.error));
    let input = text_input(UserText::new(String::new()), text)
        .on_input(move |typed| msg(PcbDefaultsMsg::Typed(field, typed)))
        .padding(Padding::ZERO)
        .size(TS.label)
        .font(Font::MONOSPACE)
        .align_x(Horizontal::Right)
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..ink },
        });
    let frame = styles::field(t, false);
    container(input)
        .padding([SP.s, SP.m])
        .width(Length::Fixed(FIELD_W))
        .style(move |theme: &Theme| {
            let mut style = frame(theme);
            if refused {
                style.border.color = err;
            }
            style
        })
        .boxed()
}
