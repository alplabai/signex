//! Mockup row 75: the Annotate dialog as laid out. The Changes / All switch
//! sits on one line, the summary banner and the Re-prefix note lead with an
//! info glyph, the Fixed options label is an uppercase section label with
//! its read-only pill at the right, and Accept Changes carries its count.

use super::{OPTIONS_COL_W, TS};
use crate::app::Signex;
use crate::app::handlers::erc::AnnotatePreviewEntry;
use crate::layout_probe::{LaidOutText, lay_out_texts};
use iced::Size;

/// The mockup's size, and the 200 % scale of a 1920 x 1080 display.
const WINDOWS: [Size; 2] = [Size::new(1100.0, 760.0), Size::new(960.0, 540.0)];

fn entry(current: &str, proposed: &str) -> AnnotatePreviewEntry {
    AnnotatePreviewEntry {
        sheet: "Power".into(),
        sheet_path: "power.snxsch".into(),
        current: current.into(),
        proposed: proposed.into(),
        uuid: uuid::Uuid::now_v7(),
        location: (0.0, 0.0),
    }
}

/// The dialog with three rows, two of them changing, laid out in `window`,
/// with the isolation marks of placeables taken out of every text.
fn laid_out(window: Size) -> Vec<LaidOutText> {
    let (mut app, _boot) = Signex::new();
    app.ui_state.window_size = (window.width, window.height);
    app.ui_state.annotate_preview.entries =
        vec![entry("C5", "C5"), entry("C?", "C9"), entry("R?", "R6")];
    lay_out_texts(app.view_annotate_dialog(), window)
        .into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            bounds: t.bounds,
        })
        .collect()
}

fn find<'t>(texts: &'t [LaidOutText], what: impl Fn(&str) -> bool, name: &str) -> &'t LaidOutText {
    texts
        .iter()
        .find(|t| what(&t.text))
        .unwrap_or_else(|| panic!("no text {name:?} in the laid-out tree: {texts:#?}"))
}

#[test]
fn changes_and_all_sit_side_by_side_on_one_line() {
    for window in WINDOWS {
        let texts = laid_out(window);
        let changes = find(&texts, |t| t == "Changes 2", "Changes 2").bounds;
        let all = find(&texts, |t| t == "All 3", "All 3").bounds;
        assert!(
            (changes.y - all.y).abs() < 1.0,
            "at {window:?} Changes ({changes:?}) and All ({all:?}) are on different lines"
        );
        assert!(
            all.x >= changes.x + changes.width,
            "at {window:?} All ({all:?}) is not right of Changes ({changes:?})"
        );
    }
}

/// The mockup packs the switch, filter and Re-prefix to the right after a
/// spacer, so Re-prefix ends where the column (and the note under the
/// table, as wide as the column) ends.
#[test]
fn the_list_header_controls_pack_to_the_right_edge() {
    for window in WINDOWS {
        let texts = laid_out(window);
        let reprefix = find(&texts, |t| t == "Re-prefix to standard", "Re-prefix").bounds;
        let note = find(
            &texts,
            |t| t.starts_with("Re-prefix to standard is"),
            "Re-prefix note",
        )
        .bounds;
        let (button_end, column_end) = (reprefix.x + reprefix.width, note.x + note.width);
        assert!(
            (button_end - column_end).abs() < 1.0,
            "at {window:?} Re-prefix ends at {button_end}, the column at {column_end} \
             ({reprefix:?} / {note:?})"
        );
    }
}

#[test]
fn the_summary_banner_leads_with_its_glyph() {
    for window in WINDOWS {
        let texts = laid_out(window);
        let heading = find(&texts, |t| t == "PROPOSED CHANGE LIST", "heading").bounds;
        let count = find(
            &texts,
            |t| t == "2 of 3 designators will change",
            "banner count",
        )
        .bounds;
        assert!(
            count.x - heading.x >= TS.body * crate::styles::LINE_HEIGHT,
            "at {window:?} the banner text ({count:?}) starts where the column does \
             ({heading:?}): no glyph before it"
        );
    }
}

#[test]
fn the_reprefix_note_leads_with_an_info_glyph() {
    for window in WINDOWS {
        let texts = laid_out(window);
        let heading = find(&texts, |t| t == "PROPOSED CHANGE LIST", "heading").bounds;
        let note = find(
            &texts,
            |t| t.starts_with("Re-prefix to standard is"),
            "Re-prefix note",
        )
        .bounds;
        assert!(
            note.x - heading.x >= TS.caption,
            "at {window:?} the note ({note:?}) starts where the column does ({heading:?})"
        );
    }
}

#[test]
fn fixed_options_is_an_uppercase_label_with_its_pill_at_the_right() {
    for window in WINDOWS {
        let texts = laid_out(window);
        let label = find(&texts, |t| t == "FIXED OPTIONS", "FIXED OPTIONS").bounds;
        let pill = find(&texts, |t| t == "read-only", "read-only").bounds;
        assert!(
            (label.y + label.height / 2.0 - (pill.y + pill.height / 2.0)).abs() < TS.body,
            "at {window:?} the pill ({pill:?}) is not on the label's ({label:?}) line"
        );
        assert!(
            pill.x - (label.x + label.width) >= OPTIONS_COL_W / 3.0,
            "at {window:?} the pill ({pill:?}) hugs the label ({label:?}) instead of \
             sitting at the right of the column"
        );
    }
}

/// Already drawn on trunk: the count follows the label inside the button.
#[test]
fn accept_changes_carries_its_count_after_the_label() {
    for window in WINDOWS {
        let texts = laid_out(window);
        let accept = find(&texts, |t| t == "Accept Changes", "Accept Changes").bounds;
        let count = texts
            .iter()
            .filter(|t| t.text == "2")
            .map(|t| t.bounds)
            .find(|b| (b.y + b.height / 2.0 - (accept.y + accept.height / 2.0)).abs() < TS.body)
            .unwrap_or_else(|| panic!("no count on the Accept Changes line: {texts:#?}"));
        assert!(
            count.x >= accept.x + accept.width,
            "at {window:?} the count ({count:?}) is not after the label ({accept:?})"
        );
        assert!(count.x - (accept.x + accept.width) <= TS.body);
    }
}
