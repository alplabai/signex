//! What the Project tab's previews hold while they are used (mockup rows 31
//! and 36): the open value label or held handle of the Schematic defaults
//! drawing, and the larger preview window. Interaction only, so it lives in
//! `UiState`; nothing here is project data.

use iced::alignment::Horizontal;
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{Space, column, container, mouse_area, opaque, row, stack};
use iced::{Background, Border, Color, Element, Length, Task, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dimension_label::{DimensionLabelMessage, DimensionLabels};
use signex_widgets::text;

use super::ProjectTabMsg;
use super::general::tab_message;
use super::look::{Glyph, icon_button, ui_font};
use super::table::{SP, TS};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::styles::{HAIRLINE, Radii, ti, tokens::resolve};

/// How much larger the Schematic defaults drawing is in the window.
const WINDOW_ZOOM: f32 = 1.3;
/// Widest the preview window grows (960 px).
const WINDOW_MAX_W: f32 = TS.body * 80.0;
/// How strongly the page behind the window is dimmed.
const BACKDROP_ALPHA: f32 = 0.38;

/// The preview shown in the larger window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewPopup {
    /// The New sheets preview.
    Sheet,
    /// The Schematic defaults drawing.
    SchematicDefaults,
}

/// An event of a Project tab preview.
#[derive(Debug, Clone)]
pub enum ProjectPreviewMsg {
    /// A value label or drag handle of the Schematic defaults drawing.
    Labels(DimensionLabelMessage),
    PopupOpened(PreviewPopup),
    PopupClosed,
}

/// The previews' interaction state, for the one project it was last used in.
#[derive(Debug, Clone, Default)]
pub struct ProjectPreviewUi {
    project: Option<ProjectId>,
    pub(super) labels: DimensionLabels,
    /// Shown for any other project: no label open, no handle held.
    idle: DimensionLabels,
    popup: Option<PreviewPopup>,
}

impl ProjectPreviewUi {
    /// Start using the previews in `project`; another project's open label
    /// and window are dropped.
    pub(super) fn adopt(&mut self, project: ProjectId) {
        if self.project != Some(project) {
            *self = Self {
                project: Some(project),
                ..Self::default()
            };
        }
    }

    /// The labels as `project`'s drawing shows them.
    pub(super) fn labels_of(&self, project: ProjectId) -> &DimensionLabels {
        if self.project == Some(project) {
            &self.labels
        } else {
            &self.idle
        }
    }

    /// The window open over `project`'s tab.
    pub(crate) fn popup_of(&self, project: ProjectId) -> Option<PreviewPopup> {
        self.popup.filter(|_| self.project == Some(project))
    }

    /// Drawing pixels per layout unit of the Schematic defaults drawing.
    pub(super) fn zoom(&self) -> f32 {
        if self.popup == Some(PreviewPopup::SchematicDefaults) {
            WINDOW_ZOOM
        } else {
            1.0
        }
    }

    /// Close an open label and drop a held handle; the window stays.
    pub(super) fn close_labels(&mut self) {
        self.labels = DimensionLabels::default();
    }
}

impl Signex {
    pub(super) fn act_on_project_preview(
        &mut self,
        project_id: ProjectId,
        msg: ProjectPreviewMsg,
    ) -> Task<Message> {
        if self.document_state.project_by_id(project_id).is_none() {
            self.ui_state.project_preview = ProjectPreviewUi::default();
            return Task::none();
        }
        match msg {
            ProjectPreviewMsg::Labels(message) => self.act_on_default_labels(project_id, message),
            ProjectPreviewMsg::PopupOpened(popup) => {
                let ui = &mut self.ui_state.project_preview;
                ui.adopt(project_id);
                ui.close_labels();
                ui.popup = Some(popup);
                Task::none()
            }
            ProjectPreviewMsg::PopupClosed => {
                let ui = &mut self.ui_state.project_preview;
                ui.close_labels();
                ui.popup = None;
                Task::none()
            }
        }
    }

