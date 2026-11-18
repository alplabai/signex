//! The Layer Stack manager of the board in view (#2401): one table row per
//! layer of the physical stack, top to bottom, with its name, type,
//! material, thickness, Er and copper weight, and the total under it. A pure
//! view over facts the app copies from the board
//! (`app/handlers/pcb_layer_stack`); every edit is a [`LayerStackMsg`] the
//! app turns into one `SetLayerStack` board step.

mod table;

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use iced::widget::{column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::pcb_stack::{LayerStack, StackLayerKind};
use signex_widgets::{button, text};

use super::{PanelContext, PanelMsg, empty_state_card, form_look};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// A typed cell of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StackField {
    Name,
    Material,
    Thickness,
    DielectricConstant,
    CopperWeight,
}

impl StackField {
    /// A cell typed as text, not as a number.
    pub fn is_text(self) -> bool {
        matches!(self, Self::Name | Self::Material)
    }
}

/// An event of the Layer Stack manager. Each names the board it was shown
/// for, so a tab change in between cannot send the edit to another board.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerStackMsg {
    RowSelected {
        board: PathBuf,
        row: usize,
    },
    FieldTyped {
        board: PathBuf,
        row: usize,
        field: StackField,
        text: String,
    },
    /// Enter in a cell: its draft is applied.
    FieldSubmitted {
        board: PathBuf,
        row: usize,
        field: StackField,
    },
    KindPicked {
        board: PathBuf,
        row: usize,
        kind: StackLayerKind,
    },
    /// A layer of `kind` added below the selected row.
    LayerAdded {
        board: PathBuf,
        kind: StackLayerKind,
    },
    /// The selected row removed.
    RowRemoved {
        board: PathBuf,
    },
    /// The selected row moved one place up or down.
    RowMoved {
        board: PathBuf,
        up: bool,
    },
}

/// What the manager shows of the board in view.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerStackFacts {
    pub board: PathBuf,
    /// The stored stack, or the one derived from the board's layers.
    pub stack: LayerStack,
    /// The board stores its stack; `false` shows the derived one.
    pub stored: bool,
    pub read_only: bool,
    /// The name each row shows: its own, else its layer's or kind's.
    pub names: Vec<String>,
    pub selected: Option<usize>,
    /// Text typed into a cell and not applied yet.
    pub drafts: BTreeMap<(usize, StackField), String>,
}

/// A copper kind in a row's Type dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindChoice {
    pub kind: StackLayerKind,
    pub label: String,
}

impl fmt::Display for KindChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The catalog name of a row kind.
pub fn kind_label(i18n: &I18n, kind: StackLayerKind) -> Localized {
    let id = match kind {
        StackLayerKind::Signal => ids::PCB_STACK_KIND_SIGNAL,
        StackLayerKind::Plane => ids::PCB_STACK_KIND_PLANE,
        StackLayerKind::Dielectric => ids::PCB_STACK_KIND_DIELECTRIC,
        StackLayerKind::SolderMask => ids::PCB_STACK_KIND_SOLDER_MASK,
        StackLayerKind::Paste => ids::PCB_STACK_KIND_PASTE,
        StackLayerKind::Overlay => ids::PCB_STACK_KIND_OVERLAY,
    };
    t!(i18n, id)
}

/// The manager, or its empty state with no board in view.
pub fn view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let Some(facts) = ctx.layer_stack.as_ref() else {
        return empty_state_card(
            &ctx.tokens,
            t!(ctx.i18n, ids::PCB_STACK_NO_BOARD_TITLE),
            t!(ctx.i18n, ids::PCB_STACK_NO_BOARD),
            None,
        );
    };
    column![
        header(ctx, facts),
        toolbar(ctx, facts),
        table::view(ctx, facts),
        notes(ctx, facts),
    ]
    .spacing(SP.s)
    .width(Length::Fill)
    .boxed()
}

fn msg(m: LayerStackMsg) -> PanelMsg {
    PanelMsg::LayerStack(m)
}

