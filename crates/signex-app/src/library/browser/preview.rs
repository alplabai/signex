//! Library Browser — the Graphite right-hand preview pane for the selected row.
//!
//! Real row data only: identity, lifecycle, binding names, update date and
//! the parameters. The symbol and footprint drawings need the off-thread
//! detail loader of the Components panel, so they are not drawn here.

use super::columns::ColumnKind;
use super::names::PrimitiveNames;
use super::*;
use iced::widget::column;

/// Default type / spacing scales.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the pane.
pub(super) const PREVIEW_W: f32 = 300.0;

/// An upper-case caption above its value; an empty value reads as a dash.
fn field<'a>(
    label: String,
    value: String,
    mono: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let shown = if value.is_empty() {
        "—".to_string()
    } else {
        value
    };
    let mut body = text(raw(shown))
        .size(TS.body)
        .color(theme_ext::text_primary(tokens));
    if mono {
        body = body.font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ));
    }
    column![
        text(raw(label.to_uppercase()))
            .size(TS.section)
            .color(r.ink3)
            .boxed(),
        body.boxed(),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// The preview pane for `row`.
pub(super) fn view_preview<'a>(
    row: &'a ComponentRow,
    names: &PrimitiveNames<'_>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let title = text(raw(row.internal_pn.as_str().to_string()))
        .size(TS.heading)
        .font(mono)
        .color(theme_ext::text_primary(tokens));
    let pill: Element<'a, LibraryMessage> = crate::library::editor::kit::toned_pill(
        look::lifecycle_label(row.state).to_string(),
        look::lifecycle_tone(row.state),
        tokens,
    );
    let head: iced::widget::Row<Element<'a, LibraryMessage>> = iced::widget::row![
        title.boxed(),
        Space::new().width(Length::Fill).boxed(),
        pill
    ]
    .align_y(iced::Alignment::Center);

    let rev = if row.released {
        format!("{} · released", row.version)
    } else {
        row.version.clone()
    };
    let mut body: Column<Element<'a, LibraryMessage>> = column![head.boxed()].spacing(SP.m);
    if !row.description.is_empty() {
        body = body.push(
            text(raw(row.description.clone()))
                .size(TS.body)
                .color(theme_ext::text_secondary(tokens))
                .boxed(),
        );
    }
    let cell = |kind: ColumnKind| kind.cell_value_named(row, names);
    let fields = [
        ("Class", row.class.as_str().to_string(), false),
        ("Manufacturer", cell(ColumnKind::Manufacturer), false),
        ("MPN", cell(ColumnKind::Mpn), true),
        ("Revision", rev, false),
        ("Symbol", cell(ColumnKind::Symbol), false),
        ("Footprint", cell(ColumnKind::Footprint), false),
        ("Updated", cell(ColumnKind::Updated), false),
    ];
    for (label, value, is_mono) in fields {
        body = body.push(field(label.to_string(), value, is_mono, tokens));
    }
    for (key, value) in &row.parameters {
        body = body.push(field(key.clone(), value.display(), true, tokens));
    }

    container(scrollable(container(body).padding(SP.l)).height(Length::Fill))
        .width(Length::Fixed(PREVIEW_W))
        .height(Length::Fill)
        .style(crate::styles::card(tokens))
        .boxed()
}
