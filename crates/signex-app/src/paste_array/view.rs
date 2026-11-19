//! The Paste Array card (#2527): Layout (copies, spacing), a read-only
//! preview, Numbering (count up, step, designators), a worked example, and a
//! footer that says what the paste places.

use super::preview::{self, PreviewSpec, preview_plan, stepped_labels};
use super::{PasteArrayField, PasteArrayMsg, PasteArrayState};
use iced::widget::{Column, Row, Space, Widget, column, container, pick_list, row};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Theme};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::enable_section::EnableSection;
use signex_widgets::property_illustration::property_illustration;
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

/// The primary button: its label, and its message unless the paste is refused.
pub(super) fn paste_button(
    state: &PasteArrayState,
    i18n: &I18n,
) -> (Localized, Option<PasteArrayMsg>) {
    let label = match state.copies() {
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
                .copies()
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

fn preview_block<'a>(
    state: &'a PasteArrayState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PasteArrayMsg> {
    let caption = text(t!(i18n, ids::PASTE_ARRAY_PREVIEW_CAPTION))
        .size(TS.caption)
        .color(tokens::resolve(tokens).ink3);
    let Ok(input) = state.input() else {
        return caption.boxed();
    };
    let length = |mm: f64| {
        format!(
            "{} {}",
            signex_types::measure::length_field_text(mm, state.unit),
            state.unit
        )
    };
    let plan = preview_plan(input.count);
    let drawing = preview::illustration(PreviewSpec {
        copies: input.count,
        dx_mm: input.dx_mm,
        dy_mm: input.dy_mm,
        step: input.step,
        copy_size_mm: state.copy_size_mm,
        grid_mm: state.grid_mm,
        sample_label: state.sample_label.clone(),
        x_label: t!(i18n, ids::PASTE_ARRAY_PREVIEW_X, "value" => length(input.dx_mm)).into(),
        y_label: t!(i18n, ids::PASTE_ARRAY_PREVIEW_Y, "value" => length(input.dy_mm)).into(),
        more_label: t!(i18n, ids::PASTE_ARRAY_PREVIEW_MORE, "count" => plan.more).into(),
    });
    column![
        caption,
        property_illustration(drawing, &state.hover, tokens, TS.caption)
            .map(PasteArrayMsg::PreviewHovered),
    ]
    .spacing(SP.xs)
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
    fn the_footer_counts_copies_times_items_per_copy() {
        let en = I18n::en_us();
        let mut s = state();
        s.count = "4".into();
        assert_eq!(
            plain(items_line(&s, &en).unwrap()),
            "12 items will be placed"
        );
        let (label, msg) = paste_button(&s, &en);
        assert_eq!(plain(label), "Paste 4 copies");
        assert!(matches!(msg, Some(PasteArrayMsg::ApplyPressed)));
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
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == "Paste 1000 copies")
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
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == "Paste 1000 copies")
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
        s.count = "1".into();
        assert!(plain(help_line(&s, &en)).ends_with("(D0 → D1)."));
        s.sample_label = None;
        assert_eq!(
            plain(help_line(&s, &en)),
            en.message_plain(ids::PASTE_ARRAY_HINT, &[])
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