/// The total thickness and the copper count.
fn header<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let total = super::pcb_primitive_properties::format_number(facts.stack.total_thickness_mm());
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    container(
        column![
            text(t!(i18n, ids::PCB_STACK_TITLE))
                .size(TS.body)
                .font(semibold)
                .color(ti(ctx.tokens.text)),
            text(t!(i18n, ids::PCB_STACK_TOTAL, "value" => total))
                .size(TS.dense)
                .color(ti(ctx.tokens.text)),
            text(t!(
                i18n,
                ids::PCB_STACK_COPPER_COUNT,
                "count" => facts.stack.copper_count()
            ))
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3),
        ]
        .spacing(SP.xs),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// Add, Remove and Move buttons. Remove and Move need a selected row; none
/// is live on a read-only board.
fn toolbar<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let board = facts.board.clone();
    let live = !facts.read_only;
    let selected = live && facts.selected.is_some();
    let add = |id: MessageId, kind: StackLayerKind| {
        let board = board.clone();
        action(
            ctx,
            id,
            live.then(|| {
                msg(LayerStackMsg::LayerAdded {
                    board: board.clone(),
                    kind,
                })
            }),
            ids::PCB_STACK_ADD_TIP,
        )
    };
    let on_selected = |id: MessageId, m: LayerStackMsg, tip: MessageId| {
        let tip = if selected {
            tip
        } else {
            ids::PCB_STACK_SELECT_TIP
        };
        action(ctx, id, selected.then(|| msg(m)), tip)
    };
    let adds = row![
        add(ids::PCB_STACK_ADD_SIGNAL, StackLayerKind::Signal),
        add(ids::PCB_STACK_ADD_PLANE, StackLayerKind::Plane),
        add(ids::PCB_STACK_ADD_DIELECTRIC, StackLayerKind::Dielectric),
    ]
    .spacing(SP.xs);
    let edits = row![
        on_selected(
            ids::PCB_STACK_REMOVE,
            LayerStackMsg::RowRemoved {
                board: board.clone()
            },
            ids::PCB_STACK_REMOVE_TIP,
        ),
        on_selected(
            ids::PCB_STACK_MOVE_UP,
            LayerStackMsg::RowMoved {
                board: board.clone(),
                up: true,
            },
            ids::PCB_STACK_MOVE_UP,
        ),
        on_selected(
            ids::PCB_STACK_MOVE_DOWN,
            LayerStackMsg::RowMoved {
                board: board.clone(),
                up: false,
            },
            ids::PCB_STACK_MOVE_DOWN,
        ),
    ]
    .spacing(SP.xs);
    container(
        iced_aw::Wrap::new()
            .spacing(SP.xs)
            .line_spacing(SP.xs)
            .push(adds)
            .push(edits),
    )
    .padding([0.0, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// A ghost button with a hint; inert without a message.
fn action<'a>(
    ctx: &'a PanelContext,
    label: MessageId,
    on_press: Option<PanelMsg>,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let mut b = button(text(t!(ctx.i18n, label)).size(TS.dense))
        .padding([SP.xs, SP.s])
        .style(form_look::ghost_button);
    if let Some(m) = on_press {
        b = b.on_press(m);
    }
    super::graphite::hint_tooltip(
        b.boxed(),
        t!(ctx.i18n, tip),
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}

/// Why nothing can be edited, that the stack is not stored yet, and the
/// Enter hint while a cell holds a draft.
fn notes<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let note = |id| text(t!(ctx.i18n, id)).size(TS.dense).color(muted);
    let mut col = column![].spacing(SP.xs);
    if facts.read_only {
        col = col.push(note(ids::PCB_STACK_READ_ONLY));
    } else if !facts.stored {
        col = col.push(note(ids::PCB_STACK_DERIVED));
    }
    if !facts.drafts.is_empty() {
        col = col.push(note(ids::PCB_STACK_ENTER_HINT));
    }
    container(col)
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .boxed()
}
