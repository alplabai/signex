//! Properties of a selected directive (#2420, #1281, #1489, mockup row 49). A Net
//! Parameters directive and a Net Blanket have their own surfaces
//! (`net_parameters.rs`, `blanket.rs`), read from the panel's
//! [`DirectivePanel`] snapshot; a Compile Mask and a Differential Pair show
//! their Location. A Net Blanket keeps its per-rule ERC exclusions here, in
//! the Suppressed rules section it shares with the No-ERC directive.
//! Typed values are one undo step per typing run (the engine's typing run);
//! each checkbox, pick or segment is one step.
//! Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use super::super::*;
use super::rows::PanelStyle;
use crate::panels::SelectionFormMsg;
use crate::panels::multi_selection::{counted_section, inset_caption, rule_matches};
use iced::Widget;
use signex_i18n::{ids, t};
use signex_types::schematic::{BlanketPour, PageOrigin, SchDirective, SchematicSheet};
use signex_widgets::enable_section::{EnableSection, dependents_veil};
use signex_widgets::inert::Inert;
use signex_widgets::property_illustration::{IllustrationHover, IllustrationMsg};

/// One Properties edit of a Net Blanket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlanketEdit {
    /// The net class, as typed or picked; blank sets none.
    NetClass(String),
    /// Report (`false`) or do not report (`true`) the rule `rule_id` inside.
    Rule {
        rule_id: String,
        excluded: bool,
    },
    /// The track width as typed, mm; blank sets none.
    TrackWidth(String),
    Pour(BlanketPour),
    IncludeEdgeNets(bool),
    /// A corner or a side of the outline as typed, mm.
    Outline(OutlineField, String),
}

/// A field of a Net Blanket's Outline section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineField {
    X,
    Y,
    Width,
    Height,
}

/// A routing rule of a Net Parameters directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetRule {
    Width,
    Clearance,
    ViaDiameter,
    ViaHole,
    Current,
}

/// One Properties edit of a Net Parameters directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetParametersEdit {
    NetClass(Option<String>),
    DiffPairClass(Option<String>),
    /// A rule value as typed; blank sets none.
    Rule(NetRule, String),
    ShowOnFlag(bool),
}

/// A typed field of a directive's Properties, whose text is kept while it
/// is typed so a half-typed number (`0.`) is not reformatted under the
/// cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftField {
    Outline(OutlineField),
    TrackWidth,
    Rule(NetRule),
}

/// What happened in a directive's Properties, under `PanelMsg::Directive`.
#[derive(Debug, Clone, PartialEq)]
pub enum DirectivePanelMsg {
    NetParametersEdited(uuid::Uuid, NetParametersEdit),
    /// The pointer moved onto (`Some`) or off (`None`) a net row of a Net
    /// Blanket's list; the net lights on the sheet.
    NetRowHovered(Option<String>),
    /// Hover on the illustration or on a linked field row.
    Illustration(IllustrationMsg),
}

/// How a Net Blanket holds a listed net.
pub use signex_net::BlanketCatch;

/// One net of a Net Blanket's "Nets under the blanket" list.
#[derive(Debug, Clone, PartialEq)]
pub struct BlanketNetRow {
    pub name: String,
    pub catch: BlanketCatch,
    /// Its pins, `U1 PH, L1 1`, cut after the first few.
    pub members: String,
    /// Its wires and junctions, lit on the sheet while the row is hovered.
    pub elements: Vec<uuid::Uuid>,
}

