use super::*;
use signex_types::theme::{ThemeId, canvas_colors};

#[test]
fn a_wide_enough_pitch_is_drawn_as_is_and_fully_faded_in() {
    // 1.27 mm at 30 px/mm = 38.1 px, past the fade band.
    let level = grid_level(1.27, 30.0).expect("drawn");
    assert_eq!(level.pitch_mm, 1.27);
    assert!((level.step_px - 38.1).abs() < 1e-3, "{level:?}");
    assert_eq!(level.fade, 1.0);
}

#[test]
fn a_dense_pitch_steps_by_five_to_the_first_level_past_the_floor() {
    // 0.1 mm at 3 px/mm = 0.3 px -> 1.5 -> 7.5 -> 37.5 px (12.5 mm).
    let level = grid_level(0.1, 3.0).expect("drawn");
    assert!((level.pitch_mm - 12.5).abs() < 1e-4, "{level:?}");
    assert!((level.step_px - 37.5).abs() < 1e-3, "{level:?}");
}

#[test]
fn fine_lines_fade_in_across_the_band_above_the_floor() {
    // At the floor the fine lines are hidden: the level is about to step.
    let at_floor = grid_level(1.0, GRID_MIN_PX).expect("drawn");
    assert_eq!(at_floor.step_px, GRID_MIN_PX);
    assert_eq!(at_floor.fade, 0.0);
    let mid = grid_level(1.0, (GRID_MIN_PX + GRID_FADE_PX) / 2.0).expect("drawn");
    assert!((mid.fade - 0.5).abs() < 1e-4, "{mid:?}");
    let past = grid_level(1.0, GRID_FADE_PX + 1.0).expect("drawn");
    assert_eq!(past.fade, 1.0);
    // Just under the floor the grid steps five-fold, fully faded in again.
    let stepped = grid_level(1.0, GRID_MIN_PX - 0.01).expect("drawn");
    assert_eq!(stepped.pitch_mm, 5.0);
    assert!(stepped.step_px > GRID_FADE_PX);
    assert_eq!(stepped.fade, 1.0);
}

#[test]
fn an_unusable_pitch_or_scale_draws_no_grid() {
    assert_eq!(grid_level(0.0, 3.0), None);
    assert_eq!(grid_level(-1.0, 3.0), None);
    assert_eq!(grid_level(f32::NAN, 3.0), None);
    assert_eq!(grid_level(1.0, 0.0), None);
    assert_eq!(grid_level(1.0, f32::INFINITY), None);
}

#[test]
fn grid_colours_come_from_the_board_palette() {
    let colors = canvas_colors(ThemeId::GraphiteDark);
    let fine = crate::render_config::to_iced(&colors.board_grid_color());
    let major = crate::render_config::to_iced(&colors.board_grid_major_color());
    let full = grid_paint(&colors, 1.0);
    assert_eq!(full.fine, fine);
    assert_eq!(full.major, major);
    // Faded out, fine lines vanish and major lines wear the fine colour.
    let out = grid_paint(&colors, 0.0);
    assert_eq!(out.fine.a, 0.0);
    assert_eq!(out.major, fine);
}

#[test]
fn both_themes_draw_the_board_with_one_palette() {
    let board = crate::styles::board_canvas_colors();
    assert_eq!(board, canvas_colors(ThemeId::GraphiteDark));
    assert!(board.board_grid.is_some());
    assert!(board.board_grid_major.is_some());
    assert!(board.board_workspace.is_some());
}

fn luma(c: signex_types::theme::Color) -> u32 {
    u32::from(c.r) * 299 + u32::from(c.g) * 587 + u32::from(c.b) * 114
}

#[test]
fn the_grid_stands_out_on_the_board_area_and_major_is_brighter() {
    let colors = crate::styles::board_canvas_colors();
    let area = luma(colors.board_area_color());
    let fine = luma(colors.board_grid_color());
    let major = luma(colors.board_grid_major_color());
    assert_eq!(colors.board_grid_color().a, 0xFF);
    // At least ~6 levels of 255 apart: readable, not loud.
    assert!(fine >= area + 6_000, "fine {fine} vs board area {area}");
    assert!(major >= fine + 6_000, "major {major} vs fine {fine}");
}

#[test]
fn the_workspace_is_a_neutral_grey_lighter_than_the_board() {
    let colors = crate::styles::board_canvas_colors();
    let w = colors.board_workspace_color();
    let spread = w.r.max(w.g).max(w.b) - w.r.min(w.g).min(w.b);
    assert!(spread <= 12, "workspace {w:?} is not neutral");
    assert!(luma(w) > luma(colors.board_area_color()) + 40_000);
    assert!(luma(w) > luma(colors.board_grid_major_color()));
}

