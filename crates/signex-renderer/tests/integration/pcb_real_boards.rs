//! The five realistic corpus boards (`signex-types/tests/corpus/pcb/real/`)
//! load into a `PcbSnapshot` and build a scene: the PCB Layout exit
//! criterion "five real `.snxpcb` fixtures render". Each board has a render
//! golden in `tests/golden/pcb_real_boards/`, drawn in the board palette:
//! item counts, a hash of the copper geometry, of the overlay geometry
//! (outline, cutouts, holes, keepouts, silkscreen and other drawings), of
//! the board and footprint texts, and of every colour drawn. A change to
//! what is drawn, or how, shows up here. Regenerate with
//! `UPDATE_GOLDEN=1 cargo test -p signex-renderer --test integration real_boards`
//! and review the diff as the render change it is.

use std::path::{Path, PathBuf};

use pcb_geom::ratsnest;
use serde::{Deserialize, Serialize};
use signex_gfx::pipeline::polygon::polygon_fill_triangles;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_renderer::pcb::{
    BOARD_OUTLINE, BoardFeaturesInput, PcbLayerView, PcbRenderer, PcbSnapshot, board_copper_layers,
    layer_rows, ratsnest_inputs,
};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;
use signex_types::format::{SnxPcb, pcb_layer_token};
use signex_types::pcb::{PadType, PcbBoard};
use signex_types::theme::{ThemeId, canvas_colors};

fn load(name: &str) -> PcbBoard {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb/real")
        .join(format!("{name}.snxpcb"));
    let text = std::fs::read_to_string(path).expect("corpus file");
    SnxPcb::parse(&text).expect("board parses").board
}

/// The one board palette every board view draws in, light or dark app
/// (the app's `styles::board_canvas_colors`).
fn board_palette() -> ResolvedTheme {
    ResolvedTheme::from_canvas_colors(canvas_colors(ThemeId::GraphiteDark))
}

/// 64-bit FNV-1a over the bit patterns of every value fed to it.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn eat(&mut self, values: &[f32]) {
        for v in values {
            self.bytes(&v.to_bits().to_le_bytes());
        }
    }

    fn ring(&mut self, ring: &[[f32; 2]]) {
        self.eat(&[ring.len() as f32]);
        for v in ring {
            self.eat(v);
        }
    }

    fn hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

/// The copper buckets' geometry: traces, vias, pads, pours, the board area.
fn geometry_hash(scene: &Scene) -> String {
    let mut h = Fnv::new();
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
    h.hex()
}

/// The overlay buckets' geometry: the outline, cutouts, holes, keepouts and
/// the footprint and board drawings.
fn overlay_hash(scene: &Scene) -> String {
    let mut h = Fnv::new();
    for l in &scene.overlay_lines {
        h.eat(&[l.p0[0], l.p0[1], l.p1[0], l.p1[1], l.width]);
    }
    for c in &scene.overlay_circles {
        h.eat(&[c.center[0], c.center[1], c.radius, c.stroke_width]);
    }
    for poly in &scene.overlay_polygons {
        h.ring(&poly.vertices);
        for hole in &poly.holes {
            h.ring(hole);
        }
        h.eat(&[poly.stroke_width]);
    }
    h.hex()
}

/// Every board and footprint text: content, anchor, turn, size, side, layer.
fn text_hash(snapshot: &PcbSnapshot) -> String {
    let mut h = Fnv::new();
    for t in &snapshot.texts {
        h.bytes(t.content.as_bytes());
        h.eat(&[t.position[0], t.position[1], t.rotation_deg, t.size_mm]);
        h.bytes(&[u8::from(t.mirrored)]);
        h.bytes(pcb_layer_token(t.layer).as_bytes());
    }
    h.hex()
}

/// Every colour the scene draws with, bucket by bucket in draw order.
fn colour_hash(scene: &Scene) -> String {
    let mut h = Fnv::new();
    for l in scene.lines.iter().chain(&scene.overlay_lines) {
        h.eat(&l.color);
    }
    for c in scene.circles.iter().chain(&scene.overlay_circles) {
        h.eat(&c.color);
    }
    let polygons = scene.polygons.iter().chain(&scene.top_polygons);
    for poly in polygons.chain(&scene.overlay_polygons) {
        h.eat(&poly.fill_color);
        h.eat(&poly.stroke_color.unwrap_or_default());
    }
    h.hex()
}

