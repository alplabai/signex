//! The board Net Classes panel (#2400): the board's project net-class table
//! (the one the schematic's net classes and DRC read), one row per class:
//! name, colour, track width, clearance, via diameter and drill, diff-pair
//! gap and width, and the member nets as chips. A pure view over
//! [`BoardNetClassView`]; every edit is a [`BoardNetClassMsg`] the app runs
//! through the same path as its catalog command, one step of the project's
//! net-class history.

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{Column, Row, Space, container, pick_list, row, scrollable, svg};
use iced::{Element, Length, Widget as _};
use iced_aw::Wrap;
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::net_class::{NetClass, format_hex_color};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{PickListPlaceholder as _, button, raw, text, text_input, theme_ext};

use super::{PanelContext, PanelMsg};
use crate::icons;
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Column widths, from the body text size so they follow the UI scale.
const NAME_W: f32 = TS.body * 8.0;
const COLOR_W: f32 = TS.body * 6.0;
const RULE_W: f32 = TS.body * 4.5;
const NETS_W: f32 = TS.body * 16.0;

/// A typed cell of a class row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoardNetClassField {
    Name,
    Color,
    Width,
    Clearance,
    ViaDiameter,
    ViaDrill,
    DiffPairGap,
    DiffPairWidth,
}

impl BoardNetClassField {
    /// The board rule cells, in table order.
    pub const RULES: [Self; 6] = [
        Self::Width,
        Self::Clearance,
        Self::ViaDiameter,
        Self::ViaDrill,
        Self::DiffPairGap,
        Self::DiffPairWidth,
    ];

    fn heading(self) -> MessageId {
        match self {
            Self::Name => ids::BOARD_NET_CLASSES_COL_NAME,
            Self::Color => ids::BOARD_NET_CLASSES_COL_COLOR,
            Self::Width => ids::BOARD_NET_CLASSES_COL_WIDTH,
            Self::Clearance => ids::BOARD_NET_CLASSES_COL_CLEARANCE,
            Self::ViaDiameter => ids::BOARD_NET_CLASSES_COL_VIA,
            Self::ViaDrill => ids::BOARD_NET_CLASSES_COL_DRILL,
            Self::DiffPairGap => ids::BOARD_NET_CLASSES_COL_DP_GAP,
            Self::DiffPairWidth => ids::BOARD_NET_CLASSES_COL_DP_WIDTH,
        }
    }

    /// The name of the field's operand of `set_net_class_rules`; the name
    /// cell is `rename_net_class`'s `name`.
    pub fn operand(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Color => "color",
            Self::Width => "width",
            Self::Clearance => "clearance",
            Self::ViaDiameter => "via_diameter",
            Self::ViaDrill => "via_drill",
            Self::DiffPairGap => "diff_pair_gap",
            Self::DiffPairWidth => "diff_pair_width",
        }
    }

    /// The stored value as the cell shows it: an unset rule is empty.
    pub fn shown(self, class: &NetClass) -> String {
        match self {
            Self::Name => class.name.clone(),
            Self::Color => class.style.color.map(format_hex_color).unwrap_or_default(),
            rule => {
                let rules = class.board_rules();
                let mm = match rule {
                    Self::Width => rules.trace_width,
                    Self::Clearance => rules.clearance,
                    Self::ViaDiameter => rules.via_diameter,
                    Self::ViaDrill => rules.via_drill,
                    Self::DiffPairGap => rules.diff_pair_gap,
                    _ => rules.diff_pair_width,
                };
                if mm > 0.0 {
                    mm.to_string()
                } else {
                    String::new()
                }
            }
        }
    }
}

/// The text typed into one cell and not yet applied with Enter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardNetClassDraft {
    pub class: String,
    pub field: BoardNetClassField,
    pub text: String,
}

/// What the board Net Classes panel shows; `None` in the panel context with
/// no board in view.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardNetClassView {
    /// The board is in an open project, so it has a table to edit.
    pub in_project: bool,
    pub classes: Vec<NetClass>,
    /// The board's net names, sorted, for the pickers.
    pub nets: Vec<String>,
    pub can_undo: bool,
    pub can_redo: bool,
    /// The panel's own state, kept across refreshes.
    pub draft: Option<BoardNetClassDraft>,
    /// Why the last edit was refused.
    pub notice: Option<String>,
}

