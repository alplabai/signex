//! The Component Classes table: a sticky header over a scrolling list of
//! editable rows (grip, key, label, ×), with an error line under a row
//! whose key is wrong.

use iced::widget::{Column, Space, column, container, mouse_area, row, scrollable};
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget as _,
};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, text_input, tooltip};

use super::rules::{ClassKeyIssue, class_key_issues, matches_filter};
use super::{ClassesViewMsg, ClassesViewState, SP, TS, classes_msg};
use crate::fonts::ComponentClassEntry;
use crate::preferences::PrefMsg;
use crate::styles::{ChromeMetrics, PillTone, Radii, ti, tokens::resolve};

/// One mockup px at the type scale's body (the mockup is drawn at 12 px).
const EM: f32 = TS.body / 12.0;
/// Height the table's frame never grows past; the rows scroll inside it.
const TABLE_MAX_H: f32 = 470.0 * EM;
/// Width of the grip column.
const GRIP_W: f32 = 30.0 * EM;
/// Width of the × column.
const REMOVE_W: f32 = 40.0 * EM;
/// Shares of the key and label columns in what the fixed columns leave.
const KEY_SHARE: u16 = 42;
const LABEL_SHARE: u16 = 58;

/// One table row's height.
fn row_h() -> f32 {
    ChromeMetrics::DEFAULT.field
}

/// The bordered table: header, then the rows, the empty state or the
/// no-match line.
pub(super) fn table<'a>(
    classes: &'a [ComponentClassEntry],
    view: &'a ClassesViewState,
    unsaved: bool,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let body = if classes.is_empty() {
        empty_state(t!(i18n, ids::PREFERENCES_CLASSES_EMPTY), tokens)
    } else {
        let issues = class_key_issues(classes);
        let folded = i18n.fold_for_search(view.filter.trim());
        let mut rows = Column::new();
        let mut shown = 0_usize;
        for (index, entry) in classes.iter().enumerate() {
            if !matches_filter(i18n, entry, &folded) {
                continue;
            }
            shown += 1;
            let issue = issues.get(index).copied().flatten();
            let look = RowLook {
                index,
                issue,
                is_new: unsaved && view.new_row == Some(index),
                view,
            };
            rows = rows.push(class_row(entry, look, tokens, i18n));
            if let Some(issue) = issue {
                rows = rows.push(issue_line(issue, entry, classes, tokens, i18n));
            }
        }
        if shown == 0 {
            empty_state(
                t!(i18n, ids::PREFERENCES_CLASSES_NO_MATCH, "filter" => view.filter.trim()),
                tokens,
            )
        } else {
            scrollable(rows.width(Length::Fill))
                .id(ClassesViewState::table_scroll_id())
                .direction(crate::styles::thin_vertical())
                .style(crate::styles::scrollbar_style(tokens))
                .width(Length::Fill)
                .height(Length::Fit.max(TABLE_MAX_H - row_h()))
                .boxed()
        }
    };
    let line = ti(tokens.border);
    container(column![header(tokens, i18n), body])
        .width(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The header: Key and Label with what each is for, on the chrome fill.
fn header<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    let r = resolve(tokens);
    let (ink3, fill, line) = (r.ink3, ti(tokens.toolbar_bg), ti(tokens.border));
    let medium = Font {
        weight: iced::font::Weight::Medium,
        ..signex_widgets::ui_font::regular()
    };
    let title = |name: MessageId, note: MessageId| {
        row![
            text(t!(i18n, name)).size(TS.body).font(medium).color(ink3),
            text(t!(i18n, note)).size(TS.caption).color(ink3),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center)
    };
    let cells = row![
        Space::new().width(GRIP_W),
        container(title(
            ids::PREFERENCES_CLASSES_KEY,
            ids::PREFERENCES_CLASSES_KEY_NOTE
        ))
        .padding([0.0, SP.m])
        .width(Length::FillPortion(KEY_SHARE)),
        container(title(
            ids::PREFERENCES_CLASSES_LABEL,
            ids::PREFERENCES_CLASSES_LABEL_NOTE
        ))
        .padding([0.0, SP.m])
        .width(Length::FillPortion(LABEL_SHARE)),
        Space::new().width(REMOVE_W),
    ]
    .align_y(Alignment::Center);
    container(cells)
        .height(row_h())
        .width(Length::Fill)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: bottom_line(line),
            ..container::Style::default()
        })
        .boxed()
}

