//! The code editor (mockup row 195): a header strip (the file, "Recompiles
//! as you type", the error count), the folded header line, the Typst source
//! with line numbers in the file's own numbering, gutter marks (red dot:
//! error, amber: warning), a current-line band and a wash on a line with an
//! error; under it the inline peek of the first error and the Problems
//! strip; over it the reference picker while it is open.

use iced::advanced::text::{self as core_text, highlighter};
use iced::widget::text::LineHeight;
use iced::widget::{Column, Space, Stack, column, container, row, scrollable, text_editor};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{ids, t};
use signex_notebook::{Diagnostic, DiagnosticSeverity, Token};
use signex_widgets::{button, text};

use super::{Look, SP, TS, user};
use crate::app::notebook_tab::highlighter::NotebookHighlighter;
use crate::app::notebook_tab::open::MAIN_FILE;
use crate::app::notebook_tab::{NotebookMsg, NotebookTab, NotebookText};
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, Radii, ti};

/// Line height of the code and its gutter, in em of the text size.
const LINE_EM: f32 = 1.3;
/// The gutter mark's size, in em of the text size.
const MARK_EM: f32 = 0.5;
/// How many problems the strip lists before "and N more".
const PROBLEM_ROWS: usize = 6;

/// The mark a body line carries in the gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::notebook_tab) enum Mark {
    Error,
    Warning,
}

/// The gutter mark of every body line with a problem of the notebook file,
/// the worse one where a line has both.
pub(in crate::app::notebook_tab) fn marks(tab: &NotebookTab) -> Vec<(usize, Mark)> {
    let mut marks: Vec<(usize, Mark)> = Vec::new();
    for problem in main_problems(tab) {
        let Some(line) = tab.body_line(problem.line) else {
            continue;
        };
        let mark = match problem.severity {
            DiagnosticSeverity::Error => Mark::Error,
            DiagnosticSeverity::Warning => Mark::Warning,
        };
        match marks.iter_mut().find(|(held, _)| *held == line) {
            Some((_, held)) if *held == Mark::Warning => *held = mark,
            Some(_) => {}
            None => marks.push((line, mark)),
        }
    }
    marks
}

/// The problems of the newest compile that sit in the notebook file.
pub(super) fn main_problems(tab: &NotebookTab) -> impl Iterator<Item = &Diagnostic> {
    tab.compile
        .last
        .iter()
        .flat_map(|outcome| outcome.status.problems.iter())
        .filter(|d| d.file == MAIN_FILE && d.line > 0)
}

pub(super) fn view<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Element<'a, NotebookMsg> {
    let Some(text) = &tab.text else {
        return Space::new().boxed();
    };
    let code = code(look, tab, text);
    let code: Element<'a, NotebookMsg> = match &tab.picker {
        Some(picker) => Stack::new()
            .push(code)
            .push(super::side::picker_card(look, picker))
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
        None => code,
    };
    let mut body: Column<Element<'a, NotebookMsg>> =
        column![header_strip(look, tab), code].spacing(SP.xs);
    if let Some(peek) = peek(look, tab) {
        body = body.push(peek);
    }
    body = body.push(problems(look, tab));
    container(body.height(Length::Fill))
        .padding(SP.s)
        .height(Length::Fill)
        .boxed()
}

/// `design.snxnbk · Typst`, "Recompiles as you type" and the error count.
fn header_strip<'a>(look: Look<'a>, tab: &NotebookTab) -> Element<'a, NotebookMsg> {
    let r = resolve(look.tokens);
    let file = t!(look.i18n, ids::NOTEBOOK_TAB_EDITOR_FILE, "file" => MAIN_FILE);
    let errors = main_problems(tab)
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .count();
    let mut strip = row![
        text(file)
            .size(TS.dense)
            .color(ti(look.tokens.text))
            .boxed(),
        Space::new().width(Length::Fill).boxed(),
        text(look.t(ids::NOTEBOOK_TAB_RECOMPILES))
            .size(TS.caption)
            .color(r.ink3)
            .boxed(),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if errors > 0 {
        strip = strip.push(
            text(t!(look.i18n, ids::NOTEBOOK_TAB_ERRORS, "count" => errors))
                .size(TS.caption)
                .color(ti(look.tokens.error))
                .boxed(),
        );
    }
    strip.boxed()
}