/// What the user did in the board Net Classes panel.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardNetClassMsg {
    /// Add Class: a new class named `Class <n>`.
    AddRequested,
    /// Delete on a class row.
    DeleteRequested(String),
    /// A cell's text changed.
    CellEdited {
        class: String,
        field: BoardNetClassField,
        text: String,
    },
    /// Enter in the cell being typed in: its text is applied.
    CellSubmitted,
    /// A net was picked into a class (out of every other class's list).
    NetPicked {
        class: String,
        net: String,
    },
    /// A member chip was pressed: the net leaves the class.
    NetRemoved {
        class: String,
        net: String,
    },
    /// The panel's Undo or Redo of the project's net-class history.
    UndoRequested,
    RedoRequested,
}

fn msg(event: BoardNetClassMsg) -> PanelMsg {
    PanelMsg::BoardNetClass(event)
}

fn note<'a>(ctx: &'a PanelContext, id: MessageId) -> Element<'a, PanelMsg> {
    container(
        text(t!(&*ctx.i18n, id))
            .size(TS.body)
            .color(theme_ext::text_secondary(&ctx.tokens)),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .into()
}

/// Add Class, Undo and Redo.
fn toolbar<'a>(ctx: &'a PanelContext, view: &BoardNetClassView) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let history = |id, on: bool, event| {
        dialog_button(
            t!(i18n, id),
            DialogButtonKind::Secondary,
            on.then(|| msg(event)),
            &ctx.tokens,
        )
    };
    row![
        dialog_button(
            t!(i18n, ids::BOARD_NET_CLASSES_ADD),
            DialogButtonKind::Primary,
            Some(msg(BoardNetClassMsg::AddRequested)),
            &ctx.tokens,
        ),
        Space::new().width(Length::Fill).height(Length::Shrink),
        history(
            ids::BOARD_NET_CLASSES_UNDO,
            view.can_undo,
            BoardNetClassMsg::UndoRequested
        ),
        history(
            ids::BOARD_NET_CLASSES_REDO,
            view.can_redo,
            BoardNetClassMsg::RedoRequested
        ),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .into()
}

fn heading<'a>(
    i18n: &I18n,
    id: MessageId,
    width: f32,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    container(
        text(t!(i18n, id))
            .size(TS.caption)
            .color(theme_ext::text_secondary(&ctx.tokens)),
    )
    .width(Length::Fixed(width))
    .into()
}

/// The column headings.
fn heading_row<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.xs);
    line = line.push(heading(
        i18n,
        BoardNetClassField::Name.heading(),
        NAME_W,
        ctx,
    ));
    line = line.push(heading(
        i18n,
        BoardNetClassField::Color.heading(),
        COLOR_W,
        ctx,
    ));
    for field in BoardNetClassField::RULES {
        line = line.push(heading(i18n, field.heading(), RULE_W, ctx));
    }
    line.push(heading(i18n, ids::BOARD_NET_CLASSES_COL_NETS, NETS_W, ctx))
        .into()
}

/// One typed cell: the draft while it is being typed in, else the value.
fn cell<'a>(
    ctx: &'a PanelContext,
    view: &BoardNetClassView,
    class: &NetClass,
    field: BoardNetClassField,
    width: f32,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let value = match &view.draft {
        Some(draft) if draft.class == class.name && draft.field == field => draft.text.clone(),
        _ => field.shown(class),
    };
    let hint = match field {
        BoardNetClassField::Name => t!(i18n, ids::BOARD_NET_CLASSES_COL_NAME),
        BoardNetClassField::Color => t!(i18n, ids::BOARD_NET_CLASSES_COLOR_HINT),
        _ => t!(i18n, ids::BOARD_NET_CLASSES_MM_HINT),
    };
    let name = class.name.clone();
    text_input(hint, value)
        .on_input(move |text| {
            msg(BoardNetClassMsg::CellEdited {
                class: name.clone(),
                field,
                text,
            })
        })
        .on_submit(msg(BoardNetClassMsg::CellSubmitted))
        .size(TS.body)
        .padding(SP.xs)
        .style(super::form_look::text_input_style)
        .width(Length::Fixed(width))
        .into()
}

