//! Shared pieces of the per-kind Properties builders: the colours every
//! builder is handed, the selection-info lookups, and the row kit every
//! schematic Properties panel builds its rows from (epic #2523).

use super::super::*;

/// The colours a Properties builder draws with.
#[derive(Clone, Copy)]
pub(super) struct PanelStyle {
    pub muted: Color,
    pub primary: Color,
    pub border_c: Color,
    pub palette: PanelPalette,
}

/// Rotation, in degrees, read from the panel's display text (`90°`). A
/// missing, malformed or non-finite value reads as `0`. Pure: this runs
/// while the panel is built, so no refusal is logged here.
pub(super) fn rotation_degrees(text: &str) -> f64 {
    signex_types::numeric::parse_finite_f64(text.trim_end_matches('°')).unwrap_or(0.0)
}

/// An engine `Yes` / `No` value in `i18n`'s language. Any other value is
/// engine text with no catalog message yet and passes through (#2060).
pub(super) fn yes_no_shown(i18n: &signex_i18n::I18n, value: String) -> String {
    match value.as_str() {
        "Yes" => i18n.message_plain(signex_i18n::ids::PROPERTIES_YES, &[]),
        "No" => i18n.message_plain(signex_i18n::ids::PROPERTIES_NO, &[]),
        _ => value,
    }
}

/// The row `key` of a selection's info, empty when the selection has none.
pub(super) fn info_row(info: &[(String, String)], key: &str) -> String {
    info.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// The horizontal justification row of a selection (#2024): the one lookup
/// every label, text note and symbol field panel shows it from.
pub(super) fn justify_h_row(info: &[(String, String)]) -> String {
    info_row(info, signex_engine::info_keys::JUSTIFY_H)
}

/// The vertical justification row of a selection (#2024).
pub(super) fn justify_v_row(info: &[(String, String)]) -> String {
    info_row(info, signex_engine::info_keys::JUSTIFY_V)
}

/// The justification a Properties row reports (`info_keys::JUSTIFY_H`).
pub(super) fn h_align_from_row(row: &str) -> signex_types::schematic::HAlign {
    use signex_types::schematic::HAlign;
    match row {
        "Left" => HAlign::Left,
        "Right" => HAlign::Right,
        _ => HAlign::Center,
    }
}

// ── The row kit (epic #2523) ──
//
// A row is a fixed label column and a control: a value field with its axis
// tag and unit inside, an X / Y or W / H pair, the 4-step Rotation control, a
// sunken read-only value, a checkbox grid. Sizes are Graphite's at a 12 px
// body, kept as ratios of the type scale; colours come from the form look.

use crate::panels::form_look::{self, FormLook};
use crate::panels::graphite::PROPERTY_FIELD_RADIUS;
use crate::styles::{Spacing, TypeScale};
use iced::widget::{Row, stack};
use iced::{Alignment, Padding};
use signex_i18n::{I18n, Localized, Translatable, UserText, ids, t};
use signex_widgets::{CheckboxLabel, Label, SegmentedStyle};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Room an axis tag (X, Y, W, H) takes inside a field, in captions.
const AXIS_EM: f32 = 1.0;
/// Room a unit (mm, mil, pt) takes inside a field, in captions.
const UNIT_EM: f32 = 2.2;
/// Height of a count pill, as a ratio of the body size (16 px at 12 px).
const PILL_HEIGHT_EM: f32 = 16.0 / 12.0;
/// Side padding of a count pill (6 px at 12 px).
const PILL_PAD_EM: f32 = 6.0 / 12.0;
/// Section header padding, as the form look's: 12 px around, 6 px below.
const SECTION_PAD_EM: f32 = 12.0 / 12.0;
const SECTION_PAD_BOTTOM_EM: f32 = 6.0 / 12.0;

/// The quarter turns the Rotation control offers, in degrees.
pub(in crate::panels) const ROTATION_STEPS: [f64; 4] = [0.0, 90.0, 180.0, 270.0];

/// The step the Rotation control shows for a stored angle: the nearest
/// quarter turn (a non-finite angle reads as 0).
pub(in crate::panels) fn rotation_step(degrees: f64) -> f64 {
    let turns = (degrees.rem_euclid(360.0) / 90.0).round();
    let index = if turns.is_finite() {
        turns as usize % 4
    } else {
        0
    };
    ROTATION_STEPS[index]
}

/// How a rotation step reads: `90°`.
pub(in crate::panels) fn rotation_step_label(i18n: &I18n, degrees: f64) -> Localized {
    t!(i18n, ids::PROPERTIES_ROTATION_STEP, "degrees" => format!("{degrees:.0}"))
}

/// The unit symbol shown at the right of a length field.
pub(in crate::panels) fn unit_suffix(i18n: &I18n, unit: signex_types::coord::Unit) -> Localized {
    t!(i18n, ids::PROPERTIES_UNIT_SUFFIX, "unit" => unit.to_string())
}

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

fn ink3(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(FormLook::of(theme).ink3),
        ..Default::default()
    }
}

