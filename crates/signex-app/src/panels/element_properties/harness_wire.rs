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
        let at = |p: signex_types::schematic::Point| format!("{:.2}, {:.2}", p.x, p.y);
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
