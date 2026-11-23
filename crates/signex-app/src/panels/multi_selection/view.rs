//! The multi-selection Properties body (mockup row 51): the header, the kind chips,
//! one section per kind with its count pill, the footer, and the empty card
//! when nothing is shared.

use super::super::*;
use super::controls::{Cell, control_row, counted_section, inset_caption, segmented_cells};
use super::{MultiEditMsg, MultiField, MultiKind, SelectionFormMsg, shared_number, shared_text};
use crate::panels::form_look;
use crate::panels::typed_pick::TypedPick;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_engine::{MultiEdit, SelectionGroups, Shared};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::schematic::{HAlign, SCHEMATIC_PT_TO_MM, SelectedKind, WireWidthPreset};
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{Label, checkbox, text_input};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The four turns a label offers, as in the single-label panel.
const LABEL_TURNS: [u32; 4] = [0, 90, 180, 270];

/// The multi-selection Properties body.
pub fn view_multi_selection<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let groups = &ctx.multi_groups;
    let form = &ctx.selection_form;
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(header(ctx, muted, primary));
    col = col.push(thin_sep(border_c));
    if groups.is_empty() {
        col = col.push(nothing_shared_card(ctx, muted, primary));
    } else {
        col = col.push(kind_chips(ctx, groups));
        if form.shows(MultiKind::Parts, groups) {
            col = col.push(parts_section(ctx, muted));
        }
        if form.effective_filter(groups) == Some(MultiKind::PowerPorts) {
            col = col.push(inset_caption(
                t!(ctx.i18n, ids::PROPERTIES_MULTI_POWER_NOTE),
                muted,
            ));
        }
        if form.shows(MultiKind::Labels, groups) {
            col = col.push(labels_section(ctx, muted));
        }
        if form.shows(MultiKind::Wires, groups) {
            col = col.push(wires_section(ctx, muted));
        }
    }
    col = col.push(Space::new().height(SP.m).boxed());
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(text(footer_text(ctx)).size(TS.caption).color(muted))
            .padding([SP.s, SP.m])
            .boxed(),
    );
    scrollable(col.boxed()).width(Length::Fill).boxed()
}

/// "Multi-Selection" with the muted "N objects selected" beside it.
fn header<'a>(ctx: &'a PanelContext, muted: Color, primary: Color) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    container(
        row![
            text(t!(i18n, ids::PANELS_MULTI_TITLE))
                .size(TS.body)
                .color(primary),
            text(t!(
                i18n,
                ids::PANELS_MULTI_OBJECTS_SELECTED,
                "count" => counted_objects(ctx).to_string()
            ))
            .size(TS.caption)
            .color(muted),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([6.0, SP.m])
    .width(Length::Fill)
    .boxed()
}

fn chip_style(tokens: &ThemeTokens) -> ChipStyle {
    let r = resolve(tokens);
    ChipStyle {
        fill: r.field,
        edge: crate::styles::ti(tokens.border),
        hover: r.raised,
        ink: crate::styles::ti(tokens.text_secondary),
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

/// The kind label of a chip and a section header.
fn kind_label(kind: MultiKind) -> MessageId {
    match kind {
        MultiKind::Parts => ids::PROPERTIES_KIND_PARTS,
        MultiKind::PowerPorts => ids::PROPERTIES_KIND_POWER_PORTS,
        MultiKind::Labels => ids::PROPERTIES_KIND_LABELS,
        MultiKind::Wires => ids::PROPERTIES_KIND_WIRES,
    }
}

/// The catalog message counting `n` objects of a chip's kind.
fn chip_count_id(kind: MultiKind) -> MessageId {
    match kind {
        MultiKind::Parts => ids::PROPERTIES_COUNT_PARTS,
        MultiKind::PowerPorts => ids::PROPERTIES_COUNT_POWER_PORTS,
        MultiKind::Labels => ids::PROPERTIES_COUNT_LABELS,
        MultiKind::Wires => ids::PROPERTIES_COUNT_WIRES,
    }
}

/// What the header, All and the footer count: the chips' sum, or the whole
/// selection when nothing in it can be edited together.
fn counted_objects(ctx: &PanelContext) -> usize {
    if ctx.multi_groups.is_empty() {
        ctx.selection_count
    } else {
        ctx.multi_groups.counted()
    }
}

/// All N and one chip per kind present; the chip in effect is on. A chip
/// only filters the sections; the canvas selection stays as it is.
fn kind_chips<'a>(ctx: &'a PanelContext, groups: &SelectionGroups) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let style = chip_style(&ctx.tokens);
    let metrics = ChipMetrics::from_text_size(TS.body);
    let on = ctx.selection_form.effective_filter(groups);
    let picked = |kind: Option<MultiKind>| {
        Some(PanelMsg::SelectionForm(SelectionFormMsg::KindChipPicked(
            kind,
        )))
    };
    let count = |n: usize| Some(u32::try_from(n).unwrap_or(u32::MAX));
    let mut chips = row![chip(
        t!(i18n, ids::PROPERTIES_KIND_ALL),
        count(counted_objects(ctx)),
        on.is_none(),
        style,
        metrics,
        i18n,
        picked(None),
    )]
    .spacing(metrics.row_gap);
    for kind in MultiKind::ALL {
        let Some(n) = kind.count_in(groups) else {
            continue;
        };
        chips = chips.push(chip(
            t!(i18n, kind_label(kind)),
            count(n),
            on == Some(kind),
            style,
            metrics,
            i18n,
            picked(Some(kind)),
        ));
    }
    container(chips.wrap().vertical_spacing(metrics.row_gap))
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// A text box; a mixed value is an empty box with the muted `<mixed>`
/// placeholder, and typing sets every object.
fn text_box<'a>(
    ctx: &'a PanelContext,
    label: Label<'a>,
    field: MultiField,
    stored: String,
    mixed: bool,
    unit: Option<Label<'a>>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let drafted = ctx
        .multi_draft
        .as_ref()
        .filter(|(f, _)| *f == field)
        .map(|(_, t)| t.clone());
    let shown = drafted.unwrap_or(stored);
    let (changed, submitted) = (field.clone(), field);
    let placeholder: Label<'a> = if mixed {
        t!(ctx.i18n, ids::PLACEHOLDER_MIXED).into()
    } else {
        signex_i18n::UserText::new(String::new()).into()
    };
    let input = text_input(placeholder, shown)
        .on_input(move |s| PanelMsg::MultiEdit(MultiEditMsg::DraftChanged(changed.clone(), s)))
        .on_submit(PanelMsg::MultiEdit(MultiEditMsg::DraftSubmitted(submitted)))
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fill)
        .style(mixed_input_style(mixed));
    let control = match unit {
        Some(unit) => row![input, text(unit).size(TS.caption).color(muted)]
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center)
            .boxed(),
        None => input.boxed(),
    };
    control_row(label, control, muted)
}

