//! Panel (#2410): the board's manufacturing panel. Without one, a line that
//! says what a panel is and Panelize Board; with one, its settings (copies,
//! gaps, rails, marks, the separation and the tabs), what it comes to, and
//! Remove Panel. Each edit is one undo step through `set_board_panel`'s
//! path. Rules: `docs/PANELISATION.md`.

use iced::Element;
use pcb_geom::panel::layout;
use signex_i18n::{Localized, MessageId, ids, t};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_panel::{BoardPanel, PanelField, PanelFieldKind, PanelSeparation};

use super::chrome::{Form, Head, Segment, Shown};
use super::{BoardField, PanelMsg, PcbBoardMsg, PcbBoardSummary, fields};

fn msg(m: PcbBoardMsg) -> Option<PanelMsg> {
    Some(PanelMsg::PcbBoard(m))
}

/// What the board's panel comes to, worked out when the summary is made.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelFacts {
    pub width_mm: f64,
    pub height_mm: f64,
    pub copies: usize,
    pub tabs: usize,
    pub bite_holes: usize,
    pub v_scores: usize,
    pub rectangular: bool,
}

impl PanelFacts {
    /// `None` without a panel; the reason when it does not lay out.
    pub fn of(board: &PcbBoard) -> Option<Result<Self, String>> {
        let panel = board.options.panel.as_ref()?;
        Some(
            layout(&board.outline, panel)
                .map(|laid| Self {
                    width_mm: laid.width_mm(),
                    height_mm: laid.height_mm(),
                    copies: laid.copies.len(),
                    tabs: laid.tabs,
                    bite_holes: laid.bite_holes.len(),
                    v_scores: laid.v_scores.len(),
                    rectangular: laid.rectangular,
                })
                .map_err(|e| e.to_string()),
        )
    }
}

fn label(field: PanelField) -> MessageId {
    match field {
        PanelField::Columns => ids::BOARD_PANEL_COLUMNS,
        PanelField::Rows => ids::BOARD_PANEL_ROWS,
        PanelField::SpacingX => ids::BOARD_PANEL_GAP_X,
        PanelField::SpacingY => ids::BOARD_PANEL_GAP_Y,
        PanelField::RailTop => ids::BOARD_PANEL_RAIL_TOP,
        PanelField::RailBottom => ids::BOARD_PANEL_RAIL_BOTTOM,
        PanelField::RailLeft => ids::BOARD_PANEL_RAIL_LEFT,
        PanelField::RailRight => ids::BOARD_PANEL_RAIL_RIGHT,
        PanelField::FiducialDiameter => ids::BOARD_PANEL_FIDUCIAL,
        PanelField::ToolingHoleDiameter => ids::BOARD_PANEL_TOOLING_HOLE,
        PanelField::Separation => ids::BOARD_PANEL_SEPARATION,
        PanelField::BiteHoleDiameter => ids::BOARD_PANEL_BITE_HOLE,
        PanelField::BitePitch => ids::BOARD_PANEL_BITE_PITCH,
        PanelField::TabWidth => ids::BOARD_PANEL_TAB_WIDTH,
        PanelField::TabsPerEdge => ids::BOARD_PANEL_TABS_PER_EDGE,
    }
}

