use chrono::{DateTime, Duration, TimeZone, Utc};
use iced::{Color, Point};
use signex_i18n::I18n;
use signex_types::theme::{ThemeId, theme_tokens};

use super::detail::status_look;
use super::row::pill_colors;
use super::*;
use crate::theme_ext::TypeScale;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 19, 12, 0, 0).unwrap()
}

fn commit(sha: &str, parents: &[&str]) -> GraphCommit {
    GraphCommit {
        sha: sha.to_string(),
        parents: parents.iter().map(|p| p.to_string()).collect(),
        refs: Vec::new(),
        author_name: "j.berg".into(),
        author_email: "j.berg@example.com".into(),
        time: now() - Duration::hours(1),
        subject: format!("subject {sha}"),
    }
}

fn with_refs(mut c: GraphCommit, refs: Vec<RefPill>) -> GraphCommit {
    c.refs = refs;
    c
}

fn straight(lane: usize) -> Edge {
    Edge {
        from_lane: lane,
        to_lane: lane,
        color_lane: lane,
        kind: EdgeKind::Straight,
        style: EdgeStyle::Solid,
    }
}

fn curve(from_lane: usize, to_lane: usize, color_lane: usize, kind: EdgeKind) -> Edge {
    Edge {
        from_lane,
        to_lane,
        color_lane,
        kind,
        style: EdgeStyle::Solid,
    }
}

fn lanes_of(g: &GraphLayout) -> Vec<usize> {
    g.rows.iter().map(|r| r.lane).collect()
}

fn style() -> HistoryGraphStyle {
    HistoryGraphStyle::from_tokens(&theme_tokens(ThemeId::GraphiteLight), &TypeScale::DEFAULT)
}

fn i18n() -> &'static I18n {
    I18n::en_us_ref()
}

/// The mockup's graph: merge on main at HEAD, two ripple-fix commits in
/// lane 1, the main-side commit, the fork point, a tagged commit, the root.
fn ripple_fix() -> Vec<GraphCommit> {
    vec![
        with_refs(
            commit("a3bbcc6", &["9e01f4d", "7f21c90"]),
            vec![
                RefPill::Branch {
                    name: "main".into(),
                    is_current: true,
                },
                RefPill::Head,
            ],
        ),
        with_refs(
            commit("7f21c90", &["e44d1a2"]),
            vec![RefPill::Branch {
                name: "ripple-fix".into(),
                is_current: false,
            }],
        ),
        commit("e44d1a2", &["5d7e210"]),
        commit("9e01f4d", &["5d7e210"]),
        commit("5d7e210", &["c41aa02"]),
        with_refs(
            commit("c41aa02", &["0b3f9e1"]),
            vec![RefPill::Tag {
                name: "v1.0".into(),
            }],
        ),
        commit("0b3f9e1", &[]),
    ]
}

#[test]
fn empty_list_has_no_rows_and_no_lanes() {
    assert_eq!(layout(&[], false), GraphLayout::default());
}

#[test]
fn working_tree_alone_is_one_dot_without_edges() {
    let g = layout(&[], true);
    assert_eq!(g.lane_count, 1);
    assert_eq!(g.rows.len(), 1);
    assert_eq!(g.rows[0].dot, DotKind::WorkingTree);
    assert!(g.rows[0].edges.is_empty());
}

#[test]
fn linear_history_stays_in_lane_zero() {
    let commits = vec![commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])];
    let g = layout(&commits, false);
    assert_eq!(lanes_of(&g), vec![0, 0, 0]);
    assert_eq!(g.lane_count, 1);
    assert_eq!(g.rows[0].edges, vec![straight(0)]);
    assert_eq!(g.rows[1].edges, vec![straight(0)]);
    assert!(g.rows[2].edges.is_empty());
    assert!(g.rows.iter().all(|r| r.dot == DotKind::Commit));
}

#[test]
fn ripple_fix_merge_matches_the_mockup() {
    let g = layout(&ripple_fix(), true);
    assert_eq!(lanes_of(&g), vec![0, 0, 1, 1, 0, 0, 0, 0]);
    assert_eq!(g.lane_count, 2);
    assert_eq!(g.rows[0].dot, DotKind::WorkingTree);
    assert_eq!(
        g.rows[0].edges,
        vec![Edge {
            style: EdgeStyle::Dashed,
            ..straight(0)
        }]
    );
    assert_eq!(g.rows[1].dot, DotKind::Merge);
    assert_eq!(
        g.rows[1].edges,
        vec![straight(0), curve(0, 1, 1, EdgeKind::Merge)]
    );
    assert_eq!(g.rows[2].edges, vec![straight(0), straight(1)]);
    assert_eq!(g.rows[3].edges, vec![straight(0), straight(1)]);
    assert_eq!(
        g.rows[4].edges,
        vec![straight(0), curve(1, 0, 1, EdgeKind::Fork)]
    );
    assert_eq!(g.rows[5].edges, vec![straight(0)]);
    assert_eq!(g.rows[6].edges, vec![straight(0)]);
    assert!(g.rows[7].edges.is_empty());
}

