//! The Finish sub-tab (mockup rows 105 and 176): the board's solder mask,
//! silkscreen and surface finish, saved with the board in
//! `[stackup.finish]`, and the board materials, the values the stack's
//! layers store. Each pick is one board undo step. The mask's thickness
//! over copper and its Dk are the Solder Mask rows' of the stack, shown here
//! and edited there.

use std::fmt;
use std::path::PathBuf;

use iced::widget::{Column, Row, column, pick_list, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::pcb_stack::{
    BoardFinish, FinishColor, FinishSides, LayerStack, MaskGloss, MaskType, StackLayer,
    StackLayerKind, SurfaceFinish,
};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::segmented::segmented_disabled;
use signex_widgets::{Label, segmented, text, text_input};

use super::page::heading;
use super::{LayerStackFacts, LayerStackMsg, SP, TS, form_look, kind_label, msg};
use crate::panels::pcb_primitive_properties::format_number;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ti, tokens::resolve};

/// What the Finish sub-tab shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FinishFacts {
    /// The stored finish, else the default.
    pub finish: BoardFinish,
    /// The board stores a finish; `false` shows the defaults.
    pub stored: bool,
    /// The mask thickness over laminate as typed, until Enter.
    pub draft: Option<String>,
}

/// An event of the Finish sub-tab.
#[derive(Debug, Clone, PartialEq)]
pub enum FinishMsg {
    /// A choice picked; carries the whole finish with it.
    Picked {
        board: PathBuf,
        finish: BoardFinish,
    },
    OverLaminateTyped {
        board: PathBuf,
        text: String,
    },
    /// Enter in the thickness over laminate: its draft is applied.
    OverLaminateSubmitted {
        board: PathBuf,
    },
}

/// A labelled value in a dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Choice<T> {
    value: T,
    label: String,
}

impl<T> fmt::Display for Choice<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The width of a setting's label, in body-text ems.
const LABEL_EMS: f32 = 10.0;
/// The width of a dropdown or field, in body-text ems.
const CONTROL_EMS: f32 = 14.0;

fn surface_id(surface: SurfaceFinish) -> MessageId {
    match surface {
        SurfaceFinish::Enig => ids::PCB_STACK_FINISH_ENIG,
        SurfaceFinish::Enepig => ids::PCB_STACK_FINISH_ENEPIG,
        SurfaceFinish::LeadFreeHasl => ids::PCB_STACK_FINISH_HASL_LEAD_FREE,
        SurfaceFinish::LeadedHasl => ids::PCB_STACK_FINISH_HASL_LEADED,
        SurfaceFinish::ImmersionSilver => ids::PCB_STACK_FINISH_IMMERSION_SILVER,
        SurfaceFinish::ImmersionTin => ids::PCB_STACK_FINISH_IMMERSION_TIN,
        SurfaceFinish::Osp => ids::PCB_STACK_FINISH_OSP,
        SurfaceFinish::HardGold => ids::PCB_STACK_FINISH_HARD_GOLD,
    }
}

fn color_id(color: FinishColor) -> MessageId {
    match color {
        FinishColor::Green => ids::PCB_STACK_FINISH_GREEN,
        FinishColor::Red => ids::PCB_STACK_FINISH_RED,
        FinishColor::Blue => ids::PCB_STACK_FINISH_BLUE,
        FinishColor::Black => ids::PCB_STACK_FINISH_BLACK,
        FinishColor::White => ids::PCB_STACK_FINISH_WHITE,
        FinishColor::Yellow => ids::PCB_STACK_FINISH_YELLOW,
        FinishColor::Purple => ids::PCB_STACK_FINISH_PURPLE,
    }
}

fn sides_id(sides: FinishSides) -> MessageId {
    match sides {
        FinishSides::Both => ids::PCB_STACK_FINISH_BOTH,
        FinishSides::Top => ids::PCB_STACK_FINISH_TOP,
        FinishSides::Bottom => ids::PCB_STACK_FINISH_BOTTOM,
    }
}

