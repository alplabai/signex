//! The two bars over the grid. The first: the domain tabs with the last
//! run's findings, the Table and Text switch and Run DRC. The second: the
//! find-a-constraint box, the Show inherited values and Overridden rows
//! only chips, Constraint sets, Add and Presets (and the Clearance
//! domain's Matrix switch). Also the info banner of the notes.

use iced::alignment::Vertical;
use iced::widget::{Row, Space, container, row};
use iced::{Background, Border, Element, Length, Padding, Theme, Widget as _};
use signex_drc_dsl::table::RuleKind;
use signex_drc_dsl::table::catalog::Domain;
use signex_i18n::{Localized, ids, t};
use signex_widgets::chips::{
    ChipMetrics, ChipStyle, ChipTone, TabChip, TabChipStyle, chip, tab_chips,
};
use signex_widgets::constraint_grid::GridMessage;
use signex_widgets::{SegmentedStyle, button, segmented, text, text_input};

use super::grid_icons::{Icon, icon};
use super::{Checked, Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::grid::domain_findings;
use crate::app::rules_tab::search::{column_label, domain_label, group_label, type_label};
use crate::app::rules_tab::{ClearanceView, RulesPanel, RulesTabMsg};
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, Radii, ti};

/// The find field is 250 px wide at the 12 px body.
const FIND_WIDTH_EM: f32 = 250.0 / 12.0;
/// The Table and Text switch is 150 px wide at the 12 px body.
const SWITCH_WIDTH_EM: f32 = 150.0 / 12.0;

