//! The Paste Array card (#2527): Layout (copies, spacing), a preview that
//! takes a drag of copy 2 and edits on its values (mockup row 69), Numbering
//! (count up, step, designators), a worked example, and a footer that says
//! what the paste places.

use super::pointer::{PreviewPointer, preview_pointer};
use super::preview::{self, PreviewLayout, PreviewSpec, preview_plan, stepped_labels};
use super::{PasteArrayField, PasteArrayInput, PasteArrayMsg, PasteArrayState};
use iced::widget::{Column, Row, Space, Stack, Widget, column, container, pick_list, row};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Size, Theme};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::enable_section::EnableSection;
use signex_widgets::property_illustration::{Anchor, Fit, Metrics, property_illustration};
use signex_widgets::{raw, text, text_input};

use crate::choice::{Choice, chosen};
use crate::paste_prefs::PasteDesignators;
use crate::styles::{
    DialogButtonKind, Radii, Spacing, TypeScale, chrome_px, dialog_body, dialog_button,
    dialog_footer_split, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Card width (460 px at the default base).
const DIALOG_W: f32 = chrome_px(460.0 * 11.0 / 12.0);
/// The label column (150 px at the default base).
const LABEL_W: f32 = chrome_px(150.0 * 11.0 / 12.0);
/// A whole-number field: Copies, Text increment step (96 px at the default base).
const SHORT_FIELD_W: f32 = chrome_px(96.0 * 11.0 / 12.0);
/// Copies the worked example names (`D0 → D1, D2, D3`).
const EXAMPLE_COPIES: usize = 3;
/// The text input of a value typed on the preview.
pub(crate) const PREVIEW_EDIT_INPUT_ID: &str = "paste-array-preview-edit";
/// Width of that input, in body text sizes.
const DRAWING_EDIT_EM: f32 = 8.0;
/// Its height, in body text sizes (the text and its padding).
const DRAWING_EDIT_TALL_EM: f32 = 2.0;

/// The primary button: its label, and its message unless the paste is refused.
pub(super) fn paste_button(
    state: &PasteArrayState,
    i18n: &I18n,
) -> (Localized, Option<PasteArrayMsg>) {
    let label = match state.new_copies() {
        Some(n) => t!(i18n, ids::PASTE_ARRAY_PASTE, "count" => n),
        None => t!(i18n, ids::PASTE_ARRAY_PASTE_NONE),
    };
    (
        label,
        state.input().is_ok().then_some(PasteArrayMsg::ApplyPressed),
    )
}

/// Whether "Count up label numbers" is ticked, so the step field under it
/// takes input. Off is step 0 (the step is the only state, #2527).
pub(super) fn step_enabled(state: &PasteArrayState) -> bool {
    state.counts_up()
}

/// The footer's result line: how many items the paste places.
pub(super) fn items_line(state: &PasteArrayState, i18n: &I18n) -> Option<Localized> {
    state
        .placed_items()
        .map(|n| t!(i18n, ids::PASTE_ARRAY_ITEMS, "count" => n))
}

/// The help line: a worked example from the copy's first label, or the
/// warning that uncounted labels join one net.
fn help_line(state: &PasteArrayState, i18n: &I18n) -> Localized {
    if !state.counts_up() {
        return t!(i18n, ids::PASTE_ARRAY_HINT_SAME);
    }
    let step = state.input().map(|input| input.step).ok();
    match (state.sample_label.as_deref(), step) {
        (Some(label), Some(step)) => {
            let n = state
                .new_copies()
                .map_or(1, |n| (n as usize).min(EXAMPLE_COPIES));
            let to = stepped_labels(label, step, n).join(", ");
            t!(i18n, ids::PASTE_ARRAY_HINT_EXAMPLE, "from" => label, "to" => to)
        }
        _ => t!(i18n, ids::PASTE_ARRAY_HINT),
    }
}

/// A text input without its own frame, for a [`field`] that draws one.
fn bare_input(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + 'static {
    let r = tokens::resolve(tokens);
    let (ink, accent) = (ti(tokens.text), ti(tokens.accent));
    move |_: &Theme, _| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: r.ink4,
        value: ink,
        selection: Color { a: 0.4, ..accent },
    }
}

