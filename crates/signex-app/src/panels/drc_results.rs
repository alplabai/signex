//! The DRC panel of a board (#87, #88, #2399). Before the first run it is
//! the idle state (`drc_idle.rs`); after it, Run DRC with Rules, the count
//! tiles that filter the list, the notices, the rule / layer / severity
//! dropdowns and Show waived, then the violations grouped by rule
//! (`drc_rows.rs`). A pure view over [`DrcPanel`]; every action is a
//! [`DrcPanelMsg`] the app runs through the same path as its catalog command.

use std::collections::BTreeSet;

use iced::widget::{column, pick_list, stack, text_editor};
use signex_drc::ViolationKind;
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::pcb_layer::CopperLayer;
use signex_types::violation::Severity;
use signex_widgets::dropdown_style::{FIELD_TEXT_SIZE, GraphiteDropdown as _, pick_list_style};
use signex_widgets::stat_tiles::{Density, StatTile, Tone as TileTone, stat_tiles};
use signex_widgets::{CheckboxLabel as _, checkbox};

use super::drc::{layer_label, row_text};
use super::drc_idle::{BaseRule, base_rules, default_clearance, idle_top, rules_button};
use super::validation_kit::{Tone, banner, link, link_ink, notice, run_button};
use super::waiver_form::WaiverReason;
use super::*;
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The prefix of a written rule's id; its name follows.
pub(super) const USER_RULE_PREFIX: &str = "user::";

/// What the panel shows of a report. View state: never board data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrcFilter {
    /// One rule id, or every rule.
    pub rule: Option<String>,
    /// One copper layer, or every layer.
    pub layer: Option<CopperLayer>,
    /// One severity, or every severity.
    pub severity: Option<Severity>,
    /// Waived violations are listed too (with their note).
    pub show_waived: bool,
}

impl DrcFilter {
    /// Whether `row` passes the filter.
    pub fn shows(&self, row: &DrcRow) -> bool {
        (self.show_waived || row.waiver.is_none())
            && self.rule.as_ref().is_none_or(|rule| *rule == row.rule)
            && self.layer.is_none_or(|layer| row.layers.contains(&layer))
            && self.severity.is_none_or(|s| row.severity == Some(s))
    }
}

/// A count tile's category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrcCategory {
    Clearance,
    Short,
    Unconnected,
    Keepout,
}

impl DrcCategory {
    /// The tile a violation counts under; `None` for the kinds no tile counts.
    pub fn of(kind: ViolationKind) -> Option<Self> {
        match kind {
            ViolationKind::Clearance => Some(Self::Clearance),
            ViolationKind::Short => Some(Self::Short),
            ViolationKind::Unconnected => Some(Self::Unconnected),
            ViolationKind::Keepout => Some(Self::Keepout),
            _ => None,
        }
    }

    fn words(self) -> (signex_i18n::MessageId, signex_i18n::MessageId) {
        match self {
            Self::Clearance => (
                ids::DRC_PANEL_TILE_CLEARANCE,
                ids::DRC_PANEL_CATEGORY_CLEARANCE,
            ),
            Self::Short => (ids::DRC_PANEL_TILE_SHORT, ids::DRC_PANEL_CATEGORY_SHORT),
            Self::Unconnected => (
                ids::DRC_PANEL_TILE_UNCONNECTED,
                ids::DRC_PANEL_CATEGORY_UNCONNECTED,
            ),
            Self::Keepout => (ids::DRC_PANEL_TILE_KEEPOUT, ids::DRC_PANEL_CATEGORY_KEEPOUT),
        }
    }
}

/// The tiles, folded groups and row hover and selection (#88). View state:
/// never board data. No tile is on at first, so the list matches what the
/// filters say.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrcView {
    /// The tile that lists its category alone.
    pub category: Option<DrcCategory>,
    /// Rule groups folded shut, by rule id.
    pub collapsed: BTreeSet<String>,
    /// The idle base-rule list is folded shut.
    pub base_rules_closed: bool,
    /// The row under the pointer, and the row last picked, by waiver key.
    pub hovered: Option<String>,
    pub selected: Option<String>,
}

