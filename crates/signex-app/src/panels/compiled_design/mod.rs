//! Compiled Design panel (#1308): the project as the compiler sees it, with
//! every repeated sheet expanded into its channels and part and net counts
//! per row. A pure view over [`CompiledDesignView`], which the app builds
//! from the last compile (`app/compiled_design`); nothing here compiles.
//! Layout and states: `docs/MULTI_CHANNEL.md`, "Compiled Design panel".

mod empty;
mod nets;
#[cfg(test)]
mod tests;
mod tree;
mod variant_summary;

pub use variant_summary::{ChannelCopies, ChannelVariantSummary, MixedPart};

use std::path::PathBuf;

use iced::widget::{Space, column, container};
use iced::{Background, Element, Length, Theme, Widget as _};
use signex_i18n::{ids, t};
use signex_types::project::ChannelNaming;
use signex_types::schematic::{ChannelFormatError, ChannelOccurrence, ChannelRepeat};
use signex_widgets::property_illustration::IllustrationHover;
use signex_widgets::segmented::segmented;
use signex_widgets::{text, text_input};
use uuid::Uuid;

use super::{PanelContext, PanelMsg};
use crate::app::CompiledDesignMsg;
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The panel's mode switch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CompiledMode {
    #[default]
    Sheets,
    Nets,
}

/// Where the panel's content comes from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CompiledStatus {
    /// No sheet of a project is open.
    #[default]
    NoProject,
    /// The project has not been compiled in this session.
    NotCompiled,
    /// A compile is running and none finished before it.
    Compiling,
    Ready,
}

/// What a tree row stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompiledRowKind {
    Sheet,
    /// A repeated placement; `open` while its channel rows show.
    Repeated {
        open: bool,
    },
    Channel,
}

/// One row of the Sheets tree.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledRow {
    /// Index into the compile's sheet rows.
    pub index: usize,
    pub depth: usize,
    pub name: String,
    /// The file name, or `Repeat(CH, 1, 4)` for a repeated placement.
    pub caption: Option<String>,
    pub kind: CompiledRowKind,
    pub parts: usize,
    pub nets: usize,
    /// Its sheet changed since the compile.
    pub is_stale: bool,
    /// The active variant, on a channel row that variant leaves out
    /// entirely: the grey "Not fitted (<variant>)" tag (#2662).
    pub not_fitted_in: Option<String>,
    pub is_selected: bool,
    /// The row lists parts or nets, so it has a fold chevron.
    pub has_children: bool,
    /// Its part and net rows show below it.
    pub is_expanded: bool,
    /// The parts shown while expanded (a search keeps the matches).
    pub part_rows: Vec<CompiledChildRow>,
    /// The nets shown while expanded (a search keeps the matches).
    pub net_rows: Vec<CompiledChildRow>,
}

/// A part or net listed under a sheet instance.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledChildRow {
    pub label: String,
    pub is_power: bool,
    /// Where a click goes.
    pub target: signex_net::CompiledTarget,
}

/// One row of the Nets list.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledNetRow {
    pub name: String,
    pub sheets: usize,
    pub is_power: bool,
    /// Where a click goes.
    pub target: Option<signex_net::CompiledTarget>,
}

/// The unique totals of the footer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompiledTotals {
    pub parts: usize,
    pub nets: usize,
    pub sheets: usize,
    pub channels: usize,
}

/// What the Compiled Design panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompiledPanelView {
    pub status: CompiledStatus,
    pub mode: CompiledMode,
    pub find: String,
    /// The tree rows the search keeps, folded rows' channels left out.
    pub rows: Vec<CompiledRow>,
    /// The nets the search keeps.
    pub nets: Vec<CompiledNetRow>,
    /// Sheets changed since the compile.
    pub stale_count: usize,
    /// When the shown compile finished (`12:41`).
    pub compiled_at: String,
    pub totals: CompiledTotals,
    /// Sheet files of the project, for the footer before a compile.
    pub sheet_files: usize,
}

/// Which naming format a menu edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamingTarget {
    Designators,
    Nets,
}