/// The selected Net Parameters directive or Net Blanket, as its Properties
/// read it; rebuilt with the selection, keeping the hover and a typed draft
/// while the same directive stays selected.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectivePanel {
    pub directive: SchDirective,
    /// The sheet's file name.
    pub sheet: String,
    /// The net a Net Parameters directive sits on, when on a wire.
    pub net: Option<String>,
    /// Whether that net is one half of a pair (both halves on the sheet).
    pub in_pair: bool,
    /// A Net Blanket's nets, caught or crossing its edge.
    pub nets: Vec<BlanketNetRow>,
    /// The wires around a Net Blanket, for its illustration: ends in sheet
    /// mm and whether their net is caught.
    pub wires: Vec<(
        signex_types::schematic::Point,
        signex_types::schematic::Point,
        bool,
    )>,
    /// The project's net classes and diff-pair classes, by name.
    pub net_classes: Vec<String>,
    pub diff_pair_classes: Vec<String>,
    /// How the sheet shows Y: a typed Y maps through it.
    pub page: (PageOrigin, f64),
    pub draft: Option<(DraftField, String)>,
    /// The draft is text its field refuses, so its box shows it is invalid.
    pub draft_refused: bool,
    pub hover: IllustrationHover,
    pub hovered_net: Option<String>,
}

/// How many pins a net row lists before it cuts.
const MEMBERS_SHOWN: usize = 3;

impl DirectivePanel {
    /// The panel of directive `id` on `sheet` (file name `sheet_name`);
    /// `None` for a directive with no surface of its own or none at all.
    pub fn build(
        sheet: &SchematicSheet,
        id: uuid::Uuid,
        sheet_name: &str,
        net_classes: Vec<String>,
        diff_pair_classes: Vec<String>,
    ) -> Option<Self> {
        let directive = sheet.directives.iter().find(|d| d.uuid() == id)?.clone();
        if !matches!(
            directive,
            SchDirective::NetParameters { .. } | SchDirective::Blanket { .. }
        ) {
            return None;
        }
        let netlist = signex_net::build_netlist(sheet);
        let mut net = None;
        let mut in_pair = false;
        if let SchDirective::NetParameters { .. } = directive
            && let Some(on) = signex_net::net_parameters(sheet, &netlist)
                .into_iter()
                .find(|on| on.directive == id)
        {
            in_pair =
                signex_types::schematic::diff_pair_halves(&on.net_name, true).is_some_and(|h| {
                    [h.positive, h.negative]
                        .iter()
                        .all(|n| netlist.nets.iter().any(|net| &net.name == n))
                });
            net = Some(on.net_name);
        }
        let nets = signex_net::blanket_nets(sheet, &netlist, &directive)
            .into_iter()
            .filter_map(|row| {
                let net = netlist.nets.iter().find(|n| n.id == row.net)?;
                let mut members: Vec<String> = net
                    .terminals
                    .iter()
                    .take(MEMBERS_SHOWN)
                    .map(|t| format!("{} {}", t.reference, t.pin))
                    .collect();
                if net.terminals.len() > MEMBERS_SHOWN {
                    members.push("…".to_string());
                }
                Some(BlanketNetRow {
                    name: row.name,
                    catch: row.catch,
                    members: members.join(", "),
                    elements: net.wires.iter().chain(&net.junctions).copied().collect(),
                })
            })
            .collect::<Vec<BlanketNetRow>>();
        let wires = blanket_wires(sheet, &directive, &nets);
        Some(Self {
            directive,
            sheet: sheet_name.to_string(),
            net,
            in_pair,
            nets,
            wires,
            net_classes,
            diff_pair_classes,
            page: (
                sheet.page.origin,
                signex_types::schematic::paper_height_mm(&sheet.paper_size),
            ),
            draft: None,
            draft_refused: false,
            hover: IllustrationHover::default(),
            hovered_net: None,
        })
    }

    /// The text a typed field shows: the draft while it is typed, else
    /// `value` as stored (blank when unset).
    pub fn shown(&self, field: DraftField, value: Option<f64>) -> String {
        match &self.draft {
            Some((typed, text)) if *typed == field => text.clone(),
            _ => value.map(format_value).unwrap_or_default(),
        }
    }

