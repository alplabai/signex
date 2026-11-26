//! The notebook tab's view (mockup row 195): the document-tab shell with
//! the toolbar and the status footer around the side panel (Outline, Pages,
//! References), the code editor and the preview, as Source, Split or
//! Preview shows them, and the margin lane beside the preview (mockup rows
//! 196 and 199); the empty state while the project has no notebook. Pure:
//! reads the notebook document and the tab state and emits messages. Every
//! text is a catalog message or the person's own.

mod editor;
mod empty;
mod footer;
mod lane;
mod preview;
mod side;
mod toolbar;

#[cfg(test)]
pub(super) use editor::{Mark, marks};

use iced::widget::{Row, Space, container, mouse_area, stack};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_notebook::{NotebookDocument, ReferenceReport};
use signex_types::theme::ThemeTokens;
use signex_widgets::doc_tab_shell::{DocTabShell, DocTabShellStyle};
use signex_widgets::text;

use super::{Load, NotebookMsg, NotebookTab};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::styles::{Spacing, TypeScale, ti};

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;

/// The side panel's width, in body-text ems.
const SIDE_EMS: f32 = 17.0;
/// The margin lane's width, in body-text ems.
const LANE_EMS: f32 = 22.0;

/// What every part of the view reads.
#[derive(Clone, Copy)]
pub(super) struct Look<'a> {
    pub id: ProjectId,
    pub tokens: &'a ThemeTokens,
    pub i18n: &'a I18n,
    /// The notebook document the tab shows.
    pub doc: &'a NotebookDocument,
    /// The notebook file's references resolved against the design, once
    /// the project's notebook read has landed (mockup row 196).
    pub references: Option<&'a ReferenceReport>,
}

impl Look<'_> {
    pub(super) fn t(&self, id: MessageId) -> Localized {
        t!(self.i18n, id)
    }
}

/// The person's own text, owned.
pub(super) fn user(text_: impl Into<String>) -> UserText {
    UserText::new(text_.into())
}

impl Signex {
    /// The notebook tab of project `id`.
    pub(crate) fn view_notebook_tab(&self, id: ProjectId) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let Some((doc, tab)) = self.notebook_parts(id) else {
            return notice(tokens, t!(self.ui_state.i18n, ids::NOTEBOOK_TAB_READING));
        };
        let look = Look {
            id,
            tokens,
            i18n: &self.ui_state.i18n,
            doc,
            references: self.notebook_references_report(id),
        };
        let lane = (doc.load == Load::Open && tab.view.shows_preview())
            .then(|| lane::view(look, tab))
            .flatten();
        tab_view(look, tab, lane)
    }
}

/// The whole tab; `lane` beside the preview when it has something to show.
pub(super) fn tab_view<'a>(
    look: Look<'a>,
    tab: &'a NotebookTab,
    lane: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let body = match &look.doc.load {
        Load::Reading => notice(look.tokens, look.t(ids::NOTEBOOK_TAB_READING)),
        Load::Failed(why) => notice(
            look.tokens,
            t!(look.i18n, ids::NOTEBOOK_TAB_READ_FAILED, "why" => why.as_str()),
        ),
        Load::Missing => empty::view(look, tab).map(Message::from).boxed(),
        Load::Open => open_body(look, tab, lane),
    };
    DocTabShell::new(body, DocTabShellStyle::from_tokens(look.tokens, TS.body))
        .toolbar(toolbar::view(look, tab))
        .footer(footer::view(look, tab))
        .view()
}

/// Side panel, editor, preview and margin lane.
fn open_body<'a>(
    look: Look<'a>,
    tab: &'a NotebookTab,
    lane: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut panes: Row<Element<'a, Message>> = Row::new().height(Length::Fill);
    panes = panes.push(
        container(side::view(look, tab).map(Message::from))
            .width(Length::Fixed(TS.body * SIDE_EMS))
            .height(Length::Fill)
            .boxed(),
    );
    if tab.view.shows_source() {
        panes = panes.push(
            container(editor::view(look, tab).map(Message::from))
                .width(Length::FillPortion(1))
                .height(Length::Fill)
                .boxed(),
        );
    }
    if tab.view.shows_preview() {
        panes = panes.push(
            container(preview::view(look, tab).map(Message::from))
                .width(Length::FillPortion(1))
                .height(Length::Fill)
                .boxed(),
        );
    }
    if let Some(lane) = lane {
        panes = panes.push(
            container(lane)
                .width(Length::Fixed(TS.body * LANE_EMS))
                .height(Length::Fill)
                .boxed(),
        );
    }
    if !tab.block_menu {
        return panes.boxed();
    }
    // The Block menu over the body's top; a press off it closes it.
    let close = Message::from(NotebookMsg::BlockMenuToggled {
        project_id: look.id,
    });
    let backdrop =
        mouse_area(Space::new().width(Length::Fill).height(Length::Fill)).on_press(close);
    let menu = container(toolbar::block_menu(look).map(Message::from))
        .padding(SP.s)
        .width(Length::Fill)
        .center_x(Length::Fill);
    stack![panes.boxed(), backdrop.boxed(), menu.boxed()]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// A line of text in the middle of the body.
fn notice<'a, M: 'a>(tokens: &ThemeTokens, line: Localized) -> Element<'a, M> {
    let ink = ti(tokens.text_secondary);
    container(text(line).size(TS.body).color(ink))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}
