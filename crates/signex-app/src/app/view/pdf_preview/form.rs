//! The settings column's building blocks: the folding section header with
//! its summary, a label-and-control row, the segmented look, checkboxes,
//! captions and the veil over dependents of a switched-off option. Sizes
//! follow the Graphite settings column (mockup row 111: 76 px labels, 22 px
//! dependent indent, 56 px numeric fields).

use iced::widget::{Space, column, container, row};
use iced::{Color, Widget as _};
use signex_i18n::{MessageId, t};
use signex_output::PdfOptions;
use signex_widgets::enable_section::dependents_veil;
use signex_widgets::inert::Inert;
use signex_widgets::{CheckboxLabel as _, Label, SegmentedStyle, checkbox, text, text_input};

use super::super::*;
use crate::app::state::PdfSection;

pub(super) const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
pub(super) const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of a settings row's label column.
pub(super) const KEY_W: f32 = 76.0;
/// Gap between a row's label and its control.
pub(super) const KEY_GAP: f32 = SP.m + SP.xs;
/// Indent of a dependent under its checkbox: the box plus its gap.
pub(super) const SUB_INDENT: f32 = SP.xl + SP.s + SP.xs;
/// Width of a numeric field (opacity, angle, scale percent), fixed by the
/// mockup rows 111 and 168.
pub(super) const NUM_W: f32 = 56.0;
/// Height of a section header.
const SECTION_H: f32 = 26.0;

/// The colours the dialog draws with, resolved from the theme tokens.
#[derive(Debug, Clone, Copy)]
pub(super) struct Look {
    pub ink: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub accent_text: Color,
    pub accent: Color,
    pub panel: Color,
    pub sunken: Color,
    pub raised: Color,
    pub shadow: Color,
    pub line_soft: Color,
    pub line_strong: Color,
}

impl Look {
    pub(super) fn of(tokens: &signex_types::theme::ThemeTokens) -> Self {
        let r = crate::styles::tokens::resolve(tokens);
        Self {
            ink: crate::styles::ti(tokens.text),
            ink3: r.ink3,
            ink4: r.ink4,
            accent_text: r.accent_text,
            accent: crate::styles::ti(tokens.accent),
            panel: crate::styles::ti(tokens.panel_bg),
            sunken: r.sunken,
            raised: r.raised,
            shadow: r.shadow,
            line_soft: r.line_soft,
            line_strong: r.line_strong,
        }
    }

    /// The segmented switch of a settings row, sharing the row's width.
    pub(super) fn segmented(&self) -> SegmentedStyle {
        SegmentedStyle::new(
            self.sunken,
            self.raised,
            self.shadow,
            self.ink3,
            self.ink,
            TS.body,
        )
        .fill_width(true)
    }
}

/// A one-line rule in `colour`, the hairline between dialog regions.
pub(super) fn rule<'a>(colour: Color, vertical: bool) -> Element<'a, Message> {
    let line = if vertical {
        container(Space::new()).width(1).height(Length::Fill)
    } else {
        container(Space::new()).width(Length::Fill).height(1)
    };
    line.style(move |_: &iced::Theme| container::Style {
        background: Some(colour.into()),
        ..container::Style::default()
    })
    .boxed()
}

/// A settings row: the muted label in its column, the control filling the
/// rest. The label stays on one line and ellipsizes.
pub(super) fn form_row<'a>(
    key: impl Into<Label<'a>>,
    look: &Look,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    row![
        text(key)
            .size(TS.body)
            .color(look.ink3)
            .width(KEY_W)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End),
        container(control).width(Length::Fill),
    ]
    .spacing(KEY_GAP)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// A caption line: small, the faintest ink.
pub(super) fn caption<'a>(label: impl Into<Label<'a>>, look: &Look) -> Element<'a, Message> {
    text(label).size(TS.caption).color(look.ink4).boxed()
}

/// A form checkbox with its label; `on_toggle` builds the message.
pub(super) fn check<'a>(
    label: impl Into<Label<'a>>,
    is_on: bool,
    on_toggle: impl Fn(bool) -> PrintPreviewMsg + 'a,
) -> Element<'a, Message> {
    checkbox(is_on)
        .labelled(label)
        .size(TS.body)
        .text_size(TS.body)
        .spacing(SP.m)
        .style(crate::panels::form_look::checkbox_style)
        .on_toggle(move |v| Message::PrintPreview(on_toggle(v)))
        .boxed()
}

