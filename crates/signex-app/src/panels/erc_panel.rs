//! The ERC panel (#85): Run ERC, the stale or clean banner, the count tiles
//! that filter the list, the findings grouped by rule, and the Waived group.
//! The rows and the Waived group are drawn in `erc_rows.rs`.

use std::collections::BTreeSet;

use signex_i18n::{ids, t};
use signex_widgets::stat_tiles::{Density, StatTile, Tone as TileTone, stat_tiles};

use super::validation_kit::{Tone, banner, hint_card, notice, run_button};
use super::*;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Flattened ERC diagnostic row for the ERC panel.
///
/// The app flattens per-sheet ERC caches into this list so the panel can
/// present one navigable table across the entire project.
#[derive(Debug, Clone, PartialEq)]
pub struct ErcDiagnosticEntry {
    pub global_index: usize,
    pub sheet_name: String,
    pub sheet_path: std::path::PathBuf,
    pub severity: ErcSeverityLite,
    pub rule_label: String,
    /// Built-in rule kind (`None` for a DSL rule): decides the row's quick
    /// action (UnusedPin places a No-Connect; the rest select and frame).
    pub rule_kind: Option<signex_erc::RuleKind>,
    pub message: String,
    pub world_x: f64,
    pub world_y: f64,
    /// `world_y` as the sheet shows it: from its page origin (#2034). The
    /// location label reads this; panning uses `world_y`.
    pub shown_y: f64,
    pub select: Option<signex_types::schematic::SelectedItem>,
    /// False when the point is not in `sheet_path`'s own coordinates (a child
    /// sheet that was not part of the run): shown, but never panned to.
    pub locatable: bool,
    /// For a net-level finding: the net to highlight and fit on click.
    pub net: Option<signex_erc::NetFinding>,
    /// Stable rule id (`builtin::..` / `user::..`), the waiver key's rule half.
    pub rule_id: String,
    /// What a per-finding waiver would match (#1285); `None` when the
    /// finding names no object and cannot be waived.
    pub waiver_key: Option<signex_erc::waivers::violation::WaiverKey>,
}

/// One stored net waiver, shown in the Waived group with a Restore button.
#[derive(Debug, Clone, PartialEq)]
pub struct ErcWaiverEntry {
    pub rule_id: String,
    pub net: String,
    /// Findings the newest run hid for it; 0 when it matched nothing (the
    /// net was renamed or no longer fails the rule).
    pub hidden: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErcSeverityLite {
    Error,
    Warning,
    Info,
}

impl ErcSeverityLite {
    pub(super) fn letter(self) -> signex_i18n::MessageId {
        match self {
            Self::Error => ids::PANELS_ERC_SEVERITY_ERROR,
            Self::Warning => ids::PANELS_ERC_SEVERITY_WARNING,
            Self::Info => ids::PANELS_ERC_SEVERITY_INFO,
        }
    }

    pub(super) fn pill(self) -> crate::styles::PillTone {
        use crate::styles::PillTone;
        match self {
            Self::Error => PillTone::Err,
            Self::Warning => PillTone::Warn,
            Self::Info => PillTone::Info,
        }
    }
}

/// What the newest ERC run checked, for the clean-run banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErcChecked {
    pub sheets: usize,
    pub symbols: usize,
    pub nets: usize,
    /// When the run finished.
    pub at: chrono::DateTime<chrono::Utc>,
}

/// What the ERC panel shows of the results: view state, never project data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ErcPanelView {
    /// The severity a tile lists alone; `None` lists every finding.
    pub only: Option<ErcSeverityLite>,
    /// Rule groups folded shut, by rule label.
    pub collapsed: BTreeSet<String>,
    /// The Waived group as the user set it; `None` is open with findings
    /// listed and shut on a clean run.
    pub waived_open: Option<bool>,
    /// The finding row under the pointer, by global index.
    pub hovered: Option<usize>,
}

impl ErcPanelView {
    /// Whether the Waived group is open, given whether any finding is listed.
    pub fn is_waived_open(&self, has_findings: bool) -> bool {
        self.waived_open.unwrap_or(has_findings)
    }
}

