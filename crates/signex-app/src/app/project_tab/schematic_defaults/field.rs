//! One default field of the section (mockup row 36): a fixed box holding the
//! text input and its unit suffix, drawn in its [`FieldState`], the reset
//! button beside an edited box, and the clamp note under it. Pure.

use iced::widget::{Row, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::project::{SchematicDefaultField, SchematicDefaultOverrides};
use signex_types::schematic::SCHEMATIC_DEFAULT_FONT;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

use super::super::look::{Glyph, glyph, icon_button, mono_font};
use super::super::table::{SP, TS};
use super::{
    FieldState, built_in_value, clamp_note, field_state, refused_range, shown_number, shown_text,
};
use crate::app::Message;
use crate::app::project_tab::ProjectTabMsg;
use crate::app::project_tab::general::tab_message;
use crate::app::state::ProjectId;
use crate::styles::{HAIRLINE, Radii, ti, tokens as ink};

/// Width of a numeric box (120 px at the default size), in `body` units.
const NUMBER_BOX_EMS: f32 = 10.0;
/// Width of the font box and the two pickers (about 170 px), in `body` units.
pub(super) const WIDE_BOX_EMS: f32 = 14.0;

/// The label of `field`, without its unit.
pub(super) fn label_words(field: SchematicDefaultField) -> MessageId {
    match field {
        SchematicDefaultField::WireWidth => ids::PROJECT_DEFAULTS_WIRE_WIDTH,
        SchematicDefaultField::BusWidth => ids::PROJECT_DEFAULTS_BUS_WIDTH,
        SchematicDefaultField::SymbolLineWidth => ids::PROJECT_DEFAULTS_SYMBOL_LINE_WIDTH,
        SchematicDefaultField::ShapeStroke => ids::PROJECT_DEFAULTS_SHAPE_STROKE,
        SchematicDefaultField::JunctionDiameter => ids::PROJECT_DEFAULTS_JUNCTION_DIAMETER,
        SchematicDefaultField::TextSize => ids::PROJECT_DEFAULTS_TEXT_SIZE,
    }
}

/// The unit suffix of `field`: pt for the text size, mm for the widths.
fn unit_words(field: SchematicDefaultField) -> MessageId {
    match field {
        SchematicDefaultField::TextSize => ids::PROJECT_DEFAULTS_UNIT_PT,
        _ => ids::PROJECT_DEFAULTS_UNIT_MM,
    }
}

/// A numeric field: the box and the reset button when it is not built-in,
/// then the clamp note, in warning ink, when the typed number was clamped.
/// The view puts the note under the whole form row.
pub(super) fn numeric_field<'a>(
    project_id: ProjectId,
    field: SchematicDefaultField,
    overrides: &SchematicDefaultOverrides,
    draft: Option<&String>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> (Element<'a, Message>, Option<Element<'a, Message>>) {
    let stored = overrides.get(field);
    let state = field_state(field, draft, stored);
    let built_in = shown_number(built_in_value(field, overrides));
    let r = ink::resolve(tokens);
    let edited = move |text: String| {
        tab_message(ProjectTabMsg::SchematicDefaultEdited {
            project_id,
            field,
            text,
        })
    };
    // Reset is the empty-field path: it clears the override and the draft.
    let reset = edited(String::new());
    // The placeholder is the built-in value, a number Signex formats.
    let input = text_input(UserText::new(built_in.clone()), shown_text(draft, stored))
        .on_input(edited)
        .size(TS.label)
        .padding(Padding::ZERO)
        .width(Length::Fill)
        .style(bare_input(tokens));
    let mut inside: Row<Element<'a, Message>> =
        row![input].spacing(SP.s).align_y(Alignment::Center);
    if state == FieldState::Invalid {
        inside = inside.push(glyph(Glyph::Warn, TS.body, ti(tokens.error)));
    }
    inside = inside.push(
        text(t!(i18n, unit_words(field)))
            .size(TS.label)
            .font(mono_font())
            .color(r.ink4)
            .boxed(),
    );
    let field_box = container(inside)
        .padding([SP.s, SP.m])
        .width(Length::Fixed(TS.body * NUMBER_BOX_EMS))
        .style(box_style(tokens, state));
    let mut line: Row<Element<'a, Message>> =
        row![field_box].spacing(SP.s).align_y(Alignment::Center);
    if state != FieldState::BuiltIn {
        let tip = t!(i18n, ids::PROJECT_DEFAULTS_RESET, "value" => built_in);
        line = line.push(icon_button(Glyph::Restore, tip, Some(reset), tokens));
    }
    let note = clamp_note(field, draft).map(|note| {
        let words = t!(
            i18n,
            ids::PROJECT_DEFAULTS_CLAMPED,
            "min" => shown_number(note.min),
            "max" => shown_number(note.max),
            "used" => shown_number(note.used),
        );
        text(words)
            .size(TS.caption)
            .color(ti(tokens.warning))
            .boxed()
    });
    let note = note.or_else(|| {
        refused_range(field, draft).map(|(min, max)| {
            let words = t!(
                i18n,
                ids::PROJECT_DEFAULTS_REFUSED,
                "min" => shown_number(min),
                "max" => shown_number(max),
            );
            text(words).size(TS.caption).color(ti(tokens.error)).boxed()
        })
    });
    (line.boxed(), note)
}

/// The font field: a wide box with the font in effect as its placeholder
/// and a reset button while the project names a font.
pub(super) fn font_field<'a>(
    project_id: ProjectId,
    overrides: &'a SchematicDefaultOverrides,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let edited = move |text: String| {
        tab_message(ProjectTabMsg::SchematicDefaultFontEdited { project_id, text })
    };
    // Reset is the empty-field path: an empty font clears the override.
    let reset = edited(String::new());
    let is_set = overrides.placed_font_family().is_some();
    let state = if is_set {
        FieldState::Edited
    } else {
        FieldState::BuiltIn
    };
    // The placeholder is the font in effect, a font name.
    let input = text_input(
        UserText::new(overrides.font_family()),
        overrides.font_family.as_deref().unwrap_or_default(),
    )
    .on_input(edited)
    .size(TS.label)
    .padding(Padding::ZERO)
    .width(Length::Fill)
    .style(bare_input(tokens));
    let field_box = container(input)
        .padding([SP.s, SP.m])
        .width(Length::Fixed(TS.body * WIDE_BOX_EMS))
        .style(box_style(tokens, state));
    let mut line: Row<Element<'a, Message>> =
        row![field_box].spacing(SP.s).align_y(Alignment::Center);
    if is_set {
        let tip = t!(i18n, ids::PROJECT_DEFAULTS_RESET, "value" => SCHEMATIC_DEFAULT_FONT);
        line = line.push(icon_button(Glyph::Restore, tip, Some(reset), tokens));
    }
    line.boxed()
}

/// A borderless, transparent input: the box around it carries the look.
fn bare_input(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + 'static {
    let (placeholder, value) = (ink::resolve(tokens).ink4, ti(tokens.text));
    move |_, _| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder,
        value,
        selection: Color { a: 0.4, ..value },
    }
}

/// The box of a field in `state`: hairline when built-in, the accent when
/// edited, the err border on an err-soft fill when invalid.
fn box_style(
    tokens: &ThemeTokens,
    state: FieldState,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let r = ink::resolve(tokens);
    let (fill, line) = match state {
        FieldState::BuiltIn => (r.field, ti(tokens.border)),
        FieldState::Edited => (r.field, ti(tokens.accent)),
        FieldState::Invalid => (r.err_soft, ti(tokens.error)),
    };
    let words = ti(tokens.text);
    move |_| container::Style {
        background: Some(Background::Color(fill)),
        text_color: Some(words),
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}