/// What one board draws: its render golden.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Drawn {
    traces: usize,
    vias: usize,
    pads: usize,
    zones: usize,
    zone_holes: usize,
    copper_layers: usize,
    layer_rows: usize,
    fill_triangles: usize,
    cutouts: usize,
    keepouts: usize,
    mounting_holes: usize,
    texts: usize,
    overlay_lines: usize,
    overlay_circles: usize,
    overlay_polygons: usize,
    geometry_hash: String,
    overlay_hash: String,
    text_hash: String,
    colour_hash: String,
}

/// Vias drawn with a drill opening over their ring.
fn via_drills(snapshot: &PcbSnapshot) -> usize {
    snapshot
        .vias
        .iter()
        .filter(|v| v.drill_mm > 0.0 && v.drill_mm < v.diameter_mm)
        .count()
}

/// Pads drawn with a drill opening over their copper.
fn pad_drills(snapshot: &PcbSnapshot) -> usize {
    // A non-plated pad draws its opening alone, in place of its copper.
    snapshot
        .pads
        .iter()
        .filter(|p| p.drill_mm > 0.0 && p.pad_type != PadType::NpThru)
        .count()
}

fn draw(name: &str) -> Drawn {
    let board = load(name);
    let snapshot = PcbSnapshot::from_board(&board);
    let mut scene = Scene::default();
    PcbRenderer::build_scene(
        &snapshot,
        &board_palette(),
        DirtyFlags::LINES | DirtyFlags::CIRCLES | DirtyFlags::POLYGONS | DirtyFlags::OVERLAY,
        &mut scene,
    );
    assert_eq!(scene.lines.len(), snapshot.traces.len(), "{name}");
    assert_eq!(
        scene.circles.len(),
        snapshot.vias.len() + via_drills(&snapshot),
        "{name}"
    );
    assert_eq!(
        scene.top_polygons.len(),
        snapshot.pads.len() + pad_drills(&snapshot),
        "{name}"
    );
    let board_area = usize::from(snapshot.board_features.outline.len() >= 3);
    assert_eq!(
        scene.polygons.len(),
        snapshot.zones.len() + board_area,
        "{name}"
    );
    let fill_triangles = scene
        .polygons
        .iter()
        .chain(&scene.top_polygons)
        .map(|poly| polygon_fill_triangles(poly).len())
        .sum();
    let features = &snapshot.board_features;
    Drawn {
        traces: snapshot.traces.len(),
        vias: snapshot.vias.len(),
        pads: snapshot.pads.len(),
        zones: snapshot.zones.len(),
        zone_holes: snapshot.zones.iter().map(|z| z.holes.len()).sum(),
        copper_layers: board_copper_layers(&board.layers).len(),
        layer_rows: layer_rows(&board.layers).len(),
        fill_triangles,
        cutouts: features.cutouts.len(),
        keepouts: features.keepouts.len(),
        mounting_holes: features.holes.len(),
        texts: snapshot.texts.len(),
        overlay_lines: scene.overlay_lines.len(),
        overlay_circles: scene.overlay_circles.len(),
        overlay_polygons: scene.overlay_polygons.len(),
        geometry_hash: geometry_hash(&scene),
        overlay_hash: overlay_hash(&scene),
        text_hash: text_hash(&snapshot),
        colour_hash: colour_hash(&scene),
    }
}

const NAMES: [&str; 5] = [
    "linear-regulator",
    "usb-c-power-input",
    "mcu-breakout-tqfp32",
    "sensor-4layer",
    "mixed-tht-smd",
];

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/pcb_real_boards")
        .join(format!("{name}.json"))
}