/// What the user did in the ERC panel's lists.
#[derive(Debug, Clone, PartialEq)]
pub enum ErcPanelMsg {
    /// A count tile was pressed: list only that severity, or all again.
    TileToggled(ErcSeverityLite),
    /// A rule group's heading was pressed: fold or unfold it.
    GroupToggled(String),
    /// The Waived group's heading was pressed.
    WaivedToggled,
    /// The pointer entered a finding row.
    RowEntered(usize),
    /// The pointer left a finding row.
    RowLeft(usize),
}

impl ErcPanelView {
    /// Apply one panel event. `has_findings` is whether a finding is listed,
    /// for the Waived group's default.
    pub fn apply(&mut self, msg: ErcPanelMsg, has_findings: bool) {
        match msg {
            ErcPanelMsg::TileToggled(severity) => {
                self.only = (self.only != Some(severity)).then_some(severity);
            }
            ErcPanelMsg::GroupToggled(rule) => {
                if !self.collapsed.remove(&rule) {
                    self.collapsed.insert(rule);
                }
            }
            ErcPanelMsg::WaivedToggled => {
                self.waived_open = Some(!self.is_waived_open(has_findings));
            }
            ErcPanelMsg::RowEntered(index) => self.hovered = Some(index),
            ErcPanelMsg::RowLeft(index) => {
                if self.hovered == Some(index) {
                    self.hovered = None;
                }
            }
        }
    }
}

/// The findings of one rule, most severe first.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ErcGroup<'e> {
    pub rule: &'e str,
    /// The most severe finding of the group.
    pub severity: ErcSeverityLite,
    pub rows: Vec<&'e ErcDiagnosticEntry>,
}

/// `entries` grouped by rule, keeping only `only` when set. Groups are
/// ordered by their most severe finding, then by where the rule first
/// shows; a group with nothing left is not listed.
pub(crate) fn erc_groups(
    entries: &[ErcDiagnosticEntry],
    only: Option<ErcSeverityLite>,
) -> Vec<ErcGroup<'_>> {
    let mut groups: Vec<ErcGroup<'_>> = Vec::new();
    for entry in entries
        .iter()
        .filter(|e| only.is_none_or(|s| e.severity == s))
    {
        match groups.iter_mut().find(|g| g.rule == entry.rule_label) {
            Some(group) => {
                group.severity = group.severity.min(entry.severity);
                group.rows.push(entry);
            }
            None => groups.push(ErcGroup {
                rule: entry.rule_label.as_str(),
                severity: entry.severity,
                rows: vec![entry],
            }),
        }
    }
    // Stable: equal severities keep the order the rules first show in.
    groups.sort_by_key(|g| g.severity);
    groups
}

/// Errors, warnings and infos among `entries`.
pub(crate) fn erc_counts(entries: &[ErcDiagnosticEntry]) -> [usize; 3] {
    let count = |s| entries.iter().filter(|e| e.severity == s).count();
    [
        count(ErcSeverityLite::Error),
        count(ErcSeverityLite::Warning),
        count(ErcSeverityLite::Info),
    ]
}

/// Label and detail text colours of an ERC result row. A focused row is
/// filled with the selection colour, so its text is chosen against that.
pub(crate) fn erc_row_text(tokens: &ThemeTokens, focused: bool) -> (Color, Color) {
    let label = theme_ext::text_primary(tokens);
    let detail = theme_ext::text_secondary(tokens);
    if !focused {
        return (label, detail);
    }
    let fill = theme_ext::selection_color(tokens);
    let readable = |c| crate::styles::tokens::keep_readable_on(c, fill);
    (readable(label), readable(detail))
}

/// Colour of a row's severity letter: on a focused row it is checked
/// against the selection fill like the rest of the row's text.
pub(crate) fn erc_severity_text(tokens: &ThemeTokens, focused: bool, severity: Color) -> Color {
    if focused {
        crate::styles::tokens::keep_readable_on(severity, theme_ext::selection_color(tokens))
    } else {
        severity
    }
}

fn erc_msg(msg: ErcPanelMsg) -> PanelMsg {
    PanelMsg::ErcView(msg)
}

