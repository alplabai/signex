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
use signex_widgets::{PickListPlaceholder as _, button, text, text_input, theme_ext};

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
    .boxed()
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
    .boxed()
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
    .boxed()
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
        .boxed()
}

/// One typed cell: the draft while it is being typed in, else the value.
fn cell<'a>(
    ctx: &'a PanelContext,
    view: &BoardNetClassView,
    class: &NetClass,
    field: BoardNetClassField,
    width: f32,
) -> Element<'a, PanelMsg> {
    let hint = match field {
        BoardNetClassField::Name => ids::BOARD_NET_CLASSES_COL_NAME,
        BoardNetClassField::Color => ids::BOARD_NET_CLASSES_COLOR_HINT,
        _ => ids::BOARD_NET_CLASSES_MM_HINT,
    };
    hinted_cell(ctx, view, class, field, width, hint)
}

/// [`cell`] with `hint` in the empty field.
fn hinted_cell<'a>(
    ctx: &'a PanelContext,
    view: &BoardNetClassView,
    class: &NetClass,
    field: BoardNetClassField,
    width: f32,
    hint: MessageId,
) -> Element<'a, PanelMsg> {
    let value = match &view.draft {
        Some(draft) if draft.class == class.name && draft.field == field => draft.text.clone(),
        _ => field.shown(class),
    };
    let hint = t!(&*ctx.i18n, hint);
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
        .boxed()
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
        let glyph = svg(icons::icon_close(ctx.theme_id))
            .width(TS.caption)
            .height(TS.caption);
        let chip = button(
            row![text(UserText::new(net.clone())).size(TS.caption), glyph]
                .spacing(SP.xs)
                .align_y(iced::Alignment::Center),
        )
        .padding([SP.xs, SP.s])
        .on_press(msg(BoardNetClassMsg::NetRemoved {
            class: class.name.clone(),
            net: net.clone(),
        }))
        .style(super::form_look::option_tile(true));
        chips = chips.push(super::graphite::hint_tooltip(
            chip.boxed(),
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
    let mut cell = Column::new().spacing(SP.xs).width(Length::Fixed(NETS_W));
    cell = cell.push(Element::from(chips));
    if let Some(picker) = add_net(ctx, view, class) {
        cell = cell.push(picker);
    }
    cell.boxed()
}

/// Add net: a picker of the board nets not in the class; none when every
/// board net is in it.
fn add_net<'a>(
    ctx: &'a PanelContext,
    view: &BoardNetClassView,
    class: &NetClass,
) -> Option<Element<'a, PanelMsg>> {
    let free: Vec<String> = view
        .nets
        .iter()
        .filter(|net| !class.nets.contains(net))
        .cloned()
        .collect();
    if free.is_empty() {
        return None;
    }
    let name = class.name.clone();
    Some(
        pick_list(None::<String>, free, |net: &String| net.clone())
            .graphite()
            .on_select(move |net: String| {
                msg(BoardNetClassMsg::NetPicked {
                    class: name.clone(),
                    net,
                })
            })
            .placeholder_label(t!(&*ctx.i18n, ids::BOARD_NET_CLASSES_ADD_NET))
            .width(Length::Fill)
            .boxed(),
    )
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
    .boxed()
}

/// The board's class table to show, or why there is none: no board in
/// view, or a board in no open project.
pub fn table_in_view<'a>(
    ctx: &'a PanelContext,
) -> Result<&'a BoardNetClassView, Element<'a, PanelMsg>> {
    match ctx.net_classes.board.as_ref() {
        None => Err(note(ctx, ids::BOARD_NET_CLASSES_NO_BOARD)),
        Some(view) if !view.in_project => Err(note(ctx, ids::BOARD_NET_CLASSES_NO_PROJECT)),
        Some(view) => Ok(view),
    }
}

/// Add Class, Undo, Redo and why the last edit was refused.
fn head<'a>(ctx: &'a PanelContext, view: &BoardNetClassView) -> Column<Element<'a, PanelMsg>> {
    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.s)
        .padding([SP.s, SP.m])
        .width(Length::Fill);
    col = col.push(toolbar(ctx, view));
    if let Some(notice) = &view.notice {
        col = col.push(
            text(t!(&*ctx.i18n, ids::BOARD_NET_CLASSES_NOT_CHANGED, "why" => notice.as_str()))
                .size(TS.caption)
                .color(theme_ext::warning_color(&ctx.tokens))
                .boxed(),
        );
    }
    col
}