fn ink4(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(FormLook::of(theme).ink4),
        ..Default::default()
    }
}

/// The label cell of a row: the fixed label column in the third ink, cut
/// with an ellipsis rather than wrapped.
pub(in crate::panels) fn row_label<'a>(label: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
    container(
        text(label)
            .size(TS.body)
            .style(ink3)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End),
    )
    .width(PROPERTY_LABEL_COLUMN)
    .clip(true)
    .boxed()
}

/// A row: the label column, then `control` in the field column.
pub(in crate::panels) fn labelled_row<'a>(
    label: impl Into<Label<'a>>,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    container(
        iced::widget::row![
            row_label(label),
            container(control).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// Two controls side by side in one row (X / Y, W / H, size and style).
pub(in crate::panels) fn pair_row<'a>(
    label: impl Into<Label<'a>>,
    first: Element<'a, PanelMsg>,
    second: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    labelled_row(
        label,
        iced::widget::row![first, second]
            .spacing(SP.s + SP.xs)
            .width(Length::Fill)
            .boxed(),
    )
}

/// The axis tag and the unit drawn over a field, left and right.
fn field_tags<'a>(axis: Option<Label<'a>>, unit: Option<Label<'a>>) -> Element<'a, PanelMsg> {
    let mut tags = Row::<Element<'a, PanelMsg>>::new()
        .align_y(Alignment::Center)
        .height(Length::Fill)
        .padding([0.0, SP.s + SP.xs]);
    if let Some(axis) = axis {
        tags = tags.push(text(axis).size(TS.caption).style(ink4).boxed());
    }
    tags = tags.push(Space::new().width(Length::Fill).boxed());
    if let Some(unit) = unit {
        tags = tags.push(text(unit).size(TS.caption).style(ink4).boxed());
    }
    tags.boxed()
}

/// An editable value field with an optional axis tag at the left and unit
/// at the right, inside the field. Typing reports each keystroke, Enter
/// submits; the field keeps the form look's hairline and focus border.
pub(in crate::panels) fn unit_input<'a>(
    axis: Option<Label<'a>>,
    value: String,
    unit: Option<Label<'a>>,
    on_input: impl Fn(String) -> PanelMsg + 'a,
    on_submit: PanelMsg,
) -> Element<'a, PanelMsg> {
    let inset = SP.s + SP.xs;
    let left = inset + axis.as_ref().map_or(0.0, |_| TS.caption * AXIS_EM + SP.s);
    let right = inset + unit.as_ref().map_or(0.0, |_| TS.caption * UNIT_EM + SP.s);
    let input = signex_widgets::text_input(UserText::new(String::new()), value)
        .on_input(on_input)
        .on_submit(on_submit)
        .size(TS.body)
        .font(mono())
        .padding(Padding {
            top: SP.s,
            bottom: SP.s,
            left,
            right,
        })
        .style(form_look::text_input_style)
        .width(Length::Fill);
    // The tags lie on top; text never captures, so the field gets every press.
    stack![input, field_tags(axis, unit)]
        .width(Length::Fill)
        .boxed()
}

/// A read-only value: sunken, in the third ink, with an optional axis tag
/// and the unit at the right (Length, a locked position).
pub(in crate::panels) fn sunken_value<'a>(
    axis: Option<Label<'a>>,
    value: String,
    unit: Option<Label<'a>>,
) -> Element<'a, PanelMsg> {
    let mut content = Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s)
        .align_y(Alignment::Center);
    if let Some(axis) = axis {
        content = content.push(text(axis).size(TS.caption).style(ink4).boxed());
    }
    // The value is a reading of the design, not catalog text.
    content = content.push(
        text(UserText::new(value))
            .size(TS.body)
            .font(mono())
            .style(ink3)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .width(Length::Fill)
            .boxed(),
    );
    if let Some(unit) = unit {
        content = content.push(text(unit).size(TS.caption).style(ink4).boxed());
    }
    container(content)
        .padding([SP.s, SP.s + SP.xs])
        .width(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(FormLook::of(theme).sunken)),
            border: Border {
                radius: PROPERTY_FIELD_RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A labelled read-only row: [`sunken_value`] with no axis tag (Length).
#[allow(
    dead_code,
    reason = "row kit of epic #2523; the g1 Properties units adopt it"
)]
pub(in crate::panels) fn sunken_value_row<'a>(
    label: impl Into<Label<'a>>,
    value: String,
    unit: Option<Label<'a>>,
) -> Element<'a, PanelMsg> {
    labelled_row(label, sunken_value(None, value, unit))
}

