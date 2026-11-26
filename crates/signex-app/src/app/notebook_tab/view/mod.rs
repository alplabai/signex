//! The notebook tab's view (mockup row 195): the document-tab shell with
//! the toolbar and the status footer around the side panel (Outline, Pages,
//! References), the code editor and the preview, as Source, Split or
//! Preview shows them; the empty state while the project has no notebook.
//! Pure: reads the tab state and emits `NotebookMsg`s. Every text is a
//! catalog message or the person's own.

mod editor;
mod empty;
mod footer;
mod preview;
mod side;
mod toolbar;

#[cfg(test)]
pub(super) use editor::{Mark, marks};

use iced::widget::{Row, Space, container, mouse_area, stack};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
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

/// What every part of the view reads.
#[derive(Clone, Copy)]
pub(super) struct Look<'a> {
    pub id: ProjectId,
    pub tokens: &'a ThemeTokens,
    pub i18n: &'a I18n,
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
        let look = Look {
            id,
            tokens: &self.document_state.panel_ctx.tokens,
            i18n: &self.ui_state.i18n,
        };
        match self.document_state.notebooks.get(&id) {
            Some(tab) => tab_view(look, tab).map(Message::from).boxed(),
            None => notice(look, look.t(ids::NOTEBOOK_TAB_READING))
                .map(Message::from)
                .boxed(),
        }
    }
}

/// The whole tab.
pub(super) fn tab_view<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Element<'a, NotebookMsg> {
    let body = match &tab.load {
        Load::Reading => notice(look, look.t(ids::NOTEBOOK_TAB_READING)),
        Load::Failed(why) => notice(
            look,
            t!(look.i18n, ids::NOTEBOOK_TAB_READ_FAILED, "why" => why.as_str()),
        ),
        Load::Missing => empty::view(look, tab),
        Load::Open => open_body(look, tab),
    };
    DocTabShell::new(body, DocTabShellStyle::from_tokens(look.tokens, TS.body))
        .toolbar(toolbar::view(look, tab))
        .footer(footer::view(look, tab))
        .view()
}

/// Side panel, editor, preview.
fn open_body<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Element<'a, NotebookMsg> {
    let mut panes: Row<Element<'a, NotebookMsg>> = Row::new().height(Length::Fill);
    panes = panes.push(
        container(side::view(look, tab))
            .width(Length::Fixed(TS.body * SIDE_EMS))
            .height(Length::Fill)
            .boxed(),
    );
    if tab.view.shows_source() {
        panes = panes.push(
            container(editor::view(look, tab))
                .width(Length::FillPortion(1))
                .height(Length::Fill)
                .boxed(),
        );
    }
    if tab.view.shows_preview() {
        panes = panes.push(
            container(preview::view(look, tab))
                .width(Length::FillPortion(1))
                .height(Length::Fill)
                .boxed(),
        );
    }
    if !tab.block_menu {
        return panes.boxed();
    }
    // The Block menu over the body's top; a press off it closes it.
    let close = NotebookMsg::BlockMenuToggled {
        project_id: look.id,
    };
    let backdrop =
        mouse_area(Space::new().width(Length::Fill).height(Length::Fill)).on_press(close);
    let menu = container(toolbar::block_menu(look))
        .padding(SP.s)
        .width(Length::Fill)
        .center_x(Length::Fill);
    stack![panes.boxed(), backdrop.boxed(), menu.boxed()]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// A line of text in the middle of the body.
fn notice<'a>(look: Look<'a>, line: Localized) -> Element<'a, NotebookMsg> {
    let ink = ti(look.tokens.text_secondary);
    container(text(line).size(TS.body).color(ink))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}
