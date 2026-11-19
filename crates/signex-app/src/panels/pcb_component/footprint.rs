//! The Footprint, Swapping Options and Schematic Reference Information
//! sections of the General tab (Graphite m94). Swapping Options starts
//! closed and says "off" in its header while both switches are off.

use iced::widget::{Column, Row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::text;

use super::rows::{self, Form, SectionSpec};
use super::{FOOTPRINT_FIELD, FootprintLibrary, PcbComponentFacts, PcbComponentMsg, Shared};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The toggle key of Swapping Options. It starts closed, so the key is in
/// the toggled set while the section is open.
const SWAPPING_KEY: &str = "pcb_component_swapping_open";

/// Push the three sections onto `col`.
pub(super) fn push_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    facts: &'a PcbComponentFacts,
) -> Column<Element<'a, PanelMsg>> {
    let form = Form::new(ctx, facts);
    let off = super::settings::swapping_off(facts).then(|| {
        text(t!(ctx.i18n, ids::PCB_COMPONENT_SWAPPING_OFF))
            .size(TS.caption)
            .color(resolve(&ctx.tokens).ink4)
            .boxed()
    });
    let swapping = SectionSpec {
        trailing: off,
        starts_closed: true,
        ..SectionSpec::new(SWAPPING_KEY, ids::PCB_COMPONENT_SECTION_SWAPPING)
    };
    let sections = [
        rows::section(
            &form,
            "pcb_component_footprint",
            ids::PCB_COMPONENT_SECTION_FOOTPRINT,
            footprint(&form, facts),
        ),
        rows::section_with(
            &form,
            swapping,
            super::settings::swapping_rows(&form, facts),
        ),
        rows::section(
            &form,
            "pcb_component_schematic",
            ids::PCB_COMPONENT_SECTION_SCHEMATIC,
            schematic(&form, facts),
        ),
    ];
    for section in sections.into_iter().flatten() {
        col = col.push(section);
    }
    col
}

/// The text of a shared optional value: the value, `missing` when every
/// component lacks it, blank when they differ.
fn optional_text(form: &Form<'_>, value: &Shared<Option<String>>, missing: MessageId) -> String {
    match value {
        Shared::Same(Some(text)) => text.clone(),
        Shared::Same(None) => form.text(missing),
        Shared::Mixed => String::new(),
    }
}

fn footprint<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Vec<Element<'a, PanelMsg>> {
    let mut rows = Vec::new();
    let label = form.text(ids::PCB_COMPONENT_FOOTPRINT_NAME);
    if form.shows(&label) {
        rows.push(rows::linked_row(
            form,
            label,
            FOOTPRINT_FIELD,
            footprint_name(form, facts),
        ));
    }
    let label = form.text(ids::PCB_COMPONENT_LIBRARY);
    if form.shows(&label) {
        let library = match &facts.library {
            Shared::Same(Some(FootprintLibrary::Open(name))) => name.clone(),
            Shared::Same(Some(FootprintLibrary::NotOpen(id))) => {
                let id = id.to_string();
                t!(form.ctx.i18n, ids::PCB_COMPONENT_LIBRARY_NOT_OPEN, "library" => id.as_str())
                    .into_string()
            }
            Shared::Same(None) => form.text(ids::PCB_COMPONENT_NO_LIBRARY),
            Shared::Mixed => String::new(),
        };
        rows.push(rows::row(form, label, rows::read_only(form, library)));
    }
    rows.extend(super::settings::footprint_rows(form, facts));
    rows
}

/// The footprint name and the Change… button that opens the footprint
/// picker (#2366); a locked or read-only selection cannot swap, and the tip
/// says why.
fn footprint_name<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Element<'a, PanelMsg> {
    let name = match &facts.footprint_name {
        Shared::Same(name) if name.is_empty() => form.text(ids::PCB_FOOTPRINT_NO_NAME),
        Shared::Same(name) => name.clone(),
        Shared::Mixed => String::new(),
    };
    let open = facts.is_editable().then(|| {
        PanelMsg::PcbComponent(PcbComponentMsg::FootprintPickRequested {
            board: facts.board.clone(),
            footprints: facts.footprints.clone(),
        })
    });
    let tip = if open.is_some() {
        ids::PCB_COMPONENT_FOOTPRINT_PICK_TIP
    } else if facts.read_only {
        ids::PCB_COMPONENT_READ_ONLY_TIP
    } else {
        ids::PCB_COMPONENT_LOCKED_TIP
    };
    let change = form.with_tip(
        rows::small_button(t!(form.ctx.i18n, ids::PCB_COMPONENT_FOOTPRINT_PICK), open),
        tip,
    );
    Row::new()
        .push(rows::value(name))
        .push(change)
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .boxed()
}

/// The mismatch note with its Update PCB button, then Designator, Hier.
/// Path, Channel Offset, Design Item ID and Source, all read-only.
fn schematic<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Vec<Element<'a, PanelMsg>> {
    let mut rows = super::identity::designator_rows(form, facts);
    let label = form.text(ids::PCB_COMPONENT_SCH_PATH);
    if form.shows(&label) || form.shows(&form.text(ids::PCB_COMPONENT_SCH_PATH_TIP)) {
        let path = optional_text(form, &facts.schematic_path, ids::PCB_COMPONENT_NOT_LINKED);
        rows.push(rows::row_with_label_tip(
            form,
            label,
            ids::PCB_COMPONENT_SCH_PATH_TIP,
            rows::read_only(form, path),
        ));
    }
    let label = form.text(ids::PCB_COMPONENT_CHANNEL_OFFSET);
    if form.shows(&label) {
        let offset = facts
            .settings
            .channel_offset
            .same()
            .map(ToString::to_string)
            .unwrap_or_default();
        rows.push(rows::row(form, label, rows::read_only(form, offset)));
    }
    rows.extend(super::identity::library_rows(form, facts));
    rows
}
