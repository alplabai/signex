//! The design-rule check results at the top of the Messages panel (#2222).
//!
//! Rows are data (`DrcRow`); the text is rendered here from the message
//! catalog, so a language switch needs no new run.

use pcb_geom::ViolationKind;
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::pcb_layer::CopperLayer;

use super::*;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Rows listed; the rest are counted, not drawn (one iced widget per row).
pub const MAX_ROWS: usize = 500;

/// One violation, ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct DrcRow {
    /// Position in the report, handed back when the row is clicked.
    pub index: usize,
    pub kind: ViolationKind,
    /// Net names (the same twice for an unconnected link).
    pub a: String,
    pub b: String,
    pub measured_mm: String,
    pub required_mm: String,
    pub x_mm: String,
    pub y_mm: String,
    /// The first copper layer the two items share.
    pub layer: Option<CopperLayer>,
}

/// What the panel shows of the active board's last check.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrcPanel {
    pub running: bool,
    /// A report exists for this board.
    pub ran: bool,
    /// The board changed after that report.
    pub stale: bool,
    /// The minimum clearance in mm, when the board set none and the default
    /// was used.
    pub default_clearance_mm: Option<String>,
    pub clearance: usize,
    pub short: usize,
    pub unconnected: usize,
    pub skipped: usize,
    pub rows: Vec<DrcRow>,
    /// Violations past [`MAX_ROWS`].
    pub hidden: usize,
}

fn layer_label(i18n: &I18n, layer: Option<CopperLayer>) -> Option<Localized> {
    Some(match layer? {
        CopperLayer::Top => t!(i18n, ids::DRC_LAYER_TOP),
        CopperLayer::Bottom => t!(i18n, ids::DRC_LAYER_BOTTOM),
        CopperLayer::Inner(inner) => {
            t!(i18n, ids::DRC_LAYER_INNER, "index" => u32::from(inner.index()))
        }
    })
}

fn row_text(i18n: &I18n, row: &DrcRow) -> (Localized, Localized, Localized) {
    let kind = match row.kind {
        ViolationKind::Clearance => t!(i18n, ids::DRC_KIND_CLEARANCE),
        ViolationKind::Short => t!(i18n, ids::DRC_KIND_SHORT),
        ViolationKind::Unconnected => t!(i18n, ids::DRC_KIND_UNCONNECTED),
        ViolationKind::Keepout => t!(i18n, ids::DRC_KIND_KEEPOUT),
    };
    let detail = match row.kind {
        ViolationKind::Clearance => t!(
            i18n, ids::DRC_DETAIL_CLEARANCE,
            "a" => row.a.clone(), "b" => row.b.clone(),
            "measured" => row.measured_mm.clone(), "required" => row.required_mm.clone()
        ),
        ViolationKind::Short => t!(
            i18n, ids::DRC_DETAIL_SHORT, "a" => row.a.clone(), "b" => row.b.clone()
        ),
        ViolationKind::Unconnected => t!(
            i18n, ids::DRC_DETAIL_UNCONNECTED,
            "net" => row.a.clone(), "length" => row.measured_mm.clone()
        ),
        ViolationKind::Keepout => t!(i18n, ids::DRC_DETAIL_KEEPOUT, "net" => row.a.clone()),
    };
    let place = t!(i18n, ids::DRC_LOCATION, "x" => row.x_mm.clone(), "y" => row.y_mm.clone());
    (kind, detail, place)
}

type Rows<'a> = Column<Element<'a, PanelMsg>>;

fn note<'a>(message: Localized, color: Color) -> Element<'a, PanelMsg> {
    text(message).size(TS.caption).color(color).boxed()
}