fn chip_style(frame: &Frame) -> ChipStyle {
    let (tokens, r) = (frame.tokens, resolve(frame.tokens));
    ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: ti(tokens.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

fn metrics() -> ChipMetrics {
    ChipMetrics::from_text_size(TS.body)
}

/// The domain tabs, the Table and Text switch and Run DRC.
pub(super) fn domains<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let r = resolve(tokens);
    let style = TabChipStyle {
        hover: ti(tokens.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
        badge: ChipTone {
            fill: ti(tokens.error),
            ink: ti(tokens.paper),
        },
        zero_ink: r.ink4,
    };
    let results = frame.results();
    let tabs = Domain::ALL.into_iter().map(|domain| {
        if domain == Domain::SignalIntegrity {
            // Signal integrity arrives with the simulation band.
            return TabChip::new(domain, t!(i18n, ids::RULES_DOMAIN_SIGNAL_INTEGRITY_LATER))
                .dimmed(true);
        }
        let tab = TabChip::new(domain, t!(i18n, domain_label(domain)));
        match results {
            Some(results) => {
                let n = domain_findings(&frame.table, results, domain);
                tab.count(u32::try_from(n).unwrap_or(u32::MAX))
            }
            None => tab,
        }
    });
    let bar = tab_chips(
        tabs,
        frame.grid().domain(),
        style,
        metrics(),
        i18n,
        move |domain| {
            up(RulesTabMsg::DomainPicked {
                project_id: id,
                domain,
            })
        },
    );
    let in_text = frame.state.text.is_some();
    let switch = segmented(
        [
            (false, t!(i18n, ids::RULES_VIEW_TABLE).into()),
            (true, t!(i18n, ids::RULES_VIEW_TEXT).into()),
        ],
        in_text,
        SegmentedStyle::new(
            r.sunken,
            r.raised,
            r.shadow,
            r.ink3,
            ti(tokens.text),
            TS.body,
        )
        .fill_width(true),
        move |on| up(RulesTabMsg::TextViewToggled { project_id: id, on }),
    );
    let drc_label = if frame.running {
        ids::RULES_DRC_RUNNING
    } else {
        ids::RULES_RUN_DRC
    };
    let can_run = !frame.running && !matches!(*frame.checked, Checked::NoBoard);
    let run = button(
        row![
            icon(Icon::Play, TS.body, r.primary_ink),
            text(t!(i18n, drc_label)).size(TS.label),
        ]
        .spacing(SP.xs)
        .align_y(Vertical::Center),
    )
    .padding([SP.xs, SP.m])
    .style(crate::styles::primary_button(tokens))
    .on_press_maybe(can_run.then(|| up(RulesTabMsg::DrcRequested { project_id: id })));
    row![
        bar,
        Space::new().width(Length::Fill),
        container(switch).width(TS.body * SWITCH_WIDTH_EM),
        run,
    ]
    .spacing(SP.s)
    .align_y(Vertical::Center)
    .boxed()
}

/// The find box, its hint, the filter chips and the actions.
pub(super) fn find_bar<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens, id, state) = (frame.i18n, frame.tokens, frame.id, frame.state);
    let r = resolve(tokens);
    let input = text_input(
        t!(i18n, ids::RULES_FIND_PLACEHOLDER),
        state.find.query.as_str(),
    )
    .on_input(move |query| {
        up(RulesTabMsg::GridActed {
            project_id: id,
            msg: GridMessage::ColumnQueryChanged(query),
        })
    })
    .on_submit(up(RulesTabMsg::GridActed {
        project_id: id,
        msg: GridMessage::ColumnQuerySubmitted,
    }))
    .size(TS.body)
    .padding(0)
    .width(Length::Fill)
    .style(bare_input(frame));
    let field = container(
        row![icon(Icon::Search, TS.body, r.ink3), input]
            .spacing(SP.xs)
            .align_y(Vertical::Center),
    )
    .padding([SP.xs, SP.s])
    .width(TS.body * FIND_WIDTH_EM)
    .style(crate::styles::field(tokens, false));
    let style = chip_style(frame);
    let m = metrics();
    let mut bar: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.s)
        .align_y(Vertical::Center)
        .push(field.boxed())
        .push(find_hint(frame))
        .push(Space::new().width(Length::Fill).boxed());
    if frame.grid().domain() == Domain::Clearance {
        let matrix = state.clearance_view == ClearanceView::Matrix;
        let view = if matrix {
            ClearanceView::List
        } else {
            ClearanceView::Matrix
        };
        bar = bar.push(chip(
            t!(i18n, ids::RULES_VIEW_MATRIX),
            None,
            matrix,
            style,
            m,
            i18n,
            Some(up(RulesTabMsg::ClearanceViewPicked {
                project_id: id,
                view,
            })),
        ));
    }
    bar = bar
        .push(chip(
            t!(i18n, ids::RULES_CHIP_INHERITED),
            None,
            !state.hide_inherited,
            style,
            m,
            i18n,
            Some(up(RulesTabMsg::InheritedToggled { project_id: id })),
        ))
        .push(chip(
            t!(i18n, ids::RULES_CHIP_OVERRIDDEN_ONLY),
            None,
            state.overridden_only,
            style,
            m,
            i18n,
            Some(up(RulesTabMsg::OverriddenOnlyToggled { project_id: id })),
        ))
        .push(divider(frame))
        .push(sets_button(frame))
        .push(add_button(frame))
        .push(presets_button(frame));
    bar.boxed()
}

/// A text input drawn inside its field: no fill or edge of its own.
fn bare_input(
    frame: &Frame,
) -> impl Fn(&Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style + 'static
{
    let r = resolve(frame.tokens);
    let ink = ti(frame.tokens.text);
    move |_: &Theme, _| iced::widget::text_input::Style {
        background: Background::Color(iced::Color::TRANSPARENT),
        border: Border::default(),
        placeholder: r.ink4,
        value: ink,
        selection: r.accent_soft,
    }
}

/// What the find box landed on, how many it matched, or that it matched
/// nothing.
fn find_hint<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, find) = (frame.i18n, &frame.state.find);
    let muted = resolve(frame.tokens).ink3;
    let line: Localized = if let Some(hit) = find.current() {
        let place = t!(
            i18n,
            ids::RULES_FIND_AT,
            "type" => t!(i18n, type_label(hit.group.id)).into_string(),
            "domain" => t!(i18n, domain_label(hit.domain)).into_string(),
            "group" => t!(i18n, group_label(hit.group.id)).into_string(),
            "column" => t!(i18n, column_label(hit.column.id)).into_string()
        );
        if find.hits.len() > 1 {
            t!(
                i18n,
                ids::RULES_FIND_AT_OF,
                "place" => place.into_string(),
                "at" => find.at + 1,
                "count" => find.hits.len()
            )
        } else {
            place
        }
    } else if find.is_listing() && find.hits.is_empty() {
        t!(i18n, ids::RULES_FIND_NONE)
    } else if find.is_listing() {
        t!(i18n, ids::RULES_FIND_COUNT, "count" => find.hits.len())
    } else {
        return Space::new().width(Length::Shrink).boxed();
    };
    text(line).size(TS.caption).color(muted).boxed()
}