/// A form field with an optional prefix ("X") and unit suffix inside its
/// frame; the frame turns the danger colour while the field is refused.
fn field<'a>(
    prefix: Option<Localized>,
    value: &'a str,
    suffix: Option<String>,
    on_input: fn(String) -> PasteArrayMsg,
    submit: Option<PasteArrayMsg>,
    is_invalid: bool,
    tokens: &ThemeTokens,
) -> Element<'a, PasteArrayMsg> {
    let r = tokens::resolve(tokens);
    let mut line: Row<Element<'a, PasteArrayMsg>> =
        Row::new().spacing(SP.s).align_y(Alignment::Center);
    if let Some(prefix) = prefix {
        line = line.push_boxed(text(prefix).size(TS.body).color(r.ink3));
    }
    line = line.push_boxed(
        text_input(UserText::new(String::new()), value)
            .on_input(on_input)
            .on_submit_maybe(submit)
            .size(TS.body)
            .padding([SP.xs, 0.0])
            .width(Length::Fill)
            .style(bare_input(tokens)),
    );
    if let Some(unit) = suffix {
        line = line.push_boxed(text(UserText::new(unit)).size(TS.body).color(r.ink3));
    }
    field_frame(line, is_invalid, tokens)
}

/// The frame of a [`field`]; the danger colour while it is refused.
fn field_frame<'a>(
    line: Row<Element<'a, PasteArrayMsg>>,
    is_invalid: bool,
    tokens: &ThemeTokens,
) -> Element<'a, PasteArrayMsg> {
    let r = tokens::resolve(tokens);
    let edge = if is_invalid {
        tokens::danger(tokens)
    } else {
        ti(tokens.border)
    };
    container(line)
        .padding([0.0, SP.s])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(r.field)),
            border: Border {
                width: 1.0,
                color: edge,
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A labelled form row.
fn form_row<'a>(
    label: Localized,
    control: Element<'a, PasteArrayMsg>,
    tokens: &ThemeTokens,
) -> Element<'a, PasteArrayMsg> {
    row![
        text(label)
            .size(TS.body)
            .color(ti(tokens.text))
            .width(LABEL_W),
        control,
    ]
    .align_y(Alignment::Center)
    .spacing(SP.m)
    .boxed()
}

fn group_title<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, PasteArrayMsg> {
    text(label)
        .size(TS.caption)
        .font(Font {
            weight: iced::font::Weight::Semibold,
            ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
        })
        .color(tokens::resolve(tokens).ink3)
        .boxed()
}

/// The refusal under the field it is about.
fn refusal<'a>(
    state: &PasteArrayState,
    fields: &[PasteArrayField],
    tokens: &ThemeTokens,
) -> Option<Element<'a, PasteArrayMsg>> {
    let (field, reason) = state.check().err()?;
    fields.contains(&field).then(|| {
        row![
            Space::new().width(LABEL_W),
            // raw: the input check words its own refusal, in English (#2061).
            text(raw(reason))
                .size(TS.caption)
                .color(tokens::danger(tokens)),
        ]
        .spacing(SP.m)
        .boxed()
    })
}

/// What the preview draws for a dialog whose fields parse.
fn preview_spec(state: &PasteArrayState, input: &PasteArrayInput, i18n: &I18n) -> PreviewSpec {
    let length = |mm: f64| {
        format!(
            "{} {}",
            signex_types::measure::length_field_text(mm, state.unit),
            state.unit
        )
    };
    let plan = preview_plan(input.positions());
    let step = if input.step > 0 {
        format!("+{}", input.step)
    } else {
        input.step.to_string()
    };
    PreviewSpec {
        positions: input.positions(),
        dx_mm: input.dx_mm,
        dy_mm: input.dy_mm,
        step: input.step,
        copy_size_mm: state.copy_size_mm,
        grid_mm: state.grid_mm,
        sample_label: state.sample_label.clone(),
        x_label: t!(i18n, ids::PASTE_ARRAY_PREVIEW_X, "value" => length(input.dx_mm)).into(),
        y_label: t!(i18n, ids::PASTE_ARRAY_PREVIEW_Y, "value" => length(input.dy_mm)).into(),
        more_label: t!(i18n, ids::PASTE_ARRAY_PREVIEW_MORE, "count" => plan.more).into(),
        copies_label: t!(i18n, ids::PASTE_ARRAY_PREVIEW_COPIES, "count" => input.positions())
            .into(),
        step_label: state
            .counts_up()
            .then(|| t!(i18n, ids::PASTE_ARRAY_PREVIEW_STEP, "step" => step).into()),
    }
}