/// The three count tiles; a tile with findings filters the list, a zero
/// tile is a neutral summary.
fn tiles<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let [errors, warnings, infos] = erc_counts(&ctx.erc_diagnostics);
    let only = ctx.validation.erc.only;
    let tile = |count: usize, label, tone, severity| {
        let n = count as u32;
        let tile = StatTile::count(n, label)
            .tone(tone)
            .on(only == Some(severity));
        if count > 0 {
            tile.on_press(erc_msg(ErcPanelMsg::TileToggled(severity)))
        } else {
            tile
        }
    };
    stat_tiles(
        vec![
            tile(
                errors,
                t!(i18n, ids::PANELS_ERC_TILE_ERRORS, "count" => errors as i64),
                TileTone::Err,
                ErcSeverityLite::Error,
            ),
            tile(
                warnings,
                t!(i18n, ids::PANELS_ERC_TILE_WARNINGS, "count" => warnings as i64),
                TileTone::Warn,
                ErcSeverityLite::Warning,
            ),
            tile(
                infos,
                t!(i18n, ids::PANELS_ERC_TILE_INFO),
                TileTone::Info,
                ErcSeverityLite::Info,
            ),
        ],
        Density::Compact,
        &ctx.tokens,
        TS.body,
        i18n,
    )
}

/// The ok banner of a clean run: what it checked and when.
fn clean_banner<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let mut words = row![
        text(t!(i18n, ids::PANELS_ERC_CLEAN))
            .size(TS.caption)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
            })
            .color(theme_ext::text_primary(tokens))
    ]
    .spacing(SP.xs);
    if let Some(checked) = ctx.validation.erc_checked {
        words = words.push(
            text(t!(
                i18n, ids::PANELS_ERC_CLEAN_DETAIL,
                "sheets" => checked.sheets as i64,
                "symbols" => checked.symbols as i64,
                "nets" => checked.nets as i64,
                "time" => super::messages_row::clock_minutes(checked.at)
            ))
            .size(TS.caption)
            .color(theme_ext::text_primary(tokens))
            .boxed(),
        );
    }
    banner(
        Tone::Ok,
        words.wrap().vertical_spacing(SP.xs).boxed(),
        None,
        tokens,
    )
}

/// The panel: Run ERC, the state banners and tiles, the rule groups and the
/// Waived group.
pub fn view_erc<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let (label, key) = if ctx.erc_running {
        (t!(i18n, ids::PANELS_ERC_RUNNING), "")
    } else {
        (
            t!(i18n, ids::PANELS_ERC_RUN_BUTTON),
            ctx.validation.run_erc_key.as_str(),
        )
    };
    let mut top: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.s)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .push(run_button(
            label,
            key,
            (!ctx.erc_running).then_some(PanelMsg::RunErc),
            tokens,
        ));
    let has_findings = !ctx.erc_diagnostics.is_empty();
    if !ctx.erc_ran && !has_findings {
        if !ctx.erc_running {
            top = top.push(hint_card(t!(i18n, ids::PANELS_ERC_EMPTY_HINT), tokens));
        }
    } else {
        if ctx.erc_results_stale {
            top = top.push(banner(
                Tone::Warn,
                notice(t!(i18n, ids::PANELS_ERC_STALE_BANNER), None, tokens),
                None,
                tokens,
            ));
        } else if !has_findings {
            top = top.push(clean_banner(ctx));
        }
        if ctx.erc_results_stale && !has_findings {
            top = top.push(
                text(t!(i18n, ids::PANELS_ERC_NO_FINDINGS_LEFT))
                    .size(TS.caption)
                    .color(theme_ext::text_secondary(tokens))
                    .boxed(),
            );
        }
        top = top.push(tiles(ctx));
    }
    let mut col: Column<Element<'a, PanelMsg>> =
        Column::new().width(Length::Fill).push(top.boxed());
    for group in erc_groups(&ctx.erc_diagnostics, ctx.validation.erc.only) {
        col = col.push(super::erc_rows::group(ctx, group));
    }
    col = col.push(super::erc_rows::waived_group(ctx, has_findings));
    col.boxed()
}

#[cfg(test)]
#[path = "erc_panel_tests.rs"]
mod tests;