    /// The error colour while `field` holds text it refuses (#2767).
    pub(super) fn refused_ink(&self, field: DraftField, ctx: &PanelContext) -> Option<Color> {
        let holds_draft = matches!(&self.draft, Some((typed, _)) if *typed == field);
        (self.draft_refused && holds_draft).then(|| crate::styles::ti(ctx.tokens.error))
    }

    /// The caught nets: inside, or crossing the edge when the blanket
    /// includes such nets.
    pub fn caught_count(&self) -> usize {
        self.nets.iter().filter(|n| n.catch.is_caught()).count()
    }
}

/// How far around a Net Blanket its illustration shows the sheet, as a
/// share of the outline's larger side.
const AROUND: f64 = 0.35;

/// The wires near a Net Blanket's outline, each marked caught or not.
fn blanket_wires(
    sheet: &SchematicSheet,
    directive: &SchDirective,
    nets: &[BlanketNetRow],
) -> Vec<(
    signex_types::schematic::Point,
    signex_types::schematic::Point,
    bool,
)> {
    let Some(region) = directive.region() else {
        return Vec::new();
    };
    let margin = region.width().max(region.height()) * AROUND;
    let near = region.expand(margin);
    sheet
        .wires
        .iter()
        .filter(|w| near.contains(w.start.x, w.start.y) || near.contains(w.end.x, w.end.y))
        .map(|w| {
            let caught = nets
                .iter()
                .any(|n| n.catch.is_caught() && n.elements.contains(&w.uuid));
            (w.start, w.end, caught)
        })
        .collect()
}

/// A stored value as a field shows it: no trailing zeros past the point.
pub fn format_value(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

/// The rule list of a directive, as the Suppressed rules section shows it.
pub(super) struct RuleList<'a> {
    /// The collapse key of the section.
    pub key: &'static str,
    /// Whether All rules is on: every rule silenced, the list disabled.
    pub all_on: bool,
    /// What ticking or unticking All rules sends.
    pub all_toggled: PanelMsg,
    /// Whether the rule `id` is ticked.
    pub ticked: Box<dyn Fn(&str) -> bool + 'a>,
    /// What ticking (`true`) or unticking a rule sends.
    pub rule_toggled: Box<dyn Fn(String, bool) -> PanelMsg + 'a>,
}

/// The built-in ERC rules: id and the name signex-erc gives it.
fn builtin_rules() -> impl Iterator<Item = (String, &'static str)> {
    signex_erc::RuleKind::ALL.into_iter().map(|kind| {
        (
            signex_erc::RuleId::builtin(kind).as_str().to_string(),
            kind.label(),
        )
    })
}

/// One row of the rule list: the rule's id, its name and whether its box
/// shows ticked. While All rules is on every box shows ticked, since every
/// rule is silenced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuleRow {
    pub id: String,
    pub name: &'static str,
    pub checked: bool,
}

/// The rows the list shows for `query`, in catalog order.
pub(super) fn rule_rows(
    i18n: &signex_i18n::I18n,
    list: &RuleList<'_>,
    query: &str,
) -> Vec<RuleRow> {
    builtin_rules()
        .filter(|(_, name)| rule_matches(i18n, name, query))
        .map(|(id, name)| RuleRow {
            checked: list.all_on || (list.ticked)(&id),
            id,
            name,
        })
        .collect()
}

/// Suppressed rules (mockup row 49): the All rules enable row first, its
/// rule total at the row's end, a hairline, then the filter (search glyph
/// first) and one row per rule, box first. While All
/// rules is on, the filter and the rows are disabled, every box shows
/// ticked, and a caption under the list says why. The header counts the
/// rules that are off.
pub(super) fn suppressed_rules_section<'a>(
    ctx: &'a PanelContext,
    list: RuleList<'a>,
    st: PanelStyle,
) -> Element<'a, PanelMsg> {
    let total = signex_erc::RuleKind::ALL.len();
    let off = rule_rows(&ctx.i18n, &list, "")
        .iter()
        .filter(|row| row.checked)
        .count();
    let pill = t!(ctx.i18n, ids::PROPERTIES_RULES_OFF, "count" => off)
        .to_string()
        .replace(['\u{2068}', '\u{2069}'], "");
    counted_section(
        ctx,
        list.key,
        ids::PROPERTIES_SUPPRESSED_RULES,
        Some(pill),
        move || rules_body(ctx, list, total, st),
    )
}