/// The hairline between the chips and the actions.
fn divider<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let line = ti(frame.tokens.border);
    container(Space::new())
        .width(HAIRLINE)
        .height(metrics().height * 0.75)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

/// A toolbar button, pressed-looking when `is_on`.
fn bar_button<'a>(
    frame: &Frame<'a>,
    content: Element<'a, Message>,
    is_on: bool,
    msg: Option<Message>,
) -> Element<'a, Message> {
    let tokens = frame.tokens;
    let face = button(content).padding([SP.xs, SP.m]).on_press_maybe(msg);
    if is_on {
        face.style(crate::styles::accent_button(tokens)).boxed()
    } else {
        face.style(crate::styles::outline_button(tokens)).boxed()
    }
}

/// Constraint sets, with how many the project has.
fn sets_button<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, id) = (frame.i18n, frame.id);
    let r = resolve(frame.tokens);
    let m = metrics();
    let count = frame.sets.len();
    let pill = container(
        text(user(signex_widgets::chips::count_text(
            i18n,
            u32::try_from(count).unwrap_or(u32::MAX),
        )))
        .size(m.badge_text),
    )
    .height(m.badge_height)
    .padding(Padding::from([0.0, m.badge_pad_h]))
    .align_y(Vertical::Center)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(r.sunken)),
        text_color: Some(r.ink3),
        border: Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let content = row![text(t!(i18n, ids::RULES_PANEL_SETS)).size(TS.label), pill]
        .spacing(SP.xs)
        .align_y(Vertical::Center)
        .boxed();
    let open = frame.state.panel == Some(RulesPanel::Sets);
    bar_button(
        frame,
        content,
        open,
        Some(up(RulesTabMsg::SetsToggled { project_id: id })),
    )
}

/// The kind Add writes: the selected column's, else the domain's first
/// checked one; `None` on a domain the check reads nothing of yet.
fn add_kind(frame: &Frame) -> Option<RuleKind> {
    let grid = frame.grid();
    let selected = grid
        .view
        .selected()
        .and_then(|c| grid.column(c.column))
        .and_then(|c| c.field);
    selected
        .or_else(|| {
            grid.groups()
                .iter()
                .flat_map(|g| g.columns.iter())
                .find_map(|c| c.field)
        })
        .map(|field| field.kind())
}

/// Add: a board-wide rule of the selected column's kind.
fn add_button<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, id) = (frame.i18n, frame.id);
    let ink = ti(frame.tokens.text);
    let msg = add_kind(frame)
        .filter(|_| !frame.table.is_unparsed())
        .map(|kind| {
            up(RulesTabMsg::RuleAdded {
                project_id: id,
                kind,
            })
        });
    let content = row![
        icon(Icon::Plus, TS.body, ink),
        text(t!(i18n, ids::RULES_GRID_ADD)).size(TS.label),
    ]
    .spacing(SP.xs)
    .align_y(Vertical::Center)
    .boxed();
    bar_button(frame, content, false, msg)
}

fn presets_button<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, id) = (frame.i18n, frame.id);
    let open = frame.state.panel == Some(RulesPanel::Presets);
    let content = text(t!(i18n, ids::RULES_PANEL_PRESETS))
        .size(TS.label)
        .boxed();
    bar_button(
        frame,
        content,
        open,
        Some(up(RulesTabMsg::PanelPicked {
            project_id: id,
            panel: (!open).then_some(RulesPanel::Presets),
        })),
    )
}

/// An info banner: the info icon and one line, on the info tint.
pub(super) fn info_banner<'a>(frame: &Frame<'a>, line: Localized) -> Element<'a, Message> {
    let r = resolve(frame.tokens);
    let ink = ti(frame.tokens.text);
    container(
        row![
            icon(Icon::Info, TS.body, r.info),
            text(line).size(TS.body).color(ink).width(Length::Fill),
        ]
        .spacing(SP.s)
        .align_y(Vertical::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(r.info_soft)),
        border: Border {
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}