/// [`check`] with a help line under its label.
pub(super) fn check_with_help<'a>(
    label: impl Into<Label<'a>>,
    help: impl Into<Label<'a>>,
    is_on: bool,
    on_toggle: impl Fn(bool) -> PrintPreviewMsg + 'a,
    look: &Look,
) -> Element<'a, Message> {
    column![
        check(label, is_on, on_toggle),
        container(caption(help, look)).padding(iced::Padding {
            left: SUB_INDENT,
            ..iced::Padding::ZERO
        }),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// `body` indented as the dependents of the checkbox above it.
pub(super) fn indented<'a>(body: Element<'a, Message>) -> Element<'a, Message> {
    container(body)
        .padding(iced::Padding {
            left: SUB_INDENT,
            ..iced::Padding::ZERO
        })
        .width(Length::Fill)
        .boxed()
}

/// `body` as given while `enabled`; greyed and deaf to input under the
/// panel veil while its switch is off (the disable-when-off rule).
pub(super) fn gated<'a>(
    body: Element<'a, Message>,
    enabled: bool,
    look: &Look,
) -> Element<'a, Message> {
    match dependents_veil(look.panel, enabled) {
        Some(veil) => Inert::new(body, veil).boxed(),
        None => body,
    }
}

/// A numeric field; read-only when `on_input` is `None`.
pub(super) fn num_field<'a>(
    value: &str,
    width: Length,
    on_input: Option<fn(String) -> PrintPreviewMsg>,
) -> Element<'a, Message> {
    let field = text_input(signex_i18n::UserText::new(String::new()), value.to_owned())
        .padding([SP.s, SP.m])
        .size(TS.body)
        .width(width)
        .style(crate::panels::form_look::text_input_style);
    match on_input {
        Some(on_input) => field
            .on_input(move |v| Message::PrintPreview(on_input(v)))
            .boxed(),
        None => field.boxed(),
    }
}

/// A unit after a field ("mm", "%", "°").
pub(super) fn unit<'a>(label: impl Into<Label<'a>>, look: &Look) -> Element<'a, Message> {
    text(label).size(TS.caption).color(look.ink4).boxed()
}

/// One checkbox of a checklist section, read from and written to
/// `PdfOptions`. The view and the section summary read the same list.
pub(super) struct Toggle {
    pub label: MessageId,
    pub is_on: fn(&PdfOptions) -> bool,
    pub on_toggle: fn(bool) -> PrintPreviewMsg,
    /// Drawn under the row above it, as its dependent.
    pub indented: bool,
}

fn toggle(
    label: MessageId,
    is_on: fn(&PdfOptions) -> bool,
    on_toggle: fn(bool) -> PrintPreviewMsg,
    indented: bool,
) -> Toggle {
    Toggle {
        label,
        is_on,
        on_toggle,
        indented,
    }
}

/// Schematics include: what the sheets print.
pub(super) fn schematics_include() -> [Toggle; 7] {
    use signex_i18n::ids;
    [
        toggle(
            ids::PDF_NO_ERC_MARKERS,
            |o| o.include_no_erc_markers,
            PrintPreviewMsg::SetIncludeNoErcMarkers,
            false,
        ),
        toggle(
            ids::PDF_NO_CONNECT_MARKERS,
            |o| o.include_no_connect_markers,
            PrintPreviewMsg::SetIncludeNoConnectMarkers,
            false,
        ),
        toggle(
            ids::PDF_PARAMETER_SETS,
            |o| o.include_parameter_sets,
            PrintPreviewMsg::SetIncludeParameterSets,
            false,
        ),
        toggle(
            ids::PDF_PROBES,
            |o| o.include_probes,
            PrintPreviewMsg::SetIncludeProbes,
            false,
        ),
        toggle(
            ids::PDF_BLANKETS,
            |o| o.include_blankets,
            PrintPreviewMsg::SetIncludeBlankets,
            false,
        ),
        toggle(
            ids::PDF_NOTES,
            |o| o.include_notes,
            PrintPreviewMsg::SetIncludeNotes,
            false,
        ),
        toggle(
            ids::PDF_COLLAPSED_NOTES,
            |o| o.include_collapsed_notes,
            PrintPreviewMsg::SetIncludeCollapsedNotes,
            true,
        ),
    ]
}