fn rules_body<'a>(
    ctx: &'a PanelContext,
    list: RuleList<'a>,
    total: usize,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = &ctx.i18n;
    let all_toggled = list.all_toggled.clone();
    let muted = st.muted;
    let header = EnableSection::new(t!(i18n, ids::PROPERTIES_ALL_RULES_NAME), list.all_on)
        .on_toggle(move |_| all_toggled.clone())
        .wrap_header(move |header| rule_total_row(header, total, i18n, muted))
        .view(
            crate::panels::enable_section::enable_section_style(&ctx.tokens),
            Space::new().boxed(),
        );
    let query = ctx.selection_form.rule_query.clone();
    let mut rules = Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(rule_filter(ctx, query.clone()));
    let rows = rule_rows(i18n, &list, &query);
    if rows.is_empty() {
        rules = rules.push(inset_caption(
            t!(i18n, ids::PROPERTIES_RULE_NO_MATCH, "query" => query.trim()),
            st.muted,
        ));
    }
    for row in rows {
        let toggled = (list.rule_toggled)(row.id, !row.checked);
        rules = rules.push(rule_row(ctx, row.name, row.checked, toggled));
    }
    let rules: Element<'a, PanelMsg> = match dependents_veil(st.palette.panel_bg, !list.all_on) {
        Some(veil) => Inert::new(rules.boxed(), veil).boxed(),
        None => rules.boxed(),
    };
    let mut body = Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(header)
        .push(thin_sep(st.border_c))
        .push(rules);
    if list.all_on {
        body = body.push(inset_caption(
            t!(i18n, ids::PROPERTIES_ALL_RULES_ON),
            st.muted,
        ));
    }
    body
}

/// The All rules `header` with the number of built-in rules right-aligned
/// at the row's end (mockup row 49), padded as the header is so both sit
/// on one line.
fn rule_total_row<'a>(
    header: Element<'a, PanelMsg>,
    total: usize,
    i18n: &signex_i18n::I18n,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let count = text(t!(i18n, ids::PROPERTIES_RULE_TOTAL, "count" => total))
        .size(TS.caption)
        .color(muted)
        .wrapping(iced::widget::text::Wrapping::None);
    let count = container(count).padding(iced::Padding {
        top: SP.m,
        right: SP.l,
        bottom: SP.xs,
        left: 0.0,
    });
    iced::widget::row![header, count]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .boxed()
}

/// The rule filter: a search glyph and a borderless input in a field frame
/// (mockup row 49).
fn rule_filter<'a>(ctx: &PanelContext, query: String) -> Element<'a, PanelMsg> {
    let r = crate::styles::tokens::resolve(&ctx.tokens);
    let (ink, ink4) = (crate::styles::ti(ctx.tokens.text), r.ink4);
    let glyph = iced::widget::svg(crate::icons::icon_chrome_search(ctx.theme_id))
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| iced::widget::svg::Style {
            color: Some(ink4),
            ..Default::default()
        });
    let input = signex_widgets::text_input(t!(ctx.i18n, ids::PROPERTIES_RULE_FILTER), query)
        .on_input(|s| PanelMsg::SelectionForm(SelectionFormMsg::RuleQueryChanged(s)))
        .size(TS.body)
        .padding(iced::Padding::ZERO)
        .width(Length::Fill)
        .style(move |_: &Theme, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: iced::Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..ink },
        });
    let field = container(
        iced::widget::row![glyph.boxed(), input.boxed()]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
    )
    .padding(SP.s)
    .width(Length::Fill)
    .style(crate::styles::field(&ctx.tokens, false));
    container(field)
        .padding([SP.xs, PAD_X])
        .width(Length::Fill)
        .boxed()
}