impl DrcView {
    /// Whether `row` is in the tile's category (every row with no tile on).
    pub fn shows(&self, row: &DrcRow) -> bool {
        self.category
            .is_none_or(|c| DrcCategory::of(row.kind) == Some(c))
    }
}

/// The waiver being written for the violation `key`: one required reason;
/// the author comes from git config (mockup row 88).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrcWaiveDraft {
    pub key: String,
    pub reason: WaiverReason,
    pub author: String,
    /// The git identity is still being read: Waive waits for it, so a
    /// waiver is never stored without the author it would have had.
    pub awaits_identity: bool,
    /// The last Waive was pressed with the reason empty.
    pub blank: bool,
}

/// One entry of a filter dropdown; `None` is "all".
#[derive(Debug, Clone, PartialEq)]
pub enum DrcFilterChoice {
    Rule(Option<String>),
    Layer(Option<CopperLayer>),
    Severity(Option<Severity>),
}

/// What the user did in the DRC panel. Violations are named by their
/// waiver key, the same in every run.
#[derive(Debug, Clone, PartialEq)]
pub enum DrcPanelMsg {
    /// A row (or its Show) was clicked: select its objects and centre the
    /// view on it.
    RowPicked(String),
    /// A filter dropdown was set.
    FilterPicked(DrcFilterChoice),
    /// Show waived was ticked or cleared.
    WaivedShownToggled(bool),
    /// A count tile was pressed: list only its category, or all again.
    TileToggled(DrcCategory),
    /// "Clear filter" under the tiles.
    TileCleared,
    /// A rule group's heading was pressed: fold or unfold it.
    GroupToggled(String),
    /// "The base rules" heading was pressed.
    BaseRulesToggled,
    /// The pointer entered or left a row.
    RowEntered(String),
    RowLeft(String),
    /// A group's Rule link: open that rule in the Rules tab.
    RuleOpened(String),
    /// "Write it" or "Set it in Rules": open the Rules tab where the base
    /// rule is written.
    BaseRuleOpened(BaseRule),
    /// "see Messages" in a notice: bring the Messages panel forward.
    MessagesShown,
    /// Waive was pressed on a row: the Reason form opens under it.
    WaiveStarted(String),
    /// The Reason field was typed in, or its cursor moved.
    ReasonActed(text_editor::Action),
    /// The form's Waive: stored with the reason and the git author.
    WaiveConfirmed,
    /// The form's Cancel: nothing is stored.
    WaiveCancelled,
    /// Unwaive was pressed on a waived row.
    Unwaived(String),
}

pub(super) fn drc_msg(msg: DrcPanelMsg) -> PanelMsg {
    PanelMsg::Drc(msg)
}

/// The rows grouped by rule id, groups in the order their first row comes
/// (the report's order: most severe kind first).
pub(crate) fn groups<'r>(rows: &[&'r DrcRow]) -> Vec<(&'r str, Vec<&'r DrcRow>)> {
    let mut out: Vec<(&'r str, Vec<&'r DrcRow>)> = Vec::new();
    for row in rows {
        match out.iter_mut().find(|(rule, _)| *rule == row.rule) {
            Some((_, members)) => members.push(*row),
            None => out.push((row.rule.as_str(), vec![*row])),
        }
    }
    out
}

/// The rows the filters, dropdowns and the tile let through. A group with
/// none of them is not listed.
pub(crate) fn shown_rows(drc: &DrcPanel) -> Vec<&DrcRow> {
    drc.rows
        .iter()
        .filter(|r| drc.filter.shows(r) && drc.view.shows(r))
        .collect()
}

/// A rule as the panel names it: a written rule by its name, a built-in one
/// by what it checks.
pub(super) fn rule_label(i18n: &I18n, row: &DrcRow) -> String {
    match row.rule.strip_prefix(USER_RULE_PREFIX) {
        Some(name) => name.to_string(),
        None => row_text(i18n, row).0.into_string(),
    }
}

