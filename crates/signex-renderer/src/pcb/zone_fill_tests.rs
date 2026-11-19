//! #2216: a pour is drawn as its computed copper, its outline only stroked.

use super::super::{PcbRenderer, PcbSnapshot};
use crate::schematic::ViewRenderer;
use crate::theme::ResolvedTheme;
use signex_gfx::pipeline::polygon::polygon_fill_triangles;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_types::pcb::PcbBoard;

/// A net-1 pour over (-5,-5)..(5,5) with a 1 mm net-2 pad in its middle.
fn board(fill_type: &str) -> PcbBoard {
    let json = format!(
        r#"{{ "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
           "generator": "test", "thickness": 1.6, "outline": [], "layers": [],
           "setup": null, "nets": [],
           "footprints": [{{ "uuid": "8e98ba95-1d0f-4d9f-84e0-27ddf90e10b5",
             "reference": "U1", "value": "", "footprint_id": "",
             "position": {{ "x": 0.0, "y": 0.0 }}, "rotation": 0.0,
             "layer": "TopCopper", "locked": false,
             "pads": [{{ "uuid": "47ceef20-d496-4e62-b8d8-7c34195c6818", "number": "1",
               "pad_type": "smd", "shape": "rect",
               "position": {{ "x": 0.0, "y": 0.0 }}, "size": {{ "x": 1.0, "y": 1.0 }},
               "drill": null, "layers": ["TopCopper"],
               "net": {{ "number": 2, "name": "N2" }}, "roundrect_ratio": 0.0 }}],
             "graphics": [], "properties": [] }}],
           "segments": [], "vias": [], "graphics": [], "texts": [],
           "zones": [{{ "uuid": "5d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "net": 1,
             "layer": "TopCopper", "fill_type": "{fill_type}", "clearance": 0.5,
             "outline": [{{ "x": -5.0, "y": -5.0 }}, {{ "x": 5.0, "y": -5.0 }},
                         {{ "x": 5.0, "y": 5.0 }}, {{ "x": -5.0, "y": 5.0 }}] }}] }}"#
    );
    serde_json::from_str(&json).expect("board")
}

fn scene_of(snapshot: &PcbSnapshot) -> Scene {
    let mut scene = Scene::default();
    let theme = ResolvedTheme::builtin_default();
    PcbRenderer::build_scene(snapshot, &theme, DirtyFlags::POLYGONS, &mut scene);
    scene
}

fn fill_area(polygon: &signex_gfx::primitive::polygon::GpuPolygon) -> f32 {
    polygon_fill_triangles(polygon)
        .iter()
        .map(|t| {
            ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]))
                .abs()
                * 0.5
        })
        .sum()
}

#[test]
fn a_pour_draws_its_fill_with_the_clearance_cut_and_strokes_its_outline() {
    let board = board("pour");
    let snapshot = PcbSnapshot::from_board(&board).with_zone_fills(&pcb_geom::zone_fills(&board));
    let scene = scene_of(&snapshot);
    let copper: Vec<_> = scene
        .polygons
        .iter()
        .filter(|p| p.fill_color[3] > 0.0)
        .collect();
    assert_eq!(copper.len(), 1, "one fill piece");
    assert_eq!(copper[0].holes.len(), 1, "cut around the pad");
    assert!(copper[0].stroke_color.is_none());
    // 10 x 10 less the pad grown by 0.5 mm (2 x 2 with round corners).
    let cut = 4.0 - (4.0 - std::f32::consts::PI) * 0.25;
    let area = fill_area(copper[0]);
    assert!((area - (100.0 - cut)).abs() < 0.05, "{area}");

    let outline = scene
        .polygons
        .iter()
        .find(|p| p.fill_color[3] == 0.0)
        .expect("the outline");
    assert!(outline.stroke_color.is_some());
    assert_eq!(outline.vertices.len(), 4);
}

#[test]
fn a_pour_without_its_fill_draws_only_its_outline_and_solid_copper_is_unchanged() {
    let pour = scene_of(&PcbSnapshot::from_board(&board("pour")));
    assert!(pour.polygons.iter().all(|p| p.fill_color[3] == 0.0));

    let solid = scene_of(&PcbSnapshot::from_board(&board("solid")));
    let zone = solid
        .polygons
        .iter()
        .find(|p| p.vertices.len() == 4 && p.fill_color[3] > 0.0)
        .expect("the solid zone");
    assert!((fill_area(zone) - 100.0).abs() < 1e-3);
}

/// #2523: a shelved pour draws its outline dashed and fills nothing.
#[test]
fn a_shelved_pour_draws_only_its_dashed_outline() {
    let mut board = board("pour");
    board.zones[0].shelved = true;
    let snapshot = PcbSnapshot::from_board(&board).with_zone_fills(&pcb_geom::zone_fills(&board));
    let scene = scene_of(&snapshot);
    // The 40 mm outline in dashes of 0.6 mm every 1 mm.
    assert_eq!(scene.polygons.len(), 40, "one quad per dash");
    for dash in &scene.polygons {
        assert_eq!(dash.vertices.len(), 4);
        assert!(dash.holes.is_empty() && dash.stroke_color.is_none());
        assert!(dash.fill_color[3] > 0.0, "a dash is inked");
        assert!(fill_area(dash) < 0.6 * 0.06 + 1e-4, "a dash, not copper");
    }

    board.zones[0].shelved = false;
    let snapshot = PcbSnapshot::from_board(&board).with_zone_fills(&pcb_geom::zone_fills(&board));
    let scene = scene_of(&snapshot);
    assert!(
        scene.polygons.iter().any(|p| p.holes.len() == 1),
        "unshelved, the fill is back"
    );
}
