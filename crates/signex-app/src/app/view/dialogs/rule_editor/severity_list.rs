//! The left column of the rule editor: the "Severity per rule" header with
//! the Changed-only chip and the name filter, then the rules under their
//! group subheads, each with its severity control.

use iced::widget::{Column, Row, Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Shadow, Theme};
use iced::{Vector, Widget};
use signex_erc::{RuleKind, Severity};
use signex_i18n::{ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::segmented::SegmentedMetrics;
use signex_widgets::{button, raw, text, text_input};

use super::{RuleEditor, RuleRow, Segment, TS, rule_sections, section_heading, severity_segments};
use crate::styles::tokens::{ResolvedTokens, hover_overlay, resolve};
use crate::styles::{ChromeMetrics, Radii, Spacing, ti};

const SP: Spacing = Spacing::DEFAULT;
const CM: ChromeMetrics = ChromeMetrics::DEFAULT;

// Graphite sizes at the 12 px body, kept as ratios of the body size.
const FILTER_W: f32 = TS.body * 180.0 / 12.0;
const ROW_H: f32 = TS.body * 28.0 / 12.0;
const DOT: f32 = TS.body * 6.0 / 12.0;
const SEG_H: f32 = TS.body * 20.0 / 12.0;
const SEG_LETTER_W: f32 = TS.body * 26.0 / 12.0;
const SEG_OFF_W: f32 = TS.body * 34.0 / 12.0;
const SEG_ON_W: f32 = TS.body * 40.0 / 12.0;
const ICON: f32 = TS.body * 13.0 / 12.0;
/// The Severity column of the page's table head: the control's width.
const SEVERITY_COL: f32 = SEG_LETTER_W * 3.0 + SEG_OFF_W + SP.s;

/// The header row and the grouped rule list.
pub(super) fn view<M: Clone + 'static>(editor: &RuleEditor<'_, M>) -> Element<'static, M> {
    let tokens = editor.tokens;
    let i18n = editor.i18n;
    let r = resolve(tokens);
    let changed = super::changed_rule_count(editor.severity, editor.shipped);

    let chip_style = ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: hover_overlay(tokens),
        ink: ti(tokens.text_secondary),
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    };
    let changed_chip = chip(
        t!(i18n, ids::DIALOG_RULES_CHANGED_ONLY),
        Some(u32::try_from(changed).unwrap_or(u32::MAX)),
        editor.changed_only,
        chip_style,
        ChipMetrics::from_text_size(TS.body),
        i18n,
        Some((editor.on_changed_only)(!editor.changed_only)),
    );
    let is_page = editor.on_open_rule.is_some();
    let lead: Element<'static, M> = if is_page {
        Space::new().width(Length::Fill).boxed()
    } else {
        container(section_heading(
            i18n,
            ids::DIALOG_RULES_SEVERITY_HEADING,
            tokens,
        ))
        .width(Length::Fill)
        .clip(true)
        .boxed()
    };
    let header = row![lead, changed_chip, filter_field(editor, &r),]
        .spacing(SP.s * 1.5)
        .align_y(Alignment::Center)
        .padding(Padding {
            top: SP.m,
            right: SP.l,
            bottom: SP.s,
            left: SP.l,
        });

    let mut list: Column<Element<'static, M>> = Column::new().padding(Padding {
        top: 0.0,
        right: SP.s * 1.5,
        bottom: SP.m,
        left: SP.s * 1.5,
    });
    let sections = rule_sections(
        editor.severity,
        editor.shipped,
        editor.changed_only,
        editor.filter,
        i18n,
    );
    if sections.is_empty() {
        list = list.push(
            container(
                text(t!(i18n, ids::DIALOG_RULES_NO_MATCH))
                    .size(TS.caption)
                    .color(r.ink3),
            )
            .padding([SP.l, SP.m])
            .boxed(),
        );
    }
    for section in sections {
        list = list.push(
            container(section_heading(
                i18n,
                super::group_heading(section.group),
                tokens,
            ))
            .padding(Padding {
                top: SP.m,
                right: SP.m,
                bottom: SP.xs * 1.5,
                left: SP.m,
            })
            .boxed(),
        );
        for rule_row in section.rows {
            list = list.push(rule_line(editor, &r, rule_row));
        }
    }

    let mut pane = column![header].width(Length::Fill);
    if is_page {
        pane = pane.push(table_head(editor));
    }
    pane.push(list.boxed()).boxed()
}

/// The Rule / Severity head of the page's table (mockup row 148).
fn table_head<M: Clone + 'static>(editor: &RuleEditor<'_, M>) -> Element<'static, M> {
    let tokens = editor.tokens;
    let (fill, edge, ink) = (
        ti(tokens.toolbar_bg),
        ti(tokens.border),
        ti(tokens.text_secondary),
    );
    container(
        row![
            text(t!(editor.i18n, ids::DIALOG_RULES_COLUMN_RULE))
                .size(TS.body)
                .color(ink)
                .width(Length::Fill),
            text(t!(editor.i18n, ids::DIALOG_RULES_COLUMN_SEVERITY))
                .size(TS.body)
                .color(ink)
                .width(SEVERITY_COL),
        ]
        .align_y(Alignment::Center),
    )
    .padding([SP.s, SP.l])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: crate::styles::HAIRLINE,
            color: edge,
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// One rule alone, as the editor opened for it shows it.
pub(super) fn one_rule<M: Clone + 'static>(
    editor: &RuleEditor<'_, M>,
    rule: RuleKind,
) -> Element<'static, M> {
    let r = resolve(editor.tokens);
    let current = super::effective_severity(editor.severity, rule);
    row![
        container(rule_name(editor, rule))
            .width(Length::Fill)
            .clip(true),
        severity_control(editor, &r, rule, current),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .height(ROW_H)
    .boxed()
}