fn severity_label(i18n: &I18n, severity: Option<Severity>) -> Localized {
    match severity {
        Some(Severity::Error) => t!(i18n, ids::DRC_PANEL_SEVERITY_ERROR),
        Some(Severity::Warning) => t!(i18n, ids::DRC_PANEL_SEVERITY_WARNING),
        Some(Severity::Info) => t!(i18n, ids::DRC_PANEL_SEVERITY_INFO),
        None => t!(i18n, ids::DRC_PANEL_SEVERITY_OFF),
    }
}

/// The text of one dropdown entry.
fn choice_label(i18n: &I18n, drc: &DrcPanel, choice: &DrcFilterChoice) -> String {
    match choice {
        DrcFilterChoice::Rule(None) => t!(i18n, ids::DRC_PANEL_FILTER_ALL_RULES).into_string(),
        DrcFilterChoice::Rule(Some(rule)) => drc
            .rows
            .iter()
            .find(|row| row.rule == *rule)
            .map_or_else(|| rule.clone(), |row| rule_label(i18n, row)),
        DrcFilterChoice::Layer(None) => t!(i18n, ids::DRC_PANEL_FILTER_ALL_LAYERS).into_string(),
        DrcFilterChoice::Layer(layer) => layer_label(i18n, *layer)
            .map(Localized::into_string)
            .unwrap_or_default(),
        DrcFilterChoice::Severity(None) => {
            t!(i18n, ids::DRC_PANEL_FILTER_ALL_SEVERITIES).into_string()
        }
        DrcFilterChoice::Severity(severity) => severity_label(i18n, *severity).into_string(),
    }
}

/// Room right of a filter dropdown's label for its arrow and a gap.
const FILTER_ARROW_ROOM: f32 = FIELD_TEXT_SIZE + SP.xs;

/// A filter dropdown's padding: the field's own, widened on the right by
/// the arrow's room. The pick list clips its label at the right padding
/// but draws its own arrow inside it, so the arrow is drawn over the field
/// instead and a long label ("All severities") ends before it (mockup
/// row 88).
pub(super) fn filter_field_padding() -> iced::Padding {
    let base = iced::widget::button::DEFAULT_PADDING;
    iced::Padding {
        right: base.right + FILTER_ARROW_ROOM,
        ..base
    }
}