/// The member nets as chips (a press takes one out), a note of the glob
/// patterns, and a picker of the board nets not in the class.
fn members<'a>(
    ctx: &'a PanelContext,
    view: &BoardNetClassView,
    class: &NetClass,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let mut chips = Wrap::new().spacing(SP.xs).line_spacing(SP.xs);
    for net in &class.nets {
        let glyph = svg(icons::icon_close(ctx.theme_id)).width(10).height(10);
        let chip = button(
            row![text(UserText::new(net.clone())).size(TS.caption), glyph]
                .spacing(SP.xs)
                .align_y(iced::Alignment::Center),
        )
        .padding([2.0, SP.s])
        .on_press(msg(BoardNetClassMsg::NetRemoved {
            class: class.name.clone(),
            net: net.clone(),
        }))
        .style(super::form_look::option_tile(true));
        chips = chips.push(super::graphite::hint_tooltip(
            chip.into(),
            t!(i18n, ids::BOARD_NET_CLASSES_REMOVE_NET, "net" => net.clone()),
            &ctx.tokens,
            iced::widget::tooltip::Position::Bottom,
        ));
    }
    if !class.patterns.is_empty() {
        chips = chips.push(
            text(t!(i18n, ids::BOARD_NET_CLASSES_PATTERNS, "count" => class.patterns.len()))
                .size(TS.caption)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    }
    let free: Vec<String> = view
        .nets
        .iter()
        .filter(|net| !class.nets.contains(net))
        .cloned()
        .collect();
    let mut cell = Column::new().spacing(SP.xs).width(Length::Fixed(NETS_W));
    cell = cell.push(chips);
    if !free.is_empty() {
        let name = class.name.clone();
        cell = cell.push(
            pick_list(None::<String>, free, |net: &String| net.clone())
                .graphite()
                .on_select(move |net: String| {
                    msg(BoardNetClassMsg::NetPicked {
                        class: name.clone(),
                        net,
                    })
                })
                .placeholder_label(t!(i18n, ids::BOARD_NET_CLASSES_ADD_NET))
                .width(Length::Fill)
                .boxed(),
        );
    }
    cell.into()
}

/// One class: its cells, its members and Delete.
fn class_row<'a>(
    ctx: &'a PanelContext,
    view: &BoardNetClassView,
    class: &NetClass,
) -> Element<'a, PanelMsg> {
    let mut line: Row<Element<'a, PanelMsg>> =
        Row::new().spacing(SP.xs).align_y(iced::Alignment::Start);
    line = line.push(cell(ctx, view, class, BoardNetClassField::Name, NAME_W));
    line = line.push(cell(ctx, view, class, BoardNetClassField::Color, COLOR_W));
    for field in BoardNetClassField::RULES {
        line = line.push(cell(ctx, view, class, field, RULE_W));
    }
    line = line.push(members(ctx, view, class));
    line.push(dialog_button(
        t!(&*ctx.i18n, ids::BOARD_NET_CLASSES_DELETE),
        DialogButtonKind::Danger,
        Some(msg(BoardNetClassMsg::DeleteRequested(class.name.clone()))),
        &ctx.tokens,
    ))
    .into()
}

/// The panel for the board in view.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let Some(view) = ctx.net_classes.board.as_ref() else {
        return note(ctx, ids::BOARD_NET_CLASSES_NO_BOARD);
    };
    if !view.in_project {
        return note(ctx, ids::BOARD_NET_CLASSES_NO_PROJECT);
    }
    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.s)
        .padding([SP.s, SP.m])
        .width(Length::Fill);
    col = col.push(toolbar(ctx, view));
    if let Some(notice) = &view.notice {
        col = col.push(
            text(raw(notice.clone()))
                .size(TS.caption)
                .color(theme_ext::warning_color(&ctx.tokens)),
        );
    }
    if view.classes.is_empty() {
        return col.push(note(ctx, ids::BOARD_NET_CLASSES_EMPTY)).into();
    }
    let mut table: Column<Element<'a, PanelMsg>> =
        Column::new().spacing(SP.s).push(heading_row(ctx));
    for class in &view.classes {
        table = table.push(class_row(ctx, view, class));
    }
    col.push(scrollable(table).direction(Direction::Horizontal(Scrollbar::default())))
        .into()
}

#[cfg(test)]
#[path = "board_net_classes_tests.rs"]
mod tests;