fn mask_type_id(kind: MaskType) -> MessageId {
    match kind {
        MaskType::Lpi => ids::PCB_STACK_FINISH_LPI,
        MaskType::DryFilm => ids::PCB_STACK_FINISH_DRY_FILM,
    }
}

fn gloss_id(gloss: MaskGloss) -> MessageId {
    match gloss {
        MaskGloss::Glossy => ids::PCB_STACK_FINISH_GLOSSY,
        MaskGloss::Matte => ids::PCB_STACK_FINISH_MATTE,
    }
}

/// The rows that name a material, each distinct set of values once, top to
/// bottom: the board materials. Copper rows carry no material.
pub(super) fn board_materials(stack: &LayerStack) -> Vec<&StackLayer> {
    let mut out: Vec<&StackLayer> = Vec::new();
    let rows = stack
        .layers
        .iter()
        .filter(|l| !l.kind.is_copper() && !l.material.is_empty());
    for row in rows {
        let same = |o: &&StackLayer| {
            o.kind == row.kind
                && o.material == row.material
                && o.thickness_mm == row.thickness_mm
                && o.dielectric_constant == row.dielectric_constant
                && o.loss_tangent == row.loss_tangent
        };
        if !out.iter().any(same) {
            out.push(row);
        }
    }
    out
}

pub(super) fn view<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let note = |id| text(t!(ctx.i18n, id)).size(TS.dense).color(muted).boxed();
    let mut col = column![
        section_title(ctx, ids::PCB_STACK_FINISH_MASK),
        mask(ctx, facts),
        section_title(ctx, ids::PCB_STACK_FINISH_SILKSCREEN),
        silkscreen(ctx, facts),
        section_title(ctx, ids::PCB_STACK_FINISH_SURFACE),
        surface(ctx, facts),
    ]
    .spacing(SP.s)
    .padding(SP.l)
    .width(Length::Fill);
    if facts.finish.finish.surface.is_hasl() {
        col = col.push(note(ids::PCB_STACK_FINISH_HASL_NOTE));
    }
    col = col.push(heading(
        ctx,
        ids::PCB_STACK_BOARD_MATERIALS,
        ids::PCB_STACK_BOARD_MATERIALS_HINT,
    ));
    col = col.push(materials_list(ctx, facts));
    if !facts.finish.stored {
        col = col.push(note(ids::PCB_STACK_FINISH_DEFAULT));
    }
    col = col.push(note(ids::PCB_STACK_FINISH_NOTE));
    scrollable(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

fn section_title<'a>(ctx: &'a PanelContext, id: MessageId) -> Element<'a, PanelMsg> {
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    text(t!(ctx.i18n, id))
        .size(TS.body)
        .font(semibold)
        .color(ti(ctx.tokens.text))
        .boxed()
}

/// A setting's label and its control on one line.
fn setting<'a>(
    ctx: &'a PanelContext,
    label: Localized,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![
        text(label)
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3)
            .width(Length::Fixed(TS.body * LABEL_EMS)),
        control,
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// The finish with one change, as the event a pick sends.
fn picked(facts: &LayerStackFacts, change: impl Fn(&mut BoardFinish)) -> PanelMsg {
    let mut finish = facts.finish.finish.clone();
    change(&mut finish);
    msg(LayerStackMsg::Finish(FinishMsg::Picked {
        board: facts.board.clone(),
        finish,
    }))
}

