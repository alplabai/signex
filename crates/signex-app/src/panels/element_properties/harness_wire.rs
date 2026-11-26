//! Properties of a selected harness wire (#1280): its type, read from the
//! harness it belongs to, and its two ends. Also the Harness type row of a
//! selected sheet entry, which makes it a harness-typed entry.

use super::super::*;
use super::harness_connector::HarnessPanelMsg;
use super::rows::PanelStyle;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{UserText, ids, t};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub(super) fn push_harness_wire_sections<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let facts = &ctx.harness;
    let muted = st.muted;
    let line = |label: signex_i18n::Localized, value: String| {
        row![
            property_label(label, muted),
            text(UserText::new(value)).size(TS.body),
        ]
        .padding([4.0, SP.m])
        .spacing(SP.s)
        .boxed()
    };
    let mut rows = Column::new().spacing(0).width(Length::Fill).push(line(
        t!(i18n, ids::PANELS_HARNESS_TYPE),
        facts.wire_type.clone().unwrap_or_default(),
    ));
    if let Some((start, end)) = facts.wire_ends {
        // Y counts from the sheet's page origin, as the connector's Y box does.
        let at = |p: signex_types::schematic::Point| format!("{:.2}, {:.2}", p.x, ctx.shown_y(p.y));
        rows = rows
            .push(line(t!(i18n, ids::PANELS_HARNESS_START), at(start)))
            .push(line(t!(i18n, ids::PANELS_HARNESS_END), at(end)));
    }
    col.push(
        ctx.collapsible(
            "sel_harness_wire",
            ids::PANELS_HARNESS_SECTION,
            &ctx.collapsed_sections,
            move || rows,
        )
        .boxed(),
    )
}

/// The Harness type row of a selected sheet entry: none (a signal entry) or
/// one of the project's definitions.
pub(super) fn push_sheet_entry_harness_type<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    pin: uuid::Uuid,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let none = i18n.message_plain(ids::PANELS_HARNESS_NO_TYPE, &[]);
    let mut options = vec![none.clone()];
    options.extend(ctx.harness.definitions.iter().map(|d| d.name.clone()));
    let current = ctx
        .harness
        .sheet_entry_type
        .clone()
        .unwrap_or_else(|| none.clone());
    if !options.contains(&current) {
        options.push(current.clone());
    }
    col.push(form_pick_row(
        t!(i18n, ids::PANELS_HARNESS_ENTRY_TYPE),
        options,
        current,
        move |picked| {
            PanelMsg::Harness(HarnessPanelMsg::SheetEntryTypePicked {
                pin,
                harness_type: (picked != none).then_some(picked),
            })
        },
        st.muted,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{PageOrigin, Point};

    fn shown_ends(origin: PageOrigin) -> Vec<String> {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.paper_size = "A4".to_string();
        ctx.page_origin = origin;
        ctx.harness.wire_ends = Some((Point::new(10.0, 20.0), Point::new(30.0, 20.0)));
        let st = PanelStyle {
            muted: Color::BLACK,
            primary: Color::BLACK,
            border_c: Color::BLACK,
            palette: crate::panels::palette::PanelPalette::from_tokens(&ctx.tokens),
        };
        let section = push_harness_wire_sections(Column::new(), &ctx, st);
        crate::layout_probe::lay_out_texts(section.boxed(), iced::Size::new(300.0, 200.0))
            .into_iter()
            .map(|t| t.text)
            .filter(|t| t.contains(", "))
            .collect()
    }

    /// #2789: a harness wire's Start and End read Y from the page origin, as
    /// every other Properties readout does.
    #[test]
    fn the_wire_ends_read_y_from_the_page_origin() {
        let paper_h = signex_types::schematic::paper_height_mm("A4");
        let flipped = format!("{:.2}", paper_h - 20.0);
        assert_eq!(
            shown_ends(PageOrigin::LowerLeft),
            [format!("10.00, {flipped}"), format!("30.00, {flipped}")]
        );
        assert_eq!(
            shown_ends(PageOrigin::UpperLeft),
            ["10.00, 20.00", "30.00, 20.00"]
        );
    }
}
