//! The Gerber Viewer tab's view (mockup rows 193 and 193f): the shared
//! document-tab shell with the toolbar, then the layer list, the board and
//! Properties, or the empty state while nothing is open. Files dragged over
//! the tab turn its frame to the accent and name how many will open.

mod empty;
mod layers;
mod properties;
mod run_banner;
mod toolbar;

use iced::widget::{Space, column, container, mouse_area, row, stack};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::doc_tab_shell::{DocTabShell, DocTabShellStyle, Footer};
use signex_widgets::{button, text};

use super::canvas::{BoardView, board_view};
use super::state::{GerberViewerState, OpenPackage};
use super::{GerberViewerMsg, OpenKind};
use crate::app::{Message, Signex};
use crate::styles::board_palette::BoardPalette;
use crate::styles::{HAIRLINE, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The width of the layer list and of Properties, in body-text ems.
const SIDE_EMS: f32 = 20.0;
/// The board's click slop, in body-text ems.
const SLOP_EMS: f32 = 0.35;
/// One wheel line, in body-text ems.
const LINE_EMS: f32 = 1.5;
/// A non-plated hole's dashed ring: dash and gap, in body-text ems.
const DASH_EMS: [f32; 2] = [0.25, 0.17];

/// What every part of the view reads.
#[derive(Clone, Copy)]
pub(super) struct Look<'a> {
    pub tokens: &'a ThemeTokens,
    pub i18n: &'a I18n,
}

impl Look<'_> {
    pub(super) fn t(&self, id: signex_i18n::MessageId) -> Localized {
        t!(self.i18n, id)
    }
}

impl Signex {
    /// The Gerber Viewer tab's content.
    pub(crate) fn view_gerber_viewer_tab(&self) -> Element<'_, Message> {
        let look = Look {
            tokens: &self.document_view.panel_ctx.tokens,
            i18n: &self.ui_state.i18n,
        };
        tab(look, &self.document_view.gerber_viewer)
            .map(Message::GerberViewer)
            .boxed()
    }
}

/// The whole tab.
pub(super) fn tab<'a>(
    look: Look<'a>,
    viewer: &'a GerberViewerState,
) -> Element<'a, GerberViewerMsg> {
    let body = match &viewer.open {
        Some(open) => package_body(look, viewer, open),
        None => empty::view(look, viewer),
    };
    // Opened from an Export Job run (mockup row 193b): its banner on top.
    let body = match &viewer.from_run {
        Some(origin) => column![run_banner::view(look, origin, viewer.run_missing), body]
            .height(Length::Fill)
            .boxed(),
        None => body,
    };
    let body = drop_frame(look, viewer, with_open_menu(look, viewer, body));
    let shell = DocTabShell::new(body, DocTabShellStyle::from_tokens(look.tokens, TS.body))
        .toolbar(toolbar::view(look, viewer));
    let shell = match &viewer.open {
        Some(open) => shell.footer(footer(look, open)).view(),
        None => shell.view(),
    };
    // A held layer row is let go wherever the button comes up on the tab;
    // with no row held the release changes nothing.
    mouse_area(shell)
        .on_release(GerberViewerMsg::GripReleased)
        .boxed()
}

/// Layers, board, Properties.
fn package_body<'a>(
    look: Look<'a>,
    viewer: &'a GerberViewerState,
    open: &'a OpenPackage,
) -> Element<'a, GerberViewerMsg> {
    let side = Length::Fixed(TS.body * SIDE_EMS);
    let board = board_view(BoardView {
        open,
        background: BoardPalette::new().canvas,
        accent: ti(look.tokens.accent),
        slop: TS.body * SLOP_EMS,
        line: TS.body * LINE_EMS,
        dash: DASH_EMS.map(|ems| TS.body * ems),
    });
    row![
        container(layers::view(look, open)).width(side).boxed(),
        container(board)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
        container(properties::view(look, viewer, open))
            .width(side)
            .boxed(),
    ]
    .height(Length::Fill)
    .boxed()
}

/// The Open menu over the body's top left while it is open (mockup row
/// 193, "Open menu").
fn with_open_menu<'a>(
    look: Look<'a>,
    viewer: &'a GerberViewerState,
    body: Element<'a, GerberViewerMsg>,
) -> Element<'a, GerberViewerMsg> {
    if !viewer.open_menu {
        return body;
    }
    let mut items = column![
        menu_item(look, ids::GERBER_VIEWER_OPEN_FOLDER, OpenKind::Folder),
        menu_item(look, ids::GERBER_VIEWER_OPEN_ZIP, OpenKind::Zip),
        menu_item(look, ids::GERBER_VIEWER_OPEN_FILES, OpenKind::Files),
    ]
    .spacing(SP.xs);
    for row in empty::recent_rows(look, viewer) {
        items = items.push(row);
    }
    let menu = container(items)
        .padding(SP.s)
        .style(crate::styles::card(look.tokens));
    // A press anywhere off the menu closes it.
    let backdrop = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(GerberViewerMsg::OpenMenuToggled);
    stack![
        body,
        backdrop.boxed(),
        container(menu).padding(SP.s).boxed()
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

fn menu_item<'a>(
    look: Look<'a>,
    label: signex_i18n::MessageId,
    kind: OpenKind,
) -> Element<'a, GerberViewerMsg> {
    button(text(look.t(label)).size(TS.dense))
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .style(crate::styles::ghost_button(look.tokens))
        .on_press(GerberViewerMsg::OpenPressed(kind))
        .boxed()
}

/// While files are dragged over a window, the tab's frame turns to the
/// accent and names how many files a drop opens (mockup row 193f).
fn drop_frame<'a>(
    look: Look<'a>,
    viewer: &'a GerberViewerState,
    body: Element<'a, GerberViewerMsg>,
) -> Element<'a, GerberViewerMsg> {
    let count = viewer.hovered.len();
    if count == 0 {
        return body;
    }
    let accent = ti(look.tokens.accent);
    let note = container(
        text(t!(look.i18n, ids::GERBER_VIEWER_DROP_FILES, "count" => count))
            .size(TS.label)
            .color(accent),
    )
    .padding(SP.s)
    .style(crate::styles::card(look.tokens));
    let frame =
        container(column![Space::new().height(Length::Fill).boxed(), note.boxed()].spacing(SP.s))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .padding(SP.l)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    color: accent,
                    width: HAIRLINE * 2.0,
                    radius: SP.xs.into(),
                    ..Border::default()
                },
                background: Some(Background::Color(iced::Color { a: 0.06, ..accent })),
                ..Default::default()
            });
    stack![body, frame.boxed()]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// "13 layers · 2 read problems in Messages", and the read-only note.
fn footer<'a>(look: Look<'a>, open: &OpenPackage) -> Footer<'a> {
    let layers =
        t!(look.i18n, ids::GERBER_VIEWER_FOOTER_LAYERS, "count" => open.package.layers.len());
    let problems = t!(
        look.i18n,
        ids::GERBER_VIEWER_FOOTER_PROBLEMS,
        "count" => open.package.problems().len()
    );
    let status = t!(
        look.i18n,
        ids::GERBER_VIEWER_FOOTER,
        "layers" => layers.into_string(),
        "problems" => problems.into_string()
    );
    Footer::new(status).note(look.t(ids::GERBER_VIEWER_FOOTER_NOTE))
}
