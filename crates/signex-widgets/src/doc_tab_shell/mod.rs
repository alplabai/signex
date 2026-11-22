//! Document-tab shell: the frame every page-style document tab sits in
//! (Layer Stack, Length Matching, Polygon Manager, BOM Studio, Export Job,
//! Variant Manager, Design Notebook, …). From the top: a toolbar row, a strip
//! of sub-tabs, the body, a footer. Each part but the body is optional, so a
//! tab kind writes only its body and fills the slots.
//!
//! Stateless: the caller owns the selected sub-tab and maps a press to its
//! own message. Every text arrives as a [`Label`](crate::Label) from the
//! caller. Sizes derive from the body text size through the Graphite scales
//! ([`DocTabMetrics`]); colours come from the theme tokens.

mod footer;
mod sub_tabs;
mod toolbar;

pub use footer::Footer;
pub use sub_tabs::{SubTab, SubTabLook, SubTabs};
pub use toolbar::{Toolbar, ToolbarItem, separator};

use iced::widget::{Column, Space, container};
use iced::{Background, Color, Element, Font, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;

use crate::theme_ext::{ChromeMetrics, DEFAULT_BASE_SIZE, HAIRLINE, Spacing, TypeScale, to_color};

/// Lengths of the shell, from one body text size. The Graphite design draws
/// them at a 12 px body; each one scales with the body size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocTabMetrics {
    /// Body text: toolbar items and sub-tab labels.
    pub text_size: f32,
    /// Caption text: the footer.
    pub caption_size: f32,
    /// Horizontal padding of every band (the `m` spacing step).
    pub pad_x: f32,
    /// Vertical padding of the toolbar.
    pub toolbar_pad_y: f32,
    /// Gap between toolbar items and between toolbar lines when it wraps.
    pub toolbar_gap: f32,
    /// Height of a toolbar separator.
    pub separator_height: f32,
    /// Clear space on each side of a toolbar separator.
    pub separator_margin: f32,
    /// Gap between two sub-tabs.
    pub sub_gap: f32,
    /// Height of a sub-tab.
    pub sub_height: f32,
    /// Horizontal padding inside a sub-tab.
    pub sub_pad_x: f32,
    /// Gap between a sub-tab's label and its tag.
    pub sub_inner_gap: f32,
    /// Weight of the active sub-tab's underline.
    pub underline: f32,
    /// How far the underline stops short of each side of its sub-tab.
    pub underline_inset: f32,
    /// Corner radius of the underline.
    pub underline_radius: f32,
    /// Smallest footer height.
    pub footer_min_height: f32,
    /// Vertical padding of the footer.
    pub footer_pad_y: f32,
    /// Gap between the footer's status and note.
    pub footer_gap: f32,
}

impl DocTabMetrics {
    /// Every length from `body`, the body step of the type scale.
    pub fn from_text_size(body: f32) -> Self {
        let ts = TypeScale::from_base(body);
        let sp = Spacing::from_base(body);
        let chrome = ChromeMetrics::DEFAULT;
        // A length the design gives in px at the 12 px body.
        let at = |px: f32| px * body / DEFAULT_BASE_SIZE;
        let scale = body / DEFAULT_BASE_SIZE;
        Self {
            text_size: ts.body,
            caption_size: ts.caption,
            pad_x: sp.m,
            toolbar_pad_y: at(6.0),
            toolbar_gap: sp.s,
            separator_height: at(16.0),
            separator_margin: at(3.0),
            sub_gap: sp.xs,
            sub_height: chrome.button * scale,
            sub_pad_x: at(10.0),
            sub_inner_gap: at(6.0),
            underline: at(2.0),
            underline_inset: at(6.0),
            underline_radius: at(2.0),
            footer_min_height: chrome.status * scale,
            footer_pad_y: at(4.0),
            footer_gap: at(6.0),
        }
    }
}

impl Default for DocTabMetrics {
    fn default() -> Self {
        Self::from_text_size(DEFAULT_BASE_SIZE)
    }
}

