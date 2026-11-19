//! What the DRC panel shows of a board's check (#2222, #2399): the panel's
//! data (`DrcPanel`, one `DrcRow` per violation) and each row's text. Rows
//! are data; the text is rendered here from the message catalog, so a
//! language switch needs no new run. The idle state is drawn in
//! `drc_idle.rs`, the results in `drc_results.rs`.

use signex_drc::ViolationKind;
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::pcb_layer::CopperLayer;
use signex_types::violation::Severity;

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
    /// Every copper layer it is on, for the layer filter (#2399).
    pub layers: Vec<CopperLayer>,
    /// Its waiver key: what a click, a waiver and the commands name it by.
    pub key: String,
    /// The rule id it is filed under (`builtin::clearance`, `user::<name>`).
    pub rule: String,
    /// Its rule's severity; `None` when the rule is switched off.
    pub severity: Option<Severity>,
    /// How many objects it involves.
    pub objects: usize,
    /// The note of its waiver, when the board waives it.
    pub waiver: Option<String>,
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
    /// was used (or, before a run, will be).
    pub default_clearance_mm: Option<String>,
    /// The run applied a written clearance rule (the default note then says
    /// it holds only where no written rule does).
    pub written_clearance: bool,
    /// Written rules the run did not apply, and whether a rule text had a
    /// syntax error (none of it applied); see Messages for which.
    pub rules_not_applied: usize,
    pub rule_text_unparsed: bool,
    /// Counts for the tiles; track width and via size findings are listed
    /// as rows but not counted here yet. Keepouts are (#2300).
    pub clearance: usize,
    pub short: usize,
    pub unconnected: usize,
    pub keepout: usize,
    /// Items, and keepouts, the run could not check (#2300).
    pub skipped: usize,
    pub skipped_keepouts: usize,
    pub rows: Vec<DrcRow>,
    /// Violations past [`MAX_ROWS`].
    pub hidden: usize,
    /// What the DRC results panel shows of `rows` (#2399).
    pub filter: super::DrcFilter,
    /// The tiles, folded groups and row hover (#88): view state.
    pub view: super::DrcView,
    /// The base rules the board's rule text writes (#2474), for the idle
    /// list; the rest show "default" or "always".
    pub written_base_rules: Vec<super::BaseRule>,
    /// The waiver being written there.
    pub waive_draft: Option<super::DrcWaiveDraft>,
}

/// The name of a copper layer, from the shared layer-name function the
/// Layers panel uses.
pub(super) fn layer_label(i18n: &I18n, layer: Option<CopperLayer>) -> Option<Localized> {
    Some(crate::pcb_canvas::layer_style::copper_name(i18n, layer?))
}

pub(super) fn row_text(i18n: &I18n, row: &DrcRow) -> (Localized, Localized, Localized) {
    let kind = t!(i18n, kind_label(row.kind));
    // An unnetted object's net cell is empty.
    let net = |name: &String| {
        if name.is_empty() {
            t!(i18n, ids::DRC_DETAIL_NO_NET).into_string()
        } else {
            name.clone()
        }
    };
    let (a, b) = (net(&row.a), net(&row.b));
    let limit = |id| {
        t!(
            i18n, id, "net" => a.clone(),
            "measured" => row.measured_mm.clone(), "required" => row.required_mm.clone()
        )
    };
    let gap = || {
        // A gap kind measured at 0 touches or overlaps.
        let id = if row.measured_mm.trim_start_matches(['0', '.']).is_empty() {
            ids::DRC_DETAIL_OVERLAP
        } else {
            ids::DRC_DETAIL_GAP
        };
        t!(
            i18n, id,
            "measured" => row.measured_mm.clone(), "required" => row.required_mm.clone()
        )
    };
    let detail = match row.kind {
        ViolationKind::Clearance => t!(
            i18n, ids::DRC_DETAIL_CLEARANCE,
            "a" => a.clone(), "b" => b.clone(),
            "measured" => row.measured_mm.clone(), "required" => row.required_mm.clone()
        ),
        ViolationKind::Short => t!(
            i18n, ids::DRC_DETAIL_SHORT, "a" => a.clone(), "b" => b.clone()
        ),
        ViolationKind::Unconnected => t!(
            i18n, ids::DRC_DETAIL_UNCONNECTED,
            "net" => a.clone(), "length" => row.measured_mm.clone()
        ),
        ViolationKind::Keepout => t!(i18n, ids::DRC_DETAIL_KEEPOUT, "net" => a.clone()),
        ViolationKind::HoleToHole
        | ViolationKind::EdgeClearance
        | ViolationKind::ViaInPad
        | ViolationKind::SilkToPad
        | ViolationKind::SilkToSilk
        | ViolationKind::CourtyardOverlap => gap(),
        _ if row.over_maximum => limit(ids::DRC_DETAIL_AT_MOST),
        ViolationKind::TrackWidth
        | ViolationKind::ViaDiameter
        | ViolationKind::ViaDrill
        | ViolationKind::HoleSize
        | ViolationKind::AnnularRing
        | ViolationKind::MaskSliver
        | ViolationKind::MaskExpansion
        | ViolationKind::PasteExpansion => limit(ids::DRC_DETAIL_AT_LEAST),
    };
    let place = t!(i18n, ids::DRC_LOCATION, "x" => row.x_mm.clone(), "y" => row.y_mm.clone());
    (kind, detail, place)
}