/// The gutter and the editor, scrolled together.
fn code<'a>(
    look: Look<'a>,
    tab: &'a NotebookTab,
    text: &'a NotebookText,
) -> Element<'a, NotebookMsg> {
    let (tokens, id) = (look.tokens, look.id);
    let r = resolve(tokens);
    let colours = Colours::of(look);
    let line_px = TS.body * LINE_EM;
    let marks = marks(tab);
    let (cursor_line, _) = text.cursor();
    let (placeholder, value, selection) = (r.ink4, ti(tokens.text), ti(tokens.selection));
    let editor = text_editor(&text.content)
        .on_action(move |action| NotebookMsg::TextActed {
            project_id: id,
            action,
        })
        .font(mono())
        .size(TS.body)
        .line_height(LineHeight::Relative(LINE_EM))
        .padding([0.0, SP.m])
        .wrapping(core_text::Wrapping::None)
        .height(Length::Shrink)
        .style(move |_: &Theme, _| text_editor::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder,
            value,
            selection,
        })
        .highlight_with::<NotebookHighlighter>(
            text.highlights.clone(),
            move |token: Token, _: &Theme| colours.style(token),
        );
    let lines = text.content.line_count().max(1);
    let mut wash: Column<Element<'a, NotebookMsg>> = Column::new();
    let mut gutter: Column<Element<'a, NotebookMsg>> = Column::new().padding([0.0, SP.s]);
    for line in 0..lines {
        let mark = marks
            .iter()
            .find(|(held, _)| *held == line)
            .map(|(_, m)| *m);
        let band = match mark {
            Some(Mark::Error) => Some(r.err_soft),
            Some(Mark::Warning) => Some(r.warn_soft),
            None if line == cursor_line => Some(r.raised),
            None => None,
        };
        wash = wash.push(
            container(Space::new())
                .width(Length::Fill)
                .height(line_px)
                .style(move |_: &Theme| container::Style {
                    background: band.map(Background::Color),
                    ..container::Style::default()
                })
                .boxed(),
        );
        gutter = gutter.push(gutter_row(look, tab.file_line(line), mark, line_px));
    }
    let code = Stack::new()
        .push(editor.boxed())
        .push_under(wash.boxed())
        .width(Length::Fill);
    let mut rows: Column<Element<'a, NotebookMsg>> = Column::new().padding([SP.s, 0.0]);
    if tab.header_lines > 0 {
        rows = rows.push(header_row(look, tab, line_px));
    }
    rows = rows.push(
        row![gutter.boxed(), code.boxed()]
            .width(Length::Fill)
            .boxed(),
    );
    let (field, line) = (r.field, ti(tokens.border));
    container(
        scrollable(rows)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(tokens))
            .height(Length::Fill),
    )
    .height(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(field)),
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// One gutter row: the mark, then the line number in the file.
fn gutter_row<'a>(
    look: Look<'a>,
    number: u32,
    mark: Option<Mark>,
    line_px: f32,
) -> Element<'a, NotebookMsg> {
    let r = resolve(look.tokens);
    let colour = match mark {
        Some(Mark::Error) => Some(ti(look.tokens.error)),
        Some(Mark::Warning) => Some(ti(look.tokens.warning)),
        None => None,
    };
    let dot = TS.body * MARK_EM;
    let mark = container(Space::new())
        .width(dot)
        .height(dot)
        .style(move |_: &Theme| container::Style {
            background: colour.map(Background::Color),
            border: Border {
                radius: (dot / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let ink = colour.unwrap_or(r.ink4);
    row![
        mark.boxed(),
        text(user(number.to_string()))
            .size(TS.body)
            .line_height(LineHeight::Relative(LINE_EM))
            .font(mono())
            .color(ink)
            .boxed(),
    ]
    .spacing(SP.xs)
    .height(line_px)
    .align_y(Alignment::Center)
    .boxed()
}

/// The header, folded to one line: Signex owns it.
fn header_row<'a>(look: Look<'a>, tab: &NotebookTab, line_px: f32) -> Element<'a, NotebookMsg> {
    let r = resolve(look.tokens);
    let count = tab
        .file
        .as_ref()
        .map_or(0, |file| file.header.references.len());
    let folded = t!(look.i18n, ids::NOTEBOOK_TAB_HEADER_LINE, "count" => count);
    row![
        text(user("1"))
            .size(TS.body)
            .font(mono())
            .color(r.ink4)
            .boxed(),
        text(folded)
            .size(TS.body)
            .font(mono())
            .color(r.ink3)
            .boxed(),
    ]
    .spacing(SP.m)
    .padding([0.0, SP.s])
    .height(line_px)
    .align_y(Alignment::Center)
    .boxed()
}

/// The first error of the notebook file with its hints, and Go to error.
fn peek<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Option<Element<'a, NotebookMsg>> {
    let first = main_problems(tab).find(|d| d.severity == DiagnosticSeverity::Error)?;
    let mut lines: Column<Element<'a, NotebookMsg>> = column![
        text(user(first.message.clone()))
            .size(TS.body)
            .color(ti(look.tokens.error))
            .boxed()
    ]
    .spacing(SP.xs);
    for hint in &first.hints {
        lines = lines.push(text(user(hint.clone())).size(TS.caption).boxed());
    }
    let go = button(text(look.t(ids::NOTEBOOK_TAB_GO_TO_ERROR)).size(TS.caption))
        .padding([SP.xs, SP.s])
        .style(crate::styles::outline_button(look.tokens))
        .on_press(NotebookMsg::LinePicked {
            project_id: look.id,
            line: first.line,
        });
    let (fill, ink) = crate::styles::pill_colors(look.tokens, crate::styles::PillTone::Err);
    Some(
        container(row![lines.width(Length::Fill).boxed(), go.boxed()].spacing(SP.s))
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                text_color: Some(ink),
                border: Border {
                    radius: Radii::GRAPHITE.control.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
    )
}

