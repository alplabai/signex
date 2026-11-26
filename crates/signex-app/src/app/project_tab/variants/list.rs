//! The Variant Manager's variant list (#1293): Base, then each definition
//! with its fitted count and changes against Base. A click selects (the
//! selection is not the active variant), a double-click on a definition
//! opens its name for typing, `+` adds "Variant N". The default variant
//! carries a star and the default pill (mockup row 32). Under the list, the
//! selected variant's Duplicate, Rename, Set as default and Delete, and the
//! reason a variant edit was refused.

use iced::widget::{Column, Space, column, container, mouse_area, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::{button, text, text_input};

use super::super::general::tab_message;
use super::super::look::with_tip;
use super::super::table::{SP, TS, problem};
use super::super::{ProjectTabMsg, VariantRenameDraft};
use super::edits::problem_words;
use super::{GLYPH_PLUS, GLYPH_STAR, Manager, VariantManagerMsg, glyph, medium, pill, semibold};
use crate::app::Message;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, ti};

/// Width of the list, in `body`-size characters.
const LIST_EMS: f32 = 222.0 / 12.0;
/// Width of the selected row's accent bar, in px.
const ACCENT_BAR: f32 = 2.0;

pub(super) fn view<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let mut rows: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
    for variant in m.variants() {
        rows = rows.push(variant_row(m, variant));
    }
    if let Some(refused) = m.tab.and_then(|tab| tab.variant_problem.as_ref()) {
        rows = rows.push(
            container(problem(problem_words(refused, m.i18n), m.tokens))
                .padding([SP.xs, SP.m])
                .boxed(),
        );
    }
    let mut col: Column<Element<'a, Message>> = column![
        header(m),
        scrollable(rows)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(m.tokens))
            .height(Length::Fill),
    ]
    .spacing(SP.xs)
    .height(Length::Fill);
    if !m.definitions.is_empty() {
        col = col.push(actions(m));
    }
    let chrome = ti(m.tokens.toolbar_bg);
    let side = iced::border::Side::default()
        .width(1.0)
        .color(ti(m.tokens.border));
    container(col)
        .width(Length::Fixed(TS.body * LIST_EMS))
        .height(Length::Fill)
        .padding([SP.m, SP.s])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(chrome)),
            border: Border::default().right(side),
            ..container::Style::default()
        })
        .boxed()
}

/// "VARIANTS", the count, and `+`.
fn header<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let title = m
        .i18n
        .upper_for_display(&m.i18n.message_plain(ids::PROJECT_VARIANTS_LIST_TITLE, &[]));
    let add = button(glyph(GLYPH_PLUS, TS.body, r.ink3))
        .padding(SP.xs)
        .style(crate::styles::ghost_button(m.tokens))
        .on_press(tab_message(ProjectTabMsg::NewVariantRequested {
            project_id: m.project_id,
        }));
    row![
        text(signex_i18n::UserText::new(title))
            .size(TS.section)
            .font(medium())
            .color(r.ink3),
        text(t!(m.i18n, ids::PROJECT_VARIANTS_COUNT, "count" => m.definitions.len() + 1))
            .size(TS.caption)
            .color(r.ink4),
        Space::new().width(Length::Fill),
        add,
    ]
    .spacing(SP.s)
    .padding(iced::Padding {
        top: 0.0,
        right: SP.xs,
        bottom: SP.s,
        left: SP.s,
    })
    .align_y(Alignment::Center)
    .boxed()
}

/// One variant: its name (or the name box while it is renamed), what it fits
/// and the active pill; the selected row has the accent fill and bar.
fn variant_row<'a>(m: &Manager<'_>, variant: Option<&str>) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let project_id = m.project_id;
    let is_selected = variant == m.selected;
    let draft = m
        .tab
        .and_then(|tab| tab.variant_rename.as_ref())
        .filter(|draft| Some(draft.from.as_str()) == variant);
    let name: Element<'a, Message> = match draft {
        Some(VariantRenameDraft { from, text: typed }) => {
            let from = from.clone();
            text_input(t!(m.i18n, ids::PROJECT_VARIANTS_COL_NAME), typed.clone())
                .on_input(move |text| {
                    tab_message(ProjectTabMsg::VariantRenameEdited {
                        project_id,
                        from: from.clone(),
                        text,
                    })
                })
                .on_submit(tab_message(ProjectTabMsg::VariantRenameSubmitted {
                    project_id,
                }))
                .style(crate::panels::form_look::text_input_style)
                .size(TS.body)
                .padding(SP.xs)
                .width(Length::Fill)
                .boxed()
        }
        None => {
            let (ink, font) = if is_selected {
                (r.accent_soft_ink, semibold())
            } else {
                (ti(m.tokens.text), medium())
            };
            text(signex_i18n::UserText::new(m.name(variant)))
                .size(TS.body)
                .font(font)
                .color(ink)
                .boxed()
        }
    };
    // The pills sit on the name's line and the fit line has the row's
    // width under them, so no pill covers it (#3037, mockup row 32).
    let mut name_line = row![container(name).width(Length::Fill).clip(true)]
        .spacing(SP.s)
        .align_y(Alignment::Center);
    if m.is_default(variant) {
        name_line = name_line.push(pill(
            t!(m.i18n, ids::VARIANT_MANAGER_DEFAULT),
            PillTone::Neutral,
            m.tokens,
        ));
    }
    if m.is_active(variant) {
        name_line = name_line.push(pill(
            t!(m.i18n, ids::PROJECT_VARIANTS_ACTIVE),
            PillTone::Accent,
            m.tokens,
        ));
    }
    let mut lines: Column<Element<'a, Message>> = column![name_line].width(Length::Fill);
    if let Some(words) = fit_line(m, variant) {
        lines = lines.push(text(words).size(TS.caption).color(r.ink4).boxed());
    }
    let line = row![default_star(m, variant), lines]
        .spacing(SP.m)
        .align_y(Alignment::Center);
    let fill = is_selected.then_some(r.accent_soft);
    let bar = if is_selected {
        r.accent_text
    } else {
        iced::Color::TRANSPARENT
    };
    let side = iced::border::Side::default().width(ACCENT_BAR).color(bar);
    let body = container(line)
        .width(Length::Fill)
        .padding([SP.s, SP.m])
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border::default().rounded(Radii::GRAPHITE.small).left(side),
            ..container::Style::default()
        });
    let mut area = mouse_area(body)
        .on_press(tab_message(ProjectTabMsg::VariantSelected {
            project_id,
            name: variant.map(str::to_string),
        }))
        .interaction(iced::mouse::Interaction::Pointer);
    if let Some(name) = variant {
        area = area.on_double_click(tab_message(ProjectTabMsg::VariantRenameStarted {
            project_id,
            name: name.to_string(),
        }));
    }
    area.boxed()
}