/// The arrow of a filter dropdown, at the field's right padding in the
/// dropdown's own glyph and colour.
fn filter_arrow<'a>() -> Element<'a, PanelMsg> {
    use iced::advanced::text::Renderer as _;
    let arrow = <iced::Renderer>::ARROW_DOWN_ICON.to_string();
    container(
        text(signex_i18n::UserText::new(arrow))
            .font(<iced::Renderer>::ICON_FONT)
            .size(FIELD_TEXT_SIZE)
            .shaping(iced::widget::text::Shaping::Basic)
            .style(|theme: &Theme| iced::widget::text::Style {
                color: Some(pick_list_style(theme, pick_list::Status::Active).handle_color),
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Right)
    .align_y(iced::alignment::Vertical::Center)
    .padding(iced::Padding {
        right: iced::widget::button::DEFAULT_PADDING.right,
        ..iced::Padding::ZERO
    })
    .boxed()
}

/// One compact row of the three dropdowns, then Show waived with the
/// waived count beside it.
fn filters<'a>(ctx: &'a PanelContext, drc: &'a DrcPanel) -> Element<'a, PanelMsg> {
    let i18n: &'a I18n = &ctx.i18n;
    let mut rules = vec![DrcFilterChoice::Rule(None)];
    let mut layers = vec![DrcFilterChoice::Layer(None)];
    for row in &drc.rows {
        let rule = DrcFilterChoice::Rule(Some(row.rule.clone()));
        if !rules.contains(&rule) {
            rules.push(rule);
        }
        for layer in &row.layers {
            let layer = DrcFilterChoice::Layer(Some(*layer));
            if !layers.contains(&layer) {
                layers.push(layer);
            }
        }
    }
    let severities = vec![
        DrcFilterChoice::Severity(None),
        DrcFilterChoice::Severity(Some(Severity::Error)),
        DrcFilterChoice::Severity(Some(Severity::Warning)),
        DrcFilterChoice::Severity(Some(Severity::Info)),
    ];
    let label = move |choice: &DrcFilterChoice| choice_label(i18n, drc, choice);
    let dropdown = |selected: DrcFilterChoice, options: Vec<DrcFilterChoice>| {
        let field = pick_list(Some(selected), options, label)
            .graphite()
            .handle(pick_list::Handle::None)
            .padding(filter_field_padding())
            .on_select(|choice: DrcFilterChoice| drc_msg(DrcPanelMsg::FilterPicked(choice)))
            .width(Length::Fill);
        stack![field, filter_arrow()].width(Length::Fill).boxed()
    };
    let filter = &drc.filter;
    let waived = drc.rows.iter().filter(|r| r.waiver.is_some()).count();
    let mut show_waived = row![
        checkbox(filter.show_waived)
            .size(TS.body)
            .labelled(t!(i18n, ids::DRC_PANEL_SHOW_WAIVED))
            .text_size(TS.caption)
            .style(super::form_look::checkbox_style)
            .on_toggle(|on| drc_msg(DrcPanelMsg::WaivedShownToggled(on)))
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center);
    if waived > 0 {
        show_waived = show_waived.push(
            text(t!(i18n, ids::DRC_PANEL_SHOW_WAIVED_COUNT, "count" => waived))
                .size(TS.caption)
                .color(crate::styles::tokens::resolve(&ctx.tokens).ink4)
                .boxed(),
        );
    }
    column![
        row![
            dropdown(DrcFilterChoice::Rule(filter.rule.clone()), rules),
            dropdown(DrcFilterChoice::Layer(filter.layer), layers),
            dropdown(DrcFilterChoice::Severity(filter.severity), severities),
        ]
        .spacing(SP.xs),
        show_waived,
    ]
    .spacing(SP.s)
    .width(Length::Fill)
    .boxed()
}

/// The four count tiles that filter by category, and the line under them:
/// the hint, or what is listed and "Clear filter".
fn tiles<'a>(ctx: &'a PanelContext, drc: &DrcPanel) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let on = drc.view.category;
    let tile = |category: DrcCategory, count: usize, tone| {
        let tile = StatTile::count(count as u32, t!(i18n, category.words().0))
            .tone(tone)
            .on(on == Some(category));
        if count > 0 {
            tile.on_press(drc_msg(DrcPanelMsg::TileToggled(category)))
        } else {
            tile
        }
    };
    let row_of_tiles = stat_tiles(
        vec![
            tile(DrcCategory::Clearance, drc.clearance, TileTone::Err),
            tile(DrcCategory::Short, drc.short, TileTone::Err),
            tile(DrcCategory::Unconnected, drc.unconnected, TileTone::Warn),
            tile(DrcCategory::Keepout, drc.keepout, TileTone::Err),
        ],
        Density::Compact,
        &ctx.tokens,
        TS.body,
        i18n,
    );
    let ink4 = crate::styles::tokens::resolve(&ctx.tokens).ink4;
    let under = match on {
        None => text(t!(i18n, ids::DRC_PANEL_TILES_HINT))
            .size(TS.caption)
            .color(ink4)
            .boxed(),
        Some(category) => row![
            text(t!(
                i18n, ids::DRC_PANEL_TILES_SHOWING,
                "category" => t!(i18n, category.words().1).into_string()
            ))
            .size(TS.caption)
            .color(ink4),
            link(
                t!(i18n, ids::DRC_PANEL_CLEAR_FILTER),
                drc_msg(DrcPanelMsg::TileCleared),
                link_ink(&ctx.tokens),
            ),
        ]
        .spacing(SP.s)
        .wrap()
        .boxed(),
    };
    column![row_of_tiles, under].spacing(SP.xs).boxed()
}

/// A warn notice that ends with "see Messages".
fn see_messages<'a>(ctx: &'a PanelContext, words: Localized) -> Element<'a, PanelMsg> {
    let see = link(
        t!(ctx.i18n, ids::DRC_PANEL_SEE_MESSAGES),
        drc_msg(DrcPanelMsg::MessagesShown),
        crate::styles::ti(ctx.tokens.warning),
    );
    banner(
        Tone::Warn,
        notice(words, Some(see), &ctx.tokens),
        None,
        &ctx.tokens,
    )
}