/// A segmented choice over `values`; greyed on a read-only board.
fn choice_bar<'a, V>(
    ctx: &'a PanelContext,
    facts: &'a LayerStackFacts,
    values: &[V],
    selected: V,
    id: fn(V) -> MessageId,
    set: fn(&mut BoardFinish, V),
) -> Element<'a, PanelMsg>
where
    V: Copy + PartialEq + 'a,
{
    let cells: Vec<(V, Label<'a>)> = values
        .iter()
        .map(|&v| (v, t!(ctx.i18n, id(v)).into()))
        .collect();
    let style = resolve(&ctx.tokens).segmented(TS.dense);
    if facts.read_only {
        return segmented_disabled(cells, selected, style);
    }
    segmented(cells, selected, style, move |v| {
        picked(facts, |f| set(f, v))
    })
}

/// A dropdown over `values`; inert on a read-only board.
fn choice_list<'a, V>(
    ctx: &'a PanelContext,
    facts: &'a LayerStackFacts,
    values: &[V],
    selected: V,
    id: fn(V) -> MessageId,
    set: fn(&mut BoardFinish, V),
) -> Element<'a, PanelMsg>
where
    V: Copy + PartialEq + 'a,
{
    let choice = |value: V| Choice {
        value,
        label: t!(ctx.i18n, id(value)).into_string(),
    };
    let options: Vec<Choice<V>> = values.iter().copied().map(choice).collect();
    let list = pick_list(Some(choice(selected)), options, |c: &Choice<V>| {
        c.to_string()
    })
    .graphite()
    .text_size(TS.dense)
    .padding(SP.xs)
    .width(Length::Fixed(TS.body * CONTROL_EMS));
    if facts.read_only {
        return list.boxed();
    }
    list.on_select(move |c: Choice<V>| picked(facts, |f| set(f, c.value)))
        .boxed()
}

fn mask<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let mask = &facts.finish.finish.mask;
    let lines: Vec<Element<'a, PanelMsg>> = vec![
        setting(
            ctx,
            t!(i18n, ids::PCB_STACK_FINISH_TYPE),
            choice_bar(
                ctx,
                facts,
                &MaskType::ALL,
                mask.kind,
                mask_type_id,
                |f, v| f.mask.kind = v,
            ),
        ),
        setting(
            ctx,
            t!(i18n, ids::PCB_STACK_FINISH_COLOUR),
            choice_list(
                ctx,
                facts,
                &FinishColor::MASK,
                mask.color,
                color_id,
                |f, v| f.mask.color = v,
            ),
        ),
        setting(
            ctx,
            t!(i18n, ids::PCB_STACK_FINISH_GLOSS),
            choice_bar(ctx, facts, &MaskGloss::ALL, mask.gloss, gloss_id, |f, v| {
                f.mask.gloss = v
            }),
        ),
        setting(
            ctx,
            t!(i18n, ids::PCB_STACK_FINISH_SIDES),
            choice_bar(
                ctx,
                facts,
                &FinishSides::ALL,
                mask.sides,
                sides_id,
                |f, v| f.mask.sides = v,
            ),
        ),
        over_laminate(ctx, facts),
        over_copper(ctx, facts),
    ];
    Column::with_children(lines).spacing(SP.xs).boxed()
}

/// The mask thickness over laminate, typed and applied with Enter.
fn over_laminate<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let unit = facts.unit;
    let shown = facts
        .finish
        .draft
        .clone()
        .unwrap_or_else(|| unit.show(facts.finish.finish.mask.over_laminate_mm));
    let mut input = text_input(UserText::new(String::new()), shown)
        .size(TS.dense)
        .padding(SP.xs)
        .style(form_look::text_input_style)
        .width(Length::Fixed(TS.body * CONTROL_EMS));
    if !facts.read_only {
        let board = facts.board.clone();
        let submitted = msg(LayerStackMsg::Finish(FinishMsg::OverLaminateSubmitted {
            board: board.clone(),
        }));
        input = input
            .on_input(move |text| {
                msg(LayerStackMsg::Finish(FinishMsg::OverLaminateTyped {
                    board: board.clone(),
                    text,
                }))
            })
            .on_submit(submitted);
    }
    let unit_name = unit.label(&ctx.i18n).into_string();
    let label = t!(ctx.i18n, ids::PCB_STACK_FINISH_OVER_LAMINATE, "unit" => unit_name);
    setting(ctx, label, input.boxed())
}

