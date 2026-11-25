//! #2480: every control of the board view configuration acts or, where the
//! board view cannot do it, is greyed with a tooltip that says why. The walk
//! hovers and clicks every 2 px along every row of both tabs; the more
//! menu's rows are checked as data.

use super::*;
use crate::layout_probe::probe_points;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// The "not yet" tooltip (`pcb-layers-not-yet`).
const NOT_YET: &str = "The board view cannot do this yet";
/// The step of the walk along a row.
const STEP: f32 = 2.0;

/// What the walk along the row of `label` must find: some control that
/// acts (`live`), and some control greyed with the "not yet" tooltip
/// (`greyed`).
struct Expected {
    label: &'static str,
    live: bool,
    greyed: bool,
}

const fn row(label: &'static str, live: bool, greyed: bool) -> Expected {
    Expected {
        label,
        live,
        greyed,
    }
}

/// Layers & Colors, System Colors open: a row of each kind, then every
/// System Colors row.
const LAYERS_AND_COLORS: &[Expected] = &[
    row("Bottom Layer", true, false),
    row("Courtyard", true, false),
    // Its dots: the board's keepout regions keep the palette colour.
    row("Keep-Out", true, true),
    row("Board Outline", true, false),
    row("Drill Drawing", true, false),
    // Their eyes work, their colour dots are greyed.
    row("Multi-Layer", true, true),
    row("Connection Lines", true, true),
    row("Origin Marker", true, true),
    row("Selection/Highlight", false, true),
    row("Pad Holes", false, true),
    row("Via Holes", false, true),
    row("Component Reference Point", false, true),
    row("3D Body Reference Point", false, true),
    row("Custom Snap Points", false, true),
    row("DRC Error / Waived DRC Error Markers", false, true),
    row("Violation / Waived Violation Markers", false, true),
    row("Board Line/Area", false, true),
    row("Sheet Line/Area", false, true),
    row("Workspace Start Color", false, true),
    row("Workspace End Color", false, true),
];

/// View Options, Additional Options open.
const VIEW_OPTIONS: &[Expected] = &[
    row("Configuration", false, true),
    row("3D", true, false),
    // Off and On work; Mono is greyed.
    row("Single Layer", true, true),
    row("Show Grid", true, false),
    // The eye works; Draft is greyed on every row.
    row("All Objects", true, true),
    row("Tracks", true, true),
    row("Texts", true, true),
    row("Keepouts", true, true),
    // Neither hidden nor faded yet: eye, Draft and slider are greyed.
    row("Rooms", false, true),
    row("Coordinates", false, true),
    row("3D Body", false, true),
    row("Dimmed Objects", false, true),
    row("Highlighted Objects", false, true),
    row("Masked Objects", false, true),
    row("Test Points", false, true),
    row("Status Info", false, true),
    row("Pad Nets", false, true),
    row("Pad Numbers", false, true),
    row("Via Nets", false, true),
    row("Via Span", false, true),
    row("All Connections in Single Layer Mode", false, true),
    row("Net Color Override", false, true),
    row("Use Layer Colors for Connection Drawing", false, true),
    row("Repeated Net Names on Tracks", false, true),
    row("Special Strings", false, true),
];

/// Walk the rows of `rows` in `ctx`. At every point a greyed control
/// publishes nothing; each row finds what it expects.
fn walk(ctx: &PanelContext, rows: &[Expected]) {
    let xs: Vec<f32> = (0..)
        .map(|i| 1.0 + STEP * i as f32)
        .take_while(|x| *x < WINDOW.width)
        .collect();
    let points: Vec<Point> = rows
        .iter()
        .flat_map(|expected| {
            let y = row_text(ctx, expected.label).center_y();
            xs.iter().map(move |&x| Point::new(x, y))
        })
        .collect();
    let probes = probe_points(panel(ctx), WINDOW, &points);
    for (expected, along) in rows.iter().zip(probes.chunks(xs.len())) {
        let (mut live, mut greyed) = (false, false);
        for (x, probe) in xs.iter().zip(along) {
            let not_yet = probe.tips.iter().any(|tip| tip == NOT_YET);
            assert!(
                !not_yet || probe.clicked.is_empty(),
                "{} at x {x}: a greyed control acts: {:?}",
                expected.label,
                shown(&probe.clicked)
            );
            live |= !probe.clicked.is_empty();
            greyed |= not_yet;
        }
        assert_eq!(
            (live, greyed),
            (expected.live, expected.greyed),
            "{}: (a control acts, a control is greyed with the tooltip)",
            expected.label
        );
    }
}

#[test]
fn every_layers_and_colors_control_acts_or_says_it_cannot_yet() {
    let ctx = with_system_colors(context(&PcbLayerView::default()));
    walk(&ctx, LAYERS_AND_COLORS);
}

#[test]
fn every_view_options_control_acts_or_says_it_cannot_yet() {
    let mut ctx = context_with(&PcbLayerView::default(), options());
    ctx.collapsed_sections
        .insert("pcb_view_additional.open".to_string());
    walk(&ctx, VIEW_OPTIONS);
}

/// Every row of the more menu acts or says in its tooltip why it cannot:
/// with no saved set shown and no Layer Stack, then with both.
#[test]
fn every_more_menu_row_acts_or_says_why_not() {
    let mut idle = context(&PcbLayerView::default()).pcb_layers.clone();
    idle.layer_stack_reason = Some(ids::COMMAND_NEEDS_PCB);
    let mut able = idle.clone();
    able.layer_set = Some(LayerSetId::Saved("Assembly".to_string()));
    able.layer_stack_live = true;
    for menu_of in [&idle, &able] {
        for entry in more_menu::entries(menu_of) {
            let DropdownEntry::Item(item) = entry else {
                continue;
            };
            let acts = item.on_press.is_some() && !item.disabled;
            assert!(
                acts || item.tooltip.as_deref().is_some_and(|tip| !tip.is_empty()),
                "{}: greyed without a reason",
                item.label
            );
        }
    }
    let tip_of = |menu_of: &PcbLayersPanel, label: &str| {
        more_menu::entries(menu_of)
            .into_iter()
            .find_map(|entry| match entry {
                DropdownEntry::Item(item) if item.label == label => item.tooltip,
                _ => None,
            })
    };
    assert_eq!(
        tip_of(&idle, "Delete current set...").as_deref(),
        Some("Show a saved layer set to delete it")
    );
    assert_eq!(
        tip_of(&idle, "Layer Stack...").as_deref(),
        Some("No PCB is open"),
        "the command's own reason"
    );
    assert_eq!(tip_of(&idle, "Import...").as_deref(), Some(NOT_YET));
}
