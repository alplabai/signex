//! The design-rule check results at the top of the Messages panel (#2222).
//!
//! Rows are data (`DrcRow`); the text is rendered here from the message
//! catalog, so a language switch needs no new run.

use signex_drc::ViolationKind;
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
    /// Over a maximum rather than under a minimum (a track width).
    pub over_maximum: bool,
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
    /// The run applied a written clearance rule (the default note then says
    /// it holds only where no written rule does).
    pub written_clearance: bool,
    /// Written rules the run did not apply, and whether a rule text had a
    /// syntax error (none of it applied); see Messages for which.
    pub rules_not_applied: usize,
    pub rule_text_unparsed: bool,
    /// Counts for the summary line; track width and via size findings are
    /// listed as rows but not counted here yet.
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
        CopperLayer::Top => t!(i18n, ids::PCB_LAYER_TOP_COPPER),
        CopperLayer::Bottom => t!(i18n, ids::PCB_LAYER_BOTTOM_COPPER),
        CopperLayer::Inner(inner) => {
            t!(i18n, ids::PCB_LAYER_INNER_COPPER, "index" => u32::from(inner.index()))
        }
    })
}

fn row_text(i18n: &I18n, row: &DrcRow) -> (Localized, Localized, Localized) {
    let kind = match row.kind {
        ViolationKind::Clearance => t!(i18n, ids::DRC_KIND_CLEARANCE),
        ViolationKind::Short => t!(i18n, ids::DRC_KIND_SHORT),
        ViolationKind::Unconnected => t!(i18n, ids::DRC_KIND_UNCONNECTED),
        ViolationKind::Keepout => t!(i18n, ids::DRC_KIND_KEEPOUT),
        ViolationKind::TrackWidth => t!(i18n, ids::DRC_KIND_TRACK_WIDTH),
        ViolationKind::ViaDiameter => t!(i18n, ids::DRC_KIND_VIA_DIAMETER),
        ViolationKind::ViaDrill => t!(i18n, ids::DRC_KIND_VIA_DRILL),
    };
    let limit = |id| {
        t!(
            i18n, id, "net" => row.a.clone(),
            "measured" => row.measured_mm.clone(), "required" => row.required_mm.clone()
        )
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
        ViolationKind::TrackWidth if row.over_maximum => limit(ids::DRC_DETAIL_AT_MOST),
        ViolationKind::TrackWidth | ViolationKind::ViaDiameter | ViolationKind::ViaDrill => {
            limit(ids::DRC_DETAIL_AT_LEAST)
        }
    };
    let place = t!(i18n, ids::DRC_LOCATION, "x" => row.x_mm.clone(), "y" => row.y_mm.clone());
    (kind, detail, place)
}

type Rows<'a> = Column<Element<'a, PanelMsg>>;

fn note<'a>(message: Localized, color: Color) -> Element<'a, PanelMsg> {
    text(message).size(TS.caption).color(color).boxed()
}