/// The segmented-control look of the form: sunken track, raised chip, the
/// caption size of the mockup's small control, segments sharing the width.
pub(in crate::panels) fn segmented_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = crate::styles::tokens::resolve(tokens);
    SegmentedStyle::new(
        r.sunken,
        r.raised,
        r.shadow,
        r.ink3,
        crate::styles::ti(tokens.text),
        TS.caption,
    )
    .fill_width(true)
}

/// The Rotation control: 0° / 90° / 180° / 270°, the stored angle shown as
/// its nearest step; a pick reports the step in degrees.
pub(in crate::panels) fn rotation_control<'a>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    degrees: f64,
    on_pick: impl Fn(f64) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    let options = ROTATION_STEPS.map(|step| (step, Label::from(rotation_step_label(i18n, step))));
    signex_widgets::segmented(
        options,
        rotation_step(degrees),
        segmented_style(tokens),
        on_pick,
    )
}

/// The Rotation row: the 4-step control, or the step as a sunken value
/// while the object is locked.
pub(in crate::panels) fn rotation_row<'a>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    degrees: f64,
    is_locked: bool,
    on_pick: impl Fn(f64) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    let label = t!(i18n, ids::PANELS_PLACED_ROTATION);
    if is_locked {
        let step = rotation_step_label(i18n, rotation_step(degrees));
        return labelled_row(label, sunken_value(None, step.to_string(), None));
    }
    labelled_row(label, rotation_control(i18n, tokens, degrees, on_pick))
}

/// A small count pill beside a section title: sunken, third ink.
pub(in crate::panels) fn count_pill<'a>(i18n: &I18n, count: usize) -> Element<'a, PanelMsg> {
    let count = u32::try_from(count).unwrap_or(u32::MAX);
    // The count in the locale's digits.
    let shown = UserText::new(signex_widgets::chips::count_text(i18n, count));
    container(
        text(shown)
            .size(TS.caption)
            .style(ink3)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .height(TS.body * PILL_HEIGHT_EM)
    .padding([0.0, TS.body * PILL_PAD_EM])
    .align_y(iced::alignment::Vertical::Center)
    .style(|theme: &Theme| container::Style {
        background: Some(Background::Color(FormLook::of(theme).sunken)),
        border: Border {
            radius: signex_widgets::theme_ext::Radii::GRAPHITE.pill.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// A collapsible section whose header carries a count pill (Parameters 4,
/// Pins 2): the chevron, the uppercase title, the pill; `None` draws no pill.
#[allow(
    dead_code,
    reason = "row kit of epic #2523; the g1 Properties units adopt it"
)]
pub(in crate::panels) fn counted_section<'a>(
    ctx: &PanelContext,
    key: &str,
    title: impl Into<Translatable>,
    count: Option<usize>,
    content: impl FnOnce() -> Column<Element<'a, PanelMsg>>,
) -> Column<Element<'a, PanelMsg>> {
    let is_collapsed = ctx.collapsed_sections.contains(key);
    let ink = crate::styles::tokens::resolve(&ctx.tokens).ink4;
    let title = title.into().render(&*ctx.i18n);
    // The catalog title, upper-cased for the locale.
    let shown = UserText::new(ctx.i18n.upper_for_display(&title));
    let mut label = Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center)
        .push(signex_widgets::chevron::chevron(!is_collapsed, TS.section, ink).boxed())
        .push(text(shown).size(TS.section).color(ink).boxed());
    if let Some(count) = count {
        label = label.push(count_pill(&ctx.i18n, count));
    }
    let pad = TS.body * SECTION_PAD_EM;
    let header = button(container(label).width(Length::Fill).padding(Padding {
        top: pad,
        right: pad,
        bottom: TS.body * SECTION_PAD_BOTTOM_EM,
        left: pad,
    }))
    .padding(0)
    .width(Length::Fill)
    .on_press(PanelMsg::ToggleSection(key.to_string()))
    .style(|_: &Theme, _| iced::widget::button::Style::default());
    let mut col = Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(header.boxed());
    if !is_collapsed {
        col = col.push(content().boxed());
    }
    col
}

