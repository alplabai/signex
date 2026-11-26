//! The margin lane's measurement views (mockup row 199), laid out with the
//! app's fonts: what they show, what their buttons publish, and the chart's
//! layout.

use iced::Size;
use signex_i18n::I18n;
use signex_notebook::attachment::DesignValues;
use signex_notebook::measurement::{Timeline, readings, timeline};
use signex_notebook::report::{AttachmentState, attachments_report, measurements_report};
use signex_notebook::{NotebookSources, Reference};
use signex_types::theme::{ThemeId, theme_tokens};

use super::chart::{Mark, layout};
use super::stale::{StaleActions, stale_card};
use super::strip_counts;
use super::timeline::{TimelineActions, TimelineView, timeline_panel};
use crate::layout_probe::{lay_out_texts, messages_on_click};

const LANE: Size = Size::new(320.0, 2000.0);

const READINGS: &str = "#measurement(id: \"MEAS-002\", target: net(\"+3V3\"), quantity: \"ripple\", value: 41.0, unit: \"mV\", limit: (max: 30), timestamp: \"2026-10-02\", rev: \"A\")\n\
#measurement(id: \"MEAS-005\", target: net(\"+3V3\"), quantity: \"ripple\", value: 27.0, unit: \"mV\", limit: (max: 30), timestamp: \"2026-10-09\", rev: \"A rework\", taken-with: (\"L1\": \"3.3 µH\"))\n\
#measurement(id: \"MEAS-009\", target: net(\"+3V3\"), quantity: \"ripple\", value: 18.0, unit: \"mV\", limit: (max: 30), computed: 16.7, timestamp: \"2026-11-24T15:12\", rev: \"B\", board: \"SN 0007\", conditions: \"12.0 V · 2.0 A · 25 °C\", operator: \"j.berg\")\n";

#[derive(Debug, Clone, PartialEq)]
enum Probe {
    Target(Reference),
    Reading(String, u32),
    Export,
    Remeasure,
    MarkValid,
    ShowChange,
}

fn values() -> DesignValues {
    [("L1", "4.7 µH")].into_iter().collect()
}

fn ripple() -> Timeline {
    let (found, _) = readings(READINGS);
    let net = Reference::Net("+3V3".into());
    timeline(&found, &net, Some("ripple"), &[], &values())
}

fn targets() -> Vec<Reference> {
    vec![
        Reference::Net("+3V3".into()),
        Reference::Component("C15".into()),
        Reference::Pin {
            component: "U3".into(),
            pin: "3".into(),
        },
    ]
}

fn actions() -> TimelineActions<Probe> {
    TimelineActions {
        target_picked: Probe::Target,
        reading_clicked: Probe::Reading,
        export_pressed: Probe::Export,
    }
}

fn texts(element: iced::Element<'_, Probe>) -> Vec<String> {
    lay_out_texts(element, LANE)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn the_timeline_lists_its_readings_newest_first_with_fail_and_stale() {
    let line = ripple();
    let targets = targets();
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    let view = TimelineView {
        timeline: &line,
        targets: &targets,
        tokens: &tokens,
        i18n: &i18n,
    };
    let shown = texts(timeline_panel(view, actions()));
    for wanted in [
        "Timeline",
        "+3V3 · ripple",
        "Net +3V3",
        "Part C15",
        "Pin U3.3",
        "MEAS-009 · rev B · board SN 0007",
        "12.0 V · 2.0 A · 25 °C · j.berg",
        "fail, limit 30\u{2009}mV",
        "stale: taken with L1 3.3 µH",
        "3 readings · 1 fail · 1 stale",
        "export CSV",
        "limit 30\u{2009}mV",
        "computed 16.7",
    ] {
        assert!(shown.iter().any(|t| t == wanted), "{wanted}: {shown:#?}");
    }
    let newest = shown.iter().position(|t| t.starts_with("MEAS-009"));
    let oldest = shown.iter().position(|t| t.starts_with("MEAS-002"));
    assert!(newest < oldest, "newest first: {shown:#?}");
}

#[test]
fn the_timeline_publishes_its_tab_reading_and_export() {
    let line = ripple();
    let targets = targets();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let i18n = I18n::en_us();
    let view = || TimelineView {
        timeline: &line,
        targets: &targets,
        tokens: &tokens,
        i18n: &i18n,
    };
    // A label as drawn: the catalog isolates each argument (U+2068/U+2069).
    let click = |label: &str| {
        let texts = lay_out_texts(timeline_panel(view(), actions()), LANE);
        let drawn = texts
            .iter()
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == label)
            .map(|t| t.text.clone())
            .unwrap_or_else(|| panic!("no text {label:?}"));
        messages_on_click(timeline_panel(view(), actions()), LANE, &drawn)
    };
    assert_eq!(click("export CSV"), [Probe::Export]);
    assert_eq!(
        click("Part C15"),
        [Probe::Target(Reference::Component("C15".into()))]
    );
    let reading = click("MEAS-005 · rev A rework");
    assert_eq!(reading, [Probe::Reading(String::new(), 2)]);
}