fn preview_block<'a>(
    state: &'a PasteArrayState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PasteArrayMsg> {
    let caption = t!(i18n, ids::PASTE_ARRAY_PREVIEW_CAPTION);
    let Ok(input) = state.input() else {
        let note = text(caption)
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink3)
            .boxed();
        // A value typed on the drawing that does not parse yet keeps its
        // input, under the note, until it does.
        return match state.preview_edit {
            Some((field, _)) => column![note, drawing_edit_input(state, field, tokens)]
                .spacing(SP.xs)
                .boxed(),
            None => note,
        };
    };
    let spec = preview_spec(state, &input, i18n);
    let layout = preview::layout(&spec);
    let drawing = preview::illustration(spec).height_em(preview::height_em(state.preview_expanded));
    let height = Metrics::new(TS.caption).height(drawing.height_in_em());
    let pointer = PreviewPointer {
        layout: layout.clone(),
        is_dragging: state.preview_drag.is_some(),
        text_size: TS.caption,
    };
    let mut layers = Stack::new()
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .push(
            property_illustration(drawing, &state.hover, tokens, TS.caption)
                .map(PasteArrayMsg::PreviewHovered)
                .boxed(),
        )
        .push(preview_pointer(pointer, height));
    if let Some((field, _)) = state.preview_edit {
        layers = layers.push(drawing_editor(state, field, layout, *tokens));
    }
    crate::app::view::dialogs::preview_frame(
        caption,
        layers.boxed(),
        Some(expand_button(state.preview_expanded, tokens, i18n)),
        tokens,
    )
}

/// The input of a value typed on the drawing: the field's own text, so the
/// field follows each keystroke. Enter ends the edit.
fn drawing_edit_input<'a>(
    state: &'a PasteArrayState,
    field: PasteArrayField,
    tokens: &ThemeTokens,
) -> Element<'a, PasteArrayMsg> {
    let (value, on_input, unit): (&str, fn(String) -> PasteArrayMsg, bool) = match field {
        PasteArrayField::Copies => (&state.count, PasteArrayMsg::CountChanged, false),
        PasteArrayField::XSpacing => (&state.x_spacing, PasteArrayMsg::XSpacingChanged, true),
        PasteArrayField::YSpacing => (&state.y_spacing, PasteArrayMsg::YSpacingChanged, true),
        PasteArrayField::Step => (&state.step, PasteArrayMsg::StepChanged, false),
    };
    let mut line: Row<Element<'a, PasteArrayMsg>> =
        Row::new().spacing(SP.s).align_y(Alignment::Center);
    line = line.push_boxed(
        text_input(UserText::new(String::new()), value)
            .id(iced::widget::Id::new(PREVIEW_EDIT_INPUT_ID))
            .on_input(on_input)
            .on_submit(PasteArrayMsg::PreviewEditSubmitted)
            .size(TS.body)
            .padding([SP.xs, 0.0])
            .width(Length::Fill)
            .style(bare_input(tokens)),
    );
    if unit {
        let ink = tokens::resolve(tokens).ink3;
        line = line.push_boxed(
            text(UserText::new(state.unit.to_string()))
                .size(TS.body)
                .color(ink),
        );
    }
    container(field_frame(
        line,
        state.invalid_field() == Some(field),
        tokens,
    ))
    .width(TS.body * DRAWING_EDIT_EM)
    .boxed()
}