/// A rule's row: its box first, then its name on one line across the
/// width, cut with an ellipsis when too long and shown whole in a tooltip.
/// A click on the name toggles the rule too.
fn rule_row<'a>(
    ctx: &PanelContext,
    name: &'static str,
    checked: bool,
    toggled: PanelMsg,
) -> Element<'a, PanelMsg> {
    let on_box = toggled.clone();
    let check = signex_widgets::checkbox(checked)
        .on_toggle(move |_| on_box.clone())
        .size(TS.body)
        .style(crate::panels::form_look::checkbox_style);
    // raw: the rule's name comes from signex-erc (#2062).
    let label = text(raw(name))
        .size(TS.body)
        .color(crate::styles::ti(ctx.tokens.text))
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .width(Length::Fill);
    let cells = iced::widget::row![check, label]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
    let tip = container(text(raw(name)).size(TS.body))
        .padding([SP.xs, SP.s])
        .style(crate::panels::form_look::value_tip);
    let row = container(cells).padding([SP.xs, PAD_X]).width(Length::Fill);
    let row = iced::widget::mouse_area(row).on_press(toggled);
    signex_widgets::tooltip(row, tip, iced::widget::tooltip::Position::Bottom).boxed()
}

pub(super) fn push_directive_sections<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let Some(panel) = ctx
        .directive_panel
        .as_ref()
        .filter(|p| Some(p.directive.uuid()) == uuid)
    else {
        return super::placed_kinds::push_placed_kind_sections(col, ctx, st);
    };
    match &panel.directive {
        SchDirective::NetParameters { .. } => {
            super::net_parameters::push_sections(col, ctx, panel, st)
        }
        SchDirective::Blanket { .. } => {
            let col = super::blanket::push_sections(col, ctx, panel, st);
            push_erc_exclusions(col, ctx, panel, st)
        }
        _ => super::placed_kinds::push_placed_kind_sections(col, ctx, st),
    }
}

/// The Suppressed rules section of a Net Blanket, shared with the No-ERC
/// directive: All rules, then one checkbox per rule, each one undo step.
fn push_erc_exclusions<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    panel: &'a DirectivePanel,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let SchDirective::Blanket { erc_exclusions, .. } = &panel.directive else {
        return col;
    };
    let id = panel.directive.uuid();
    let excluded: &'a [String] = erc_exclusions;
    let all = signex_types::schematic::ALL_ERC_RULES;
    let all_on = excluded.iter().any(|r| r == all);
    let list = RuleList {
        key: "sel_blanket_rules",
        all_on,
        all_toggled: PanelMsg::EditBlanket(
            id,
            BlanketEdit::Rule {
                rule_id: all.to_string(),
                excluded: !all_on,
            },
        ),
        ticked: Box::new(move |rule: &str| excluded.iter().any(|r| r == rule)),
        rule_toggled: Box::new(move |rule_id: String, excluded: bool| {
            PanelMsg::EditBlanket(id, BlanketEdit::Rule { rule_id, excluded })
        }),
    };
    col.push(suppressed_rules_section(ctx, list, st))
}

// ── Row pieces both directive surfaces draw with ──

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// A directive Properties message.
pub(super) fn msg(m: DirectivePanelMsg) -> PanelMsg {
    PanelMsg::Directive(m)
}

