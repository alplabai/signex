//! The Footprint, Swapping Options and Schematic Reference Information
//! sections of the General tab.

use iced::widget::{Column, Row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::{button, text};

use super::rows::{self, Form};
use super::{FootprintLibrary, PcbComponentFacts, PcbComponentMsg, Shared};
use crate::panels::{PanelContext, PanelMsg, form_look};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Push the three sections onto `col`.
pub(super) fn push_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    facts: &'a PcbComponentFacts,
) -> Column<Element<'a, PanelMsg>> {
    let form = Form::new(ctx, facts);
    let sections = [
        rows::section(
            &form,
            "pcb_component_footprint",
            ids::PCB_COMPONENT_SECTION_FOOTPRINT,
            footprint(&form, facts),
        ),
        rows::section(
            &form,
            "pcb_component_swapping",
            ids::PCB_COMPONENT_SECTION_SWAPPING,
            swapping(&form),
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
        let name = match &facts.footprint_name {
            Shared::Same(name) if name.is_empty() => form.text(ids::PCB_FOOTPRINT_NO_NAME),
            Shared::Same(name) => name.clone(),
            Shared::Mixed => String::new(),
        };
        // Opens the footprint picker; a locked or read-only selection
        // cannot swap, and the tip says why.
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
        let pick = form.with_tip(
            button(text(t!(form.ctx.i18n, ids::PCB_COMPONENT_FOOTPRINT_PICK)).size(TS.body))
                .padding([SP.s, SP.m])
                .style(form_look::ghost_button)
                .on_press_maybe(open)
                .boxed(),
            tip,
        );
        let control = Row::new()
            .push(rows::value(name))
            .push(pick)
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
            .boxed();
        rows.push(rows::row(form, label, control));
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
        rows.push(rows::row(form, label, rows::value(library)));
    }
    // The library footprint's own description is not kept on the board.
    rows.extend(rows::unsupported(
        form,
        ids::PCB_COMPONENT_DESCRIPTION,
        ids::PCB_COMPONENT_NOT_STORED,
    ));
    rows
}

fn swapping<'a>(form: &Form<'a>) -> Vec<Element<'a, PanelMsg>> {
    [ids::PCB_COMPONENT_PIN_SWAP, ids::PCB_COMPONENT_PART_SWAP]
        .into_iter()
        .filter_map(|label_id| {
            let label = form.text(label_id);
            form.shows(&label).then(|| {
                let control = form.with_tip(
                    rows::check(&Shared::Same(false), None),
                    ids::PCB_COMPONENT_NOT_STORED,
                );
                rows::row(form, label, control)
            })
        })
        .collect()
}

fn schematic<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Vec<Element<'a, PanelMsg>> {
    let mut rows = Vec::new();
    // The schematic's designator is read from the sheet, not the board.
    rows.extend(rows::unsupported(
        form,
        ids::PCB_COMPONENT_SCH_DESIGNATOR,
        ids::PCB_COMPONENT_NEEDS_SCHEMATIC,
    ));
    let label = form.text(ids::PCB_COMPONENT_SCH_PATH);
    if form.shows(&label) {
        let path = optional_text(form, &facts.schematic_path, ids::PCB_COMPONENT_NOT_LINKED);
        rows.push(rows::row(form, label, rows::value(path)));
    }
    rows.extend(rows::unsupported(
        form,
        ids::PCB_COMPONENT_CHANNEL_OFFSET,
        ids::PCB_COMPONENT_NOT_STORED,
    ));
    rows
}