/// Where the input of a value typed on the drawing goes, in a drawing of
/// `size`: over the value's label, kept inside the drawing.
fn drawing_edit_origin(layout: &PreviewLayout, field: PasteArrayField, size: Size) -> (f32, f32) {
    let fit = Fit::new(layout.view_size(), size);
    let (wide, tall) = (TS.body * DRAWING_EDIT_EM, TS.body * DRAWING_EDIT_TALL_EM);
    let (at, anchor) = layout.value_point(&fit, field).unwrap_or((
        iced::Point::new(size.width / 2.0, size.height / 2.0),
        Anchor::Middle,
    ));
    let left = match anchor {
        Anchor::Start => at.x,
        Anchor::Middle => at.x - wide / 2.0,
        Anchor::End => at.x - wide,
    };
    (
        left.min(size.width - wide).max(0.0),
        (at.y - tall * 0.75).min(size.height - tall).max(0.0),
    )
}

/// The layer that puts the input over the value being typed.
fn drawing_editor<'a>(
    state: &'a PasteArrayState,
    field: PasteArrayField,
    layout: PreviewLayout,
    tokens: ThemeTokens,
) -> Element<'a, PasteArrayMsg> {
    iced::widget::responsive(move |size: Size| -> Element<'a, PasteArrayMsg> {
        let (left, top) = drawing_edit_origin(&layout, field, size);
        column![
            Space::new().height(top),
            row![
                Space::new().width(left),
                drawing_edit_input(state, field, &tokens)
            ],
        ]
        .boxed()
    })
    .boxed()
}

/// The tooltip of the preview's expand button.
fn expand_tip(expanded: bool) -> signex_i18n::MessageId {
    if expanded {
        ids::PASTE_ARRAY_PREVIEW_COLLAPSE
    } else {
        ids::PASTE_ARRAY_PREVIEW_EXPAND
    }
}

/// The button that draws the preview taller, or back.
fn expand_button<'a>(
    expanded: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PasteArrayMsg> {
    let ink = tokens::resolve(tokens).ink3;
    signex_widgets::tooltip(
        signex_widgets::button(signex_widgets::chevron::chevron(expanded, TS.body, ink))
            .padding(SP.xs)
            .on_press(PasteArrayMsg::PreviewExpandToggled)
            .style(crate::styles::ghost_button(tokens)),
        text(t!(i18n, expand_tip(expanded))).size(TS.caption),
        iced::widget::tooltip::Position::Left,
    )
    .boxed()
}

fn layout_group<'a>(
    state: &'a PasteArrayState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Column<Element<'a, PasteArrayMsg>> {
    let submit = state.input().is_ok().then_some(PasteArrayMsg::ApplyPressed);
    let bad = state.invalid_field();
    let unit = || Some(state.unit.to_string());
    let copies = container(field(
        None,
        &state.count,
        None,
        PasteArrayMsg::CountChanged,
        submit.clone(),
        bad == Some(PasteArrayField::Copies),
        tokens,
    ))
    .width(SHORT_FIELD_W)
    .boxed();
    let spacing = row![
        field(
            Some(t!(i18n, ids::PASTE_ARRAY_AXIS_X)),
            &state.x_spacing,
            unit(),
            PasteArrayMsg::XSpacingChanged,
            submit.clone(),
            bad == Some(PasteArrayField::XSpacing),
            tokens,
        ),
        field(
            Some(t!(i18n, ids::PASTE_ARRAY_AXIS_Y)),
            &state.y_spacing,
            unit(),
            PasteArrayMsg::YSpacingChanged,
            submit,
            bad == Some(PasteArrayField::YSpacing),
            tokens,
        ),
    ]
    .spacing(SP.m)
    .boxed();
    let mut group = column![
        group_title(t!(i18n, ids::PASTE_ARRAY_GROUP_LAYOUT), tokens),
        form_row(t!(i18n, ids::PASTE_ARRAY_COPIES), copies, tokens),
    ]
    .spacing(SP.m);
    if let Some(why) = refusal(state, &[PasteArrayField::Copies], tokens) {
        group = group.push(why);
    }
    group = group.push(form_row(
        t!(i18n, ids::PASTE_ARRAY_SPACING),
        spacing,
        tokens,
    ));
    if let Some(why) = refusal(
        state,
        &[PasteArrayField::XSpacing, PasteArrayField::YSpacing],
        tokens,
    ) {
        group = group.push(why);
    }
    group.push(preview_block(state, tokens, i18n))
}