/// A Repeat field of a sheet symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepeatField {
    Channel,
    From,
    To,
}

/// The open naming format menu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NamingMenuView {
    pub target: Option<NamingTarget>,
    /// The Custom format… text.
    pub custom: String,
    /// Why the typed custom format is refused.
    pub error: Option<ChannelFormatError>,
}

/// The multi-channel rows of a selected sheet symbol in Properties.
#[derive(Debug, Clone, Default)]
pub struct SheetSymbolRepeatView {
    pub sheet_symbol: Uuid,
    pub file: String,
    pub designator: String,
    /// The designator parsed as a repeat.
    pub repeat: Option<ChannelRepeat>,
    /// The Channel name, From and To fields as shown (typed text wins).
    pub channel_text: String,
    pub from_text: String,
    pub to_text: String,
    /// Enter refused the typed From / To: the box shows it is invalid.
    pub is_from_refused: bool,
    pub is_to_refused: bool,
    pub naming: ChannelNaming,
    /// The project's settings section is unreadable: naming cannot be edited.
    pub is_naming_locked: bool,
    pub menu: NamingMenuView,
    /// The symbol's entries: name and whether it is a `Repeat(…)` entry.
    pub entries: Vec<(String, bool)>,
    /// Width and height in mm, for the illustration.
    pub size_mm: (f64, f64),
    pub hover: IllustrationHover,
}

impl SheetSymbolRepeatView {
    /// The project's naming with `target`'s format replaced by `format`, for
    /// the examples of a menu.
    pub fn naming_with(&self, target: NamingTarget, format: &str) -> ChannelNaming {
        let mut naming = self.naming.clone();
        match target {
            NamingTarget::Designators => naming.designator_format = format.to_string(),
            NamingTarget::Nets => naming.net_format = format.to_string(),
        }
        naming
    }
}

/// One channel tab of the strip under a repeated sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelTabView {
    pub channel: ChannelOccurrence,
    /// Counts of the last compile, `None` before one.
    pub parts: Option<usize>,
    pub nets: Option<usize>,
    /// The sheet's designators as this channel names them, in the
    /// designator map's order.
    pub designators: Vec<String>,
    /// The card's drawing of the channel, from the last compile (#2658).
    pub miniature: Option<std::sync::Arc<signex_net::SheetMiniature>>,
    /// The channel's parts as the last compile lists them, each with its
    /// fit in [`ChannelStripView::variant`] (#2662); empty before a compile.
    pub fits: Vec<ChannelPartFit>,
}

impl ChannelTabView {
    /// How many of the channel's parts the variant leaves out.
    pub fn not_fitted(&self) -> usize {
        self.fits.iter().filter(|fit| !fit.is_fitted).count()
    }

    /// Whether the variant leaves out every part of the channel.
    pub fn is_not_fitted(&self) -> bool {
        !self.fits.is_empty() && self.not_fitted() == self.fits.len()
    }

    /// Whether the part this channel names `designator` is left out.
    pub fn is_part_not_fitted(&self, designator: &str) -> bool {
        self.fits
            .iter()
            .any(|fit| !fit.is_fitted && fit.part.designator == designator)
    }
}

/// One part of a channel and its fit in the active variant.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelPartFit {
    /// The channel's copy of the part, as the variant settings name it.
    pub part: signex_types::project::PartRef,
    pub is_fitted: bool,
}

/// The channel strip of the active sheet, when it is repeated somewhere.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChannelStripView {
    pub sheet: PathBuf,
    pub file: String,
    pub tabs: Vec<ChannelTabView>,
    /// The chosen channel's index (1-based); `None` is Editor.
    pub active: Option<u32>,
    /// The chosen channel's designator map: logical, value, channel.
    pub designator_map: Vec<(String, String, String)>,
    /// The active variant the cards show the fit in; `None` is Base, which
    /// is the sheets and is not set from a card.
    pub variant: Option<String>,
    /// The project's variant settings could not be read: a card cannot
    /// change them.
    pub is_fit_locked: bool,
}

