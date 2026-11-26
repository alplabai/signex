//! #3037, mockup row 32: the Variant Manager tab at the size the audit
//! captures it (mockup-audit-5, m32-base and m32-cell). No pill covers the
//! text beside it, and no heading or cell label is wrapped, squeezed or
//! pushed into the next column; a room too narrow for the matrix scrolls
//! it sideways instead.

use iced::{Font, Rectangle, Size};

use super::manager_tests::{app, open_manager};
use super::medium;
use crate::app::Signex;
use crate::app::state::ProjectId;
use crate::dock::measure::text_width_in;
use crate::layout_probe::{LaidOutText, lay_out_texts};
use crate::styles::TypeScale;

const TS: TypeScale = TypeScale::DEFAULT;
/// The tab's room in the audit capture: a 1440 × 900 window less the two
/// docks, the tab strip and the Messages panel.
const ROOM: Size = Size::new(890.0, 580.0);
/// A room whose matrix is narrower than its columns need.
const NARROW: Size = Size::new(700.0, 580.0);

/// Every text of the tab laid out in `room`, isolation marks dropped.
fn laid_out(app: &Signex, id: ProjectId, room: Size) -> Vec<LaidOutText> {
    lay_out_texts(app.view_variant_manager_tab(id), room)
        .into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            bounds: t.bounds,
        })
        .collect()
}

/// Every text reading `label`.
fn all<'t>(texts: &'t [LaidOutText], label: &str) -> Vec<&'t LaidOutText> {
    let found: Vec<_> = texts.iter().filter(|t| t.text == label).collect();
    assert!(!found.is_empty(), "no {label:?} in {texts:#?}");
    found
}

/// The leftmost text reading `label`: the variant list's.
fn in_list<'t>(texts: &'t [LaidOutText], label: &str) -> &'t LaidOutText {
    all(texts, label)
        .into_iter()
        .min_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x))
        .expect("one at least")
}

fn overlaps(a: Rectangle, b: Rectangle) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// `t` lies on one line at its natural width in `font` at `size`.
fn assert_whole(t: &LaidOutText, size: f32, font: Font) {
    let natural = text_width_in(&t.text, size, font);
    assert!(
        t.bounds.width + 0.5 >= natural,
        "{:?} squeezed to {} of {natural}",
        t.text,
        t.bounds.width
    );
    assert!(
        t.bounds.height < 2.0 * size,
        "{:?} wraps: {} high",
        t.text,
        t.bounds.height
    );
}

/// The project with Base default and active, Lite leaving R2 out, its
/// Variant Manager open through `Signex::update`.
fn opened() -> (Signex, ProjectId, tempfile::TempDir) {
    let (mut app, id, dir) = app();
    open_manager(&mut app, id);
    (app, id, dir)
}

#[test]
fn the_base_rows_pills_sit_beside_its_name_and_clear_its_fit_line() {
    let (app, id, _dir) = opened();
    let texts = laid_out(&app, id, ROOM);
    let fit_line = in_list(&texts, "2 / 2 fitted · reference");
    let name = in_list(&texts, "Base");
    for pill in ["default", "active"] {
        let pill = in_list(&texts, pill);
        assert!(
            !overlaps(pill.bounds, fit_line.bounds),
            "{:?} at {:?} covers the fit line at {:?}",
            pill.text,
            pill.bounds,
            fit_line.bounds
        );
        assert!(!overlaps(pill.bounds, name.bounds), "{:?}", pill.text);
        assert!(
            pill.bounds.y + pill.bounds.height <= fit_line.bounds.y + 0.5,
            "{:?} at {:?} reaches down into the fit line at {:?}",
            pill.text,
            pill.bounds,
            fit_line.bounds
        );
        assert!(
            (pill.bounds.center_y() - name.bounds.center_y()).abs() < TS.body,
            "{:?} on the name's line",
            pill.text
        );
    }
    assert!(
        fit_line.bounds.height < 2.0 * TS.caption,
        "the fit line keeps one line: {:?}",
        fit_line.bounds
    );
}

#[test]
fn the_active_heading_and_a_not_fitted_cell_keep_their_words_whole() {
    let (app, id, _dir) = opened();
    let texts = laid_out(&app, id, ROOM);
    let base = all(&texts, "BASE")[0];
    let lite = all(&texts, "LITE")[0];
    let active = all(&texts, "active")
        .into_iter()
        .find(|t| (t.bounds.center_y() - base.bounds.center_y()).abs() < TS.body)
        .expect("the active pill on the heading");
    assert_whole(base, TS.section, medium());
    assert_whole(active, TS.caption, medium());
    assert!(
        active.bounds.x + active.bounds.width <= lite.bounds.x,
        "the active pill {:?} runs into the Lite heading {:?}",
        active.bounds,
        lite.bounds
    );
    // The right pane's hint: the matrix ends before it.
    let pane = in_list(
        &texts,
        "Click a cell in the matrix to see that part in that variant.",
    );
    let not_fitted = all(&texts, "Not fitted");
    assert!(not_fitted.len() >= 2, "R2's Lite cell and the legend");
    for cell in not_fitted {
        assert_whole(cell, TS.caption, medium());
        assert!(
            cell.bounds.x + cell.bounds.width <= pane.bounds.x,
            "{:?} past the matrix",
            cell.bounds
        );
    }
    assert!(lite.bounds.x + lite.bounds.width <= pane.bounds.x);
}

#[test]
fn a_room_too_narrow_scrolls_the_matrix_and_cuts_no_cell() {
    let (app, id, _dir) = opened();
    let texts = laid_out(&app, id, NARROW);
    // The matrix comes before the legend: the first is R2's Lite cell.
    assert_whole(all(&texts, "Not fitted")[0], TS.caption, medium());
    let base = all(&texts, "BASE")[0];
    let lite = all(&texts, "LITE")[0];
    assert_whole(base, TS.section, medium());
    assert_whole(lite, TS.section, medium());
    assert!(base.bounds.x + base.bounds.width < lite.bounds.x);
}
