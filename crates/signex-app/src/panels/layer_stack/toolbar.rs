//! The tab's toolbar (mockup row 105): the stackup's name, Add, Move, Remove,
//! Presets, Import, Export, Symmetric stack, the units, the fab profile and
//! the total. Presets, Import and Export are greyed with their reason.

use iced::widget::{pick_list, row};
use iced::{Alignment, Element, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::pcb_stack::StackLayerKind;
use signex_widgets::doc_tab_shell::Toolbar;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{ToggleSwitchStyle, button, segmented, text, toggle_switch};

use super::{
    KindChoice, LayerStackFacts, LayerStackMsg, SP, StackUnit, TS, form_look, kind_label, msg,
};
use crate::panels::graphite::hint_tooltip;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::ti;
use crate::styles::tokens::{on_accent, resolve};

/// The kinds Add offers.
const ADDABLE: [StackLayerKind; 3] = [
    StackLayerKind::Signal,
    StackLayerKind::Plane,
    StackLayerKind::Dielectric,
];

pub(super) fn view<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Toolbar<'a, PanelMsg> {
    let board = facts.board.clone();
    let moved = |up| LayerStackMsg::RowMoved {
        board: board.clone(),
        up,
    };
    let removed = LayerStackMsg::RowRemoved {
        board: board.clone(),
    };
    Toolbar::new()
        .push(stackup_name(ctx, facts))
        .separator()
        .push(add_list(ctx, facts))
        .push(on_row(
            ctx,
            facts,
            ids::PCB_STACK_MOVE_UP,
            moved(true),
            ids::PCB_STACK_MOVE_UP,
        ))
        .push(on_row(
            ctx,
            facts,
            ids::PCB_STACK_MOVE_DOWN,
            moved(false),
            ids::PCB_STACK_MOVE_DOWN,
        ))
        .push(on_row(
            ctx,
            facts,
            ids::PCB_STACK_REMOVE,
            removed,
            ids::PCB_STACK_REMOVE_TIP,
        ))
        .separator()
        .push(action(
            ctx,
            ids::PCB_STACK_PRESETS,
            None,
            ids::PCB_STACK_PRESETS_UNAVAILABLE,
        ))
        .push(action(
            ctx,
            ids::PCB_STACK_IMPORT,
            None,
            ids::PCB_STACK_IMPORT_UNAVAILABLE,
        ))
        .push(action(
            ctx,
            ids::PCB_STACK_EXPORT,
            None,
            ids::PCB_STACK_EXPORT_UNAVAILABLE,
        ))
        .push(symmetric(ctx, facts))
        .push(units(ctx, facts))
        .push(fab_profile(ctx, facts))
        .push_end(total(ctx, facts))
}

/// The stackup's label and the board it belongs to.
fn stackup_name<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let stem = facts
        .board
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    row![
        text(t!(ctx.i18n, ids::PCB_STACK_STACKUP))
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3),
        text(UserText::new(stem))
            .size(TS.dense)
            .color(ti(ctx.tokens.text)),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}

/// Add: a list of the kinds that can be inserted. Inert on a read-only board.
fn add_list<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let options: Vec<KindChoice> = ADDABLE
        .into_iter()
        .map(|kind| KindChoice {
            kind,
            label: kind_label(&ctx.i18n, kind).into_string(),
        })
        .collect();
    let list = pick_list(None::<KindChoice>, options, |c: &KindChoice| c.to_string())
        .placeholder(t!(ctx.i18n, ids::PCB_STACK_ADD).into_string())
        .graphite()
        .text_size(TS.dense)
        .padding([SP.xs, SP.s]);
    let list = if facts.read_only {
        list
    } else {
        let board = facts.board.clone();
        list.on_select(move |c: KindChoice| {
            msg(LayerStackMsg::LayerAdded {
                board: board.clone(),
                kind: c.kind,
            })
        })
    };
    hint_tooltip(
        list.boxed(),
        t!(ctx.i18n, ids::PCB_STACK_ADD_TIP),
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}

/// A button that acts on the selected row; inert without one.
fn on_row<'a>(
    ctx: &'a PanelContext,
    facts: &LayerStackFacts,
    label: MessageId,
    event: LayerStackMsg,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let live = !facts.read_only && facts.selected.is_some();
    let tip = if live { tip } else { ids::PCB_STACK_SELECT_TIP };
    action(ctx, label, live.then(|| msg(event)), tip)
}

/// A ghost button with a hint; inert without a message.
fn action<'a>(
    ctx: &'a PanelContext,
    label: MessageId,
    on_press: Option<PanelMsg>,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let face = button(text(t!(ctx.i18n, label)).size(TS.dense))
        .padding([SP.xs, SP.s])
        .style(form_look::ghost_button)
        .on_press_maybe(on_press);
    hint_tooltip(
        face.boxed(),
        t!(ctx.i18n, tip),
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}

/// Symmetric stack: reads the stack, so it is on exactly while it mirrors.
fn symmetric<'a>(ctx: &'a PanelContext, facts: &LayerStackFacts) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let style =
        ToggleSwitchStyle::new(ti(ctx.tokens.accent), r.line_strong, on_accent(&ctx.tokens));
    let line = row![
        toggle_switch(facts.stack.is_symmetric(), style, None::<PanelMsg>),
        text(t!(ctx.i18n, ids::PCB_STACK_SYMMETRIC))
            .size(TS.dense)
            .color(ti(ctx.tokens.text)),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    hint_tooltip(
        line.boxed(),
        t!(ctx.i18n, ids::PCB_STACK_SYMMETRIC_TIP),
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}

/// The mm | µm | mil switch.
fn units<'a>(ctx: &'a PanelContext, facts: &LayerStackFacts) -> Element<'a, PanelMsg> {
    let board = facts.board.clone();
    let cells: Vec<(StackUnit, signex_widgets::Label<'a>)> = StackUnit::ALL
        .into_iter()
        .map(|unit| (unit, unit.label(&ctx.i18n).into()))
        .collect();
    segmented(
        cells,
        facts.unit,
        resolve(&ctx.tokens).segmented(TS.dense),
        move |unit| {
            msg(LayerStackMsg::UnitPicked {
                board: board.clone(),
                unit,
            })
        },
    )
}

/// Fab profile: a link to the project's Rules, where it is set.
fn fab_profile<'a>(ctx: &'a PanelContext, facts: &LayerStackFacts) -> Element<'a, PanelMsg> {
    let link = action(
        ctx,
        ids::PCB_STACK_FAB_OPEN,
        facts.has_project.then_some(PanelMsg::OpenRules),
        ids::PCB_STACK_FAB_TIP,
    );
    row![
        text(t!(ctx.i18n, ids::PCB_STACK_FAB_PROFILE))
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3),
        link,
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}

/// The total thickness, at the right edge.
fn total<'a>(ctx: &'a PanelContext, facts: &LayerStackFacts) -> Element<'a, PanelMsg> {
    let value = facts.unit.show(facts.stack.total_thickness_mm());
    let unit = facts.unit.label(&ctx.i18n).into_string();
    text(t!(ctx.i18n, ids::PCB_STACK_TOTAL, "value" => value, "unit" => unit))
        .size(TS.dense)
        .color(ti(ctx.tokens.text))
        .boxed()
}
