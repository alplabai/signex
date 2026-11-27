//! The outputs list (mockup row 114, left): the outputs by group, each with
//! its tick (in the shown profile) and its Enabled switch, the picked one
//! marked; Add Output and its menu by group; the empty state.

use iced::widget::{Column, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::output_job::{OutputGroup, OutputKind};

use super::{Frame, Look, SP, kit};
use crate::app::Message;
use crate::app::output_job::tab::JobTabMsg;

/// The catalog name of a group.
pub(super) fn group_label(group: OutputGroup) -> MessageId {
    match group {
        OutputGroup::Fabrication => ids::OUTPUT_JOB_GROUP_FABRICATION,
        OutputGroup::Assembly => ids::OUTPUT_JOB_GROUP_ASSEMBLY,
        OutputGroup::Documentation => ids::OUTPUT_JOB_GROUP_DOCUMENTATION,
        OutputGroup::Reports => ids::OUTPUT_JOB_GROUP_REPORTS,
    }
}

/// The catalog name of a kind.
pub(super) fn kind_label(kind: OutputKind) -> MessageId {
    match kind {
        OutputKind::SchematicPdf => ids::OUTPUT_JOB_KIND_SCHEMATIC_PDF,
        OutputKind::SchematicPng => ids::OUTPUT_JOB_KIND_SCHEMATIC_PNG,
        OutputKind::Bom => ids::OUTPUT_JOB_KIND_BOM,
        OutputKind::Netlist => ids::OUTPUT_JOB_KIND_NETLIST,
        OutputKind::Gerber => ids::OUTPUT_JOB_KIND_GERBER,
        OutputKind::NcDrill => ids::OUTPUT_JOB_KIND_NC_DRILL,
        OutputKind::PickAndPlace => ids::OUTPUT_JOB_KIND_PICK_AND_PLACE,
        OutputKind::TestPoints => ids::OUTPUT_JOB_KIND_TEST_POINTS,
        OutputKind::Ipc2581 => ids::OUTPUT_JOB_KIND_IPC2581,
        OutputKind::Odb => ids::OUTPUT_JOB_KIND_ODB,
        OutputKind::Step => ids::OUTPUT_JOB_KIND_STEP,
        OutputKind::AssemblyPdf => ids::OUTPUT_JOB_KIND_ASSEMBLY_PDF,
    }
}

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let job = frame.job_path();
    let add = kit::primary(
        look.tokens,
        look.t(ids::OUTPUT_JOB_ADD_OUTPUT),
        Some(Message::from(JobTabMsg::AddMenuToggled {
            job: job.clone(),
        })),
    );
    let mut list: Column<Element<'a, Message>> = column![add].spacing(SP.s).padding(SP.m);
    if frame.ui.add_menu {
        list = list.push(add_menu(look, &job));
    }
    if frame.draft.outputs.is_empty() {
        list = list
            .push(kit::line(look, look.t(ids::OUTPUT_JOB_EMPTY_TITLE)))
            .push(kit::quiet(look, look.t(ids::OUTPUT_JOB_EMPTY_BODY)));
    }
    for group in OutputGroup::ALL {
        let members: Vec<_> = frame
            .draft
            .outputs
            .iter()
            .filter(|o| o.known_kind().is_some_and(|k| k.group() == group))
            .collect();
        if members.is_empty() {
            continue;
        }
        let ticked = members
            .iter()
            .filter(|o| shown_ticked(frame, &o.name, o.enabled))
            .count();
        let head = row![
            kit::head(look, group_label(group)),
            kit::quiet(
                look,
                t!(look.i18n, ids::OUTPUT_JOB_GROUP_COUNT, "ticked" => ticked, "total" => members.len())
            ),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center);
        list = list.push_boxed(head);
        for output in members {
            list = list.push(output_row(frame, &output.name, output.enabled));
        }
    }
    let unknown: Vec<_> = frame
        .draft
        .outputs
        .iter()
        .filter(|o| o.known_kind().is_none())
        .collect();
    for output in unknown {
        list = list.push(kit::quiet(
            look,
            t!(look.i18n, ids::OUTPUT_JOB_UNKNOWN_KIND, "name" => output.name.as_str(), "kind" => output.kind.as_str()),
        ));
    }
    list = list.push(kit::quiet(look, look.t(ids::OUTPUT_JOB_TICK_NOTE)));
    scrollable(list)
        .direction(crate::styles::thin_vertical())
        .height(Length::Fill)
        .boxed()
}

/// Whether the row of output `name` shows its tick: in the shown profile,
/// or, with no profile yet, every enabled output (each one runs).
fn shown_ticked(frame: &Frame<'_>, name: &str, enabled: bool) -> bool {
    frame.profile.map_or(enabled, |profile| profile.ticks(name))
}

/// One output: its tick, its name (pressed to pick it) and its switch.
fn output_row<'a>(frame: &Frame<'a>, name: &str, enabled: bool) -> Element<'a, Message> {
    let look = frame.look;
    let job = frame.job_path();
    let ticked = shown_ticked(frame, name, enabled);
    let tick = kit::tick(
        ticked,
        UserText::new(String::new()),
        enabled.then(|| {
            Message::from(JobTabMsg::OutputTicked {
                job: job.clone(),
                name: name.to_string(),
            })
        }),
    );
    let picked = frame.ui.selected.as_deref() == Some(name);
    let pick = Message::from(JobTabMsg::OutputPicked {
        job: job.clone(),
        name: name.to_string(),
    });
    let label = match picked {
        true => kit::primary(look.tokens, UserText::new(name.to_string()), Some(pick)),
        false => kit::ghost(look.tokens, UserText::new(name.to_string()), Some(pick)),
    };
    let switch = kit::tick(
        enabled,
        look.t(ids::OUTPUT_JOB_ENABLED),
        Some(Message::from(JobTabMsg::OutputEnabledToggled {
            job,
            name: name.to_string(),
        })),
    );
    row![tick, container(label).width(Length::Fill), switch]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .boxed()
}

/// Add Output's menu: every kind, by group.
fn add_menu<'a>(look: Look<'a>, job: &std::path::Path) -> Element<'a, Message> {
    let mut menu: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
    for group in OutputGroup::ALL {
        menu = menu.push(kit::head(look, group_label(group)));
        for kind in OutputKind::ALL.into_iter().filter(|k| k.group() == group) {
            let msg = Message::from(JobTabMsg::OutputAdded {
                job: job.to_path_buf(),
                kind,
            });
            menu = menu.push(kit::ghost(look.tokens, look.t(kind_label(kind)), Some(msg)));
        }
    }
    container(menu)
        .padding(SP.s)
        .width(Length::Fill)
        .style(crate::styles::card(look.tokens))
        .boxed()
}
