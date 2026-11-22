//! The Project tab's section list (mockup row 29): the project header, the
//! sections in their three groups, an icon per section and its status badge.
//! Pure: reads the facts it is given and emits `SectionSelected`.

use iced::alignment::Vertical;
use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::look::{Glyph, ICON, glyph, mono_font, pill, ui_font};
use super::table::{SP, TS};
use super::{ProjectSection, ProjectTabMsg, requirements};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Message, ProjectMsg, Signex};
use crate::styles::tokens::{Overlay, danger, overlay, resolve};
use crate::styles::{HAIRLINE, PillTone, Radii, ti};

/// Width of the list (216 px).
const LIST_W: f32 = TS.body * 18.0;
/// Height of a section row (30 px).
const ROW_H: f32 = TS.body * 2.5;
/// Side of the folder tile in the header (26 px).
const TILE: f32 = TS.body * 13.0 / 6.0;
/// Width of the selected row's accent edge.
const EDGE_W: f32 = 2.0;
/// Side of the dot of a library in trouble (6 px).
const DOT: f32 = TS.body / 2.0;

/// The sections by group, in list order: Project, Rules and defaults,
/// Tracking.
pub(super) const GROUPS: [&[ProjectSection]; 3] = [
    &[
        ProjectSection::General,
        ProjectSection::Parameters,
        ProjectSection::Sheets,
        ProjectSection::Variants,
        ProjectSection::Libraries,
    ],
    &[
        ProjectSection::ErcAnnotation,
        ProjectSection::SchematicDefaults,
        ProjectSection::Setup,
    ],
    &[ProjectSection::VersionControl, ProjectSection::Requirements],
];

/// What the badges of the list read, taken from the Model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SectionFacts {
    pub sheets: usize,
    pub variants: usize,
    pub libraries: usize,
    pub library_missing: bool,
    pub commit_on_save: bool,
    pub requirements_failed: usize,
}

/// The status at the right of a section row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SectionBadge {
    Count(usize),
    /// A count with the error dot: a listed library is missing.
    CountWithDot(usize),
    On,
    Failed(usize),
}

/// The badge of `section`, if it has one now.
pub(super) fn section_badge(section: ProjectSection, facts: &SectionFacts) -> Option<SectionBadge> {
    match section {
        ProjectSection::Sheets => Some(SectionBadge::Count(facts.sheets)),
        ProjectSection::Variants => Some(SectionBadge::Count(facts.variants)),
        ProjectSection::Libraries if facts.library_missing => {
            Some(SectionBadge::CountWithDot(facts.libraries))
        }
        ProjectSection::Libraries => Some(SectionBadge::Count(facts.libraries)),
        ProjectSection::VersionControl if facts.commit_on_save => Some(SectionBadge::On),
        ProjectSection::Requirements if facts.requirements_failed > 0 => {
            Some(SectionBadge::Failed(facts.requirements_failed))
        }
        _ => None,
    }
}

const fn section_glyph(section: ProjectSection) -> Glyph {
    match section {
        ProjectSection::General => Glyph::Info,
        ProjectSection::Parameters => Glyph::Tag,
        ProjectSection::Sheets => Glyph::File,
        ProjectSection::Variants => Glyph::Layers,
        ProjectSection::Libraries => Glyph::Library,
        ProjectSection::ErcAnnotation => Glyph::Shield,
        ProjectSection::SchematicDefaults => Glyph::Sliders,
        ProjectSection::Setup => Glyph::Gear,
        ProjectSection::VersionControl => Glyph::Branch,
        ProjectSection::Requirements => Glyph::Target,
    }
}

impl Signex {
    /// The badge facts of `project`, read from the Model.
    pub(super) fn section_facts(&self, project: &LoadedProject) -> SectionFacts {
        let libraries = self.project_library_rows(project);
        let tab = self.document_state.project_tabs.get(&project.id);
        SectionFacts {
            sheets: project.data.sheets.len(),
            variants: project.data.variant_definitions.len(),
            library_missing: libraries
                .iter()
                .any(|row| row.mount == super::libraries::LibraryMount::Missing),
            libraries: libraries.len(),
            commit_on_save: project.data.enable_git,
            requirements_failed: requirements::failed_count(
                &project.data.requirements,
                tab.map(|tab| &tab.requirements),
            ),
        }
    }
}

