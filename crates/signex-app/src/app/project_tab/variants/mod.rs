//! Variants section of the Project tab in the Variant Manager layout (#1690,
//! #1293): a top bar (active variant, parts filter, Only rows that differ),
//! the variant list with the selected variant's actions, a summary card per
//! variant and the parts by variants matrix, read off the UI thread from the
//! project's sheets. Every variant edit is one step of the General history; a
//! refused one shows its reason and changes nothing. The matrix is read-only
//! here. See `docs/PROJECT_TAB.md`, "Variants".

mod bar;
mod edits;
mod empty;
mod list;
mod matrix;
mod read;
mod summary;
#[cfg(test)]
mod tests;

use crate::app::core::parts_read_error::PartsReadError;

use iced::font::Weight;
use iced::widget::{Column, column, container, row, svg};
use iced::{Background, Border, Color, Element, Font, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_output::VariantMatrix;
use signex_types::project::is_base_variant;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::ProjectTabState;
use super::table::{SP, TS, note, problem};
use crate::app::Message;
use crate::app::state::{LoadedProject, ProjectId};
use crate::styles::{PillTone, Radii, ti};

/// The parts read, as the view shows it.
#[derive(Clone, Copy)]
enum Parts<'v> {
    Reading,
    Failed(&'v PartsReadError),
    Read(&'v VariantMatrix),
}

/// The catalog message of a failed parts read.
fn read_failed(i18n: &I18n, why: &PartsReadError) -> Localized {
    match why {
        PartsReadError::NoRootSheet => t!(i18n, ids::PROJECT_VARIANTS_READ_NO_ROOT),
        PartsReadError::RootUnreadable(path) => t!(
            i18n,
            ids::PROJECT_VARIANTS_READ_ROOT_UNREADABLE,
            "path" => path.display().to_string()
        ),
        PartsReadError::ChannelNamingUnreadable => {
            t!(i18n, ids::DIALOG_SCHEMATIC_DESIGN_UNREADABLE)
        }
        PartsReadError::Interrupted(reason) => {
            t!(i18n, ids::PROJECT_VARIANTS_READ_FAILED, "reason" => reason.as_str())
        }
    }
}

/// Everything the Variants views read, borrowed from the model.
struct Manager<'v> {
    project_id: ProjectId,
    /// The variant definitions, live from the project.
    definitions: &'v [String],
    /// The active variant's definition; `None` is Base.
    active: Option<&'v str>,
    /// The variant selected in the list; `None` is Base.
    selected: Option<&'v str>,
    tab: Option<&'v ProjectTabState>,
    parts: Parts<'v>,
    /// The parts were read before the latest sheet edit, or for other
    /// variant names than the project has now.
    stale: bool,
    i18n: &'v I18n,
    tokens: &'v ThemeTokens,
}

impl<'v> Manager<'v> {
    fn new(
        project: &'v LoadedProject,
        tab: Option<&'v ProjectTabState>,
        sheet_epoch: u64,
        i18n: &'v I18n,
        tokens: &'v ThemeTokens,
    ) -> Self {
        let definitions = project.data.variant_definitions.as_slice();
        let find = |name: &str| {
            definitions
                .iter()
                .find(|defined| defined.eq_ignore_ascii_case(name.trim()))
                .map(String::as_str)
        };
        let active = project
            .data
            .active_variant
            .as_deref()
            .filter(|name| !is_base_variant(name))
            .and_then(find);
        let selected = match tab.and_then(|tab| tab.variant_selected.as_deref()) {
            None => active,
            Some(name) if is_base_variant(name) => None,
            Some(name) => find(name).or(active),
        };
        let parts = match tab.and_then(|tab| tab.variant_parts.as_deref()) {
            None => Parts::Reading,
            Some(Err(why)) => Parts::Failed(why),
            Some(Ok(matrix)) => Parts::Read(matrix),
        };
        let stale = match (parts, tab) {
            (Parts::Read(matrix), Some(tab)) => !read_holds(tab, matrix, project, sheet_epoch),
            _ => false,
        };
        Self {
            project_id: project.id,
            definitions,
            active,
            selected,
            tab,
            parts,
            stale,
            i18n,
            tokens,
        }
    }

    fn matrix(&self) -> Option<&'v VariantMatrix> {
        match self.parts {
            Parts::Read(matrix) => Some(matrix),
            _ => None,
        }
    }

    /// The matrix column of `variant` (`None` is Base), when it was read.
    fn column(&self, variant: Option<&str>) -> Option<usize> {
        self.matrix().and_then(|matrix| matrix.column(variant))
    }

    /// Base, then each definition, as the list, cards and columns show them.
    fn variants(&self) -> impl Iterator<Item = Option<&'v str>> + 'v {
        let definitions: &'v [String] = self.definitions;
        std::iter::once(None).chain(definitions.iter().map(|name| Some(name.as_str())))
    }

    /// A variant's name as shown: Base's from the catalog.
    fn name(&self, variant: Option<&str>) -> String {
        match variant {
            None => self.i18n.message_plain(ids::PROJECT_VARIANTS_BASE, &[]),
            Some(name) => name.to_string(),
        }
    }

    fn is_active(&self, variant: Option<&str>) -> bool {
        variant == self.active
    }
}