/// Footprint (mockup row 51): a dropdown of the footprints the selected
/// parts' library rows have in common, which also takes typing. With none
/// in common it is the plain text box.
fn footprint_box<'a>(
    ctx: &'a PanelContext,
    footprint: &Shared<String>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let label: Label<'a> = t!(ctx.i18n, ids::PANELS_MULTI_FOOTPRINT).into();
    let field = MultiField::SymbolFootprint;
    let is_mixed = matches!(footprint, Shared::Mixed);
    if ctx.multi_footprint_choices.is_empty() {
        return text_box(
            ctx,
            label,
            field,
            shared_text(footprint),
            is_mixed,
            None,
            muted,
        );
    }
    let drafted = (ctx.multi_draft.as_ref())
        .filter(|(f, _)| *f == field)
        .map(|(_, typed)| typed.clone());
    let placeholder: Label<'a> = if is_mixed {
        t!(ctx.i18n, ids::PLACEHOLDER_MIXED).into()
    } else {
        signex_i18n::UserText::new(String::new()).into()
    };
    let (changed, submitted) = (field.clone(), field);
    let control = TypedPick {
        placeholder,
        shown: drafted.unwrap_or_else(|| shared_text(footprint)),
        error: None,
        unit: None,
    }
    .view(
        ctx.multi_footprint_choices.clone(),
        |choice: &crate::panels::FootprintChoiceItem| choice.label.clone(),
        move |s| PanelMsg::MultiEdit(MultiEditMsg::DraftChanged(changed.clone(), s)),
        PanelMsg::MultiEdit(MultiEditMsg::DraftSubmitted(submitted)),
        |choice: crate::panels::FootprintChoiceItem| {
            PanelMsg::MultiEdit(MultiEditMsg::FootprintPicked(choice.binding))
        },
    );
    control_row(label, control, muted)
}

/// The form field look; a mixed field's placeholder is the muted italic
/// `<mixed>` (italic is the font, set by the caller's face).
fn mixed_input_style(
    mixed: bool,
) -> impl Fn(&Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    move |theme, status| {
        let mut style = form_look::text_input_style(theme, status);
        if mixed {
            style.placeholder = form_look::FormLook::of(theme).ink3;
        }
        style
    }
}

