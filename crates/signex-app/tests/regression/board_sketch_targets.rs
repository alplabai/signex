//! Decision 2 (Caner, 2026-11-20): the board sketch targets Slot, Pour,
//! Mechanical layer and Component anchor, driven through `Signex::update`.
//! A slot bakes a routed cutout and a non-slot loop is refused by name; a
//! pour, a mechanical drawing and an anchored component reach the board in
//! the same undo step as the outline; a board without them saves under its
//! old token.

use signex_app::app::{BoardSketchMsg, EditMsg, FileMsg, Message, PcbMsg, Signex};
use signex_app::dock::DockMessage;
use signex_app::library::messages::RoleTag;
use signex_app::panels::{BoardSketchAttrMsg, PanelMsg};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::PlaneId;
use signex_types::format::{SNXPCB_FORMAT_BOARD_SKETCH_TARGETS, SNXPCB_FORMAT_V1, SnxPcb};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::{CopperLayer, MechanicalLayer, PcbLayer};
use uuid::Uuid;

use crate::drive::drive;
use crate::menu_support::menu_row;

const U1: Uuid = Uuid::from_u128(0x0001_0001);

struct Fixture {
    app: Signex,
    path: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

/// The board sketch fixture's board with U1 placed at (2, 2) and nets GND
/// and VCC.
fn board() -> PcbBoard {
    let mut board = crate::board_sketch::board();
    board.nets.push(signex_types::pcb::NetDef {
        number: 2,
        name: "VCC".into(),
    });
    board.footprints.push(
        serde_json::from_value(serde_json::json!({
            "uuid": U1,
            "reference": "U1",
            "position": {"x": 2.0, "y": 2.0},
            "layer": "TopCopper",
        }))
        .expect("a footprint"),
    );
    board
}

fn open() -> Fixture {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("targets.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    drive(&mut app, task);
    let mut f = Fixture {
        app,
        path,
        _tmp: tmp,
    };
    send(&mut f, menu_row("board_sketch"));
    assert!(
        f.app.interaction_state.board_sketch.is_some(),
        "sketch mode"
    );
    f
}

fn send(f: &mut Fixture, message: Message) {
    let task = f.app.update(message);
    drive(&mut f.app, task);
}

fn panel(f: &mut Fixture, msg: PanelMsg) {
    send(f, Message::Dock(DockMessage::Panel(msg)));
}

fn role(f: &mut Fixture, id: SketchEntityId, role: RoleTag) {
    panel(f, PanelMsg::FpEditorSetRole { id, role });
}

fn bake(f: &mut Fixture) {
    send(
        f,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::BakeRequested)),
    );
}

fn live(f: &Fixture) -> PcbBoard {
    let docs = &f.app.document_state;
    docs.tab_board_engine(docs.active_tab)
        .map(|engine| engine.board())
        .expect("a board is open")
        .clone()
}

fn snapshot(f: &Fixture) -> String {
    serde_json::to_string(&live(f)).unwrap()
}

/// Add sketch entities to the working copy, as the drawing tools would.
struct Draw<'a> {
    f: &'a mut Fixture,
    plane: PlaneId,
}

impl<'a> Draw<'a> {
    fn on(f: &'a mut Fixture) -> Self {
        Draw {
            f,
            plane: PlaneId::new(),
        }
    }

    fn add(&mut self, kind: EntityKind) -> SketchEntityId {
        let id = SketchEntityId::new();
        let session = self.f.app.interaction_state.board_sketch.as_mut().unwrap();
        let sketch = session
            .editor
            .primitive_mut()
            .sketch
            .get_or_insert_with(Default::default);
        sketch.entities.push(Entity::new(id, self.plane, kind));
        id
    }

    fn point(&mut self, x: f64, y: f64) -> SketchEntityId {
        self.add(EntityKind::Point { x, y })
    }

    fn line(&mut self, start: SketchEntityId, end: SketchEntityId) -> SketchEntityId {
        self.add(EntityKind::Line { start, end })
    }

    fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<SketchEntityId> {
        let c = [
            self.point(x0, y0),
            self.point(x1, y0),
            self.point(x1, y1),
            self.point(x0, y1),
        ];
        (0..4).map(|i| self.line(c[i], c[(i + 1) % 4])).collect()
    }

    /// A 12 x 3 slot along X from (`x`, `y`).
    fn slot(&mut self, x: f64, y: f64) -> SketchEntityId {
        let (a, b) = (self.point(x, y), self.point(x + 9.0, y));
        let (c, d) = (self.point(x + 9.0, y + 3.0), self.point(x, y + 3.0));
        let (right, left) = (self.point(x + 9.0, y + 1.5), self.point(x, y + 1.5));
        let bottom = self.line(a, b);
        self.add(EntityKind::Arc {
            center: right,
            start: b,
            end: c,
            sweep_ccw: true,
        });
        self.line(c, d);
        self.add(EntityKind::Arc {
            center: left,
            start: d,
            end: a,
            sweep_ccw: true,
        });
        bottom
    }
}

/// A 40 x 30 board outline; the role picker tags each edge.
fn outline(f: &mut Fixture) {
    let edges = Draw::on(f).rect(0.0, 0.0, 40.0, 30.0);
    for edge in edges {
        role(f, edge, RoleTag::BoardOutline);
    }
}