/// The panel for the board in view.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let view = match table_in_view(ctx) {
        Ok(view) => view,
        Err(why) => return why,
    };
    let col = head(ctx, view);
    if view.classes.is_empty() {
        return col.push(note(ctx, ids::BOARD_NET_CLASSES_EMPTY)).boxed();
    }
    let mut table: Column<Element<'a, PanelMsg>> =
        Column::new().spacing(SP.s).push(heading_row(ctx));
    for class in &view.classes {
        table = table.push(class_row(ctx, view, class));
    }
    col.push(
        scrollable(table)
            .direction(Direction::Horizontal(Scrollbar::default()))
            .boxed(),
    )
    .boxed()
}

// The PCB panel's Net Classes mode (mockup row 106): the classes as a list
// over the selected class's rules and members.

/// Width of the label column of the class detail.
const LABEL_W: f32 = TS.body * 7.0;

/// The class list's head: Add Class, Undo, Redo, why the last edit was
/// refused, and the precedence rule.
pub fn list_head<'a>(ctx: &'a PanelContext, view: &BoardNetClassView) -> Element<'a, PanelMsg> {
    head(ctx, view)
        .push(
            text(t!(&*ctx.i18n, ids::PANELS_PCB_CLASSES_FIRST_WINS))
                .size(TS.caption)
                .color(theme_ext::text_secondary(&ctx.tokens))
                .boxed(),
        )
        .boxed()
}

/// A class's rules in one line, `0.500 / 0.250 · via 0.800/0.400`: an
/// unset rule reads `—`, and the via part shows once a via rule is set.
/// Engineering values: `.` in every locale.
pub fn rule_summary(i18n: &I18n, class: &NetClass) -> signex_i18n::Localized {
    let rules = class.board_rules();
    let mm = |value: f64| {
        if value > 0.0 {
            format!("{value:.3}")
        } else {
            t!(i18n, ids::PANELS_PCB_CLASSES_UNSET).into_string()
        }
    };
    let line = t!(
        i18n,
        ids::PANELS_PCB_CLASSES_RULE_SUMMARY,
        "width" => mm(rules.trace_width),
        "clearance" => mm(rules.clearance)
    );
    if rules.via_diameter > 0.0 || rules.via_drill > 0.0 {
        t!(
            i18n,
            ids::PANELS_PCB_CLASSES_VIA_SUMMARY,
            "rules" => line.into_string(),
            "diameter" => mm(rules.via_diameter),
            "drill" => mm(rules.via_drill)
        )
    } else {
        line
    }
}

/// How many nets a class names, and how many patterns once it has any.
pub fn member_count(i18n: &I18n, class: &NetClass) -> signex_i18n::Localized {
    if class.patterns.is_empty() {
        t!(i18n, ids::PANELS_PCB_CLASSES_NETS, "count" => class.nets.len())
    } else {
        t!(
            i18n,
            ids::PANELS_PCB_CLASSES_NETS_PATTERNS,
            "nets" => class.nets.len(),
            "patterns" => class.patterns.len()
        )
    }
}