/// What decides how a row is drawn.
#[derive(Clone, Copy)]
struct RowLook<'a> {
    index: usize,
    issue: Option<ClassKeyIssue>,
    is_new: bool,
    view: &'a ClassesViewState,
}

impl RowLook<'_> {
    fn is_drop_target(&self) -> bool {
        self.view.dragging.is_some_and(|from| from != self.index)
            && self.view.hovered == Some(self.index)
    }

    fn shows_remove(&self) -> bool {
        self.view.dragging.is_none() && self.view.hovered == Some(self.index)
    }

    /// The row fill: the drop target, a wrong key, the new row, the hovered
    /// row, in that order.
    fn fill(&self, tokens: &ThemeTokens) -> Option<Color> {
        let r = resolve(tokens);
        if self.is_drop_target() || (self.is_new && self.issue.is_none()) {
            Some(r.accent_soft)
        } else if self.issue.is_some() {
            Some(Color {
                a: r.err_soft.a * 0.7,
                ..r.err_soft
            })
        } else if self.view.hovered == Some(self.index) {
            Some(crate::styles::tokens::hover_overlay(tokens))
        } else {
            None
        }
    }
}

/// One class: grip, key (mono), label with the "new" pill, ×.
fn class_row<'a>(
    entry: &'a ComponentClassEntry,
    look: RowLook<'a>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let index = look.index;
    let grip = mouse_area(
        container(crate::library::settings::distributor_apis::grip_handle(
            tokens,
        ))
        .center_x(GRIP_W)
        .center_y(row_h()),
    )
    .on_press(classes_msg(ClassesViewMsg::DragStarted(index)))
    .interaction(iced::mouse::Interaction::Grab);

    let mut key = text_input(
        t!(i18n, ids::PLACEHOLDER_COMPONENT_CLASS_KEY),
        entry.key.as_str(),
    )
    .on_input(move |key| PrefMsg::ComponentClassEditKey { index, key })
    .font(crate::fonts::iced_font_for_family(
        crate::fonts::CHROME_MONO_FONT,
    ))
    .size(TS.body)
    .padding([SP.xs, SP.s])
    .width(Length::Fill)
    .style(cell_input_style(tokens, look.issue.is_some()));
    if look.is_new {
        key = key.id(ClassesViewState::new_key_input_id());
    }
    let label = text_input(
        t!(i18n, ids::PLACEHOLDER_COMPONENT_CLASS_LABEL),
        entry.label.as_str(),
    )
    .on_input(move |label| PrefMsg::ComponentClassEditLabel { index, label })
    .size(TS.body)
    .padding([SP.xs, SP.s])
    .width(Length::Fill)
    .style(cell_input_style(tokens, false));
    let mut label_cell = row![label].spacing(SP.s).align_y(Alignment::Center);
    if look.is_new {
        label_cell = label_cell.push(
            container(text(t!(i18n, ids::PREFERENCES_CLASSES_NEW)).size(TS.caption))
                .padding([SP.xs / 2.0, SP.s + SP.xs])
                .style(crate::styles::pill(tokens, PillTone::Accent))
                .boxed(),
        );
    }

    let cells = row![
        grip,
        container(key)
            .padding([0.0, SP.xs])
            .width(Length::FillPortion(KEY_SHARE)),
        container(label_cell)
            .padding([0.0, SP.xs])
            .width(Length::FillPortion(LABEL_SHARE)),
        container(remove_button(look, tokens, i18n)).center_x(REMOVE_W),
    ]
    .align_y(Alignment::Center);

    let (fill, line) = (look.fill(tokens), resolve(tokens).line_soft);
    let framed = container(cells)
        .height(row_h())
        .width(Length::Fill)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: bottom_line(line),
            ..container::Style::default()
        });
    let mut area = mouse_area(framed)
        .on_enter(classes_msg(ClassesViewMsg::RowEntered(index)))
        .on_exit(classes_msg(ClassesViewMsg::RowLeft(index)));
    if let Some(from) = look.view.dragging {
        area = area.on_release(classes_msg(ClassesViewMsg::RowMoved { from, to: index }));
    }
    area.boxed()
}