/// The section header: its title and a Graphite Run DRC button, disabled
/// while a check runs.
fn header<'a>(ctx: &'a PanelContext, drc: &DrcPanel, col: Rows<'a>) -> Rows<'a> {
    let i18n = &*ctx.i18n;
    let primary = theme_ext::text_primary(&ctx.tokens);
    let run = (!drc.running).then_some(PanelMsg::RunDrc);
    col.push(
        container(
            row![
                text(t!(i18n, ids::DRC_HEADING))
                    .size(TS.label)
                    .font(iced::Font {
                        weight: iced::font::Weight::Semibold,
                        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
                    })
                    .color(primary),
                Space::new().width(Length::Fill).height(Length::Shrink),
                crate::styles::dialog_button(
                    t!(i18n, ids::DRC_RUN),
                    crate::styles::DialogButtonKind::Secondary,
                    run,
                    &ctx.tokens,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .height(Length::Fixed(super::graphite::TABLE_ROW_HEIGHT + SP.m))
        .align_y(iced::alignment::Vertical::Center)
        .boxed(),
    )
}

/// The hairline under the section, as between the Messages rows.
fn rule<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let line = crate::styles::tokens::resolve(&ctx.tokens).line_soft;
    container(Space::new())
        .height(Length::Fixed(1.0))
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
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
        let id = if drc.written_clearance {
            ids::DRC_DEFAULT_CLEARANCE_WRITTEN
        } else {
            ids::DRC_DEFAULT_CLEARANCE
        };
        col = col.push(note(t!(i18n, id, "clearance" => mm.clone()), muted));
    }
    if drc.stale {
        col = col.push(note(t!(i18n, ids::DRC_STALE), warn));
    }
    if drc.rule_text_unparsed {
        col = col.push(note(t!(i18n, ids::DRC_RULE_TEXT_UNPARSED), warn));
    }
    if drc.rules_not_applied > 0 {
        col = col.push(note(
            t!(i18n, ids::DRC_RULES_NOT_APPLIED, "count" => drc.rules_not_applied),
            warn,
        ));
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
    let col: Rows<'a> = Column::new()
        .spacing(SP.xs)
        .padding([SP.xs, 12.0])
        .width(Length::Fill);
    let mut col = status(ctx, drc, header(ctx, drc, col));
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
    Some(Column::new().push(col.boxed()).push(rule(ctx)).boxed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::messages_on_click;

    fn context(running: bool) -> PanelContext {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.pcb_drc = Some(DrcPanel {
            running,
            ..DrcPanel::default()
        });
        ctx
    }

    /// Round 9 of #2262: Run DRC is the section header's button; it runs the
    /// check, and does nothing while a check is already running.
    #[test]
    fn run_drc_is_a_header_button_disabled_while_running() {
        let window = iced::Size::new(900.0, 300.0);
        let idle = context(false);
        let idle_section = section(&idle).expect("section");
        let pressed = messages_on_click(idle_section, window, "Run DRC");
        assert!(
            matches!(pressed.as_slice(), [PanelMsg::RunDrc]),
            "{pressed:?}"
        );
        let busy = context(true);
        let busy_section = section(&busy).expect("section");
        assert!(messages_on_click(busy_section, window, "Run DRC").is_empty());
    }

    /// #2307: with written rules the default-clearance note says the board
    /// value holds only where no written rule does; without, it is as before.
    #[test]
    fn the_default_clearance_note_follows_whether_written_rules_ran() {
        let texts = |written_clearance: bool| {
            let mut ctx = context(false);
            ctx.pcb_drc = Some(DrcPanel {
                ran: true,
                default_clearance_mm: Some("0.2".into()),
                written_clearance,
                ..DrcPanel::default()
            });
            let section = section(&ctx).expect("section");
            crate::layout_probe::lay_out_texts(section, iced::Size::new(1400.0, 300.0))
                .into_iter()
                .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
                .collect::<Vec<_>>()
        };
        let plain =
            "No clearance rule set on this board; checked with 0.2 mm. Set it in Board Setup.";
        let written = "No clearance rule set on this board; 0.2 mm applies where no written rule does. Set it in Board Setup.";
        let without = texts(false);
        assert!(without.iter().any(|t| t == plain), "{without:?}");
        assert!(!without.iter().any(|t| t == written), "{without:?}");
        let with = texts(true);
        assert!(with.iter().any(|t| t == written), "{with:?}");
        assert!(!with.iter().any(|t| t == plain), "{with:?}");
    }

    /// An inner layer's DRC row uses the Layers panel's name (Mid-Layer n).
    #[test]
    fn an_inner_layer_row_uses_the_layers_panel_name() {
        let i18n = I18n::en_us();
        let inner = CopperLayer::Inner(signex_types::pcb_layer::InnerCopper::new(2).expect("2"));
        let label = layer_label(&i18n, Some(inner))
            .expect("label")
            .into_string();
        let panel = crate::pcb_canvas::layer_style::copper_name(&i18n, inner).into_string();
        assert_eq!(label, panel);
        assert!(label.contains("Mid-Layer"), "{label}");
    }

    /// #2307 step 2b: a width or via-size row says which limit it breaks.
    #[test]
    fn a_size_row_says_the_limit_it_breaks() {
        let i18n = I18n::en_us();
        let row = |kind, over_maximum| DrcRow {
            index: 0,
            kind,
            a: "GND".into(),
            b: "GND".into(),
            measured_mm: "0.100".into(),
            required_mm: "0.150".into(),
            over_maximum,
            x_mm: "0".into(),
            y_mm: "0".into(),
            layer: None,
        };
        let text = |kind, over| {
            let (kind, detail, _) = row_text(&i18n, &row(kind, over));
            format!("{} | {}", kind.into_string(), detail.into_string())
                .replace(['\u{2068}', '\u{2069}'], "")
        };
        assert_eq!(
            text(ViolationKind::TrackWidth, false),
            "Track width | GND: 0.100 mm, needs at least 0.150 mm"
        );
        assert_eq!(
            text(ViolationKind::TrackWidth, true),
            "Track width | GND: 0.100 mm, needs at most 0.150 mm"
        );
        assert_eq!(
            text(ViolationKind::ViaDrill, false),
            "Via drill | GND: 0.100 mm, needs at least 0.150 mm"
        );
        assert!(text(ViolationKind::ViaDiameter, false).starts_with("Via diameter | "));
    }
}