/// Colours of the shell, plus its metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocTabShellStyle {
    pub metrics: DocTabMetrics,
    /// Toolbar and footer band (the chrome surface).
    pub chrome: Color,
    /// Body fill.
    pub panel: Color,
    /// Sub-tab strip baseline, footer top line, separators.
    pub border: Color,
    /// Toolbar bottom line.
    pub line_soft: Color,
    /// Active sub-tab label.
    pub ink: Color,
    /// Idle sub-tab label.
    pub ink2: Color,
    /// Footer text.
    pub ink3: Color,
    /// Fill of a hovered sub-tab.
    pub hover: Color,
    /// Active sub-tab underline.
    pub accent: Color,
}

impl DocTabShellStyle {
    /// Colours from the theme; an extended token a theme leaves unset falls
    /// back to the nearest base token.
    pub fn from_tokens(tokens: &ThemeTokens, text_size: f32) -> Self {
        let ext = &tokens.extended;
        let or = |c: Option<signex_types::theme::Color>, base| to_color(&c.unwrap_or(base));
        Self {
            metrics: DocTabMetrics::from_text_size(text_size),
            chrome: to_color(&tokens.toolbar_bg),
            panel: to_color(&tokens.panel_bg),
            border: to_color(&tokens.border),
            line_soft: or(ext.line_soft, tokens.border),
            ink: to_color(&tokens.text),
            ink2: to_color(&tokens.text_secondary),
            ink3: or(ext.ink3, tokens.text_secondary),
            hover: to_color(&tokens.hover),
            accent: to_color(&tokens.accent),
        }
    }
}

/// The shell. Build with [`DocTabShell::new`], fill the slots, then
/// [`view`](DocTabShell::view) (or `.into()`).
pub struct DocTabShell<'a, M> {
    style: DocTabShellStyle,
    toolbar: Option<Toolbar<'a, M>>,
    sub_tabs: Option<SubTabs<'a, M>>,
    body: Element<'a, M>,
    footer: Option<Footer<'a>>,
}

impl<'a, M: Clone + 'a> DocTabShell<'a, M> {
    pub fn new(body: Element<'a, M>, style: DocTabShellStyle) -> Self {
        Self {
            style,
            toolbar: None,
            sub_tabs: None,
            body,
            footer: None,
        }
    }

    pub fn toolbar(mut self, toolbar: Toolbar<'a, M>) -> Self {
        self.toolbar = Some(toolbar);
        self
    }

    pub fn sub_tabs(mut self, sub_tabs: SubTabs<'a, M>) -> Self {
        self.sub_tabs = Some(sub_tabs);
        self
    }

    pub fn footer(mut self, footer: Footer<'a>) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Which slots are filled: toolbar, sub-tabs, footer.
    pub fn slots(&self) -> (bool, bool, bool) {
        (
            self.toolbar.is_some(),
            self.sub_tabs.is_some(),
            self.footer.is_some(),
        )
    }

    pub fn view(self) -> Element<'a, M> {
        let s = self.style;
        let mut shell: Column<Element<'a, M>> = Column::new().width(Length::Fill);
        if let Some(toolbar) = self.toolbar {
            shell = shell.push(toolbar.view(&s));
        }
        if let Some(sub_tabs) = self.sub_tabs {
            shell = shell.push(sub_tabs.view(&s));
        }
        let panel = s.panel;
        shell = shell.push(
            container(self.body)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &Theme| filled(panel))
                .boxed(),
        );
        if let Some(footer) = self.footer {
            shell = shell.push(footer.view(&s));
        }
        shell.height(Length::Fill).boxed()
    }
}

impl<'a, M: Clone + 'a> From<DocTabShell<'a, M>> for Element<'a, M> {
    fn from(shell: DocTabShell<'a, M>) -> Self {
        shell.view()
    }
}

/// A container style that only fills.
fn filled(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(color)),
        ..container::Style::default()
    }
}

/// A full-width divider line, one hairline thick.
fn rule<'a, M: 'a>(color: Color) -> Element<'a, M> {
    container(Space::new().width(Length::Fill).height(HAIRLINE))
        .width(Length::Fill)
        .style(move |_: &Theme| filled(color))
        .boxed()
}

/// The medium weight of the active sub-tab.
fn medium() -> Font {
    Font {
        weight: iced::font::Weight::Medium,
        ..crate::ui_font::regular()
    }
}

#[cfg(test)]
mod tests;