/// Everything the multi-channel surfaces of the panels read.
#[derive(Debug, Clone, Default)]
pub struct CompiledDesignView {
    pub panel: CompiledPanelView,
    /// The selected sheet symbol, when one is selected.
    pub sheet_symbol: Option<SheetSymbolRepeatView>,
    /// The active sheet's channel strip, when it is repeated.
    pub strip: Option<ChannelStripView>,
}

pub(crate) fn msg(m: CompiledDesignMsg) -> PanelMsg {
    PanelMsg::CompiledDesign(m)
}

/// Render the panel.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let view = &ctx.compiled_design.panel;
    let is_live = view.status == CompiledStatus::Ready;
    // The title and Recompile are the dock title bar's (`title_actions`).
    let mut panel = column![controls(ctx, view, is_live), rule(ctx)]
        .width(Length::Fill)
        .height(Length::Fill);
    if is_live && view.stale_count > 0 {
        panel = panel.push(tree::stale_banner(ctx, view));
    }
    let body: Element<'a, PanelMsg> = match (view.status, view.mode) {
        (CompiledStatus::Ready, CompiledMode::Sheets) => tree::view(ctx, view),
        (CompiledStatus::Ready, CompiledMode::Nets) => nets::view(ctx, view),
        (status, _) => empty::view(ctx, status),
    };
    panel
        .push(body)
        .push(rule(ctx))
        .push(footer(ctx, view))
        .boxed()
}

/// The Sheets / Nets switch and the search field; both disabled until the
/// project is compiled.
fn controls<'a>(
    ctx: &'a PanelContext,
    view: &'a CompiledPanelView,
    is_live: bool,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let r = resolve(&ctx.tokens);
    let style = r.segmented(TS.caption).fill_width(true);
    let sheets: signex_widgets::Label<'a> =
        t!(i18n, ids::PANELS_COMPILED_DESIGN_MODE_SHEETS).into();
    let nets: signex_widgets::Label<'a> = t!(i18n, ids::PANELS_COMPILED_DESIGN_MODE_NETS).into();
    let options = [(CompiledMode::Sheets, sheets), (CompiledMode::Nets, nets)];
    let switch = segmented(options, view.mode, style, |mode| {
        msg(CompiledDesignMsg::ModePicked(mode))
    });
    let mut find = text_input(t!(i18n, ids::PANELS_COMPILED_DESIGN_FIND), &view.find)
        .style(super::form_look::text_input_style)
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .width(Length::Fill);
    if is_live {
        find = find.on_input(|s| msg(CompiledDesignMsg::FindChanged(s)));
    }
    let switch: Element<'a, PanelMsg> = if is_live {
        switch
    } else {
        signex_widgets::inert::Inert::new(switch, veil(ctx)).boxed()
    };
    container(column![switch, find].spacing(SP.s + SP.xs))
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// The panel-coloured veil over a disabled control.
fn veil(ctx: &PanelContext) -> iced::Color {
    signex_widgets::enable_section::dependents_veil(ti(ctx.tokens.panel_bg), false)
        .unwrap_or(iced::Color::TRANSPARENT)
}

/// The unique totals, or the sheet-file count before a compile.
fn footer<'a>(ctx: &'a PanelContext, view: &'a CompiledPanelView) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = resolve(&ctx.tokens).ink3;
    let line = match view.status {
        CompiledStatus::Ready => t!(
            i18n,
            ids::PANELS_COMPILED_DESIGN_FOOTER,
            "parts" => view.totals.parts,
            "nets" => view.totals.nets,
            "sheets" => view.totals.sheets,
            "channels" => view.totals.channels
        ),
        _ => t!(
            i18n,
            ids::PANELS_COMPILED_DESIGN_FOOTER_NONE,
            "files" => view.sheet_files
        ),
    };
    let chrome = ti(ctx.tokens.toolbar_bg);
    container(text(line).size(TS.caption).color(muted))
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(chrome)),
            ..container::Style::default()
        })
        .boxed()
}

/// A one-pixel soft rule across the panel.
pub(super) fn rule<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let color = resolve(&ctx.tokens).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}