/// The rows of each group, in the order they show.
const ARRAY: [PanelField; 4] = [
    PanelField::Columns,
    PanelField::Rows,
    PanelField::SpacingX,
    PanelField::SpacingY,
];
const RAILS: [PanelField; 4] = [
    PanelField::RailTop,
    PanelField::RailBottom,
    PanelField::RailLeft,
    PanelField::RailRight,
];
const MARKS: [PanelField; 2] = [
    PanelField::FiducialDiameter,
    PanelField::ToolingHoleDiameter,
];
const BITES: [PanelField; 4] = [
    PanelField::BiteHoleDiameter,
    PanelField::BitePitch,
    PanelField::TabWidth,
    PanelField::TabsPerEdge,
];

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &'a PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let title = form.t(ids::BOARD_PANEL_SECTION);
    let why = board.read_only.then(|| form.t(ids::BOARD_PANEL_READ_ONLY));
    let live = |m: PcbBoardMsg| (!board.read_only).then_some(m).and_then(msg);
    let mut rows: Vec<Shown<'a>> = Vec::new();
    let Some(panel) = board.options.panel.as_ref() else {
        let none = form.t(ids::BOARD_PANEL_NONE);
        rows.push(form.row(&[&title, &none], form.note(none.clone())));
        let add = form.t(ids::BOARD_PANEL_ADD);
        let button = form.action(add.clone(), live(PcbBoardMsg::PanelizeClicked), why);
        rows.push(form.row(&[&title, &add], button));
        let head = Head::closed().summary(form.header_caption(form.t(ids::PCB_BOARD_PANEL_NONE)));
        return form.section_with("pcb_board_panel", ids::BOARD_PANEL_SECTION, head, rows);
    };
    for field in ARRAY {
        rows.push(setting(form, board, panel, field));
    }
    let rails = form.t(ids::BOARD_PANEL_RAILS);
    rows.push(form.row(&[&rails], form.caption(rails.clone())));
    for field in RAILS {
        rows.push(setting(form, board, panel, field));
    }
    let marks = form.t(ids::BOARD_PANEL_MARKS);
    rows.push(form.row(&[&marks], form.caption(marks.clone())));
    for field in MARKS {
        rows.push(setting(form, board, panel, field));
    }
    rows.push(separation(form, board, panel));
    if panel.separation == PanelSeparation::MouseBites {
        for field in BITES {
            rows.push(setting(form, board, panel, field));
        }
    }
    rows.extend(summary(form, board));
    let remove = form.t(ids::BOARD_PANEL_REMOVE);
    let button = form.action(remove.clone(), live(PcbBoardMsg::PanelRemoveClicked), why);
    rows.push(form.row(&[&title, &remove], button));
    // Closed by default (#2523, mockup row 93), as without a panel.
    form.section_with(
        "pcb_board_panel",
        ids::BOARD_PANEL_SECTION,
        Head::closed(),
        rows,
    )
}

/// Label | the setting's field, or its value when the board is read-only.
fn setting<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
    panel: &BoardPanel,
    field: PanelField,
) -> Shown<'a> {
    let name = form.t(label(field));
    let key = BoardField::Panel(field);
    let value = match field.kind() {
        PanelFieldKind::Length => {
            let mm = field.length_mm(panel).unwrap_or_default();
            if board.read_only {
                form.value(fields::measure(form, mm))
            } else {
                fields::length(form, board, key, name.clone(), mm)
            }
        }
        _ => {
            let count = field.count(panel).unwrap_or_default().to_string();
            if board.read_only {
                form.value(signex_i18n::UserText::new(count))
            } else {
                fields::text(form, board, key, name.clone(), count)
            }
        }
    };
    form.row(&[&name], form.field(&name, value))
}

/// Mouse Bites | V-Score.
fn separation<'a>(form: &Form<'a>, board: &PcbBoardSummary, panel: &BoardPanel) -> Shown<'a> {
    let name = form.t(ids::BOARD_PANEL_SEPARATION);
    let cell = |value: PanelSeparation, id: MessageId| Segment {
        label: form.t(id),
        active: panel.separation == value,
        msg: (!board.read_only)
            .then_some(PcbBoardMsg::PanelSeparationPicked(value))
            .and_then(msg),
    };
    let toggle = form.segmented(vec![
        cell(PanelSeparation::MouseBites, ids::BOARD_PANEL_MOUSE_BITES),
        cell(PanelSeparation::VScore, ids::BOARD_PANEL_V_SCORE),
    ]);
    form.row(&[&name], form.field(&name, toggle))
}

/// The panel's size and copies, its tabs or score lines, and a warning for
/// an outline that is not a rectangle; or why it does not lay out.
fn summary<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Vec<Shown<'a>> {
    let line = |text: Localized| form.row(&[&text], form.note(text.clone()));
    let facts = match &board.panel {
        Some(Ok(facts)) => facts,
        Some(Err(why)) => {
            let text = t!(form.i18n, ids::BOARD_PANEL_LAYOUT_ERROR, "why" => why.as_str());
            return vec![line(text)];
        }
        None => return Vec::new(),
    };
    let measure = |mm: f64| fields::measure(form, mm).into_string();
    let mut lines = vec![line(t!(
        form.i18n,
        ids::BOARD_PANEL_SIZE,
        "width" => measure(facts.width_mm),
        "height" => measure(facts.height_mm),
        "copies" => facts.copies
    ))];
    lines.push(line(if facts.v_scores > 0 {
        t!(form.i18n, ids::BOARD_PANEL_SCORES, "count" => facts.v_scores)
    } else {
        t!(
            form.i18n,
            ids::BOARD_PANEL_TABS,
            "tabs" => facts.tabs,
            "holes" => facts.bite_holes
        )
    }));
    if !facts.rectangular {
        lines.push(line(form.t(ids::BOARD_PANEL_NOT_RECTANGULAR)));
    }
    lines
}