/// A checkbox over a flag the group may disagree on: a mixed flag is drawn
/// in the tri-state look and a click sets it on every object.
fn flag_row<'a>(
    i18n: &I18n,
    label: MessageId,
    shared: &Shared<bool>,
    edit: fn(bool) -> MultiEdit,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let mixed = matches!(shared, Shared::Mixed);
    let checked = mixed || matches!(shared, Shared::Same(true));
    let target = !matches!(shared, Shared::Same(true));
    let mut boxed = checkbox(checked)
        .on_toggle(move |_| PanelMsg::MultiEdit(MultiEditMsg::Picked(edit(target))))
        .size(TS.heading)
        .spacing(SP.s);
    boxed = if mixed {
        boxed.style(form_look::mixed_checkbox_style)
    } else {
        boxed.style(form_look::checkbox_style)
    };
    control_row(t!(i18n, label), boxed.boxed(), muted)
}

fn parts_section<'a>(ctx: &'a PanelContext, muted: Color) -> Element<'a, PanelMsg> {
    let Some(g) = ctx.multi_groups.symbols.as_ref() else {
        return Space::new().boxed();
    };
    let i18n = &ctx.i18n;
    counted_section(
        ctx,
        "multi_parts",
        ids::PROPERTIES_KIND_PARTS,
        Some(i18n.count(g.count as i64)),
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(text_box(
                ctx,
                t!(i18n, ids::PANELS_MULTI_VALUE).into(),
                MultiField::SymbolValue,
                shared_text(&g.value),
                matches!(g.value, Shared::Mixed),
                None,
                muted,
            ));
            c = c.push(footprint_box(ctx, &g.footprint, muted));
            c = c.push(flag_row(
                i18n,
                ids::PANELS_MULTI_DNP,
                &g.dnp,
                MultiEdit::SymbolDnp,
                muted,
            ));
            c = c.push(flag_row(
                i18n,
                ids::PANELS_MULTI_IN_BOM,
                &g.in_bom,
                MultiEdit::SymbolInBom,
                muted,
            ));
            c = c.push(flag_row(
                i18n,
                ids::PANELS_MULTI_ON_BOARD,
                &g.on_board,
                MultiEdit::SymbolOnBoard,
                muted,
            ));
            for (key, shared) in &g.parameters {
                c = c.push(text_box(
                    ctx,
                    signex_i18n::UserText::new(key.clone()).into(),
                    MultiField::SymbolParameter(key.clone()),
                    shared_text(shared),
                    matches!(shared, Shared::Mixed),
                    None,
                    muted,
                ));
            }
            c
        },
    )
}

/// The caption under Justify and Rotation when either is mixed.
fn mixed_caption(justify_mixed: bool, rotation_mixed: bool) -> Option<MessageId> {
    match (justify_mixed, rotation_mixed) {
        (true, true) => Some(ids::PROPERTIES_MULTI_MIXED_BOTH),
        (true, false) => Some(ids::PROPERTIES_MULTI_MIXED_JUSTIFY),
        (false, true) => Some(ids::PROPERTIES_MULTI_MIXED_ROTATION),
        (false, false) => None,
    }
}

/// The turn a group shares, when it is one of the four the form offers.
fn shared_turn(shared: &Shared<f64>) -> Option<u32> {
    let degrees = shared.same()?;
    LABEL_TURNS
        .into_iter()
        .find(|turn| (f64::from(*turn) - degrees).abs() < 1e-6)
}

