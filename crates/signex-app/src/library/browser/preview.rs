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

/// A caption, already in display case, above its value; an empty value
/// reads as a dash.
fn field<'a>(
    caption: signex_widgets::Label<'a>,
    value: String,
    mono: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let mut body = if value.is_empty() {
        text(t!(i18n, ids::LIBRARY_BROWSER_NO_VALUE))
    } else {
        text(UserText::new(value))
    }
    .size(TS.body)
    .color(theme_ext::text_primary(tokens));
    if mono {
        body = body.font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ));
    }
    column![
        text(caption).size(TS.section).color(r.ink3).boxed(),
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
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let title = text(signex_i18n::UserText::new(
        row.internal_pn.as_str().to_string(),
    ))
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
        t!(i18n, ids::LIBRARY_BROWSER_REVISION_RELEASED, "version" => row.version.as_str())
            .into_string()
    } else {
        row.version.clone()
    };
    let mut body: Column<Element<'a, LibraryMessage>> = column![head.boxed()].spacing(SP.m);
    if !row.description.is_empty() {
        body = body.push(
            text(signex_i18n::UserText::new(row.description.clone()))
                .size(TS.body)
                .color(theme_ext::text_secondary(tokens))
                .boxed(),
        );
    }
    let cell = |kind: ColumnKind| kind.cell_value_named(row, names);
    let fields = [
        (
            ids::LIBRARY_PREVIEW_CLASS,
            row.class.as_str().to_string(),
            false,
        ),
        (
            ids::LIBRARY_PREVIEW_MANUFACTURER,
            cell(ColumnKind::Manufacturer),
            false,
        ),
        (ids::LIBRARY_PREVIEW_MPN, cell(ColumnKind::Mpn), true),
        (ids::LIBRARY_PREVIEW_REVISION, rev, false),
        (ids::LIBRARY_PREVIEW_SYMBOL, cell(ColumnKind::Symbol), false),
        (
            ids::LIBRARY_PREVIEW_FOOTPRINT,
            cell(ColumnKind::Footprint),
            false,
        ),
        (
            ids::LIBRARY_PREVIEW_UPDATED,
            cell(ColumnKind::Updated),
            false,
        ),
    ];
    for (caption, value, is_mono) in fields {
        body = body.push(field(
            t!(i18n, caption).into(),
            value,
            is_mono,
            tokens,
            i18n,
        ));
    }
    // A parameter key is user content: shown in capitals like the fixed
    // captions, through the casemap of the UI language.
    for (key, value) in &row.parameters {
        let caption = UserText::new(i18n.upper_for_display(key));
        body = body.push(field(caption.into(), value.display(), true, tokens, i18n));
    }

    container(scrollable(container(body).padding(SP.l)).height(Length::Fill))
        .width(Length::Fixed(PREVIEW_W))
        .height(Length::Fill)
        .style(crate::styles::card(tokens))
        .boxed()
}