#[test]
fn a_target_with_no_readings_says_so() {
    let (found, _) = readings(READINGS);
    let empty = timeline(&found, &Reference::Net("VIN".into()), None, &[], &values());
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    let view = TimelineView {
        timeline: &empty,
        targets: &[],
        tokens: &tokens,
        i18n: &i18n,
    };
    let shown = texts(timeline_panel(view, actions()));
    assert!(
        shown.iter().any(|t| t == "No readings of VIN yet."),
        "{shown:#?}"
    );
    assert!(layout(&empty).is_none(), "no chart");
}

#[test]
fn the_chart_runs_oldest_to_newest_against_the_limit() {
    let laid = layout(&ripple()).expect("a chart");
    let xs: Vec<f32> = laid.points.iter().map(|p| p.0).collect();
    assert_eq!(xs.first(), Some(&0.0));
    assert_eq!(xs.last(), Some(&1.0));
    assert!(xs.windows(2).all(|w| w[0] <= w[1]), "{xs:?}");
    let marks: Vec<Mark> = laid.points.iter().map(|p| p.2).collect();
    assert_eq!(marks, [Mark::Fail, Mark::Stale, Mark::Normal]);
    let limit = laid.limit.expect("the limit line");
    // 41 mV is above the 30 mV line, 18 mV below it (y grows downwards).
    assert!(laid.points[0].1 < limit && laid.points[2].1 > limit);
    let computed = laid.computed.expect("the computed line");
    assert!(computed > limit, "16.7 mV is below 30 mV");
}

fn stale_state() -> AttachmentState {
    let text = "#attachment(\"thermal/u3.png\", taken: \"2026-10-09\", rev: \"A rework\", taken-with: (\"L1\": \"3.3 µH\"))\n\
                #measurement(id: \"MEAS-005\", value: 27, attach: \"thermal/u3.png\")\n";
    let mut sources = NotebookSources::new("design.snxnbk", text);
    sources.set_design_values(values());
    attachments_report(&sources).attachments.remove(0)
}

fn stale_actions() -> StaleActions<Probe> {
    StaleActions {
        remeasure: Probe::Remeasure,
        mark_valid: Probe::MarkValid,
        show_change: Probe::ShowChange,
    }
}

#[test]
fn the_stale_card_names_the_change_and_the_readings_taken_with_it() {
    let state = stale_state();
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    let card = stale_card(&state, &tokens, &i18n, stale_actions()).expect("stale");
    let shown = texts(card);
    for wanted in [
        "attachments/thermal/u3.png is stale",
        "a part it depends on changed after it was taken",
        "Taken 2026-10-09, rev A rework",
        "Changed L1 3.3 µH → 4.7 µH",
        "Also stale",
        "MEAS-005",
        "Re-measure…",
        "Mark still valid",
        "Show change",
    ] {
        assert!(shown.iter().any(|t| t == wanted), "{wanted}: {shown:#?}");
    }
}

#[test]
fn the_stale_card_publishes_its_actions_and_is_absent_when_current() {
    let state = stale_state();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let i18n = I18n::en_us();
    let card = || stale_card(&state, &tokens, &i18n, stale_actions()).expect("stale");
    let click = |label: &str| messages_on_click(card(), LANE, label);
    assert_eq!(click("Re-measure…"), [Probe::Remeasure]);
    assert_eq!(click("Mark still valid"), [Probe::MarkValid]);
    assert_eq!(click("Show change"), [Probe::ShowChange]);
    let mut current = stale_state();
    current.changes.clear();
    assert!(stale_card(&current, &tokens, &i18n, stale_actions()).is_none());
}

#[test]
fn the_status_strip_counts_measurements_and_stale_attachments() {
    let text = format!("{READINGS}#attachment(\"a.png\", taken-with: (\"L1\": \"3.3 µH\"))\n");
    let mut sources = NotebookSources::new("design.snxnbk", text);
    sources.set_design_values(values());
    let summary = measurements_report(&sources).summary;
    let i18n = I18n::en_us();
    let parts: Vec<String> = strip_counts(&i18n, &summary)
        .into_iter()
        .map(|p| p.into_string().replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    assert_eq!(
        parts,
        ["3 measurements · 2 pass · 1 fail", "1 stale attachment"]
    );
}