/// The Problems strip: every problem of the newest compile with its
/// `line:column`; a row moves the cursor there.
fn problems<'a>(look: Look<'a>, tab: &'a NotebookTab) -> Element<'a, NotebookMsg> {
    let r = resolve(look.tokens);
    let all: Vec<&Diagnostic> = tab
        .compile
        .last
        .iter()
        .flat_map(|outcome| outcome.status.problems.iter())
        .collect();
    let mut title = row![
        text(look.t(ids::NOTEBOOK_TAB_PROBLEMS))
            .size(TS.section)
            .color(r.ink3)
            .boxed()
    ]
    .spacing(SP.s);
    if tab.compile.failed()
        && let Some(good) = &tab.compile.last_good
    {
        let at = t!(look.i18n, ids::NOTEBOOK_TAB_LAST_GOOD, "time" => good.finished_at.as_str());
        title = title.push(text(at).size(TS.caption).color(r.ink3).boxed());
    }
    let mut strip: Column<Element<'a, NotebookMsg>> = column![title.boxed()].spacing(SP.xs);
    if all.is_empty() {
        strip = strip.push(
            text(look.t(ids::NOTEBOOK_TAB_NO_PROBLEMS))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    for problem in all.iter().take(PROBLEM_ROWS) {
        strip = strip.push(problem_row(look, problem));
    }
    if all.len() > PROBLEM_ROWS {
        let more =
            t!(look.i18n, ids::NOTEBOOK_TAB_PROBLEMS_MORE, "count" => all.len() - PROBLEM_ROWS);
        strip = strip.push(text(more).size(TS.caption).color(r.ink3).boxed());
    }
    strip.boxed()
}

fn problem_row<'a>(look: Look<'a>, problem: &Diagnostic) -> Element<'a, NotebookMsg> {
    let ink = match problem.severity {
        DiagnosticSeverity::Error => ti(look.tokens.error),
        DiagnosticSeverity::Warning => ti(look.tokens.warning),
    };
    let at = t!(
        look.i18n,
        ids::NOTEBOOK_TAB_PROBLEM_AT,
        "file" => problem.file.as_str(),
        "line" => problem.line,
        "column" => problem.column
    );
    let face = row![
        text(at).size(TS.caption).font(mono()).color(ink).boxed(),
        text(user(problem.message.clone())).size(TS.caption).boxed(),
    ]
    .spacing(SP.s);
    let in_main = problem.file == MAIN_FILE && problem.line > 0;
    button(face)
        .padding([0.0, SP.xs])
        .width(Length::Fill)
        .style(crate::styles::ghost_button(look.tokens))
        .on_press_maybe(in_main.then_some(NotebookMsg::LinePicked {
            project_id: look.id,
            line: problem.line,
        }))
        .boxed()
}

/// The Typst colours, from the theme.
#[derive(Clone, Copy)]
struct Colours {
    heading: Color,
    function: Color,
    reference: Color,
    string: Color,
    number: Color,
    math: Color,
    label: Color,
    comment: Color,
    keyword: Color,
}

impl Colours {
    fn of(look: Look<'_>) -> Self {
        let r = resolve(look.tokens);
        Colours {
            heading: ti(look.tokens.text),
            function: r.accent_text,
            reference: r.accent_text,
            string: ti(look.tokens.success),
            number: r.info,
            math: ti(look.tokens.warning),
            label: r.info,
            comment: r.ink4,
            keyword: r.accent_text,
        }
    }

    fn style(self, token: Token) -> highlighter::Style {
        let (color, font) = match token {
            Token::Heading => (self.heading, None),
            Token::Function => (self.function, None),
            Token::Reference => (self.reference, None),
            Token::String | Token::Raw => (self.string, None),
            Token::Number => (self.number, None),
            Token::Math => (self.math, None),
            Token::Label => (self.label, None),
            Token::Comment => (self.comment, Some(iced::font::Style::Italic)),
            Token::Keyword => (self.keyword, None),
            Token::Emphasis => (self.heading, Some(iced::font::Style::Italic)),
        };
        highlighter::Style {
            color: Some(color),
            style: font,
        }
    }
}

pub(super) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}