/// The × of the hovered row; an empty cell on the others.
fn remove_button<'a>(look: RowLook<'a>, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    if !look.shows_remove() {
        return Space::new().width(TS.label).height(TS.label).boxed();
    }
    let r = resolve(tokens);
    let (ink4, hover, tip_ink) = (
        r.ink4,
        crate::styles::tokens::hover_overlay(tokens),
        ti(tokens.text),
    );
    let glyph = iced::widget::svg(crate::icons::icon_close(
        signex_types::theme::ThemeId::GraphiteDark,
    ))
    .width(TS.label)
    .height(TS.label)
    .style(move |_: &Theme, _| iced::widget::svg::Style {
        color: Some(ink4),
        ..Default::default()
    });
    let press = button(glyph)
        .padding(SP.xs)
        .on_press(classes_msg(ClassesViewMsg::RemoveRequested(look.index)))
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover)),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    let tip = container(
        text(t!(i18n, ids::PREFERENCES_CLASSES_REMOVE))
            .size(TS.caption)
            .color(tip_ink),
    )
    .padding([SP.xs, SP.s])
    .style(crate::styles::context_menu(tokens));
    tooltip(press.boxed(), tip.boxed(), tooltip::Position::Top)
        .gap(SP.xs)
        .boxed()
}

/// The error line under a row whose key is wrong.
fn issue_line<'a>(
    issue: ClassKeyIssue,
    entry: &ComponentClassEntry,
    classes: &[ComponentClassEntry],
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let message = match issue {
        ClassKeyIssue::Missing => t!(i18n, ids::PREFERENCES_CLASSES_KEY_MISSING),
        ClassKeyIssue::Malformed => t!(
            i18n,
            ids::PREFERENCES_CLASSES_KEY_MALFORMED,
            "key" => entry.key.trim()
        ),
        ClassKeyIssue::Duplicate { first } => {
            let owner = classes.get(first).map_or("", |c| {
                if c.label.trim().is_empty() {
                    c.key.trim()
                } else {
                    c.label.trim()
                }
            });
            t!(
                i18n,
                ids::PREFERENCES_CLASSES_KEY_DUPLICATE,
                "key" => entry.key.trim(),
                "label" => owner
            )
        }
    };
    let line = resolve(tokens).line_soft;
    container(text(message).size(TS.caption).color(ti(tokens.error)))
        .padding(Padding {
            top: SP.xs,
            bottom: SP.xs,
            left: GRIP_W + SP.m,
            right: SP.m,
        })
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: bottom_line(line),
            ..container::Style::default()
        })
        .boxed()
}

/// A centred line in the table area: the empty list, or no filter match.
fn empty_state<'a>(message: signex_i18n::Localized, tokens: &ThemeTokens) -> Element<'a, PrefMsg> {
    container(text(message).size(TS.body).color(resolve(tokens).ink3))
        .center_x(Length::Fill)
        .center_y(row_h() * 4.0)
        .padding([0.0, SP.xl])
        .boxed()
}

/// A hairline under a row.
fn bottom_line(color: Color) -> Border {
    Border {
        color,
        bottom: iced::border::Side::default().width(1.0),
        ..Border::default()
    }
}

/// An inline cell editor: plain text at rest, a field when focused, an
/// error border while `invalid`.
fn cell_input_style(
    tokens: &ThemeTokens,
    invalid: bool,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + 'static {
    let r = resolve(tokens);
    let (field, accent, error, ink, hint) = (
        r.field,
        ti(tokens.accent),
        ti(tokens.error),
        ti(tokens.text),
        r.ink4,
    );
    move |_: &Theme, status: text_input::Status| {
        let focused = matches!(status, text_input::Status::Focused { .. });
        let (background, line) = match (invalid, focused) {
            (true, _) => (Background::Color(field), error),
            (false, true) => (Background::Color(field), accent),
            (false, false) => (Background::Color(Color::TRANSPARENT), Color::TRANSPARENT),
        };
        text_input::Style {
            background,
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            placeholder: hint,
            value: ink,
            selection: Color { a: 0.3, ..accent },
        }
    }
}
