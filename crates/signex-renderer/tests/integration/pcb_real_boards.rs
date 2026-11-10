//! The five realistic corpus boards (`signex-types/tests/corpus/pcb/real/`)
//! load into a `PcbSnapshot` and build a scene: the PCB Layout exit
//! criterion "five real `.snxpcb` fixtures render". Item counts and a hash
//! of the emitted geometry are pinned per board, so a change to what is
//! drawn shows up here. Colours are left out of the hash: a theme change is
//! not a geometry change.

use std::path::Path;

use pcb_geom::ratsnest;
use signex_gfx::pipeline::polygon::polygon_fill_triangles;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_renderer::pcb::{
    BoardFeaturesInput, PcbLayerView, PcbRenderer, PcbSnapshot, board_copper_layers, layer_rows,
    ratsnest_inputs,
};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;
use signex_types::format::SnxPcb;
use signex_types::pcb::PcbBoard;

fn load(name: &str) -> PcbBoard {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb/real")
        .join(format!("{name}.snxpcb"));
    let text = std::fs::read_to_string(path).expect("corpus file");
    SnxPcb::parse(&text).expect("board parses").board
}

/// 64-bit FNV-1a over the bit patterns of every coordinate and size.
struct Fnv(u64);

impl Fnv {
    fn eat(&mut self, values: &[f32]) {
        for v in values {
            for byte in v.to_bits().to_le_bytes() {
                self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }

    fn ring(&mut self, ring: &[[f32; 2]]) {
        self.eat(&[ring.len() as f32]);
        for v in ring {
            self.eat(v);
        }
    }
}

fn geometry_hash(scene: &Scene) -> u64 {
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    for l in &scene.lines {
        h.eat(&[l.p0[0], l.p0[1], l.p1[0], l.p1[1], l.width]);
    }
    for c in &scene.circles {
        h.eat(&[c.center[0], c.center[1], c.radius, c.stroke_width]);
    }
    for poly in scene.polygons.iter().chain(&scene.top_polygons) {
        h.ring(&poly.vertices);
        for hole in &poly.holes {
            h.ring(hole);
        }
    }
    h.0
}

/// What one board draws.
#[derive(Debug, PartialEq, Eq)]
struct Drawn {
    name: &'static str,
    traces: usize,
    vias: usize,
    pads: usize,
    zones: usize,
    zone_holes: usize,
    copper_layers: usize,
    layer_rows: usize,
    fill_triangles: usize,
    hash: u64,
}

fn draw(name: &'static str) -> Drawn {
    let board = load(name);
    let snapshot = PcbSnapshot::from_board(&board);
    let mut scene = Scene::default();
    PcbRenderer::build_scene(
        &snapshot,
        &ResolvedTheme::builtin_default(),
        DirtyFlags::LINES | DirtyFlags::CIRCLES | DirtyFlags::POLYGONS,
        &mut scene,
    );
    assert_eq!(scene.lines.len(), snapshot.traces.len(), "{name}");
    assert_eq!(scene.circles.len(), snapshot.vias.len(), "{name}");
    assert_eq!(scene.top_polygons.len(), snapshot.pads.len(), "{name}");
    assert_eq!(scene.polygons.len(), snapshot.zones.len(), "{name}");
    let fill_triangles = scene
        .polygons
        .iter()
        .chain(&scene.top_polygons)
        .map(|poly| polygon_fill_triangles(poly).len())
        .sum();
    Drawn {
        name,
        traces: snapshot.traces.len(),
        vias: snapshot.vias.len(),
        pads: snapshot.pads.len(),
        zones: snapshot.zones.len(),
        zone_holes: snapshot.zones.iter().map(|z| z.holes.len()).sum(),
        copper_layers: board_copper_layers(&board.layers).len(),
        layer_rows: layer_rows(&board.layers).len(),
        fill_triangles,
        hash: geometry_hash(&scene),
    }
}

const NAMES: [&str; 5] = [
    "linear-regulator",
    "usb-c-power-input",
    "mcu-breakout-tqfp32",
    "sensor-4layer",
    "mixed-tht-smd",
];

/// Per board: `[traces, vias, pads, zones, zone holes, copper layers,
/// layer rows, fill triangles]` and the geometry hash.
const EXPECTED: [(&str, [usize; 8], u64); 5] = [
    (
        "linear-regulator",
        [16, 4, 16, 1, 0, 2, 31, 306],
        14501496464384911031,
    ),
    (
        "usb-c-power-input",
        [23, 5, 26, 1, 0, 2, 31, 558],
        15673827995218981839,
    ),
    (
        "mcu-breakout-tqfp32",
        [109, 9, 78, 1, 0, 2, 31, 1526],
        3554543249757466849,
    ),
    (
        "sensor-4layer",
        [34, 15, 27, 2, 22, 4, 33, 693],
        4591899967919963799,
    ),
    (
        "mixed-tht-smd",
        [20, 1, 22, 1, 0, 2, 31, 396],
        6283896836866658454,
    ),
];

#[test]
fn the_five_real_boards_render_with_the_pinned_counts_and_geometry() {
    let drawn: Vec<Drawn> = NAMES.iter().map(|&name| draw(name)).collect();
    let expected: Vec<Drawn> = EXPECTED
        .iter()
        .map(|&(name, counts, hash)| {
            let [
                traces,
                vias,
                pads,
                zones,
                zone_holes,
                copper_layers,
                layer_rows,
                fill_triangles,
            ] = counts;
            Drawn {
                name,
                traces,
                vias,
                pads,
                zones,
                zone_holes,
                copper_layers,
                layer_rows,
                fill_triangles,
                hash,
            }
        })
        .collect();
    assert_eq!(drawn, expected);
}

#[test]
fn every_zone_hole_of_the_real_boards_is_drawn_as_a_cut_out() {
    for name in NAMES {
        let board = load(name);
        for zone in &board.zones {
            assert!(
                zone.hole_problems().is_empty(),
                "{name}: {:?}",
                zone.hole_problems()
            );
        }
    }
}

#[test]
fn each_copper_layer_of_the_real_boards_renders_as_the_active_layer() {
    let theme = ResolvedTheme::builtin_default();
    for name in NAMES {
        let board = load(name);
        let all = PcbSnapshot::from_board(&board);
        for layer in board_copper_layers(&board.layers) {
            let mut view = PcbLayerView::default();
            view.set_active(layer);
            let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
            // Nothing is hidden, so the active layer only reorders and dims.
            assert_eq!(snapshot.traces.len(), all.traces.len(), "{name} {layer:?}");
            assert_eq!(snapshot.pads.len(), all.pads.len(), "{name} {layer:?}");
            let mut scene = Scene::default();
            PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::all(), &mut scene);
            assert_eq!(scene.top_polygons.len(), all.pads.len(), "{name} {layer:?}");
        }
    }
}

/// The overlay also carries the board outline, openings and keepouts, so the
/// drawn overlay must be exactly what the board features alone draw.
#[test]
fn the_real_boards_draw_no_ratsnest_because_every_net_is_routed() {
    let theme = ResolvedTheme::builtin_default();
    for name in NAMES {
        let board = load(name);
        let lines = ratsnest(&board);
        assert!(lines.is_empty(), "{name}: {lines:?}");
        let snapshot = PcbSnapshot::from_board(&board)
            .with_ratsnest_lines(ratsnest_inputs(&lines, &Default::default()));
        let mut scene = Scene::default();
        PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::OVERLAY, &mut scene);

        let features_only = PcbSnapshot {
            board_features: BoardFeaturesInput::from_board(&board),
            ..PcbSnapshot::default()
        };
        let mut expected = Scene::default();
        PcbRenderer::build_scene(&features_only, &theme, DirtyFlags::OVERLAY, &mut expected);
        assert!(!expected.overlay_lines.is_empty(), "{name}: the outline");
        assert_eq!(scene.overlay_lines, expected.overlay_lines, "{name}");
    }
}
