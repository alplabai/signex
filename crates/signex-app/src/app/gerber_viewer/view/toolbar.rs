//! The toolbar (mockup row 193): Open (its menu, Ctrl+O), Flip, Drills,
//! mm / mil and Highlight net. With nothing open the tools that need files
//! are greyed with "Open files first" (mockup row 193f); Highlight net is
//! greyed with its reason when the files carry no X2 nets or nothing on a
//! net is selected.

use iced::{Element, Widget as _};
use signex_i18n::{MessageId, ids};
use signex_widgets::doc_tab_shell::Toolbar;
use signex_widgets::{button, segmented, text};

use super::{Look, SP, TS};
use crate::app::gerber_viewer::GerberViewerMsg;
use crate::app::gerber_viewer::state::{GerberViewerState, LengthUnit};
use crate::panels::graphite::hint_tooltip;
use crate::styles::ti;
use crate::styles::tokens::resolve;

pub(super) fn view<'a>(
    look: Look<'a>,
    viewer: &'a GerberViewerState,
) -> Toolbar<'a, GerberViewerMsg> {
    let open = viewer.open.as_ref();
    let flipped = open.is_some_and(|open| open.flipped);
    let drills = open.is_some_and(|open| open.drills_on_top);
    let mut bar = Toolbar::new()
        .push(tool(
            look,
            ids::GERBER_VIEWER_OPEN,
            viewer.open_menu,
            Some(GerberViewerMsg::OpenMenuToggled),
            ids::GERBER_VIEWER_OPEN_TIP,
        ))
        .separator()
        .push(needs_files(
            look,
            viewer,
            ids::GERBER_VIEWER_FLIP,
            flipped,
            GerberViewerMsg::FlipToggled,
            ids::GERBER_VIEWER_FLIP_TIP,
        ))
        .push(needs_files(
            look,
            viewer,
            ids::GERBER_VIEWER_DRILLS,
            drills,
            GerberViewerMsg::DrillsToggled,
            ids::GERBER_VIEWER_DRILLS_TIP,
        ))
        .push(highlight_net(look, viewer))
        .separator()
        .push(units(look, viewer));
    if viewer.reading.is_some() {
        bar = bar.push_end(
            text(look.t(ids::GERBER_VIEWER_READING))
                .size(TS.dense)
                .color(resolve(look.tokens).ink3)
                .boxed(),
        );
    }
    bar
}

/// A tool that acts only on open files.
fn needs_files<'a>(
    look: Look<'a>,
    viewer: &GerberViewerState,
    label: MessageId,
    on: bool,
    event: GerberViewerMsg,
    tip: MessageId,
) -> Element<'a, GerberViewerMsg> {
    if viewer.has_files() {
        tool(look, label, on, Some(event), tip)
    } else {
        tool(look, label, false, None, ids::GERBER_VIEWER_NEEDS_FILES)
    }
}

/// Highlight net, or why it cannot work now.
fn highlight_net<'a>(look: Look<'a>, viewer: &GerberViewerState) -> Element<'a, GerberViewerMsg> {
    let label = ids::GERBER_VIEWER_HIGHLIGHT_NET;
    let Some(open) = viewer.open.as_ref() else {
        return tool(look, label, false, None, ids::GERBER_VIEWER_NEEDS_FILES);
    };
    let on = open.net.is_some();
    let why_not = if !open.package.has_net_attributes() {
        Some(ids::GERBER_VIEWER_HIGHLIGHT_NET_NO_X2)
    } else if !on && open.selected_net().is_none() {
        Some(ids::GERBER_VIEWER_HIGHLIGHT_NET_NO_NET)
    } else {
        None
    };
    match why_not {
        Some(reason) => tool(look, label, false, None, reason),
        None => tool(
            look,
            label,
            on,
            Some(GerberViewerMsg::NetHighlightToggled),
            ids::GERBER_VIEWER_HIGHLIGHT_NET_TIP,
        ),
    }
}

/// A toolbar button with its hint: accent text while on, greyed and deaf
/// without a message.
fn tool<'a>(
    look: Look<'a>,
    label: MessageId,
    on: bool,
    on_press: Option<GerberViewerMsg>,
    tip: MessageId,
) -> Element<'a, GerberViewerMsg> {
    let r = resolve(look.tokens);
    let ink = match (on_press.is_some(), on) {
        (false, _) => r.ink4,
        (true, true) => ti(look.tokens.accent),
        (true, false) => ti(look.tokens.text),
    };
    let face = button(text(look.t(label)).size(TS.dense).color(ink))
        .padding([SP.xs, SP.s])
        .style(crate::styles::ghost_button(look.tokens))
        .on_press_maybe(on_press);
    hint_tooltip(
        face.boxed(),
        look.t(tip),
        look.tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}

/// The mm | mil switch: the unit Properties reads lengths in.
fn units<'a>(look: Look<'a>, viewer: &GerberViewerState) -> Element<'a, GerberViewerMsg> {
    let cells: Vec<(LengthUnit, signex_widgets::Label<'a>)> = LengthUnit::ALL
        .into_iter()
        .map(|unit| (unit, look.t(unit_label(unit)).into()))
        .collect();
    segmented(
        cells,
        viewer.unit,
        resolve(look.tokens).segmented(TS.dense),
        GerberViewerMsg::UnitPicked,
    )
}

pub(super) fn unit_label(unit: LengthUnit) -> MessageId {
    match unit {
        LengthUnit::Mm => ids::GERBER_VIEWER_UNIT_MM,
        LengthUnit::Mil => ids::GERBER_VIEWER_UNIT_MIL,
    }
}