/// One checkbox with its label beside it.
pub(in crate::panels) fn check_item<'a>(
    label: impl Into<Label<'a>>,
    is_checked: bool,
    on_toggle: PanelMsg,
) -> Element<'a, PanelMsg> {
    signex_widgets::checkbox(is_checked)
        .labelled(label)
        .on_toggle(move |_| on_toggle.clone())
        .size(TS.heading)
        .spacing(SP.m)
        .text_size(TS.body)
        .style(form_look::checkbox_style)
        .width(Length::Fill)
        .boxed()
}

/// Checkboxes two to a row (Mirror X / Mirror Y, Locked / DNP).
#[allow(
    dead_code,
    reason = "row kit of epic #2523; the g1 Properties units adopt it"
)]
pub(in crate::panels) fn check_grid<'a>(
    items: Vec<(Localized, bool, PanelMsg)>,
) -> Element<'a, PanelMsg> {
    let mut grid = Column::<Element<'a, PanelMsg>>::new().spacing(SP.xs);
    let mut items = items.into_iter();
    while let Some((label, is_checked, msg)) = items.next() {
        let mut pair = Row::<Element<'a, PanelMsg>>::new()
            .spacing(SP.m)
            .push(check_item(label, is_checked, msg));
        pair = match items.next() {
            Some((label, is_checked, msg)) => pair.push(check_item(label, is_checked, msg)),
            None => pair.push(Space::new().width(Length::Fill).boxed()),
        };
        grid = grid.push(pair.boxed());
    }
    container(grid)
        .padding([SP.xs, TS.body * SECTION_PAD_EM])
        .width(Length::Fill)
        .boxed()
}

/// The State section with its Locked box, the last section of a placed
/// object's Properties.
#[allow(
    dead_code,
    reason = "row kit of epic #2523; the g1 Properties units adopt it"
)]
pub(in crate::panels) fn push_state_section<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    is_locked: bool,
    on_toggle: PanelMsg,
) -> Column<Element<'a, PanelMsg>> {
    col.push(
        ctx.collapsible(
            "sel_state",
            ids::PROPERTIES_SECTION_STATE,
            &ctx.collapsed_sections,
            move || {
                let locked = t!(ctx.i18n, ids::PROPERTIES_LOCKED);
                Column::new().spacing(0).width(Length::Fill).push(
                    container(check_item(locked, is_locked, on_toggle))
                        .padding([SP.xs, TS.body * SECTION_PAD_EM])
                        .width(Length::Fill)
                        .boxed(),
                )
            },
        )
        .boxed(),
    )
}

#[cfg(test)]
mod kit_tests {
    use super::*;

    #[test]
    fn a_stored_angle_shows_as_its_nearest_quarter_turn() {
        assert_eq!(rotation_step(0.0), 0.0);
        assert_eq!(rotation_step(89.9999), 90.0);
        assert_eq!(rotation_step(181.0), 180.0);
        assert_eq!(rotation_step(-90.0), 270.0);
        assert_eq!(rotation_step(359.0), 0.0);
        assert_eq!(rotation_step(f64::NAN), 0.0);
    }

    #[test]
    fn every_step_is_a_stored_angle_that_maps_to_itself() {
        for step in ROTATION_STEPS {
            assert_eq!(rotation_step(step), step);
        }
    }

    #[test]
    fn a_step_reads_as_degrees_with_the_degree_sign() {
        let en = I18n::en_us();
        let shown = rotation_step_label(&en, 270.0)
            .to_string()
            .replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(shown, "270°");
    }

    /// A pick on the Rotation control hands back the stored degrees of
    /// that step: a press on the 180° segment reports 180.
    #[test]
    fn a_rotation_segment_reports_its_degrees() {
        use crate::layout_probe::{lay_out_texts, messages_for_events};
        let en = I18n::en_us();
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
        let size = iced::Size::new(240.0, 40.0);
        let control = || {
            rotation_control(&en, &tokens, 0.0, |deg| {
                PanelMsg::SetPrePlacementRotation(deg)
            })
        };
        let shown = lay_out_texts(control(), size);
        let at = shown
            .iter()
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == "180°")
            .expect("the 180° segment");
        let centre = iced::Point::new(
            at.bounds.x + at.bounds.width / 2.0,
            at.bounds.y + at.bounds.height / 2.0,
        );
        let press = [
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                iced::mouse::Button::Left,
            )),
        ];
        let picked: Vec<f64> = messages_for_events(control(), size, centre, &press)
            .into_iter()
            .filter_map(|m| match m {
                PanelMsg::SetPrePlacementRotation(deg) => Some(deg),
                _ => None,
            })
            .collect();
        assert_eq!(picked, [180.0]);
    }
}
