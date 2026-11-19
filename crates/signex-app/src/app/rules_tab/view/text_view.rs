//! The text view (#2398, #2530): the same rules as rule text in a code
//! field with line numbers and keyword colours, edited as text and applied
//! as one undo step. A line the compiler rejects sits on an `err_soft`
//! wash with its span in the error ink; under the field one banner per
//! problem ("Line 9, column 32: ...") or a green "No problems in the
//! text.". Apply is off while the text has an error.

use std::ops::Range;

use iced::advanced::text::{self as core_text, highlighter};
use iced::widget::text::LineHeight;
use iced::widget::{Column, Row, Space, Stack, container, scrollable, text_editor};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_drc_dsl::{Diagnostic, Level};
use signex_i18n::{ids, t};
use signex_widgets::{button, text};

use super::{Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::{RulesTabMsg, RulesText};
use crate::styles::{Radii, ti, tokens::resolve};

/// Line height of the code field and its gutter, in em of the text size.
const LINE_EM: f32 = 1.3;

/// Words of the rule language drawn in the keyword ink.
const KEYWORDS: [&str; 12] = [
    "rule", "disabled", "on", "between", "and", "when", "matches", "require", "then", "error",
    "warning", "info",
];

/// What a span of a line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Token {
    Keyword,
    Number,
    Quoted,
    /// The span a problem points at.
    Problem,
}

/// Where problems sit: the 0-based line and the byte range in it.
type Marks = Vec<(usize, Range<usize>)>;

/// The rule text highlighter: stateless per line, so a changed line only
/// re-reads itself.
pub(super) struct RuleHighlighter {
    line: usize,
    marks: Marks,
}

impl core_text::Parser for RuleHighlighter {
    type Settings = Marks;
    type Output = Token;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Token)>;

    fn new(settings: &Self::Settings) -> Self {
        RuleHighlighter {
            line: 0,
            marks: settings.clone(),
        }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        self.marks = new_settings.clone();
        self.line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.line = self.line.min(line);
    }

    fn parse_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let at = self.line;
        self.line += 1;
        let marks: Vec<Range<usize>> = self
            .marks
            .iter()
            .filter(|(mark_line, _)| *mark_line == at)
            .map(|(_, span)| span.clone())
            .collect();
        tokens(line, &marks).into_iter()
    }

    fn current_line(&self) -> usize {
        self.line
    }
}

/// The coloured spans of `line`; a problem span replaces what it covers.
pub(super) fn tokens(line: &str, problems: &[Range<usize>]) -> Vec<(Range<usize>, Token)> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let c = bytes[at];
        let start = at;
        if c == b'"' {
            at += 1;
            while at < bytes.len() && bytes[at] != b'"' {
                at += 1;
            }
            at = (at + 1).min(bytes.len());
            out.push((start..at, Token::Quoted));
        } else if c.is_ascii_digit()
            || (c == b'.' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit))
        {
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'.') {
                at += 1;
            }
            out.push((start..at, Token::Number));
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                at += 1;
            }
            if KEYWORDS.contains(&&line[start..at]) {
                out.push((start..at, Token::Keyword));
            }
        } else {
            // One whole character, so a span never splits a code point.
            at += line[at..].chars().next().map_or(1, char::len_utf8);
        }
    }
    let overlaps = |span: &Range<usize>| {
        problems
            .iter()
            .any(|p| span.start < p.end && p.start < span.end)
    };
    out.retain(|(span, _)| !overlaps(span));
    out.extend(problems.iter().map(|p| (p.clone(), Token::Problem)));
    out.sort_by_key(|(span, _)| span.start);
    out
}

/// Whether the compiler rejects the text as typed.
fn has_error(rules_text: &RulesText) -> bool {
    problems(rules_text).any(|d| d.level == Level::Error)
}

/// Everything the compiler says about the text as typed: syntax first,
/// then each rule's problems.
fn problems(rules_text: &RulesText) -> impl Iterator<Item = &Diagnostic> {
    let table = &rules_text.checked;
    table
        .syntax
        .iter()
        .chain(table.rows.iter().flat_map(|r| r.problems.iter()))
}

/// Apply stores typed text that the compiler accepts.
pub(super) fn apply_enabled(rules_text: &RulesText) -> bool {
    rules_text.edited && !has_error(rules_text)
}