#[test]
fn a_slot_bakes_a_routed_cutout() {
    let mut f = open();
    outline(&mut f);
    let seed = Draw::on(&mut f).slot(10.0, 10.0);
    role(&mut f, seed, RoleTag::Slot);
    bake(&mut f);
    assert!(
        f.app.interaction_state.board_sketch.is_none(),
        "Bake applies and leaves"
    );
    let board = live(&f);
    let cutouts = &board.sketch.as_ref().expect("stored").cutouts;
    assert_eq!(cutouts.len(), 1);
    assert_eq!(cutouts[0].uuid, seed.0);
    assert!(
        (cutouts[0].edge_radius - 1.5).abs() < 1e-6,
        "routed at the slot's radius"
    );
}

#[test]
fn a_slot_role_on_a_rectangle_is_refused_naming_it() {
    let mut f = open();
    let before = snapshot(&f);
    outline(&mut f);
    let edges = Draw::on(&mut f).rect(10.0, 10.0, 22.0, 13.0);
    role(&mut f, edges[0], RoleTag::Slot);
    bake(&mut f);
    let session = f.app.interaction_state.board_sketch.as_ref();
    let refusal = session
        .and_then(|s| s.refusal.as_ref())
        .map(signex_i18n::RenderTranslatable::render_en_us)
        .expect("the bake is refused");
    assert!(refusal.contains("is not a slot"), "{refusal}");
    assert!(
        refusal.contains(&edges[0].to_string()),
        "names the edge: {refusal}"
    );
    assert_eq!(snapshot(&f), before, "the board is unchanged");
}

#[test]
fn a_pour_is_a_board_zone_on_its_layer_and_net_in_one_step_with_the_outline() {
    let mut f = open();
    let before = snapshot(&f);
    outline(&mut f);
    let edges = Draw::on(&mut f).rect(5.0, 5.0, 20.0, 15.0);
    role(&mut f, edges[0], RoleTag::BoardPour);
    panel(
        &mut f,
        PanelMsg::BoardSketch(BoardSketchAttrMsg::PourLayerPicked {
            id: edges[0],
            layer: PcbLayer::Copper(CopperLayer::Bottom),
        }),
    );
    panel(
        &mut f,
        PanelMsg::FpEditorSetPourNet {
            id: edges[0],
            value: "GND".into(),
        },
    );
    bake(&mut f);
    let board = live(&f);
    assert_eq!(board.outline.len(), 4, "the outline reached the board");
    let pour = board
        .zones
        .iter()
        .find(|z| z.uuid == edges[0].0)
        .expect("the pour is a board zone");
    assert!(pour.is_pour());
    assert_eq!(pour.layer, "BottomCopper");
    assert_eq!((pour.net, pour.net_name.as_str()), (1, "GND"));

    send(&mut f, Message::Edit(EditMsg::Undo));
    assert_eq!(
        snapshot(&f),
        before,
        "the outline and the pour are one undo step"
    );
}

#[test]
fn a_mechanical_layer_role_draws_on_that_mechanical_layer() {
    let mut f = open();
    outline(&mut f);
    let mut draw = Draw::on(&mut f);
    let (a, b) = (draw.point(0.0, 35.0), draw.point(40.0, 35.0));
    let line = draw.line(a, b);
    role(&mut f, line, RoleTag::MechanicalLayer);
    panel(
        &mut f,
        PanelMsg::BoardSketch(BoardSketchAttrMsg::MechanicalLayerPicked {
            id: line,
            layer: MechanicalLayer::new(5).unwrap(),
        }),
    );
    bake(&mut f);
    let board = live(&f);
    let drawing = board
        .graphics
        .iter()
        .find(|g| g.uuid == line.0)
        .expect("the drawing reached the board");
    assert_eq!(drawing.layer, "Mechanical5");
    assert_eq!(drawing.start, Some(Point { x: 0.0, y: 35.0 }));
}

#[test]
fn a_component_anchor_moves_u1_and_undo_moves_it_back() {
    let mut f = open();
    let point = Draw::on(&mut f).point(12.5, 7.5);
    role(&mut f, point, RoleTag::ComponentAnchor);
    panel(
        &mut f,
        PanelMsg::BoardSketch(BoardSketchAttrMsg::AnchorComponentPicked {
            id: point,
            footprint: U1,
        }),
    );
    bake(&mut f);
    let u1 = |f: &Fixture| {
        live(f)
            .footprints
            .iter()
            .find(|fp| fp.uuid == U1)
            .map(|fp| fp.position)
            .expect("U1")
    };
    assert_eq!(
        u1(&f),
        Point { x: 12.5, y: 7.5 },
        "U1's origin is at the point"
    );
    send(&mut f, Message::Edit(EditMsg::Undo));
    assert_eq!(u1(&f), Point { x: 2.0, y: 2.0 }, "undo moves it back");
}

#[test]
fn the_tokens_follow_the_targets() {
    let token = |f: &Fixture| {
        let text = std::fs::read_to_string(&f.path).unwrap();
        text.lines().next().unwrap_or_default().to_string()
    };
    let mut f = open();
    outline(&mut f);
    bake(&mut f);
    send(&mut f, Message::File(FileMsg::Save));
    assert_eq!(
        token(&f),
        format!("format = \"{SNXPCB_FORMAT_V1}\""),
        "a board without the new roles keeps its old token"
    );

    send(&mut f, menu_row("board_sketch"));
    let mut draw = Draw::on(&mut f);
    let (a, b) = (draw.point(0.0, 35.0), draw.point(40.0, 35.0));
    let line = draw.line(a, b);
    role(&mut f, line, RoleTag::MechanicalLayer);
    bake(&mut f);
    send(&mut f, Message::File(FileMsg::Save));
    assert_eq!(
        token(&f),
        format!("format = \"{SNXPCB_FORMAT_BOARD_SKETCH_TARGETS}\"")
    );
}