/// The notices, stacked above the filters: rules not applied, rule text
/// unparsed, keepouts or items not checked, stale (with Run DRC), the
/// default clearance, and a clean board.
fn notices<'a>(
    ctx: &'a PanelContext,
    drc: &DrcPanel,
    mut col: Column<Element<'a, PanelMsg>>,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    if drc.rules_not_applied > 0 {
        let words = t!(i18n, ids::DRC_PANEL_RULES_NOT_APPLIED, "count" => drc.rules_not_applied);
        col = col.push(see_messages(ctx, words));
    }
    if drc.rule_text_unparsed {
        col = col.push(see_messages(
            ctx,
            t!(i18n, ids::DRC_PANEL_RULE_TEXT_UNPARSED),
        ));
    }
    if drc.skipped_keepouts > 0 {
        let words = t!(i18n, ids::DRC_PANEL_KEEPOUTS_SKIPPED, "count" => drc.skipped_keepouts);
        col = col.push(see_messages(ctx, words));
    }
    if drc.skipped > 0 {
        let words = t!(i18n, ids::DRC_SKIPPED, "count" => drc.skipped);
        col = col.push(banner(
            Tone::Warn,
            notice(words, None, tokens),
            None,
            tokens,
        ));
    }
    if drc.stale {
        let again = dialog_button(
            t!(i18n, ids::DRC_RUN),
            DialogButtonKind::Primary,
            (!drc.running).then_some(PanelMsg::RunDrc),
            tokens,
        );
        col = col.push(banner(
            Tone::Warn,
            notice(t!(i18n, ids::DRC_STALE), None, tokens),
            Some(again),
            tokens,
        ));
    }
    if let Some(note) = default_clearance(ctx, drc) {
        col = col.push(note);
    }
    if drc.rows.is_empty() && drc.skipped == 0 && drc.skipped_keepouts == 0 {
        col = col.push(banner(
            Tone::Ok,
            notice(t!(i18n, ids::DRC_CLEAN), None, tokens),
            None,
            tokens,
        ));
    }
    col
}

/// The top of the panel after a run.
fn results_top<'a>(ctx: &'a PanelContext, drc: &'a DrcPanel) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let (label, on_press) = if drc.running {
        (t!(i18n, ids::DRC_RUNNING), None)
    } else {
        (t!(i18n, ids::DRC_RUN), Some(PanelMsg::RunDrc))
    };
    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.s)
        .push(
            row![
                run_button(label, "", on_press, &ctx.tokens),
                rules_button(ctx)
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .boxed(),
        )
        .push(tiles(ctx, drc));
    col = notices(ctx, drc, col);
    col.push(filters(ctx, drc)).boxed()
}

/// The panel for the board in view.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let Some(drc) = ctx.pcb_drc.as_ref() else {
        return container(
            text(t!(i18n, ids::DRC_PANEL_NO_BOARD))
                .size(TS.caption)
                .color(muted),
        )
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed();
    };
    let pad = [SP.s, SP.m];
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    if !drc.ran {
        col = col.push(container(idle_top(ctx, drc)).padding(pad).boxed());
        if !drc.running {
            col = col.push(base_rules(ctx, drc));
        }
        return col.boxed();
    }
    col = col.push(container(results_top(ctx, drc)).padding(pad).boxed());
    let shown = shown_rows(drc);
    if shown.is_empty() && !drc.rows.is_empty() {
        col = col.push(
            container(
                text(t!(i18n, ids::DRC_PANEL_NONE_SHOWN))
                    .size(TS.caption)
                    .color(muted),
            )
            .padding(pad)
            .boxed(),
        );
    }
    for (_, members) in groups(&shown) {
        col = col.push(super::drc_rows::group(ctx, drc, members));
    }
    if drc.hidden > 0 {
        col = col.push(
            container(
                text(t!(i18n, ids::DRC_MORE, "count" => drc.hidden))
                    .size(TS.caption)
                    .color(muted),
            )
            .padding(pad)
            .boxed(),
        );
    }
    col.boxed()
}

#[cfg(test)]
#[path = "drc_results_tests.rs"]
mod tests;