/// The Kind cell of a row.
pub(super) fn kind_label(kind: ViolationKind) -> signex_i18n::MessageId {
    match kind {
        ViolationKind::Clearance => ids::DRC_KIND_CLEARANCE,
        ViolationKind::Short => ids::DRC_KIND_SHORT,
        ViolationKind::Unconnected => ids::DRC_KIND_UNCONNECTED,
        ViolationKind::Keepout => ids::DRC_KIND_KEEPOUT,
        ViolationKind::TrackWidth => ids::DRC_KIND_TRACK_WIDTH,
        ViolationKind::ViaDiameter => ids::DRC_KIND_VIA_DIAMETER,
        ViolationKind::ViaDrill => ids::DRC_KIND_VIA_DRILL,
        ViolationKind::HoleSize => ids::DRC_KIND_HOLE_SIZE,
        ViolationKind::AnnularRing => ids::DRC_KIND_ANNULAR_RING,
        ViolationKind::HoleToHole => ids::DRC_KIND_HOLE_TO_HOLE,
        ViolationKind::EdgeClearance => ids::DRC_KIND_EDGE_CLEARANCE,
        ViolationKind::ViaInPad => ids::DRC_KIND_VIA_IN_PAD,
        ViolationKind::SilkToPad => ids::DRC_KIND_SILK_TO_PAD,
        ViolationKind::SilkToSilk => ids::DRC_KIND_SILK_TO_SILK,
        ViolationKind::MaskSliver => ids::DRC_KIND_MASK_SLIVER,
        ViolationKind::MaskExpansion => ids::DRC_KIND_MASK_EXPANSION,
        ViolationKind::PasteExpansion => ids::DRC_KIND_PASTE_EXPANSION,
        ViolationKind::CourtyardOverlap => ids::DRC_KIND_COURTYARD,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{PanelContext, PanelMsg};
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

    fn texts(ctx: &PanelContext) -> Vec<String> {
        let section = super::super::drc_results::view(ctx);
        crate::layout_probe::lay_out_texts(section, iced::Size::new(300.0, 4000.0))
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    /// Round 9 of #2262: Run DRC is the DRC panel's button; it runs the
    /// check, and while a check runs it reads "Checking the board..." and
    /// does nothing (#87).
    #[test]
    fn run_drc_is_the_panel_button_and_reads_checking_while_running() {
        let messages = crate::layout_probe::lay_out_texts(
            super::super::view_messages(&context(false)),
            iced::Size::new(900.0, 300.0),
        );
        // #2262: the check lives in the DRC panel, not at the top of Messages.
        assert!(messages.iter().all(|t| t.text != "Run DRC"), "{messages:?}");
        let window = iced::Size::new(300.0, 4000.0);
        let idle = context(false);
        let idle_section = super::super::drc_results::view(&idle);
        let pressed = messages_on_click(idle_section, window, "Run DRC");
        assert!(
            matches!(pressed.as_slice(), [PanelMsg::RunDrc]),
            "{pressed:?}"
        );
        let busy = context(true);
        assert!(texts(&busy).iter().any(|t| t == "Checking the board..."));
        let busy_section = super::super::drc_results::view(&busy);
        assert!(messages_on_click(busy_section, window, "Run DRC").is_empty());
    }

    /// #2307, #87: with written rules the default-clearance notice says the
    /// board value holds only where no written rule does; without, it says
    /// what was checked. Either way "Set it in Rules" follows it.
    #[test]
    fn the_default_clearance_note_follows_whether_written_rules_ran() {
        let shown = |written_clearance: bool| {
            let mut ctx = context(false);
            ctx.pcb_drc = Some(DrcPanel {
                ran: true,
                default_clearance_mm: Some("0.2".into()),
                written_clearance,
                ..DrcPanel::default()
            });
            texts(&ctx)
        };
        let plain = "No clearance rule set on this board; checked with 0.2 mm.";
        let written =
            "No clearance rule set on this board; 0.2 mm applies where no written rule does.";
        let without = shown(false);
        assert!(without.iter().any(|t| t == plain), "{without:?}");
        assert!(!without.iter().any(|t| t == written), "{without:?}");
        let with = shown(true);
        assert!(with.iter().any(|t| t == written), "{with:?}");
        assert!(!with.iter().any(|t| t == plain), "{with:?}");
    }

    /// A DRC row names its layer with the shared layer-name function, the
    /// one the Layers panel uses, never its own words.
    #[test]
    fn an_inner_layer_row_uses_the_layers_panel_name() {
        let i18n = I18n::en_us();
        let inner = CopperLayer::Inner(signex_types::pcb_layer::InnerCopper::new(2).expect("2"));
        let label = layer_label(&i18n, Some(inner))
            .expect("label")
            .into_string();
        let panel = crate::pcb_canvas::layer_style::copper_name(&i18n, inner).into_string();
        assert_eq!(label, panel);
        assert_eq!(
            layer_label(&i18n, Some(CopperLayer::Top)).map(Localized::into_string),
            Some(
                crate::pcb_canvas::layer_style::copper_name(&i18n, CopperLayer::Top).into_string()
            )
        );
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
            layers: Vec::new(),
            key: String::new(),
            rule: String::new(),
            severity: None,
            objects: 0,
            waiver: None,
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
        let unnetted = DrcRow {
            a: String::new(),
            b: String::new(),
            ..row(ViolationKind::HoleSize, true)
        };
        let (_, detail, _) = row_text(&i18n, &unnetted);
        assert_eq!(
            detail.into_string().replace(['\u{2068}', '\u{2069}'], ""),
            "No net: 0.100 mm, needs at most 0.150 mm"
        );
    }

    /// #2300, #88: a board whose only violations are keepouts counts them in
    /// the keepout tile, and a keepout that was not checked is said, never
    /// "clean".
    #[test]
    fn the_tiles_count_keepouts_and_an_unchecked_keepout_is_said() {
        let keepout_only = {
            let mut ctx = context(false);
            ctx.pcb_drc = Some(DrcPanel {
                ran: true,
                keepout: 1,
                rows: vec![DrcRow {
                    index: 0,
                    kind: ViolationKind::Keepout,
                    a: "GND".into(),
                    b: "GND".into(),
                    measured_mm: "0.000".into(),
                    required_mm: "0.000".into(),
                    over_maximum: false,
                    x_mm: "0".into(),
                    y_mm: "0".into(),
                    layer: None,
                    layers: Vec::new(),
                    key: String::new(),
                    rule: String::new(),
                    severity: None,
                    objects: 0,
                    waiver: None,
                }],
                ..DrcPanel::default()
            });
            texts(&ctx)
        };
        assert!(
            keepout_only.iter().any(|t| t == "keepout"),
            "{keepout_only:?}"
        );
        assert_eq!(
            keepout_only.iter().filter(|t| *t == "1").count(),
            1,
            "{keepout_only:?}"
        );
        assert!(!keepout_only.iter().any(|t| t == "No violations."));

        let mut ctx = context(false);
        ctx.pcb_drc = Some(DrcPanel {
            ran: true,
            skipped_keepouts: 2,
            ..DrcPanel::default()
        });
        let unchecked = texts(&ctx);
        assert!(
            !unchecked.iter().any(|t| t == "No violations."),
            "{unchecked:?}"
        );
        assert!(
            unchecked
                .iter()
                .any(|t| t == "2 keepouts could not be checked;"),
            "{unchecked:?}"
        );
    }
}