#[test]
fn two_concurrent_branches_take_their_own_lanes_and_fork_back() {
    let commits = vec![
        commit("a1", &["a0"]),
        commit("b1", &["b0"]),
        with_refs(commit("h", &["base"]), vec![RefPill::Head]),
        commit("a0", &["base"]),
        commit("b0", &["base"]),
        commit("base", &[]),
    ];
    let g = layout(&commits, false);
    // Lane 0 is held for HEAD, so the newer branch tips go right of it.
    assert_eq!(lanes_of(&g), vec![1, 2, 0, 1, 2, 0]);
    assert_eq!(g.lane_count, 3);
    // Nothing is drawn in lane 0 above HEAD.
    assert_eq!(g.rows[0].edges, vec![straight(1)]);
    assert_eq!(g.rows[1].edges, vec![straight(1), straight(2)]);
    assert_eq!(
        g.rows[4].edges,
        vec![
            straight(0),
            curve(1, 0, 1, EdgeKind::Fork),
            curve(2, 0, 2, EdgeKind::Fork),
        ]
    );
}

#[test]
fn octopus_merge_opens_one_lane_per_extra_parent() {
    let commits = vec![
        commit("m", &["a", "b", "c"]),
        commit("a", &[]),
        commit("b", &[]),
        commit("c", &[]),
    ];
    let g = layout(&commits, false);
    assert_eq!(g.rows[0].dot, DotKind::Merge);
    assert_eq!(lanes_of(&g), vec![0, 0, 1, 2]);
    assert_eq!(g.lane_count, 3);
    assert_eq!(
        g.rows[0].edges,
        vec![
            straight(0),
            curve(0, 1, 1, EdgeKind::Merge),
            curve(0, 2, 2, EdgeKind::Merge),
        ]
    );
    // Lane 0 ends at the root "a"; lanes 1 and 2 carry on.
    assert_eq!(g.rows[1].edges, vec![straight(1), straight(2)]);
    assert_eq!(g.rows[2].edges, vec![straight(2)]);
}

#[test]
fn branch_tip_without_merge_forks_back_above_its_parent() {
    let commits = vec![
        commit("tip", &["a"]),
        with_refs(commit("h", &["a"]), vec![RefPill::Head]),
        commit("a", &[]),
    ];
    let g = layout(&commits, false);
    assert_eq!(lanes_of(&g), vec![1, 0, 0]);
    assert_eq!(g.rows[0].edges, vec![straight(1)]);
    assert_eq!(
        g.rows[1].edges,
        vec![straight(0), curve(1, 0, 1, EdgeKind::Fork)]
    );
}

#[test]
fn freed_lanes_are_reused() {
    let commits = vec![
        commit("m2", &["x", "f2"]),
        commit("f2", &["x"]),
        commit("x", &["m1"]),
        commit("m1", &["y", "f1"]),
        commit("f1", &["y"]),
        commit("y", &[]),
    ];
    let g = layout(&commits, false);
    assert_eq!(lanes_of(&g), vec![0, 1, 0, 0, 1, 0]);
    assert_eq!(g.lane_count, 2);
}

#[test]
fn a_tag_on_an_old_commit_does_not_move_lanes() {
    let mut commits = vec![commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])];
    let plain = layout(&commits, false);
    commits[2].refs.push(RefPill::Tag {
        name: "v1.0".into(),
    });
    assert_eq!(layout(&commits, false), plain);
}

#[test]
fn parents_outside_the_list_draw_no_lane() {
    let commits = vec![commit("m", &["b", "gone"]), commit("b", &["also-gone"])];
    let g = layout(&commits, false);
    assert_eq!(g.rows[0].dot, DotKind::Merge);
    assert_eq!(g.rows[0].edges, vec![straight(0)]);
    assert_eq!(g.lane_count, 1);
}

#[test]
fn layout_is_deterministic() {
    assert_eq!(layout(&ripple_fix(), true), layout(&ripple_fix(), true));
}

#[test]
fn geometry_at_the_12_px_body_matches_the_mockup() {
    let g = GraphGeometry::new(12.0);
    assert_eq!(g.dot(0, 0), Point::new(14.0, 23.0));
    assert_eq!(g.dot(1, 0), Point::new(14.0, 69.0));
    assert_eq!(g.dot(2, 1), Point::new(32.0, 115.0));
    assert!((g.width(2) - 46.0).abs() < 1e-4);
    assert!((g.width(1) - 46.0).abs() < 1e-4);
    assert!((g.width(3) - 64.0).abs() < 1e-4);
    assert!((g.height(8) - 368.0).abs() < 1e-4);
}

#[test]
fn a_merge_curve_spans_one_row_with_mid_height_handles() {
    let g = GraphGeometry::new(12.0);
    let shape = g.segment(1, &curve(0, 1, 1, EdgeKind::Merge));
    assert_eq!(
        shape,
        SegmentShape::Curve {
            from: Point::new(14.0, 69.0),
            handle_from: Point::new(14.0, 92.0),
            handle_to: Point::new(32.0, 92.0),
            to: Point::new(32.0, 115.0),
        }
    );
    assert_eq!(
        g.segment(2, &straight(1)),
        SegmentShape::Line {
            from: Point::new(32.0, 115.0),
            to: Point::new(32.0, 161.0),
        }
    );
}