/// The muted caption line under the header, or under a section.
pub(super) fn caption_row<'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let ink3 = crate::styles::tokens::resolve(&ctx.tokens).ink3;
    container(
        text(label)
            .size(TS.caption)
            .color(ink3)
            .wrapping(iced::widget::text::Wrapping::Word),
    )
    .padding([SP.xs, PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// The row inset, as the house rows pad.
const PAD_X: f32 = PROPERTY_ROW_PAD_X as f32;

/// A typed value box, `width` wide, outlined in `error` while it holds text
/// its field refuses.
pub(super) fn value_input<'a>(
    value: String,
    on_input: impl Fn(String) -> PanelMsg + 'a,
    width: f32,
    error: Option<Color>,
) -> Element<'a, PanelMsg> {
    signex_widgets::text_input(signex_i18n::UserText::new(String::new()), value)
        .on_input(on_input)
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .style(super::super::form_look::refusable_input_style(error))
        .width(Length::Fixed(width))
        .boxed()
}

/// A unit after a value box (`mm`, `A`).
pub(super) fn unit<'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    text(label).size(TS.caption).color(muted).boxed()
}

/// `content` as a row linked to the illustration field `field`: hovering
/// the row lights the callout, and a hovered callout lights the row.
pub(super) fn linked_row<'a>(
    content: Element<'a, PanelMsg>,
    field: signex_widgets::property_illustration::FieldId,
    panel: &DirectivePanel,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let lit = panel.hover.is_field_highlighted(&field);
    let tint = crate::styles::tokens::resolve(&ctx.tokens).accent_soft;
    let framed = container(content)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: lit.then_some(Background::Color(tint)),
            ..container::Style::default()
        });
    iced::widget::mouse_area(framed)
        .on_enter(msg(DirectivePanelMsg::Illustration(
            IllustrationMsg::FieldHovered(field.clone()),
        )))
        .on_exit(msg(DirectivePanelMsg::Illustration(
            IllustrationMsg::FieldLeft(field),
        )))
        .boxed()
}

/// The illustration of a directive, its hover routed to the panel.
pub(super) fn illustration_view<'a>(
    illustration: signex_widgets::property_illustration::Illustration,
    panel: &DirectivePanel,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let drawing = signex_widgets::property_illustration::property_illustration(
        illustration,
        &panel.hover,
        &ctx.tokens,
        TS.caption,
    )
    .map(|m| msg(DirectivePanelMsg::Illustration(m)));
    container(drawing)
        .padding([SP.s, PAD_X])
        .width(Length::Fill)
        .boxed()
}

/// A label | control row in the house column widths.
pub(super) fn control_row<'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    control: Element<'a, PanelMsg>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    container(
        iced::widget::row![
            property_label(label, muted),
            container(control).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs / 2.0, PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A class dropdown: `None`, then `classes`, then the stored name when the
/// project table no longer lists it, so a pick never drops it unseen.
pub(super) fn class_picker<'a>(
    i18n: &signex_i18n::I18n,
    none: signex_i18n::MessageId,
    classes: &[String],
    current: Option<&str>,
    on_pick: impl Fn(Option<String>) -> PanelMsg + 'a,
    enabled: bool,
) -> Element<'a, PanelMsg> {
    use signex_widgets::dropdown_style::GraphiteDropdown as _;
    let mut choices = vec![crate::choice::Choice {
        value: None,
        label: i18n.message_plain(none, &[]),
    }];
    choices.extend(classes.iter().map(|c| crate::choice::Choice {
        value: Some(c.clone()),
        label: c.clone(),
    }));
    if let Some(name) = current
        && !classes.iter().any(|c| c == name)
    {
        choices.push(crate::choice::Choice {
            value: Some(name.to_string()),
            label: name.to_string(),
        });
    }
    let selected = crate::choice::chosen(&choices, &current.map(str::to_string));
    let list = iced::widget::pick_list(
        selected,
        choices,
        |c: &crate::choice::Choice<Option<String>>| c.label.clone(),
    )
    .graphite()
    .padding([SP.xs, 6.0])
    .width(Length::Fill);
    if enabled {
        list.on_select(move |c| on_pick(c.value)).boxed()
    } else {
        list.boxed()
    }
}
