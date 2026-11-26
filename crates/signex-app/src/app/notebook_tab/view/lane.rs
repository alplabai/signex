//! The margin lane beside the preview (mockup rows 196 and 199): the card
//! of the reference picked in References, the timeline of the target opened
//! from a card, a badge or Properties, and the card of each stale
//! attachment. The views are the reference and measurement slices'; this
//! mounts them and turns what they publish into the tab's messages.

use iced::widget::{Column, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::ids;
use signex_notebook::{CardAction, Reference};
use signex_widgets::{button, text};

use super::{Look, SP, TS};
use crate::app::notebook_measurements::views::stale::{StaleActions, stale_card};
use crate::app::notebook_measurements::views::timeline::{
    TimelineActions, TimelineView, timeline_panel,
};
use crate::app::notebook_references::NotebookReferenceMsg;
use crate::app::notebook_references::views::reference_card;
use crate::app::notebook_tab::open::MAIN_FILE;
use crate::app::notebook_tab::{NotebookMsg, NotebookTab};
use crate::app::{Message, ProjectId};

/// What the lane's timeline and stale cards publish, before the project
/// is named.
#[derive(Debug, Clone)]
enum LaneMsg {
    TargetPicked(Reference),
    ReadingClicked(String, u32),
    ExportPressed,
    Remeasure,
    MarkValid(u32),
    ShowChange(String),
}

impl LaneMsg {
    fn of(self, project_id: ProjectId) -> NotebookMsg {
        match self {
            LaneMsg::TargetPicked(reference) => NotebookMsg::TimelineOpened {
                project_id,
                reference,
            },
            LaneMsg::ReadingClicked(file, line) => NotebookMsg::TimelineReadingClicked {
                project_id,
                file,
                line,
            },
            LaneMsg::ExportPressed => NotebookMsg::TimelineExportPressed { project_id },
            LaneMsg::Remeasure => NotebookMsg::RemeasurePressed { project_id },
            LaneMsg::MarkValid(line) => NotebookMsg::AttachmentMarkedValid { project_id, line },
            LaneMsg::ShowChange(part) => NotebookMsg::ChangeShown { project_id, part },
        }
    }
}

/// A card's button: Timeline opens in this lane; the rest go to the
/// references slice, as they do from the schematic.
fn card_message(project_id: ProjectId, msg: NotebookReferenceMsg) -> Message {
    match msg {
        NotebookReferenceMsg::CardActionPicked {
            reference,
            action: CardAction::Timeline,
        } => Message::from(NotebookMsg::TimelineOpened {
            project_id,
            reference,
        }),
        other => Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::NotebookReferences(other),
        )),
    }
}

/// The lane, or `None` when it has nothing to show.
pub(super) fn view<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Option<Element<'a, Message>> {
    let id = look.id;
    let mut lane: Column<Element<'a, Message>> = Column::new().spacing(SP.s).width(Length::Fill);
    let mut shown = false;
    if let Some((card, stale)) = &tab.card {
        let close = Message::from(NotebookMsg::ReferenceShown {
            project_id: id,
            reference: None,
        });
        lane = lane.push(close_button(look, close)).push(
            reference_card(card, MAIN_FILE, *stale, look.i18n, look.tokens)
                .map(move |msg| card_message(id, msg))
                .boxed(),
        );
        shown = true;
    }
    let measured = tab.compile.lane();
    if let (Some(open), Some(timeline)) = (
        &tab.timeline,
        measured.and_then(|lane| lane.timeline.as_ref()),
    ) {
        let close = Message::from(NotebookMsg::TimelineClosed { project_id: id });
        let panel = timeline_panel(
            TimelineView {
                timeline,
                targets: &open.targets,
                tokens: look.tokens,
                i18n: look.i18n,
            },
            TimelineActions {
                target_picked: LaneMsg::TargetPicked,
                reading_clicked: LaneMsg::ReadingClicked,
                export_pressed: LaneMsg::ExportPressed,
            },
        );
        lane = lane
            .push(close_button(look, close))
            .push(panel.map(move |msg| Message::from(msg.of(id))).boxed());
        shown = true;
    }
    for state in measured.map_or(&[][..], |lane| &lane.attachments) {
        let part = state
            .changes
            .first()
            .map(|change| change.part.clone())
            .unwrap_or_default();
        let actions = StaleActions {
            remeasure: LaneMsg::Remeasure,
            mark_valid: LaneMsg::MarkValid(state.attachment.line),
            show_change: LaneMsg::ShowChange(part),
        };
        if let Some(card) = stale_card(state, look.tokens, look.i18n, actions) {
            lane = lane.push(card.map(move |msg| Message::from(msg.of(id))).boxed());
            shown = true;
        }
    }
    shown.then(|| {
        scrollable(lane.padding(SP.s))
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    })
}

/// Close, at the top right of a lane card.
fn close_button<'a>(look: Look<'a>, close: Message) -> Element<'a, Message> {
    button(text(look.t(ids::NOTEBOOK_TIMELINE_CLOSE)).size(TS.caption))
        .padding([0.0, SP.xs])
        .style(crate::styles::ghost_button(look.tokens))
        .on_press(close)
        .boxed()
}