fn labels_section<'a>(ctx: &'a PanelContext, muted: Color) -> Element<'a, PanelMsg> {
    let Some(g) = ctx.multi_groups.labels.as_ref() else {
        return Space::new().boxed();
    };
    let i18n = &ctx.i18n;
    counted_section(
        ctx,
        "multi_labels",
        ids::PROPERTIES_KIND_LABELS,
        Some(i18n.count(g.count as i64)),
        move || {
            let pick = |edit: MultiEdit| PanelMsg::MultiEdit(MultiEditMsg::Picked(edit));
            let justify = segmented_cells(
                vec![
                    Cell::new(HAlign::Left, t!(i18n, ids::PANELS_MULTI_JUSTIFY_LEFT)),
                    Cell::new(HAlign::Center, t!(i18n, ids::PANELS_MULTI_JUSTIFY_CENTER)),
                    Cell::new(HAlign::Right, t!(i18n, ids::PANELS_MULTI_JUSTIFY_RIGHT)),
                ],
                g.justify.same().copied(),
                3,
                move |to| pick(MultiEdit::LabelJustify(to)),
            );
            let turns = LABEL_TURNS
                .into_iter()
                .map(|turn| {
                    Cell::new(
                        turn,
                        t!(i18n, ids::PROPERTIES_DEGREES_SHORT, "degrees" => turn),
                    )
                })
                .collect();
            let rotation =
                segmented_cells(turns, shared_turn(&g.rotation_degrees), 4, move |turn| {
                    pick(MultiEdit::LabelRotationDegrees(f64::from(turn)))
                });
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(text_box(
                ctx,
                t!(i18n, ids::PROPERTIES_FONT_SIZE).into(),
                MultiField::LabelFontPt,
                shared_number(&g.font_size_mm, SCHEMATIC_PT_TO_MM),
                matches!(g.font_size_mm, Shared::Mixed),
                Some(t!(i18n, ids::PROPERTIES_UNIT_PT).into()),
                muted,
            ));
            c = c.push(control_row(
                t!(i18n, ids::PANELS_MULTI_JUSTIFY),
                justify,
                muted,
            ));
            c = c.push(control_row(
                t!(i18n, ids::PANELS_MULTI_ROTATION),
                rotation,
                muted,
            ));
            let caption = mixed_caption(
                matches!(g.justify, Shared::Mixed),
                matches!(g.rotation_degrees, Shared::Mixed),
            );
            if let Some(caption) = caption {
                c = c.push(inset_caption(t!(i18n, caption, "count" => g.count), muted));
            }
            c
        },
    )
}

/// The preset the wires share; `None` when they differ or share a width no
/// preset stores.
fn shared_preset(shared: &Shared<f64>) -> Option<WireWidthPreset> {
    let mm = *shared.same()?;
    WireWidthPreset::ALL
        .into_iter()
        .find(|preset| (preset.stroke_mm() - mm).abs() < 1e-9)
}

fn preset_label(preset: WireWidthPreset) -> MessageId {
    match preset {
        WireWidthPreset::Smallest => ids::PROPERTIES_WIRE_WIDTH_SMALLEST,
        WireWidthPreset::Small => ids::PROPERTIES_WIRE_WIDTH_SMALL,
        WireWidthPreset::Medium => ids::PROPERTIES_WIRE_WIDTH_MEDIUM,
        WireWidthPreset::Large => ids::PROPERTIES_WIRE_WIDTH_LARGE,
    }
}

fn wires_section<'a>(ctx: &'a PanelContext, muted: Color) -> Element<'a, PanelMsg> {
    let Some(g) = ctx.multi_groups.wires.as_ref() else {
        return Space::new().boxed();
    };
    let i18n = &ctx.i18n;
    counted_section(
        ctx,
        "multi_wires",
        ids::PROPERTIES_KIND_WIRES,
        Some(i18n.count(g.count as i64)),
        move || {
            let cells = WireWidthPreset::ALL
                .into_iter()
                .map(|preset| Cell::new(preset, t!(i18n, preset_label(preset))))
                .collect();
            let presets = segmented_cells(cells, shared_preset(&g.width_mm), 2, |preset| {
                PanelMsg::MultiEdit(MultiEditMsg::Picked(MultiEdit::WireWidthMm(
                    preset.stroke_mm(),
                )))
            });
            // Under the presets, any width typed in the panel's unit.
            let unit = ctx.unit;
            let typed = text_box(
                ctx,
                t!(i18n, ids::PROPERTIES_WIRE_WIDTH_EXACT).into(),
                MultiField::WireWidth,
                shared_number(&g.width_mm, signex_types::measure::mm_per(unit)),
                matches!(g.width_mm, Shared::Mixed),
                Some(t!(i18n, ids::PROPERTIES_UNIT_SUFFIX, "unit" => unit.to_string()).into()),
                muted,
            );
            Column::new()
                .spacing(0)
                .width(Length::Fill)
                .push(control_row(
                    t!(i18n, ids::PROPERTIES_WIRE_WIDTH),
                    presets,
                    muted,
                ))
                .push(typed)
        },
    )
}