fn numbering_group<'a>(
    state: &'a PasteArrayState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Column<Element<'a, PasteArrayMsg>> {
    let submit = state.input().is_ok().then_some(PasteArrayMsg::ApplyPressed);
    let step = container(field(
        None,
        &state.step,
        None,
        PasteArrayMsg::StepChanged,
        submit,
        state.invalid_field() == Some(PasteArrayField::Step),
        tokens,
    ))
    .width(SHORT_FIELD_W)
    .boxed();
    let mut step_rows =
        column![form_row(t!(i18n, ids::PASTE_ARRAY_STEP), step, tokens)].spacing(SP.s);
    if let Some(why) = refusal(state, &[PasteArrayField::Step], tokens) {
        step_rows = step_rows.push(why);
    }
    let count_up = EnableSection::new(t!(i18n, ids::PASTE_ARRAY_COUNT_UP), step_enabled(state))
        .on_toggle(PasteArrayMsg::CountUpToggled)
        .view(
            crate::panels::enable_section::enable_section_style(tokens),
            step_rows.boxed(),
        );

    let modes = Choice::named(PasteDesignators::ALL, PasteDesignators::label, i18n);
    let designators = pick_list(
        chosen(&modes, &state.designators),
        modes,
        |mode: &Choice<PasteDesignators>| mode.label.clone(),
    )
    .graphite()
    .on_select(|mode: Choice<PasteDesignators>| PasteArrayMsg::DesignatorsChosen(mode.value))
    .width(Length::Fill);

    column![
        group_title(t!(i18n, ids::PASTE_ARRAY_GROUP_NUMBERING), tokens),
        count_up,
        form_row(
            t!(i18n, ids::PASTE_ARRAY_DESIGNATORS),
            designators.boxed(),
            tokens
        ),
        text(help_line(state, i18n))
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink3),
    ]
    .spacing(SP.m)
}