/// Whether `matrix`, the read `tab` keeps, still holds: read at the sheet
/// edit count `sheet_epoch`, for the variant names and with the parameters
/// `project` has now.
fn read_holds(
    tab: &ProjectTabState,
    matrix: &VariantMatrix,
    project: &LoadedProject,
    sheet_epoch: u64,
) -> bool {
    tab.variant_parts_epoch == sheet_epoch
        && read_for(matrix, &project.data.variant_definitions)
        && tab.variant_parts_parameters.as_ref() == Some(&project.data.parameters)
}

/// Whether `matrix` was read for the names the project defines now.
fn read_for(matrix: &VariantMatrix, definitions: &[String]) -> bool {
    matrix.variants.len() == definitions.len()
        && matrix
            .variants
            .iter()
            .zip(definitions)
            .all(|(read, defined)| read.eq_ignore_ascii_case(defined))
}

/// The section: its title and help, then the Variant Manager filling the
/// rest of the tab.
pub(super) fn view<'a>(
    project: &LoadedProject,
    tab: Option<&ProjectTabState>,
    sheet_epoch: u64,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let manager = Manager::new(project, tab, sheet_epoch, i18n, tokens);
    let centre = if manager.definitions.is_empty() {
        empty::view(&manager)
    } else {
        main(&manager)
    };
    let body = row![list::view(&manager), centre].height(Length::Fill);
    let line = ti(tokens.border);
    let panel = ti(tokens.panel_bg);
    let frame = container(column![bar::top(&manager), body, bar::status(&manager)])
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(panel)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    column![
        text(t!(i18n, ids::PROJECT_SECTION_VARIANTS))
            .size(TS.heading)
            .color(ti(tokens.text)),
        note(t!(i18n, ids::PROJECT_VARIANTS_HELP), tokens),
        frame,
    ]
    .spacing(SP.l)
    .padding(SP.xl)
    .height(Length::Fill)
    .boxed()
}

/// The centre: the summary cards over the matrix, or where the read stands.
fn main<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let mut col: Column<Element<'a, Message>> = column![summary::view(m)]
        .spacing(SP.l)
        .padding(SP.l)
        .width(Length::Fill)
        .height(Length::Fill);
    col = col.push(match m.parts {
        Parts::Reading => note(t!(m.i18n, ids::PROJECT_VARIANTS_READING), m.tokens),
        Parts::Failed(why) => problem(read_failed(m.i18n, why), m.tokens),
        Parts::Read(matrix) => matrix::view(m, matrix),
    });
    col.boxed()
}

/// Horizontal padding of a pill, as a share of `SP.m`.
const PILL_PAD_RATIO: f32 = 0.75;

/// A rounded pill of catalog words in `tone`.
fn pill<'a>(words: Localized, tone: PillTone, tokens: &ThemeTokens) -> Element<'a, Message> {
    container(text(words).size(TS.caption).font(medium()))
        .padding([0.0, SP.m * PILL_PAD_RATIO])
        .style(crate::styles::pill(tokens, tone))
        .boxed()
}

/// A 16 × 16 stroke glyph (`currentColor`) drawn `size` square in `color`.
fn glyph<'a>(bytes: &'static [u8], size: f32, color: Color) -> Element<'a, Message> {
    svg(crate::icons::static_glyph(bytes))
        .width(size)
        .height(size)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(color),
            ..Default::default()
        })
        .boxed()
}

const GLYPH_PLUS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M8 3.5v9M3.5 8h9" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>"#;
const GLYPH_CHECK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M4 8.4l2.6 2.6L12 5.4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const GLYPH_CROSS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M5.2 5.2l5.6 5.6M10.8 5.2l-5.6 5.6" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>"#;

fn medium() -> Font {
    weighted(Weight::Medium)
}

fn semibold() -> Font {
    weighted(Weight::Semibold)
}

fn weighted(weight: Weight) -> Font {
    Font {
        weight,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// The mono face at medium weight: a designator.
fn weighted_mono() -> Font {
    Font {
        weight: Weight::Medium,
        ..mono()
    }
}

/// `color` at `share` of its strength over the panel.
fn tint_over_panel(color: Color, share: f32, tokens: &ThemeTokens) -> Color {
    crate::styles::tokens::composite_over(
        Color {
            a: color.a * share,
            ..color
        },
        ti(tokens.panel_bg),
    )
}