    /// Tab, Shift+Tab and Esc on the open label or held handle, then Esc on
    /// the open window, while `window` shows that project's tab.
    pub(crate) fn project_preview_key(
        &self,
        window: iced::window::Id,
        event: &keyboard::Event,
    ) -> Option<Message> {
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };
        let shown = self
            .document_state
            .tab_shown_in_window(window, &self.ui_state)?;
        let project_id = shown.kind.as_project()?;
        // Esc lets go of a row held by its grip, moving nothing.
        let holds_row = self
            .document_state
            .project_tabs
            .get(&project_id)
            .is_some_and(|tab| tab.is_dragging_row());
        if holds_row && matches!(key.as_ref(), Key::Named(Named::Escape)) {
            return Some(tab_message(ProjectTabMsg::RowDragged {
                project_id,
                msg: super::RowDragMsg::Cancelled,
            }));
        }
        let ui = &self.ui_state.project_preview;
        let msg = match ui.labels_of(project_id).key_message(key, *modifiers) {
            Some(message) => ProjectPreviewMsg::Labels(message),
            None if ui.popup_of(project_id).is_some()
                && matches!(key.as_ref(), Key::Named(Named::Escape)) =>
            {
                ProjectPreviewMsg::PopupClosed
            }
            None => return None,
        };
        Some(preview_message(project_id, msg))
    }
}

/// `msg` as a message of `project_id`'s tab.
pub(super) fn preview_message(project_id: ProjectId, msg: ProjectPreviewMsg) -> Message {
    tab_message(ProjectTabMsg::PreviewActed { project_id, msg })
}

/// The expand button at the top right of a drawing; stack it over one.
pub(super) fn expand_layer<'a>(
    project_id: ProjectId,
    popup: PreviewPopup,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let open = preview_message(project_id, ProjectPreviewMsg::PopupOpened(popup));
    let tip = t!(i18n, ids::PROJECT_PREVIEW_EXPAND);
    container(icon_button(Glyph::Expand, tip, Some(open), tokens))
        .width(Length::Fill)
        .align_x(Horizontal::Right)
        .padding(SP.xs)
        .boxed()
}

/// The larger preview window over the tab: a title bar with `hint` and
/// Close over `drawing`. A press on the dimmed page closes it.
pub(super) fn window<'a>(
    project_id: ProjectId,
    hint: MessageId,
    drawing: Element<'a, Message>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let close = preview_message(project_id, ProjectPreviewMsg::PopupClosed);
    let dim = Color {
        a: BACKDROP_ALPHA,
        ..ti(tokens.text)
    };
    let backdrop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(dim)),
                ..container::Style::default()
            }),
    )
    .on_press(close.clone());
    let (fill, line) = (ti(tokens.panel_bg), ti(tokens.border));
    let card = container(column![title_bar(hint, close, i18n, tokens), drawing])
        .width(Length::Fill.max(WINDOW_MAX_W))
        .padding(HAIRLINE)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let centred = container(opaque(card))
        .center(Length::Fill)
        .padding(SP.l)
        .boxed();
    stack![opaque(backdrop).capture_scroll(true).boxed(), centred]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// The window's title, its hint and Close.
fn title_bar<'a>(
    hint: MessageId,
    close: Message,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let line = row![
        text(t!(i18n, ids::PROJECT_SHEETS_PREVIEW))
            .size(TS.body)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text)),
        text(t!(i18n, hint))
            .size(TS.caption)
            .color(r.ink4)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .width(Length::Fill),
        icon_button(
            Glyph::Close,
            t!(i18n, ids::PROJECT_PREVIEW_CLOSE),
            Some(close),
            tokens
        ),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    let (fill, soft) = (r.sunken, r.line_soft);
    container(line)
        .width(Length::Fill)
        .padding([SP.xs, SP.m])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: soft,
                ..Border::default()
            }
            .bottom(iced::border::Side::default().width(HAIRLINE)),
            ..container::Style::default()
        })
        .boxed()
}