/// The star of the default variant; the same room, empty, on every other
/// row so the names line up.
fn default_star<'a>(m: &Manager<'_>, variant: Option<&str>) -> Element<'a, Message> {
    if !m.is_default(variant) {
        return Space::new().width(TS.body).height(TS.body).boxed();
    }
    let star = glyph(GLYPH_STAR, TS.body, resolve(m.tokens).accent_text);
    with_tip(
        star,
        t!(m.i18n, ids::VARIANT_MANAGER_DEFAULT_STAR),
        m.tokens,
    )
}

/// "N / M fitted · reference" for Base, "N / M fitted · K changes" for a
/// definition; nothing until the parts are read for it.
fn fit_line(m: &Manager<'_>, variant: Option<&str>) -> Option<signex_i18n::Localized> {
    let matrix = m.matrix()?;
    let column = m.column(variant)?;
    let fitted = matrix.fitted_count(column);
    let total = matrix.rows.len();
    Some(match variant {
        None => t!(
            m.i18n,
            ids::PROJECT_VARIANTS_ROW_BASE,
            "fitted" => fitted,
            "total" => total,
        ),
        Some(_) => t!(
            m.i18n,
            ids::PROJECT_VARIANTS_ROW,
            "fitted" => fitted,
            "total" => total,
            "changes" => matrix.changes(column),
        ),
    })
}

/// The selected variant's Duplicate, Rename, Set as default and Delete.
/// Base is duplicated and made the default, never renamed or deleted.
/// Duplicate waits for a parts read that holds, so the copy carries every
/// fit the sheets give the source.
fn actions<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let presses = ActionPresses::of(m);
    let line_side = iced::border::Side::default().width(1.0).color(r.line_soft);
    let col = column![
        text(t!(
            m.i18n,
            ids::PROJECT_VARIANTS_SELECTED,
            "name" => m.name(m.selected),
        ))
        .size(TS.caption)
        .color(r.ink3),
        row![
            small_button(m, ids::VARIANT_MANAGER_DUPLICATE, presses.duplicate, false),
            small_button(m, ids::PROJECT_VARIANTS_RENAME, presses.rename, false),
        ]
        .spacing(SP.xs),
        row![
            small_button(
                m,
                ids::VARIANT_MANAGER_SET_DEFAULT,
                presses.make_default,
                false
            ),
            small_button(m, ids::PROJECT_VARIANTS_DELETE, presses.delete, true),
        ]
        .spacing(SP.xs),
        text(t!(m.i18n, ids::PROJECT_VARIANTS_BASE_NOTE))
            .size(TS.caption)
            .color(r.ink4),
    ]
    .spacing(SP.s);
    container(col)
        .width(Length::Fill)
        .padding([SP.m, SP.s])
        .style(move |_: &Theme| container::Style {
            border: Border::default().top(line_side),
            ..container::Style::default()
        })
        .boxed()
}

/// What each action of the selected variant sends; `None` greys it.
struct ActionPresses {
    duplicate: Option<Message>,
    rename: Option<Message>,
    make_default: Option<Message>,
    delete: Option<Message>,
}

impl ActionPresses {
    fn of(m: &Manager<'_>) -> Self {
        let project_id = m.project_id;
        let manager =
            |msg: VariantManagerMsg| tab_message(ProjectTabMsg::VariantManager { project_id, msg });
        let selected = m.selected.map(str::to_string);
        let can_copy = m.matrix().is_some() && !m.stale && !m.settings_locked;
        let can_default = !m.is_default(m.selected) && !m.settings_locked;
        Self {
            duplicate: can_copy.then(|| {
                manager(VariantManagerMsg::DuplicateRequested {
                    from: selected.clone(),
                })
            }),
            make_default: can_default.then(|| {
                manager(VariantManagerMsg::DefaultPicked {
                    name: selected.clone(),
                })
            }),
            rename: selected
                .clone()
                .map(|name| tab_message(ProjectTabMsg::VariantRenameStarted { project_id, name })),
            delete: selected
                .map(|name| tab_message(ProjectTabMsg::VariantRemoved { project_id, name })),
        }
    }
}

/// One outlined action button of the list's foot; a `danger` one is red.
fn small_button<'a>(
    m: &Manager<'_>,
    words: signex_i18n::MessageId,
    on_press: Option<Message>,
    danger: bool,
) -> Element<'a, Message> {
    let face = text(t!(m.i18n, words)).size(TS.caption);
    let face = if danger {
        face.color(ti(m.tokens.error))
    } else {
        face
    };
    button(face)
        .width(Length::Fill)
        .padding([SP.xs, SP.m])
        .style(crate::styles::outline_button(m.tokens))
        .on_press_maybe(on_press)
        .boxed()
}