#[test]
fn geometry_scales_with_the_text_size() {
    let small = GraphGeometry::new(12.0);
    let large = GraphGeometry::new(15.0);
    assert!(large.row_height > small.row_height);
    assert!(large.dot(1, 1).x > small.dot(1, 1).x);
}

#[test]
fn initials_take_first_and_last_words() {
    assert_eq!(initials("j.berg", "j.berg@example.com"), "JB");
    assert_eq!(initials("Ayse Nur Yilmaz", ""), "AY");
    assert_eq!(initials("alice", ""), "A");
    assert_eq!(initials("", "m.demir@example.com"), "MD");
    assert_eq!(initials("--", ""), "");
}

#[test]
fn author_colour_is_stable_and_case_blind() {
    let a = author_color_index("j.berg@example.com", 5);
    assert_eq!(a, author_color_index("j.berg@example.com", 5));
    assert_eq!(a, author_color_index(" J.Berg@Example.com ", 5));
    assert!(a < 5);
    assert_eq!(
        HistoryGraphStyle::avatar_fill("j.berg@example.com"),
        HistoryGraphStyle::avatar_fill("J.BERG@example.com")
    );
}

#[test]
fn author_colour_spreads_over_the_palette() {
    let picks: std::collections::BTreeSet<usize> = (0..40)
        .map(|i| author_color_index(&format!("user{i}@example.com"), 5))
        .collect();
    assert!(picks.len() > 2, "{picks:?}");
}

#[test]
fn a_row_click_sends_the_commit_sha() {
    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        Selected(String),
    }
    let commits = ripple_fix();
    let graph = HistoryGraph::new(&commits, style(), i18n(), now()).on_select(Msg::Selected);
    assert_eq!(
        graph.press_message(3),
        Some(Msg::Selected("9e01f4d".into()))
    );
    assert_eq!(graph.press_message(99), None);
    let inert = HistoryGraph::<Msg>::new(&commits, style(), i18n(), now());
    assert_eq!(inert.press_message(0), None);
}

#[test]
fn the_graph_and_detail_strip_build_in_both_themes() {
    let commits = ripple_fix();
    let files = vec![ChangedFile {
        status: FileStatus::Modified,
        path: "power.snxsch".into(),
    }];
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let s = HistoryGraphStyle::from_tokens(&theme_tokens(id), &TypeScale::DEFAULT);
        let _: Element<'_, ()> = HistoryGraph::new(&commits, s, i18n(), now())
            .working_tree(Some(()))
            .selected(Some("9e01f4d"))
            .on_select(|_| ())
            .view();
        let _: Element<'_, ()> = commit_detail(&s, i18n(), &commits[3], &files, Vec::new());
    }
}

#[test]
fn relative_time_uses_the_history_catalog() {
    let i = i18n();
    let at = |d: Duration| relative_time(i, now() - d, now()).into_string();
    assert_eq!(at(Duration::minutes(12)), "12m ago");
    assert_eq!(at(Duration::hours(1)), "1h ago");
    assert_eq!(at(Duration::days(5)), "5d ago");
    assert_eq!(at(Duration::days(40)), "1mo ago");
    assert_eq!(at(-Duration::seconds(5)), "0s ago");
}

#[test]
fn pills_take_the_branch_lane_colour() {
    let s = style();
    let current = RefPill::Branch {
        name: "main".into(),
        is_current: true,
    };
    let other = RefPill::Branch {
        name: "ripple-fix".into(),
        is_current: false,
    };
    assert_eq!(pill_colors(&s, &current, 1).0, s.accent);
    assert_eq!(pill_colors(&s, &other, 1), (s.lanes[1], None, s.lanes[1]));
    let tag = RefPill::Tag {
        name: "v1.0".into(),
    };
    assert_eq!(pill_colors(&s, &tag, 0), (s.ink3, Some(s.sunken), s.border));
}

#[test]
fn lane_palette_starts_with_accent_then_green_and_wraps() {
    let s = style();
    assert_eq!(s.lane_color(0), s.accent);
    assert_eq!(s.lane_color(1), Color::from_rgb8(0x3C, 0xA8, 0x6A));
    assert_eq!(s.lane_color(style::LANE_COLORS), s.lane_color(0));
}

#[test]
fn status_letters_and_colours() {
    let s = style();
    let modified = status_look(&s, FileStatus::Modified);
    assert_eq!(modified.1, Color::from_rgb8(0xC8, 0x9A, 0x2A));
    assert_eq!(status_look(&s, FileStatus::Deleted).1, s.err);
    let letter = |st| i18n().message(status_look(&s, st).0, &[]).into_string();
    assert_eq!(letter(FileStatus::Modified), "M");
    assert_eq!(letter(FileStatus::Added), "A");
    assert_eq!(letter(FileStatus::Deleted), "D");
}