/// A rule's name on one line.
fn rule_name<M: 'static>(editor: &RuleEditor<'_, M>, rule: RuleKind) -> Element<'static, M> {
    // raw: the rule names come from signex-erc (#2062).
    text(raw(rule.label().to_string()))
        .size(TS.body)
        .color(ti(editor.tokens.text))
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// The "Filter rules" field: a search glyph and the input on the field fill.
fn filter_field<M: Clone + 'static>(
    editor: &RuleEditor<'_, M>,
    r: &ResolvedTokens,
) -> Element<'static, M> {
    let tokens = editor.tokens;
    let (ink, ink4) = (ti(tokens.text), r.ink4);
    let icon = svg(crate::icons::icon_chrome_search(
        signex_types::theme::ThemeId::GraphiteDark,
    ))
    .width(ICON)
    .height(ICON)
    .style(move |_: &Theme, _| svg::Style {
        color: Some(ink4),
        ..Default::default()
    });
    let input = text_input(
        t!(editor.i18n, ids::DIALOG_RULES_FILTER_PLACEHOLDER),
        editor.filter.to_owned(),
    )
    .on_input(editor.on_filter)
    .padding(Padding::ZERO)
    .size(TS.body)
    .width(Length::Fill)
    .style(move |_: &Theme, _| iced::widget::text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: ink4,
        value: ink,
        selection: Color { a: 0.4, ..ink },
    });
    container(
        row![icon, input]
            .spacing(SP.s * 1.5)
            .align_y(Alignment::Center),
    )
    .padding([0.0, SP.m])
    .width(FILTER_W)
    .height(CM.field)
    .align_y(iced::alignment::Vertical::Center)
    .style(crate::styles::field(tokens, false))
    .boxed()
}

/// One rule: the changed dot, the rule's name and its severity control.
fn rule_line<M: Clone + 'static>(
    editor: &RuleEditor<'_, M>,
    r: &ResolvedTokens,
    rule_row: RuleRow,
) -> Element<'static, M> {
    let accent = ti(editor.tokens.accent);
    let dot_fill = if rule_row.changed {
        accent
    } else {
        Color::TRANSPARENT
    };
    let dot = container(Space::new())
        .width(DOT)
        .height(DOT)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(dot_fill)),
            border: Border {
                radius: (DOT / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    // Clipped to its column: a long name never runs under the control.
    let name = container(rule_name(editor, rule_row.rule))
        .width(Length::Fill)
        .clip(true);
    let name: Element<'static, M> = match editor.on_open_rule {
        Some(open) => button(name)
            .padding(0)
            .width(Length::Fill)
            .on_press(open(rule_row.rule))
            .style(crate::styles::ghost_button(editor.tokens))
            .boxed(),
        None => name.boxed(),
    };
    row![
        dot,
        name,
        severity_control(editor, r, rule_row.rule, rule_row.severity),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .height(ROW_H)
    .padding([0.0, SP.m])
    .boxed()
}

/// The fill and the label colour of a selected segment.
fn selected_colors(tokens: &ThemeTokens, r: &ResolvedTokens, value: Severity) -> (Color, Color) {
    let paper = ti(tokens.paper);
    match value {
        Severity::Error => (ti(tokens.error), paper),
        Severity::Warning => (ti(tokens.warning), paper),
        Severity::Info => (r.info, paper),
        Severity::Off => (r.raised, ti(tokens.text)),
    }
}

/// A rule's severity control: a sunken track of E / W / I / Off segments
/// (On / Off for an on/off-only rule), the selected one filled in its
/// severity colour. Inert when the editor is read-only.
fn severity_control<M: Clone + 'static>(
    editor: &RuleEditor<'_, M>,
    r: &ResolvedTokens,
    rule: RuleKind,
    current: Severity,
) -> Element<'static, M> {
    let tokens = editor.tokens;
    let m = SegmentedMetrics::from_text_size(TS.body);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let (ink4, shadow) = (r.ink4, r.shadow);
    let mut track: Row<Element<'static, M>> = Row::new().spacing(m.gap);
    for Segment {
        value,
        label,
        selected,
    } in severity_segments(rule, current)
    {
        // Off and On are words; E / W / I are mono letters.
        let is_word = value == Severity::Off || rule.is_on_off_only();
        let width = match (value, is_word) {
            (Severity::Off, _) => SEG_OFF_W,
            (_, true) => SEG_ON_W,
            (_, false) => SEG_LETTER_W,
        };
        let (fill, ink) = if selected {
            selected_colors(tokens, r, value)
        } else {
            (Color::TRANSPARENT, ink4)
        };
        let raised = selected && value == Severity::Off;
        let font = if is_word {
            Font {
                weight: iced::font::Weight::Medium,
                ..Font::DEFAULT
            }
        } else {
            Font {
                weight: iced::font::Weight::Semibold,
                ..mono
            }
        };
        let segment = button(
            text(t!(editor.i18n, label))
                .size(TS.caption)
                .font(font)
                .color(ink)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
        )
        .padding(0)
        .width(width)
        .height(SEG_H)
        .on_press_maybe(editor.editable.then(|| (editor.on_severity)(rule, value)))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(fill)),
            text_color: ink,
            border: Border {
                radius: m.segment_radius.into(),
                ..Border::default()
            },
            shadow: if raised {
                Shadow {
                    color: shadow,
                    offset: Vector::new(0.0, m.shadow_offset),
                    blur_radius: m.shadow_blur,
                }
            } else {
                Shadow::default()
            },
            ..button::Style::default()
        });
        track = track.push(segment.boxed());
    }
    let sunken = r.sunken;
    container(track)
        .padding(m.track_pad)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            border: Border {
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