/// The problem marks of `typed`: each diagnostic's span cut to its first
/// line, at least one character wide.
fn marks(rules_text: &RulesText, typed: &str) -> Marks {
    problems(rules_text)
        .filter(|d| d.level == Level::Error)
        .filter_map(|d| {
            let start = d.span.start.min(typed.len());
            let line_start = typed[..start].rfind('\n').map_or(0, |i| i + 1);
            let line_end = typed[start..].find('\n').map_or(typed.len(), |i| start + i);
            let wide = typed[start..line_end]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
            let end = d.span.end.clamp(start + wide, line_end.max(start + wide));
            let line = typed[..start].matches('\n').count();
            (end > start).then(|| (line, start - line_start..end - line_start))
        })
        .collect()
}

pub(super) fn view<'a>(frame: &Frame<'a>, rules_text: &'a RulesText) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let typed = rules_text.content.text();
    let marks = marks(rules_text, &typed);
    let bad_lines: Vec<usize> = marks.iter().map(|(line, _)| *line).collect();
    let r = resolve(tokens);
    let (keyword, number, quoted, error) =
        (r.accent_text, r.info, ti(tokens.success), ti(tokens.error));
    let (ink, faint, selection) = (ti(tokens.text), r.ink4, ti(tokens.selection));
    let line_px = TS.body * LINE_EM;
    let editor = text_editor(&rules_text.content)
        .on_action(move |action| {
            up(RulesTabMsg::TextActed {
                project_id: id,
                action,
            })
        })
        .font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
        .size(TS.body)
        .line_height(LineHeight::Relative(LINE_EM))
        .padding([SP.m, SP.m])
        .wrapping(core_text::Wrapping::None)
        .height(Length::Shrink)
        .style(move |_: &Theme, _| text_editor::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: faint,
            value: ink,
            selection,
        })
        .highlight_with::<RuleHighlighter>(marks, move |token: Token, _: &Theme| match token {
            Token::Keyword => highlighter::Style {
                color: Some(keyword),
                style: None,
            },
            Token::Number => highlighter::Style {
                color: Some(number),
                style: None,
            },
            Token::Quoted => highlighter::Style {
                color: Some(quoted),
                style: None,
            },
            Token::Problem => highlighter::Style {
                color: Some(error),
                style: Some(iced::font::Style::Italic),
            },
        });
    // One strip per line under the editor; a rejected line gets the wash.
    let lines = rules_text.content.line_count().max(1);
    let mut wash: Column<Element<'a, Message>> = Column::new().padding([SP.m, 0.0]);
    let mut gutter: Column<Element<'a, Message>> = Column::new().padding([SP.m, SP.s]);
    let err_soft = r.err_soft;
    for line in 0..lines {
        let bad = bad_lines.contains(&line);
        wash = wash.push(
            container(Space::new())
                .width(Length::Fill)
                .height(line_px)
                .style(move |_: &Theme| container::Style {
                    background: bad.then_some(Background::Color(err_soft)),
                    ..container::Style::default()
                })
                .boxed(),
        );
        gutter = gutter.push(
            text(user((line + 1).to_string()))
                .size(TS.body)
                .line_height(LineHeight::Relative(LINE_EM))
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .color(if bad { error } else { faint })
                .align_x(iced::alignment::Horizontal::Right)
                .boxed(),
        );
    }
    let code = Stack::new()
        .push(editor.boxed())
        .push_under(wash.boxed())
        .width(Length::Fill);
    let (field, line, sunken) = (r.field, ti(tokens.border), r.sunken);
    let gutter = container(gutter).style(move |_: &Theme| container::Style {
        background: Some(Background::Color(sunken)),
        ..container::Style::default()
    });
    let code_field = container(
        scrollable(
            Row::new()
                .push(gutter.boxed())
                .push(code.boxed())
                .width(Length::Fill),
        )
        .direction(crate::styles::thin_vertical())
        .style(crate::styles::scrollbar_style(tokens))
        .height(Length::Fill),
    )
    .height(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(field)),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let mut body: Column<Element<'a, Message>> = Column::new()
        .spacing(SP.s)
        .height(Length::Fill)
        .push(code_field.boxed());
    let mut any = false;
    for d in problems(rules_text) {
        any = true;
        let (line, column) = d.line_col(&typed);
        let place = t!(i18n, ids::RULES_TEXT_PROBLEM_AT, "line" => line, "column" => column);
        let tone = if d.level == Level::Error {
            crate::styles::PillTone::Err
        } else {
            crate::styles::PillTone::Warn
        };
        let message = Row::new()
            .spacing(SP.xs)
            .push(text(place).size(TS.body).font(medium()).boxed())
            .push(text(user(d.message.clone())).size(TS.body).boxed());
        body = body.push(banner(message.boxed(), tone, tokens));
    }
    if !any {
        let clean = text(t!(i18n, ids::RULES_TEXT_CLEAN)).size(TS.body).boxed();
        body = body.push(banner(clean, crate::styles::PillTone::Ok, tokens));
    }
    let apply = button(text(t!(i18n, ids::RULES_TEXT_APPLY)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::primary_button(tokens))
        .on_press_maybe(
            apply_enabled(rules_text).then(|| up(RulesTabMsg::TextApplied { project_id: id })),
        );
    let discard = button(text(t!(i18n, ids::RULES_TEXT_DISCARD)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::outline_button(tokens))
        .on_press_maybe(
            rules_text
                .edited
                .then(|| up(RulesTabMsg::TextDiscarded { project_id: id })),
        );
    body = body.push(
        Row::new()
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .push(
                text(t!(i18n, ids::RULES_TEXT_UNDO_NOTE))
                    .size(TS.caption)
                    .color(faint)
                    .boxed(),
            )
            .push(Space::new().width(Length::Fill).boxed())
            .push(discard.boxed())
            .push(apply.boxed())
            .boxed(),
    );
    body.boxed()
}

fn medium() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Medium,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// A banner: the tone's soft fill and ink.
fn banner<'a>(
    content: Element<'a, Message>,
    tone: crate::styles::PillTone,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let (fill, ink) = crate::styles::pill_colors(tokens, tone);
    container(content)
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
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> RulesText {
        let mut view = RulesText::new("");
        view.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
            std::sync::Arc::new(text.to_string()),
        )));
        view
    }

    #[test]
    fn apply_is_off_while_the_text_has_a_syntax_error_and_on_when_clean() {
        let bad = typed("rule a on clearance require clearance >= 0,1mm then error\n");
        assert!(bad.edited);
        assert!(!apply_enabled(&bad), "a syntax error turns Apply off");
        let clean = typed("rule a on clearance require clearance >= 0.1mm then error\n");
        assert!(apply_enabled(&clean), "clean text can be applied");
        let untouched =
            RulesText::new("rule a on clearance require clearance >= 0.1mm then error\n");
        assert!(
            !apply_enabled(&untouched),
            "nothing typed, nothing to apply"
        );
    }

    #[test]
    fn the_bad_line_is_marked_on_its_own_line() {
        let src = "rule a on clearance require clearance >= 0.1mm then error\nrule b on track require width >= 0,1mm then error\n";
        let view = typed(src);
        let marks = marks(&view, &view.content.text());
        assert!(!marks.is_empty(), "the error is marked");
        assert!(
            marks
                .iter()
                .all(|(line, span)| *line == 1 && !span.is_empty())
        );
    }

    #[test]
    fn keywords_numbers_and_quoted_names_are_coloured() {
        let line = "rule HV on clearance when class == \"HV\" require clearance >= 0.8mm";
        let spans = tokens(line, &[]);
        let of = |token| {
            spans
                .iter()
                .filter(|(_, t)| *t == token)
                .map(|(span, _)| &line[span.clone()])
                .collect::<Vec<_>>()
        };
        assert_eq!(of(Token::Keyword), ["rule", "on", "when", "require"]);
        assert_eq!(of(Token::Number), ["0.8mm"]);
        assert_eq!(of(Token::Quoted), ["\"HV\""]);
        let at = line.find("0.8mm").expect("the value");
        let marked = tokens(line, &[at..at + 5]);
        assert!(marked.contains(&(at..at + 5, Token::Problem)));
        assert!(!marked.iter().any(|(_, t)| *t == Token::Number));
    }
}