/// The catalog message counting `n` objects of `kind`.
pub(super) fn kind_count_id(kind: SelectedKind) -> MessageId {
    match kind {
        SelectedKind::Symbol => ids::PROPERTIES_COUNT_PARTS,
        SelectedKind::SymbolRefField | SelectedKind::SymbolValField => {
            ids::PROPERTIES_COUNT_PART_FIELDS
        }
        SelectedKind::Label => ids::PROPERTIES_COUNT_LABELS,
        SelectedKind::Wire => ids::PROPERTIES_COUNT_WIRES,
        SelectedKind::Bus => ids::PROPERTIES_COUNT_BUSES,
        SelectedKind::BusEntry => ids::PROPERTIES_COUNT_BUS_ENTRIES,
        SelectedKind::Junction => ids::PROPERTIES_COUNT_JUNCTIONS,
        SelectedKind::NoConnect => ids::PROPERTIES_COUNT_NO_CONNECTS,
        SelectedKind::NoErcDirective => ids::PROPERTIES_COUNT_NO_ERC,
        SelectedKind::SheetPin => ids::PROPERTIES_COUNT_SHEET_ENTRIES,
        SelectedKind::TextNote => ids::PROPERTIES_COUNT_TEXT_NOTES,
        SelectedKind::ChildSheet => ids::PROPERTIES_COUNT_SHEET_SYMBOLS,
        SelectedKind::Drawing => ids::PROPERTIES_COUNT_DRAWINGS,
        SelectedKind::Directive => ids::PROPERTIES_COUNT_DIRECTIVES,
        SelectedKind::HarnessConnector => ids::PROPERTIES_COUNT_HARNESS_CONNECTORS,
        SelectedKind::HarnessEntry => ids::PROPERTIES_COUNT_HARNESS_ENTRIES,
        SelectedKind::HarnessWire => ids::PROPERTIES_COUNT_HARNESS_WIRES,
    }
}

/// "9 parts, 3 labels, 2 wires": every kind of the selection with its count.
/// The two part-field kinds count as one, and power ports are not parts.
pub(super) fn kinds_text(ctx: &PanelContext) -> String {
    let mut merged: Vec<(MessageId, usize)> = Vec::new();
    for (kind, n) in ctx.selection_form.kinds.iter() {
        if kind == SelectedKind::Symbol {
            let ports = ctx.multi_groups.power_ports.min(n);
            if n > ports {
                merged.push((ids::PROPERTIES_COUNT_PARTS, n - ports));
            }
            if ports > 0 {
                merged.push((ids::PROPERTIES_COUNT_POWER_PORTS, ports));
            }
            continue;
        }
        let id = kind_count_id(kind);
        match merged.iter_mut().find(|(seen, _)| *seen == id) {
            Some((_, total)) => *total += n,
            None => merged.push((id, n)),
        }
    }
    merged
        .into_iter()
        .map(|(id, n)| ctx.i18n.message_plain(id, &[("count", n.into())]))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The chips' kinds with their counts, in chip order.
fn counted_kinds_text(ctx: &PanelContext) -> String {
    MultiKind::ALL
        .into_iter()
        .filter_map(|kind| {
            let n = kind.count_in(&ctx.multi_groups)?;
            Some(
                ctx.i18n
                    .message_plain(chip_count_id(kind), &[("count", n.into())]),
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The footer: "14 selected: 9 parts, 3 labels, 2 wires". It counts what All
/// counts; with nothing editable it lists the whole selection.
pub(super) fn footer_text(ctx: &PanelContext) -> signex_i18n::Localized {
    let kinds = if ctx.multi_groups.is_empty() {
        kinds_text(ctx)
    } else {
        counted_kinds_text(ctx)
    };
    let count = counted_objects(ctx);
    if kinds.is_empty() {
        t!(ctx.i18n, ids::PROPERTIES_MULTI_FOOTER_PLAIN, "count" => count)
    } else {
        t!(
            ctx.i18n,
            ids::PROPERTIES_MULTI_FOOTER,
            "count" => count,
            "kinds" => kinds
        )
    }
}

/// The empty card shown when no field is shared: it says so and lists what
/// is selected.
fn nothing_shared_card<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let card = Column::new()
        .spacing(SP.s)
        .width(Length::Fill)
        .push(
            text(t!(i18n, ids::PROPERTIES_NOTHING_SHARED_TITLE))
                .size(TS.body)
                .color(primary),
        )
        .push(
            text(t!(
                i18n,
                ids::PROPERTIES_NOTHING_SHARED_BODY,
                "kinds" => kinds_text(ctx)
            ))
            .size(TS.caption)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::Word),
        );
    let r = resolve(&ctx.tokens);
    let (fill, edge) = (r.sunken, r.line_soft);
    container(
        container(card)
            .padding(SP.l)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                border: Border {
                    width: 1.0,
                    color: edge,
                    radius: crate::panels::graphite::PROPERTY_FIELD_RADIUS.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding(SP.l)
    .width(Length::Fill)
    .boxed()
}