fn header<'a>(ctx: &'a PanelContext, col: Rows<'a>) -> Rows<'a> {
    let i18n = &*ctx.i18n;
    let primary = theme_ext::text_primary(&ctx.tokens);
    col.push(
        row![
            text(t!(i18n, ids::DRC_HEADING))
                .size(TS.body)
                .color(primary),
            Space::new().width(Length::Fill).height(Length::Shrink),
            signex_widgets::button(text(t!(i18n, ids::DRC_RUN)).size(TS.body).color(primary))
                .padding([3, 10])
                .on_press(PanelMsg::RunDrc)
                .style(crate::styles::menu_item(&ctx.tokens)),
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
    )
}

/// The state line(s) above the rows: running, idle, or what the last run
/// found. The default-clearance notice comes first among them.
fn status<'a>(ctx: &'a PanelContext, drc: &DrcPanel, mut col: Rows<'a>) -> Rows<'a> {
    let i18n = &*ctx.i18n;
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let warn = theme_ext::warning_color(&ctx.tokens);
    if drc.running {
        return col.push(note(t!(i18n, ids::DRC_RUNNING), muted));
    }
    if !drc.ran {
        return col.push(note(t!(i18n, ids::DRC_IDLE), muted));
    }
    if let Some(mm) = &drc.default_clearance_mm {
        col = col.push(note(
            t!(i18n, ids::DRC_DEFAULT_CLEARANCE, "clearance" => mm.clone()),
            muted,
        ));
    }
    if drc.stale {
        col = col.push(note(t!(i18n, ids::DRC_STALE), warn));
    }
    col = if drc.rows.is_empty() && drc.skipped == 0 {
        col.push(note(
            t!(i18n, ids::DRC_CLEAN),
            theme_ext::success_color(&ctx.tokens),
        ))
    } else {
        col.push(note(
            t!(
                i18n, ids::DRC_SUMMARY,
                "clearance" => drc.clearance, "short" => drc.short,
                "unconnected" => drc.unconnected
            ),
            muted,
        ))
    };
    if drc.skipped > 0 {
        col = col.push(note(
            t!(i18n, ids::DRC_SKIPPED, "count" => drc.skipped),
            warn,
        ));
    }
    col
}

/// One clickable violation row.
fn violation_row<'a>(ctx: &'a PanelContext, violation: &DrcRow) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let (kind, detail, place) = row_text(i18n, violation);
    let color = match violation.kind {
        ViolationKind::Short => theme_ext::error_color(&ctx.tokens),
        _ => theme_ext::warning_color(&ctx.tokens),
    };
    let mut line = row![
        text(kind).size(TS.body).color(color),
        Space::new().width(6).height(Length::Shrink),
        text(detail).size(TS.body).color(primary),
        Space::new().width(6).height(Length::Shrink),
        text(place)
            .size(TS.body)
            .wrapping(iced::widget::text::Wrapping::None)
            .color(muted),
    ]
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);
    if let Some(layer) = layer_label(i18n, violation.layer) {
        line = line
            .push(Space::new().width(6).height(Length::Shrink).boxed())
            .push(text(layer).size(TS.body).color(muted).boxed());
    }
    signex_widgets::button(line)
        .width(Length::Fill)
        .padding([SP.xs, 6.0])
        .on_press(PanelMsg::DrcViolationPicked(violation.index))
        .style(crate::styles::menu_item(&ctx.tokens))
        .boxed()
}

/// The section, or nothing when no PCB is in view.
pub(super) fn section<'a>(ctx: &'a PanelContext) -> Option<Element<'a, PanelMsg>> {
    let drc = ctx.pcb_drc.as_ref()?;
    let col: Rows<'a> = Column::new().spacing(SP.xs).padding(6).width(Length::Fill);
    let mut col = status(ctx, drc, header(ctx, col));
    if !drc.running {
        for violation in &drc.rows {
            col = col.push(violation_row(ctx, violation));
        }
    }
    if drc.hidden > 0 && !drc.running {
        let muted = theme_ext::text_secondary(&ctx.tokens);
        col = col.push(note(
            t!(&*ctx.i18n, ids::DRC_MORE, "count" => drc.hidden),
            muted,
        ));
    }
    Some(col.push(separator(&ctx.tokens).boxed()).boxed())
}