/// The folder tile, the project's name and the folder that holds it.
fn header<'a>(project: &LoadedProject, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = resolve(tokens);
    let (soft, soft_ink, line_soft) = (r.accent_soft, r.accent_soft_ink, r.line_soft);
    let tile = container(glyph(Glyph::Folder, ICON, soft_ink))
        .center_x(Length::Fixed(TILE))
        .center_y(Length::Fixed(TILE))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(soft)),
            border: Border {
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let parent = project
        .dir()
        .parent()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default();
    let words = column![
        text(UserText::new(project.data.name.clone()))
            .size(TS.body)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End),
        text(UserText::new(parent))
            .size(TS.caption)
            .font(mono_font())
            .color(r.ink3)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End),
    ]
    .width(Length::Fill);
    container(row![tile, words].spacing(SP.m).align_y(Alignment::Center))
        .width(Length::Fill)
        .padding([SP.m, SP.m])
        .style(move |_: &Theme| container::Style {
            border: Border {
                color: line_soft,
                ..Border::default()
            }
            .bottom(iced::border::Side::default().width(HAIRLINE)),
            ..container::Style::default()
        })
        .boxed()
}

fn badge_view<'a>(badge: SectionBadge, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let count = |n: usize| {
        text(UserText::new(i18n.count(n as i64)))
            .size(TS.caption)
            .font(mono_font())
            .color(resolve(tokens).ink3)
    };
    match badge {
        SectionBadge::Count(n) => count(n).boxed(),
        SectionBadge::CountWithDot(n) => {
            let fill = danger(tokens);
            let dot = container(Space::new())
                .width(Length::Fixed(DOT))
                .height(Length::Fixed(DOT))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(fill)),
                    border: Border {
                        radius: (DOT / 2.0).into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                });
            row![count(n), dot]
                .spacing(SP.s)
                .align_y(Alignment::Center)
                .boxed()
        }
        SectionBadge::On => pill(t!(i18n, ids::DIALOG_RULES_ON), PillTone::Ok, None, tokens),
        SectionBadge::Failed(n) => pill(
            t!(i18n, ids::REQUIREMENTS_FAILED_COUNT, "count" => n),
            PillTone::Err,
            None,
            tokens,
        ),
    }
}

/// A row's fill: the soft accent when selected, else the hover overlay.
fn entry_style(
    tokens: &ThemeTokens,
    is_selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let soft = resolve(tokens).accent_soft;
    let hover = overlay(tokens, Overlay::Hover);
    let pressed = overlay(tokens, Overlay::Pressed);
    move |_: &Theme, status: button::Status| button::Style {
        background: if is_selected {
            Some(Background::Color(soft))
        } else {
            match status {
                button::Status::Hovered => Some(Background::Color(hover)),
                button::Status::Pressed => Some(Background::Color(pressed)),
                _ => None,
            }
        },
        border: Border {
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn entry<'a>(
    project_id: ProjectId,
    section: ProjectSection,
    selected: ProjectSection,
    facts: &SectionFacts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let is_selected = section == selected;
    let ink = if is_selected {
        resolve(tokens).accent_soft_ink
    } else {
        ti(tokens.text_secondary)
    };
    let mut face: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center)
        .push(glyph(section_glyph(section), ICON, ink))
        .push(
            text(t!(i18n, section.title()))
                .size(TS.body)
                .color(ink)
                .width(Length::Fill)
                .boxed(),
        );
    if let Some(badge) = section_badge(section, facts) {
        face = face.push(badge_view(badge, i18n, tokens));
    }
    let pressable = button(
        container(face)
            .height(Length::Fill)
            .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(ROW_H))
    .padding([0.0, SP.m])
    .style(entry_style(tokens, is_selected))
    .on_press(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id,
            section,
        },
    )));
    let edge_ink = ti(tokens.accent);
    let edge = container(Space::new())
        .width(Length::Fixed(EDGE_W))
        .height(Length::Fixed(ROW_H))
        .style(move |_: &Theme| container::Style {
            background: is_selected.then_some(Background::Color(edge_ink)),
            ..container::Style::default()
        });
    row![edge, pressable]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .boxed()
}

/// The list: the header, then each group's rows with a gap between groups.
pub(super) fn section_list<'a>(
    project: &LoadedProject,
    facts: &SectionFacts,
    selected: ProjectSection,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut groups: Column<Element<'a, Message>> =
        Column::new().spacing(SP.l).padding([SP.m, SP.s]);
    for sections in GROUPS {
        let mut group: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
        for section in sections {
            group = group.push(entry(project.id, *section, selected, facts, i18n, tokens));
        }
        groups = groups.push(group);
    }
    container(column![header(project, tokens), groups])
        .width(Length::Fixed(LIST_W))
        .height(Length::Fill)
        .style(crate::styles::panel_region(tokens))
        .boxed()
}
