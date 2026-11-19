//! Library ▸ Component Classes page: the classes every new library starts
//! with (`prefs.json::component_classes`). Edits land on the draft list and
//! are kept by Save; this page's own view state (filter, drag, hover, the
//! pending Reset and the row Add Class appended) is [`ClassesViewState`].

mod rules;
mod table;

use iced::widget::{Space, container, mouse_area, row};
use iced::{Background, Border, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

use super::setting_row;
use super::{PrefMsg, PrefNav, PrefsView};
use crate::styles::{
    DialogButtonKind, Radii, Spacing, TypeScale, dialog_button, ti, tokens::resolve,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the filter field in the toolbar.
const FILTER_W: f32 = 240.0;

/// View state of the Component Classes page. Reset to its default whenever
/// Preferences opens; never saved.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClassesViewState {
    /// Text of the filter field. It narrows the rows shown, never the list.
    pub filter: String,
    /// Reset to Defaults… was pressed and waits for its confirmation.
    pub reset_pending: bool,
    /// Index of the row whose grip is being dragged.
    pub dragging: Option<usize>,
    /// Index of the row under the pointer: its × shows, and during a drag
    /// it is the drop target.
    pub hovered: Option<usize>,
    /// Index of the row Add Class appended: tinted with a "new" pill until
    /// the list is saved.
    pub new_row: Option<usize>,
}

impl ClassesViewState {
    /// Widget id of the key field of the row Add Class appended.
    pub fn new_key_input_id() -> iced::widget::Id {
        iced::widget::Id::new("signex.preferences.classes.new_key")
    }

    /// Widget id of the table's scroller, scrolled to the end on Add Class.
    pub fn table_scroll_id() -> iced::widget::Id {
        iced::widget::Id::new("signex.preferences.classes.table")
    }

    /// Row `from` was moved to `to`: the drag is over and the new row keeps
    /// its marker.
    pub fn row_moved(&mut self, from: usize, to: usize) {
        self.dragging = None;
        self.hovered = None;
        self.new_row = self.new_row.map(|row| rules::follow_move(row, from, to));
    }

    /// Row `index` was removed: indices after it shift up by one.
    pub fn row_removed(&mut self, index: usize) {
        self.dragging = None;
        self.hovered = None;
        self.new_row = self
            .new_row
            .and_then(|row| rules::follow_remove(row, index));
    }
}

/// Messages of the Component Classes page's own view state. Key and label
/// edits stay on `PrefMsg::ComponentClassEditKey` / `EditLabel`.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassesViewMsg {
    /// The filter text changed.
    FilterChanged(String),
    /// "+ Add Class" was pressed: a row is appended and its key focused.
    AddRequested,
    /// "Reset to Defaults…" was pressed: the confirmation shows.
    ResetRequested,
    /// The Reset confirmation was accepted: the list becomes the defaults.
    ResetConfirmed,
    /// The Reset confirmation was dismissed: nothing changes.
    ResetCancelled,
    /// The × of a row was pressed.
    RemoveRequested(usize),
    /// The pointer entered a row.
    RowEntered(usize),
    /// The pointer left a row.
    RowLeft(usize),
    /// The grip of a row was pressed: a drag began.
    DragStarted(usize),
    /// The drag was released over row `to`: row `from` takes its place.
    RowMoved { from: usize, to: usize },
    /// The mouse button was released anywhere on the page: the drag ends.
    DragEnded,
}

fn classes_msg(msg: ClassesViewMsg) -> PrefMsg {
    PrefMsg::ClassesView(msg)
}