/// The dialog as a centred card on a scrim in the main window.
pub fn view<'a>(
    state: &'a PasteArrayState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PasteArrayMsg> {
    let header = crate::app::view::dialogs::graphite_header(
        t!(i18n, ids::PASTE_ARRAY_TITLE).into_string(),
        Some(t!(i18n, ids::PASTE_ARRAY_SUBTITLE, "count" => state.items_per_copy).into_string()),
        None,
        PasteArrayMsg::Closed,
        crate::app::view::dialogs::DIALOG_HEADER_H,
        tokens,
    );
    let body = column![
        layout_group(state, tokens, i18n),
        numbering_group(state, tokens, i18n),
    ]
    .spacing(SP.l);

    let (paste_label, paste) = paste_button(state, i18n);
    let items: Vec<Element<'a, PasteArrayMsg>> = items_line(state, i18n)
        .map(|line| {
            text(line)
                .size(TS.caption)
                .color(tokens::resolve(tokens).ink3)
                .boxed()
        })
        .into_iter()
        .collect();
    let footer = dialog_footer_split(
        items,
        vec![
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(PasteArrayMsg::Closed),
                tokens,
            ),
            dialog_button(paste_label, DialogButtonKind::Primary, paste, tokens),
        ],
    );

    let dialog =
        container(column![header, dialog_body(body), footer].width(Length::Fixed(DIALOG_W)))
            .style(crate::styles::modal_card(tokens))
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
    use signex_types::coord::Unit;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn state() -> PasteArrayState {
        let mut s = PasteArrayState::new(Unit::Mm, 10.16, 0.0, 1, PasteDesignators::Keep, 3, None);
        s.sample_label = Some("D0".into());
        s.copy_size_mm = Some((8.0, 4.0));
        s.grid_mm = 1.27;
        s
    }

    fn plain(text: Localized) -> String {
        text.into_string().replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn the_footer_counts_new_copies_times_items_per_copy() {
        let en = I18n::en_us();
        let mut s = state();
        s.count = "4".into();
        assert_eq!(
            plain(items_line(&s, &en).unwrap()),
            "9 items will be placed"
        );
        let (label, msg) = paste_button(&s, &en);
        assert_eq!(plain(label), "Paste 3 copies");
        assert!(matches!(msg, Some(PasteArrayMsg::ApplyPressed)));
    }

    /// #2847: the default places one new copy; Copies = 1 places nothing, so
    /// the button has no action and the field says why.
    #[test]
    fn the_default_is_one_new_copy_and_copies_1_is_refused() {
        let en = I18n::en_us();
        let mut s = state();
        assert_eq!(s.count, "2");
        assert_eq!(plain(paste_button(&s, &en).0), "Paste 1 copy");
        s.count = "1".into();
        let (label, msg) = paste_button(&s, &en);
        assert_eq!(plain(label), "Paste");
        assert!(msg.is_none());
        assert!(items_line(&s, &en).is_none());
        assert_eq!(s.invalid_field(), Some(PasteArrayField::Copies));
    }

    #[test]
    fn above_the_limit_the_paste_button_has_no_action() {
        let en = I18n::en_us();
        let mut s = state();
        s.count = "1001".into();
        let (label, msg) = paste_button(&s, &en);
        assert_eq!(plain(label), "Paste");
        assert!(msg.is_none(), "over MAX_COPIES");
        assert_eq!(s.invalid_field(), Some(PasteArrayField::Copies));

        s.items_per_copy = 101;
        s.count = "1000".into();
        let (_, msg) = paste_button(&s, &en);
        assert!(msg.is_none(), "over MAX_PLACED_ITEMS");
        assert_eq!(s.invalid_field(), Some(PasteArrayField::Copies));

        // Laid out, a click on the disabled button sends nothing.
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let window = iced::Size::new(1200.0, 900.0);
        let texts = crate::layout_probe::lay_out_texts(view(&s, &tokens, &en), window);
        let button = texts
            .iter()
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == "Paste 999 copies")
            .expect("the paste button is drawn");
        let sent = crate::layout_probe::messages_on_click_at(
            view(&s, &tokens, &en),
            window,
            None,
            button.bounds.center(),
        );
        assert!(
            !sent
                .iter()
                .any(|m| matches!(m, PasteArrayMsg::ApplyPressed)),
            "{sent:?}"
        );

        // Under the cap the same click pastes, so the probe hits the button.
        s.items_per_copy = 3;
        let texts = crate::layout_probe::lay_out_texts(view(&s, &tokens, &en), window);
        let button = texts
            .iter()
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == "Paste 999 copies")
            .expect("the paste button is drawn");
        let sent = crate::layout_probe::messages_on_click_at(
            view(&s, &tokens, &en),
            window,
            None,
            button.bounds.center(),
        );
        assert!(
            sent.iter()
                .any(|m| matches!(m, PasteArrayMsg::ApplyPressed)),
            "{sent:?}"
        );
    }

    #[test]
    fn count_up_off_shows_step_zero_greyed_and_warns_about_one_net() {
        let en = I18n::en_us();
        let mut s = state();
        s.set_count_up(false, 1);
        assert_eq!(s.step, "0");
        assert!(
            !step_enabled(&s),
            "the section is drawn off, its step field inert"
        );
        assert_eq!(s.input().unwrap().step, 0);
        assert_eq!(
            plain(help_line(&s, &en)),
            en.message_plain(ids::PASTE_ARRAY_HINT_SAME, &[])
        );
        s.set_count_up(true, 1);
        assert!(step_enabled(&s));
    }

    #[test]
    fn the_help_line_works_an_example_from_the_copy() {
        let en = I18n::en_us();
        let mut s = state();
        s.count = "4".into();
        assert!(plain(help_line(&s, &en)).ends_with("(D0 → D1, D2, D3)."));
        s.count = "2".into();
        assert!(plain(help_line(&s, &en)).ends_with("(D0 → D1)."));
        s.sample_label = None;
        assert_eq!(
            plain(help_line(&s, &en)),
            en.message_plain(ids::PASTE_ARRAY_HINT, &[])
        );
    }

    /// Mockup row 69: the expand button's tooltip says what a press does,
    /// and the card builds either way in both themes.
    #[test]
    fn the_expand_button_names_its_action() {
        let en = I18n::en_us();
        assert_eq!(en.message_plain(expand_tip(false), &[]), "Expand preview");
        assert_eq!(en.message_plain(expand_tip(true), &[]), "Collapse preview");
        let mut s = state();
        s.preview_expanded = true;
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let _ = view(&s, &theme_tokens(id), &en);
        }
    }

    const WINDOW: iced::Size = iced::Size::new(1200.0, 900.0);

    fn inputs_showing(s: &PasteArrayState, value: &str) -> Vec<iced::Rectangle> {
        let en = I18n::en_us();
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        crate::layout_probe::lay_out_inputs(view(s, &tokens, &en), WINDOW)
            .into_iter()
            .filter(|input| input.text == value)
            .map(|input| input.bounds)
            .collect()
    }

    /// Mockup row 69: the caption says the drawing takes a drag of copy 2
    /// and a click on a value.
    #[test]
    fn the_preview_caption_names_the_drag_and_the_click() {
        let en = I18n::en_us();
        let texts = crate::layout_probe::lay_out_texts(
            view(&state(), &theme_tokens(ThemeId::GraphiteLight), &en),
            WINDOW,
        );
        crate::layout_probe::find(
            &texts,
            "Preview · copy 1 is the original · drag copy 2 or click a value",
        );
    }

    /// Mockup row 69: a clicked value is typed over the drawing, in an input
    /// that shows the field's own text, below the caption and inside the card.
    #[test]
    fn a_clicked_value_gets_an_input_on_the_drawing() {
        let mut s = state();
        let x = s.x_spacing.clone();
        assert_eq!(inputs_showing(&s, &x).len(), 1, "the X field only");
        s.start_preview_edit(PasteArrayField::XSpacing);
        let shown = inputs_showing(&s, &x);
        assert_eq!(shown.len(), 2, "the field and the input on the drawing");

        let en = I18n::en_us();
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let texts = crate::layout_probe::lay_out_texts(view(&s, &tokens, &en), WINDOW);
        let caption = crate::layout_probe::find(
            &texts,
            "Preview · copy 1 is the original · drag copy 2 or click a value",
        )
        .bounds;
        let on_drawing = shown[1];
        assert!(on_drawing.y > caption.y + caption.height, "{on_drawing:?}");
        assert!(
            on_drawing.x >= caption.x - SP.s
                && on_drawing.x + on_drawing.width <= caption.x + DIALOG_W,
            "{on_drawing:?} {caption:?}"
        );

        // Typed text the dialog refuses keeps the input open.
        s.x_spacing = "x".into();
        assert_eq!(inputs_showing(&s, "x").len(), 2);
    }

    /// Mockup row 69: copies and the step are drawn on the drawing, the step
    /// only while label numbers count up.
    #[test]
    fn the_drawing_carries_the_copies_and_the_step() {
        let en = I18n::en_us();
        let mut s = state();
        s.count = "4".into();
        let bare = |t: &signex_widgets::property_illustration::Text| {
            t.as_str().replace(['\u{2068}', '\u{2069}'], "")
        };
        let spec = preview_spec(&s, &s.input().unwrap(), &en);
        assert_eq!(bare(&spec.copies_label), "4 copies");
        assert_eq!(
            spec.step_label.as_ref().map(bare).as_deref(),
            Some("+1 per copy")
        );
        s.set_count_up(false, 1);
        assert!(
            preview_spec(&s, &s.input().unwrap(), &en)
                .step_label
                .is_none()
        );
    }

    #[test]
    fn the_card_builds_in_both_graphite_themes() {
        let en = I18n::en_us();
        let mut s = state();
        for count in ["2", "40", "0"] {
            s.count = count.into();
            for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
                let _ = view(&s, &theme_tokens(id), &en);
            }
        }
    }
}