/// The first Solder Mask row's thickness and Dk, edited in Stack.
fn over_copper<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let row = facts
        .stack
        .layers
        .iter()
        .find(|l| l.kind == StackLayerKind::SolderMask);
    let line = match row {
        Some(row) => t!(
            ctx.i18n,
            ids::PCB_STACK_FINISH_OVER_COPPER,
            "value" => facts.unit.show(row.thickness_mm),
            "unit" => facts.unit.label(&ctx.i18n).into_string(),
            "dk" => format_number(row.dielectric_constant)
        ),
        None => t!(ctx.i18n, ids::PCB_STACK_FINISH_NO_MASK_ROW),
    };
    text(line).size(TS.dense).color(muted).boxed()
}

fn silkscreen<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let silk = &facts.finish.finish.silkscreen;
    column![
        setting(
            ctx,
            t!(i18n, ids::PCB_STACK_FINISH_COLOUR),
            choice_list(
                ctx,
                facts,
                &FinishColor::SILKSCREEN,
                silk.color,
                color_id,
                |f, v| f.silkscreen.color = v
            ),
        ),
        setting(
            ctx,
            t!(i18n, ids::PCB_STACK_FINISH_SIDES),
            choice_bar(
                ctx,
                facts,
                &FinishSides::ALL,
                silk.sides,
                sides_id,
                |f, v| { f.silkscreen.sides = v }
            ),
        ),
    ]
    .spacing(SP.xs)
    .boxed()
}

fn surface<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    setting(
        ctx,
        t!(ctx.i18n, ids::PCB_STACK_FINISH_SURFACE),
        choice_list(
            ctx,
            facts,
            &SurfaceFinish::ALL,
            facts.finish.finish.surface,
            surface_id,
            |f, v| f.surface = v,
        ),
    )
}

/// One line per board material: name, kind, thickness, Dk and Df.
fn materials_list<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let rows = board_materials(&facts.stack);
    if rows.is_empty() {
        return text(t!(ctx.i18n, ids::PCB_STACK_BOARD_MATERIALS_NONE))
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3)
            .boxed();
    }
    let mut lines: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.xs);
    for layer in rows {
        lines = lines.push(material_line(ctx, &ctx.i18n, facts, layer));
    }
    lines.boxed()
}

fn material_line<'a>(
    ctx: &'a PanelContext,
    i18n: &I18n,
    facts: &LayerStackFacts,
    layer: &StackLayer,
) -> Element<'a, PanelMsg> {
    let dash = t!(i18n, ids::PCB_STACK_NONE).into_string();
    let number = |v: f64| {
        if v == 0.0 {
            dash.clone()
        } else {
            format_number(v)
        }
    };
    let unit = facts.unit.label(i18n).into_string();
    let dk = t!(i18n, ids::PCB_STACK_COL_DK).into_string();
    let df = t!(i18n, ids::PCB_STACK_COL_DF).into_string();
    let cells = [
        layer.material.clone(),
        kind_label(i18n, layer.kind).into_string(),
        format!("{} {unit}", facts.unit.show(layer.thickness_mm)),
        format!("{dk} {}", number(layer.dielectric_constant)),
        format!("{df} {}", number(layer.loss_tangent)),
    ];
    let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.m);
    for cell in cells {
        line = line.push(
            text(UserText::new(cell))
                .size(TS.dense)
                .color(ti(ctx.tokens.text))
                .boxed(),
        );
    }
    line.boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_materials_lists_each_named_value_set_once_and_no_copper() {
        let mut stack = LayerStack::derived(&[], 1.6);
        let core = StackLayer {
            material: "Prepreg 1080".into(),
            loss_tangent: 0.02,
            ..StackLayer::new(StackLayerKind::Dielectric)
        };
        stack.layers.insert(4, core.clone());
        stack.layers.insert(4, core);
        let listed = board_materials(&stack);
        let names: Vec<&str> = listed.iter().map(|l| l.material.as_str()).collect();
        assert_eq!(names, ["Solder Resist", "Prepreg 1080", "FR-4"]);
        assert!(listed.iter().all(|l| !l.kind.is_copper()));
    }
}