/// The page: heading, toolbar, the Reset confirmation while it is asked,
/// and the table.
pub(super) fn content_component_classes<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (i18n, tokens, classes, view) = (
        v.i18n,
        v.panel_tokens,
        v.draft_component_classes,
        v.classes_view,
    );
    let unsaved = v.dirty_pages.contains(&PrefNav::ComponentClasses);
    let mut items = vec![
        setting_row::page_heading(
            t!(i18n, ids::PREFERENCES_CLASSES_TITLE),
            t!(i18n, ids::PREFERENCES_CLASSES_HELP),
            tokens,
        ),
        toolbar(classes.len(), &view.filter, tokens, i18n),
    ];
    if view.reset_pending {
        items.push(reset_confirmation(classes.len(), tokens, i18n));
    }
    items.push(table::table(classes, view, unsaved, tokens, i18n));
    // A release anywhere on the page ends a drag; one over a row has already
    // moved it there. Always a mouse area, so the tree keeps its shape (and
    // a field its focus) when a drag starts.
    let mut area = mouse_area(setting_row::page(items));
    if view.dragging.is_some() {
        area = area.on_release(classes_msg(ClassesViewMsg::DragEnded));
    }
    area.boxed()
}

/// Filter field, class count, then Reset to Defaults… and + Add Class at
/// the right end.
fn toolbar<'a>(
    count: usize,
    filter: &'a str,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    row![
        filter_field(filter, tokens, i18n),
        text(t!(i18n, ids::PREFERENCES_CLASSES_COUNT, "count" => count))
            .size(TS.caption)
            .color(resolve(tokens).ink3),
        Space::new().width(Length::Fill),
        dialog_button(
            t!(i18n, ids::PREFERENCES_CLASSES_RESET),
            DialogButtonKind::Ghost,
            Some(classes_msg(ClassesViewMsg::ResetRequested)),
            tokens,
        ),
        dialog_button(
            t!(i18n, ids::PREFERENCES_CLASSES_ADD),
            DialogButtonKind::Secondary,
            Some(classes_msg(ClassesViewMsg::AddRequested)),
            tokens,
        ),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The "Filter classes" field: a search glyph and a borderless input in a
/// Graphite field frame.
fn filter_field<'a>(query: &'a str, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    let r = resolve(tokens);
    let (hint, ink) = (r.ink4, ti(tokens.text));
    let glyph = iced::widget::svg(crate::icons::icon_chrome_search(
        signex_types::theme::ThemeId::GraphiteDark,
    ))
    .width(TS.label)
    .height(TS.label)
    .style(move |_: &Theme, _| iced::widget::svg::Style {
        color: Some(hint),
        ..Default::default()
    });
    let input = text_input(t!(i18n, ids::PREFERENCES_CLASSES_FILTER), query)
        .on_input(|q| classes_msg(ClassesViewMsg::FilterChanged(q)))
        .padding(Padding::ZERO)
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(iced::Color::TRANSPARENT),
            border: Border::default(),
            placeholder: hint,
            value: ink,
            selection: iced::Color { a: 0.3, ..ink },
        });
    container(
        row![glyph, input]
            .spacing(SP.s + SP.xs / 2.0)
            .align_y(iced::Alignment::Center),
    )
    .padding([SP.s, SP.m])
    .width(FILTER_W)
    .style(crate::styles::field(tokens, false))
    .boxed()
}

/// The inline confirmation Reset to Defaults… asks before replacing the list.
fn reset_confirmation<'a>(count: usize, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    let (fill, line) = (resolve(tokens).warn_soft, ti(tokens.warning));
    let question = t!(
        i18n,
        ids::PREFERENCES_CLASSES_RESET_CONFIRM,
        "count" => count,
        "defaults" => crate::fonts::DEFAULT_COMPONENT_CLASSES.len()
    );
    container(
        row![
            text(question).size(TS.body).width(Length::Fill),
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Secondary,
                Some(classes_msg(ClassesViewMsg::ResetCancelled)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::PREFERENCES_CLASSES_RESET_CONFIRM_YES),
                DialogButtonKind::Danger,
                Some(classes_msg(ClassesViewMsg::ResetConfirmed)),
                tokens,
            ),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}
