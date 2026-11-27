//! Compare variants (#2949, mockup row 59, view d), shown by the Variant
//! Manager in place of its cards and matrix: a select for each side with a
//! swap button between them, one line per difference (the part, its left
//! side, its right side as a pill), and a footer with the count of
//! differences, the fitted parts on each side and Close. The rows come from
//! `signex_output::VariantMatrix::compare` over the parts read, with the
//! project's variant settings over them, as the matrix shows them.

use iced::widget::{Column, Row, column, container, pick_list, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_output::{ComparedPart, PartChange, PartInVariant, VariantComparison};
use signex_types::project::{VALUE_PARAMETER, is_base_variant};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, text};

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::{ButtonKind, text_button};
use super::super::table::{SP, TS, note, problem};
use super::manager::CompareSides;
use super::{Manager, Parts, VariantManagerMsg, medium, mono};
use crate::app::Message;
use crate::choice::Choice;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, ti};

/// Width of a side select, in `body`-size characters.
const SELECT_EMS: f32 = 128.0 / 12.0;

/// The comparison of `sides`, or where the parts read stands.
pub(super) fn view<'a>(m: &Manager<'_>, sides: &CompareSides) -> Element<'a, Message> {
    let compared = match m.parts {
        Parts::Reading => return note(t!(m.i18n, ids::PROJECT_VARIANTS_READING), m.tokens),
        Parts::Failed(why) => return problem(super::read_failed(m.i18n, why), m.tokens),
        Parts::Read(matrix) => {
            matrix.compare(m.settings, sides.from.as_deref(), sides.to.as_deref())
        }
    };
    let body: Element<'a, Message> = match compared {
        Ok(compared) => lines(m, &compared),
        Err(why) => {
            let reason = why.to_string();
            problem(
                t!(m.i18n, ids::VARIANT_MANAGER_COMPARE_UNKNOWN, "reason" => reason.as_str()),
                m.tokens,
            )
        }
    };
    column![header(m, sides), body]
        .spacing(SP.m)
        .padding(SP.l)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

fn manager_message(m: &Manager<'_>, msg: VariantManagerMsg) -> Message {
    tab_message(ProjectTabMsg::VariantManager {
        project_id: m.project_id,
        msg,
    })
}

/// The two side selects with the swap button between them.
fn header<'a>(m: &Manager<'_>, sides: &CompareSides) -> Element<'a, Message> {
    let swap = button(text(t!(m.i18n, ids::VARIANT_MANAGER_COMPARE_SWAP_MARK)).size(TS.body))
        .padding([0.0, SP.m])
        .style(crate::styles::ghost_button(m.tokens))
        .on_press(manager_message(m, VariantManagerMsg::CompareSwapped));
    let swap = signex_widgets::tooltip(
        swap,
        container(
            text(t!(m.i18n, ids::VARIANT_MANAGER_COMPARE_SWAP))
                .size(TS.caption)
                .color(ti(m.tokens.text)),
        )
        .padding([SP.xs, SP.s])
        .style(crate::styles::context_menu(m.tokens)),
        iced::widget::tooltip::Position::Bottom,
    )
    .gap(SP.xs)
    .boxed();
    row![
        side_picker(m, sides.from.as_deref(), true),
        swap,
        side_picker(m, sides.to.as_deref(), false),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// The select of one side: Base, then each definition.
fn side_picker<'a>(m: &Manager<'_>, picked: Option<&str>, from: bool) -> Element<'a, Message> {
    let project_id = m.project_id;
    let choices: Vec<Choice<Option<String>>> = m
        .variants()
        .map(|variant| Choice {
            value: variant.map(str::to_string),
            label: m.name(variant),
        })
        .collect();
    let selected = choices
        .iter()
        .find(|choice| match (&choice.value, picked) {
            (None, None) => true,
            (Some(value), Some(picked)) => value.eq_ignore_ascii_case(picked),
            _ => false,
        })
        .cloned();
    pick_list(selected, choices, |choice: &Choice<Option<String>>| {
        choice.label.clone()
    })
    .graphite()
    .on_select(move |choice: Choice<Option<String>>| {
        tab_message(ProjectTabMsg::VariantManager {
            project_id,
            msg: VariantManagerMsg::CompareSidePicked {
                from,
                name: choice.value,
            },
        })
    })
    .width(Length::Fixed(TS.body * SELECT_EMS))
    .boxed()
}