/// The class's colour as a dot; a ring when it has none.
fn swatch<'a>(ctx: &PanelContext, class: &NetClass) -> Element<'a, PanelMsg> {
    let fill = class.style.color.map(crate::styles::ti);
    let edge = theme_ext::text_secondary(&ctx.tokens);
    container(Space::new())
        .width(TS.caption)
        .height(TS.caption)
        .style(move |_: &iced::Theme| container::Style {
            background: fill.map(iced::Background::Color),
            border: iced::Border {
                color: edge,
                width: if fill.is_some() {
                    0.0
                } else {
                    crate::styles::HAIRLINE
                },
                radius: (TS.caption / 2.0).into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// One class of the list: dot, name, rule summary and member count; a
/// click selects it.
fn class_item<'a>(
    ctx: &'a PanelContext,
    class: &NetClass,
    is_selected: bool,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let name = Column::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(
            text(UserText::new(class.name.clone()))
                .size(TS.body)
                .color(crate::styles::ti(ctx.tokens.text))
                .boxed(),
        )
        .push(
            text(rule_summary(i18n, class))
                .size(TS.caption)
                .font(mono)
                .color(muted)
                .boxed(),
        );
    let line = row![
        swatch(ctx, class),
        name.boxed(),
        text(member_count(i18n, class))
            .size(TS.caption)
            .color(muted)
            .boxed(),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    button(line)
        .width(Length::Fill)
        .padding([SP.s, SP.m])
        .on_press(PanelMsg::PcbPanel(super::PcbPanelMsg::ClassClicked(
            class.name.clone(),
        )))
        .style(crate::styles::hover_row(&ctx.tokens, is_selected))
        .boxed()
}

/// The classes in table order, or why there are none.
pub fn class_list<'a>(
    ctx: &'a PanelContext,
    view: &'a BoardNetClassView,
) -> Column<Element<'a, PanelMsg>> {
    let mut list: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    if view.classes.is_empty() {
        list = list.push(note(ctx, ids::BOARD_NET_CLASSES_EMPTY));
    }
    let selected = ctx.pcb_panel.class.as_deref();
    for class in &view.classes {
        list = list.push(class_item(
            ctx,
            class,
            selected == Some(class.name.as_str()),
        ));
    }
    list
}

/// A detail row: its label, then `value`.
fn detail_row<'a>(
    ctx: &PanelContext,
    label: MessageId,
    value: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![
        container(
            text(t!(&*ctx.i18n, label))
                .size(TS.body)
                .color(theme_ext::text_secondary(&ctx.tokens))
        )
        .width(Length::Fixed(LABEL_W))
        .boxed(),
        value,
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// Two rule fields of a row and their unit; an empty field reads `board`.
/// The pair detail edits its class's Gap / Width through it too.
pub fn rule_pair<'a>(
    ctx: &'a PanelContext,
    view: &BoardNetClassView,
    class: &NetClass,
    fields: [BoardNetClassField; 2],
) -> Element<'a, PanelMsg> {
    let board = ids::PANELS_PCB_BOARD_VALUE;
    row![
        hinted_cell(ctx, view, class, fields[0], RULE_W, board),
        hinted_cell(ctx, view, class, fields[1], RULE_W, board),
        text(t!(&*ctx.i18n, ids::BOARD_NET_CLASSES_MM_HINT))
            .size(TS.caption)
            .color(theme_ext::text_secondary(&ctx.tokens))
            .boxed(),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// One exact member: its name, nodes and routed length on this board, and
/// the button that takes it out of the class.
fn member_row<'a>(
    ctx: &'a PanelContext,
    class: &NetClass,
    net: &str,
    facts: Option<&super::pcb_nets::PcbNetRow>,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let unset = || t!(i18n, ids::PANELS_PCB_CLASSES_UNSET);
    let nodes = facts.map_or_else(
        unset,
        |n| t!(i18n, ids::PANELS_PCB_CLASSES_NODES, "count" => n.nodes),
    );
    let length = match facts.map(|n| n.length_mm) {
        Some(mm) if mm > 0.0 => {
            t!(i18n, ids::PANELS_PCB_NETS_DETAIL_MM, "length" => format!("{mm:.2}"))
        }
        _ => t!(i18n, ids::PANELS_PCB_NETS_NO_LENGTH),
    };
    let glyph = svg(icons::icon_close(ctx.theme_id))
        .width(TS.caption)
        .height(TS.caption);
    let remove = button(glyph)
        .padding(SP.xs)
        .on_press(msg(BoardNetClassMsg::NetRemoved {
            class: class.name.clone(),
            net: net.to_owned(),
        }))
        .style(crate::styles::hover_row(&ctx.tokens, false));
    let remove = super::graphite::hint_tooltip(
        remove.boxed(),
        t!(i18n, ids::BOARD_NET_CLASSES_REMOVE_NET, "net" => net.to_owned()),
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    row![
        text(UserText::new(net.to_owned()))
            .size(TS.body)
            .font(crate::fonts::iced_font_for_family(
                crate::fonts::CHROME_MONO_FONT
            ))
            .color(crate::styles::ti(ctx.tokens.text))
            .width(Length::Fill)
            .boxed(),
        text(nodes).size(TS.caption).color(muted).boxed(),
        text(length).size(TS.caption).color(muted).boxed(),
        remove,
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// One glob pattern of the class and how many board nets it matches.
fn pattern_row<'a>(ctx: &PanelContext, pattern: &str, matches: usize) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    row![
        text(UserText::new(pattern.to_owned()))
            .size(TS.body)
            .color(muted)
            .width(Length::Fill)
            .boxed(),
        text(t!(&*ctx.i18n, ids::PANELS_PCB_CLASSES_MATCHES, "count" => matches))
            .size(TS.caption)
            .color(muted)
            .boxed(),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The selected class: its name and colour, its rules (Width / Clear., Via,
/// Diff pair, Impedance), Delete, its member nets and patterns with what
/// they hold on the board, and Add net. Every edit goes through the class
/// table's own messages; this detail is the board's only class editor.
pub fn class_detail<'a>(
    ctx: &'a PanelContext,
    view: &BoardNetClassView,
    class: &NetClass,
    nets: &[super::pcb_nets::PcbNetRow],
) -> Element<'a, PanelMsg> {
    use BoardNetClassField as F;
    let i18n = &*ctx.i18n;
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let caption = |label: signex_i18n::Localized| text(label).size(TS.section).color(muted);
    let title = row![
        caption(t!(i18n, ids::PANELS_PCB_CLASSES_RULES, "class" => class.name.clone()))
            .width(Length::Fill)
            .boxed(),
        dialog_button(
            t!(i18n, ids::BOARD_NET_CLASSES_DELETE),
            DialogButtonKind::Danger,
            Some(msg(BoardNetClassMsg::DeleteRequested(class.name.clone()))),
            &ctx.tokens,
        ),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    let impedance = super::graphite::hint_tooltip(
        text(t!(i18n, ids::PANELS_PCB_CLASSES_UNSET))
            .size(TS.body)
            .color(muted)
            .boxed(),
        t!(i18n, ids::PANELS_PCB_CLASSES_IMPEDANCE_TIP),
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.s)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .push(title.boxed())
        .push(detail_row(
            ctx,
            ids::BOARD_NET_CLASSES_COL_NAME,
            cell(ctx, view, class, F::Name, NAME_W),
        ))
        .push(detail_row(
            ctx,
            ids::BOARD_NET_CLASSES_COL_COLOR,
            row![
                swatch(ctx, class),
                cell(ctx, view, class, F::Color, COLOR_W)
            ]
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center)
            .boxed(),
        ))
        .push(detail_row(
            ctx,
            ids::PANELS_PCB_CLASSES_WIDTH_CLEARANCE,
            rule_pair(ctx, view, class, [F::Width, F::Clearance]),
        ))
        .push(detail_row(
            ctx,
            ids::PANELS_PCB_CLASSES_VIA,
            rule_pair(ctx, view, class, [F::ViaDiameter, F::ViaDrill]),
        ))
        .push(detail_row(
            ctx,
            ids::PANELS_PCB_CLASSES_DIFF_PAIR,
            rule_pair(ctx, view, class, [F::DiffPairGap, F::DiffPairWidth]),
        ))
        .push(detail_row(
            ctx,
            ids::PANELS_PCB_CLASSES_IMPEDANCE,
            impedance,
        ))
        .push(caption(t!(i18n, ids::PANELS_PCB_CLASSES_MEMBERS)).boxed());
    for net in &class.nets {
        let facts = nets.iter().find(|n| &n.name == net);
        col = col.push(member_row(ctx, class, net, facts));
    }
    for pattern in &class.patterns {
        let matches = nets
            .iter()
            .filter(|n| signex_types::net_class::glob_matches(pattern, &n.name))
            .count();
        col = col.push(pattern_row(ctx, pattern, matches));
    }
    if let Some(picker) = add_net(ctx, view, class) {
        col = col.push(picker);
    }
    scrollable(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::form_look::scrollbar_style)
        .boxed()
}

#[cfg(test)]
#[path = "board_net_classes_tests.rs"]
mod tests;