#[test]
fn the_five_real_boards_render_as_their_goldens() {
    let updating = std::env::var_os("UPDATE_GOLDEN").is_some_and(|v| v == "1");
    let mut changed = Vec::new();
    for name in NAMES {
        let drawn = draw(name);
        let path = golden_path(name);
        if updating {
            let json = serde_json::to_string_pretty(&drawn).expect("golden serialises");
            std::fs::create_dir_all(path.parent().expect("golden dir")).expect("golden dir");
            std::fs::write(&path, json + "\n").expect("golden writes");
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{name}: no golden at {}: {e}", path.display()));
        let golden: Drawn = serde_json::from_str(&text).expect("golden parses");
        if golden != drawn {
            changed.push(format!("{name}:\n  golden {golden:?}\n  drawn  {drawn:?}"));
        }
    }
    assert!(
        changed.is_empty(),
        "render differs from the golden (an intentional change: rerun with UPDATE_GOLDEN=1 and review the diff)\n{}",
        changed.join("\n")
    );
}

/// The board features the generator puts on the boards reach the scene.
#[test]
fn the_real_boards_draw_their_cutout_keepout_holes_and_texts() {
    let mixed = draw("mixed-tht-smd");
    assert_eq!(mixed.cutouts, 1, "the cable-tie slot");
    assert_eq!(mixed.keepouts, 1);
    assert_eq!(mixed.mounting_holes, 4);
    for name in NAMES {
        let board = load(name);
        let drawn = draw(name);
        assert!(drawn.texts >= board.texts.len(), "{name}: board texts");
        assert!(drawn.overlay_lines > 0, "{name}: silkscreen and outline");
    }
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
            assert_eq!(
                scene.top_polygons.len(),
                all.pads.len() + pad_drills(&all),
                "{name} {layer:?}"
            );
        }
    }
}

/// The overlay also carries the board outline, openings and keepouts and the
/// footprint and board drawings (#2387), so the drawn overlay must be
/// exactly what those alone draw.
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
            graphics: snapshot.graphics.clone(),
            ..PcbSnapshot::default()
        };
        let mut expected = Scene::default();
        PcbRenderer::build_scene(&features_only, &theme, DirtyFlags::OVERLAY, &mut expected);
        assert!(!expected.overlay_lines.is_empty(), "{name}: the outline");
        assert_eq!(scene.overlay_lines, expected.overlay_lines, "{name}");
    }
}

/// Hiding the Board Outline row on a real board drops every board-edge
/// stroke from the overlay, and the `.snxpcb` still writes back byte for
/// byte with its `EdgeCuts` token: the row is view state, not a file change.
#[test]
fn the_board_outline_row_is_view_state_and_the_real_boards_round_trip() {
    use signex_gfx::style::ColorSlot;
    let theme = ResolvedTheme::builtin_default();
    let edge = theme.color(ColorSlot::BoardEdge);
    for name in NAMES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../signex-types/tests/corpus/pcb/real")
            .join(format!("{name}.snxpcb"));
        let text = std::fs::read_to_string(path).expect("corpus file");
        assert!(text.contains("EdgeCuts"), "{name}: stored token");
        let doc = SnxPcb::parse(&text).expect("board parses");
        let board = doc.board.clone();

        let edge_strokes = |view: &PcbLayerView| {
            let snapshot = PcbSnapshot::from_board_with_view(&board, view);
            let mut scene = Scene::default();
            PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::OVERLAY, &mut scene);
            scene
                .overlay_lines
                .iter()
                .filter(|l| l.color == edge)
                .count()
                + scene
                    .overlay_circles
                    .iter()
                    .filter(|c| c.color == edge)
                    .count()
                + scene
                    .overlay_polygons
                    .iter()
                    .filter(|p| p.stroke_color == Some(edge))
                    .count()
        };
        let mut view = PcbLayerView::default();
        assert!(edge_strokes(&view) > 0, "{name}: the edge is drawn");
        view.toggle_visibility(BOARD_OUTLINE);
        assert_eq!(edge_strokes(&view), 0, "{name}: the edge is hidden");
        view.toggle_visibility(BOARD_OUTLINE);
        assert!(edge_strokes(&view) > 0, "{name}: the edge is back");

        let written = doc.write_string().expect("writes");
        assert_eq!(written, text, "{name}: byte-identical round trip");
    }
}