/// The column captions, a line per difference and the footer.
fn lines<'a>(m: &Manager<'_>, compared: &VariantComparison) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let caption = |words: String| {
        text(UserText::new(words))
            .size(TS.caption)
            .color(r.ink4)
            .font(medium())
    };
    // Base's name in the message language, a definition's as written.
    let side_name = |name: &str| m.name(Some(name).filter(|name| !is_base_variant(name)));
    let (from_name, to_name) = (side_name(&compared.from), side_name(&compared.to));
    let mut rows: Column<Element<'a, Message>> = column![three(
        caption(t!(m.i18n, ids::PROJECT_VARIANTS_COL_PART).into_string()).boxed(),
        caption(from_name.clone()).boxed(),
        caption(to_name.clone()).boxed(),
    )]
    .spacing(SP.xs);
    if compared.parts.is_empty() {
        rows = rows.push(note(
            t!(
                m.i18n,
                ids::VARIANT_MANAGER_COMPARE_NONE,
                "from" => from_name.as_str(),
                "to" => to_name.as_str()
            ),
            m.tokens,
        ));
    }
    for part in &compared.parts {
        for (index, change) in part.changes.iter().enumerate() {
            rows = rows.push(difference(m, part, change, index == 0));
        }
    }
    column![scrollable(rows).height(Length::Fill), footer(m, compared)]
        .spacing(SP.m)
        .boxed()
}

/// Three columns: the part, then the two sides at equal widths.
fn three<'a>(
    part: Element<'a, Message>,
    from: Element<'a, Message>,
    to: Element<'a, Message>,
) -> Element<'a, Message> {
    let cell = |content: Element<'a, Message>, portion: u16| {
        container(content)
            .width(Length::FillPortion(portion))
            .padding([SP.xs, SP.s])
            .boxed()
    };
    let line: Row<Element<'a, Message>> = row![cell(part, 2), cell(from, 3), cell(to, 3)];
    line.align_y(Alignment::Center).boxed()
}

/// One difference of `part`: its designator on its first line only.
fn difference<'a>(
    m: &Manager<'_>,
    part: &ComparedPart,
    change: &PartChange,
    first: bool,
) -> Element<'a, Message> {
    let ink = ti(m.tokens.text);
    let designator = if first {
        part.designator.clone()
    } else {
        String::new()
    };
    let designator = text(UserText::new(designator))
        .size(TS.body)
        .color(ink)
        .font(mono())
        .boxed();
    let (from, (to, tone)) = match change {
        PartChange::Fit => (
            fit_words(m, &part.from),
            (fit_words(m, &part.to), fit_tone(&part.to)),
        ),
        PartChange::Parameter(name) => (
            parameter_words(m, &part.from, name),
            (parameter_words(m, &part.to, name), parameter_tone(name)),
        ),
    };
    let from = text(UserText::new(from)).size(TS.body).color(ink).boxed();
    three(designator, from, value_pill(to, tone, m))
}

fn fit_words(m: &Manager<'_>, side: &PartInVariant) -> String {
    let id = if side.fitted {
        ids::PROJECT_VARIANTS_FITTED
    } else {
        ids::PROJECT_VARIANTS_NOT_FITTED
    };
    t!(m.i18n, id).into_string()
}

fn fit_tone(side: &PartInVariant) -> PillTone {
    if side.fitted {
        PillTone::Ok
    } else {
        PillTone::Err
    }
}

/// The value of `name` on one side: the value alone, another parameter
/// with its name; "—" when the side has none.
fn parameter_words(m: &Manager<'_>, side: &PartInVariant, name: &str) -> String {
    let value = side.parameters.get(name).map_or_else(
        || t!(m.i18n, ids::PROJECT_VARIANTS_NO_VALUE).into_string(),
        Clone::clone,
    );
    if name == VALUE_PARAMETER {
        return value;
    }
    t!(
        m.i18n,
        ids::VARIANT_MANAGER_COMPARE_PARAMETER,
        "parameter" => name,
        "value" => value
    )
    .into_string()
}

fn parameter_tone(name: &str) -> PillTone {
    if name == VALUE_PARAMETER {
        PillTone::Accent
    } else {
        PillTone::Info
    }
}

/// The right side of a difference: a pill of `words` in `tone`.
fn value_pill<'a>(words: String, tone: PillTone, m: &Manager<'_>) -> Element<'a, Message> {
    container(text(UserText::new(words)).size(TS.caption).font(medium()))
        .padding([0.0, SP.m * 0.75])
        .style(crate::styles::pill(m.tokens, tone))
        .boxed()
}

/// The count of differences, the fitted parts on each side, and Close.
fn footer<'a>(m: &Manager<'_>, compared: &VariantComparison) -> Element<'a, Message> {
    let differences: usize = compared.parts.iter().map(|part| part.changes.len()).sum();
    let words = t!(
        m.i18n,
        ids::VARIANT_MANAGER_COMPARE_FOOTER,
        "differences" => differences,
        "from" => compared.from_fitted,
        "to" => compared.to_fitted
    );
    row![
        container(text(words).size(TS.caption).color(resolve(m.tokens).ink3)).width(Length::Fill),
        text_button(
            None,
            t!(m.i18n, ids::VARIANT_MANAGER_COMPARE_CLOSE),
            Some(manager_message(m, VariantManagerMsg::CompareClosed)),
            ButtonKind::Small,
            m.tokens,
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}