/// Bookmarks and Nets: what a net bookmark lists (indented under the help
/// line), then the global bookmarks.
pub(super) fn net_bookmarks() -> [Toggle; 4] {
    use signex_i18n::ids;
    [
        toggle(
            ids::PDF_BOOKMARK_PINS,
            |o| o.bookmark_pins,
            PrintPreviewMsg::SetBookmarkPins,
            true,
        ),
        toggle(
            ids::PDF_BOOKMARK_NET_LABELS,
            |o| o.bookmark_net_labels,
            PrintPreviewMsg::SetBookmarkNetLabels,
            true,
        ),
        toggle(
            ids::PDF_BOOKMARK_PORTS,
            |o| o.bookmark_ports,
            PrintPreviewMsg::SetBookmarkPorts,
            true,
        ),
        toggle(
            ids::PDF_GLOBAL_BOOKMARKS,
            |o| o.global_bookmarks,
            PrintPreviewMsg::SetGlobalBookmarks,
            false,
        ),
    ]
}

/// How many of `toggles` are ticked in `options`.
pub(super) fn ticked(toggles: &[Toggle], options: &PdfOptions) -> usize {
    toggles.iter().filter(|t| (t.is_on)(options)).count()
}

/// The rows of a checklist, indented where marked.
pub(super) fn checklist<'a>(
    toggles: &[Toggle],
    options: &PdfOptions,
    i18n: &signex_i18n::I18n,
) -> Vec<Element<'a, Message>> {
    toggles
        .iter()
        .map(|toggle| {
            let on_toggle = toggle.on_toggle;
            let row = check(t!(i18n, toggle.label), (toggle.is_on)(options), on_toggle);
            if toggle.indented { indented(row) } else { row }
        })
        .collect()
}

impl Signex {
    /// A settings section: its folding header, then `body` while open. The
    /// first section has no rule above it.
    pub(super) fn pdf_section<'a>(
        &'a self,
        preview: &'a crate::app::state::PreviewState,
        section: PdfSection,
        title: MessageId,
        summary: Option<Label<'a>>,
        body: impl FnOnce() -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        let look = Look::of(&self.document_state.panel_ctx.tokens);
        let is_open = preview.dialog.sections.is_open(section);
        let mut col: iced::widget::Column<Element<'a, Message>> = column![].width(Length::Fill);
        if section != PdfSection::Sheets {
            col = col.push(
                container(rule(look.line_soft, false))
                    .padding(iced::Padding {
                        top: SP.s,
                        ..iced::Padding::ZERO
                    })
                    .boxed(),
            );
        }
        col = col.push(self.pdf_section_header(section, title, summary, is_open));
        if is_open {
            col = col.push(
                container(body())
                    .padding([SP.s, SP.s])
                    .width(Length::Fill)
                    .boxed(),
            );
        }
        col.boxed()
    }

    /// A section header: the chevron, the upper-case title and, at the right
    /// end, the summary caption. A click folds or unfolds the section.
    pub(in crate::app::view) fn pdf_section_header<'a>(
        &'a self,
        section: PdfSection,
        title: MessageId,
        summary: Option<Label<'a>>,
        is_open: bool,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = Look::of(&self.document_state.panel_ctx.tokens);
        let title = i18n.upper_for_display(&i18n.message_plain(title, &[]));
        let mut line = row![
            signex_widgets::chevron::chevron(is_open, TS.section, look.ink4),
            // raw: the catalog title, upper-cased for the locale.
            text(signex_widgets::raw(title))
                .size(TS.section)
                .color(look.ink3)
                .font(iced::Font {
                    weight: iced::font::Weight::Medium,
                    ..iced::Font::DEFAULT
                })
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End),
        ]
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center);
        if let Some(summary) = summary {
            line = line.push(text(summary).size(TS.caption).color(look.ink4).boxed());
        }
        signex_widgets::button(
            container(line)
                .height(SECTION_H)
                .padding([0.0, SP.s])
                .align_y(iced::alignment::Vertical::Center)
                .width(Length::Fill),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(Message::PrintPreview(PrintPreviewMsg::SectionToggled(
            section,
        )))
        .style(|_: &iced::Theme, _| signex_widgets::button::Style::default())
        .boxed()
    }
}