#[test]
fn a_board_without_an_outline_uses_the_default_area() {
    assert_eq!(board_region(None), &EMPTY_BOARD_AREA);
    let two: [[f32; 2]; 2] = [[0.0, 0.0], [1.0, 1.0]];
    assert_eq!(board_region(Some(&two[..])), &EMPTY_BOARD_AREA);
    let tri: [[f32; 2]; 3] = [[0.0, 0.0], [10.0, 0.0], [0.0, 10.0]];
    assert_eq!(board_region(Some(&tri[..])), &tri[..]);
}

/// The segments `each_segment` yields: (index, from, to) in board mm.
fn segments(
    region: &[[f32; 2]],
    view: ([f32; 2], [f32; 2]),
    pitch: f32,
) -> Vec<(i64, [f32; 2], [f32; 2])> {
    let mut out = Vec::new();
    let mut scratch = Vec::new();
    each_segment(region, view, pitch, &mut scratch, |i, a, b| {
        out.push((i, [a.x, a.y], [b.x, b.y]));
    });
    out
}

#[test]
fn grid_lines_stay_inside_the_board_shape() {
    // A 10 x 10 mm square from the origin, viewed with room all round.
    let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let view = ([-50.0, -50.0], [50.0, 50.0]);
    let all = segments(&square, view, 5.0);
    // The half-open crossing rule keeps the lines on the low edges (x = 0,
    // y = 0) and drops the ones on the high edges (x = 10, y = 10).
    for (_, a, b) in &all {
        for p in [a, b] {
            assert!(
                (0.0..=10.0).contains(&p[0]) && (0.0..=10.0).contains(&p[1]),
                "{all:?}"
            );
        }
    }
    let vertical_at_5 = all.iter().find(|(i, a, _)| *i == 1 && a[0] == 5.0);
    assert_eq!(vertical_at_5.map(|s| (s.1[1], s.2[1])), Some((0.0, 10.0)));
    // Nothing outside the square: no line at x = 15.
    assert!(all.iter().all(|(i, _, _)| (0..=2).contains(i)));
}

#[test]
fn grid_lines_skip_a_notch_in_the_board_shape() {
    // A U shape: the notch between x = 4 and x = 6 above y = 5 is open.
    let u = [
        [0.0, 0.0],
        [10.0, 0.0],
        [10.0, 10.0],
        [6.0, 10.0],
        [6.0, 5.0],
        [4.0, 5.0],
        [4.0, 10.0],
        [0.0, 10.0],
    ];
    let view = ([-20.0, -20.0], [20.0, 20.0]);
    let all = segments(&u, view, 1.0);
    // The horizontal line y = 8 is cut in two by the notch.
    let at_8: Vec<_> = all
        .iter()
        .filter(|(_, a, b)| a[1] == 8.0 && b[1] == 8.0)
        .map(|(_, a, b)| (a[0], b[0]))
        .collect();
    assert_eq!(at_8, vec![(0.0, 4.0), (6.0, 10.0)]);
}

#[test]
fn grid_lines_are_clipped_to_the_view() {
    let square = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
    let view = ([10.0, 20.0], [30.0, 40.0]);
    let all = segments(&square, view, 10.0);
    assert!(!all.is_empty());
    for (_, a, b) in &all {
        for p in [a, b] {
            assert!((10.0..=30.0).contains(&p[0]), "{all:?}");
            assert!((20.0..=40.0).contains(&p[1]), "{all:?}");
        }
    }
    // x = 10, 20, 30 and y = 20, 30, 40: six lines, none outside.
    assert_eq!(all.len(), 6);
}

#[test]
fn every_fifth_line_from_the_origin_is_major() {
    let square = [[-10.0, -10.0], [10.0, -10.0], [10.0, 10.0], [-10.0, 10.0]];
    let view = ([-50.0, -50.0], [50.0, 50.0]);
    let majors: Vec<f32> = segments(&square, view, 1.0)
        .into_iter()
        .filter(|(i, a, b)| i.rem_euclid(MAJOR_EVERY) == 0 && a[0] == b[0])
        .map(|(_, a, _)| a[0])
        .collect();
    // -10 is the low edge (kept), 10 the high edge (dropped).
    assert_eq!(majors, vec![-10.0, -5.0, 0.0, 5.0]);
}
