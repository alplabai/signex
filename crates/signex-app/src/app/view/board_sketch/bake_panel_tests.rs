//! Mockup row 179 (#2843, #2830): the board sketch's bake preview is the
//! Properties Bake panel, not a card over the canvas. It lists the values
//! Bake writes, baked at the current solve; a refusal, Close gap and the
//! failed outline checks are in the panel; a click on an entity keeps it.

use iced::{Size, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_sketch::attr::{BoardCutoutAttr, KeepoutAttr, KeepoutKinds, MountingHoleAttr};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_types::format::SnxPcb;
use signex_types::pcb::Point as BoardPoint;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::sketch::board_attr::{BoardSlotAttr, ComponentAnchorAttr};
use uuid::Uuid;

use crate::app::{BoardSketchMsg, FileMsg, Message, PcbMsg, Signex};
use crate::dock::DockMessage;
use crate::layout_probe;
use crate::panels::{
    BakedHole, BakedOutline, BoardBakePreview, BoardSketchAttrMsg, BoardSketchProps, PanelMsg,
    SketchFixMsg,
};
use crate::styles::PillTone;

const DOCK: Size = Size::new(320.0, 4000.0);
const U1: Uuid = Uuid::from_u128(0x0001_0001);

fn drive(app: &mut Signex, task: iced::Task<Message>) {
    use futures::StreamExt as _;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let messages = runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    });
    for message in messages {
        let follow_up = app.update(message);
        drive(app, follow_up);
    }
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    drive(app, task);
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    send(app, Message::Dock(DockMessage::Panel(msg)));
}

fn toggle_preview(app: &mut Signex) {
    send(
        app,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::BakePreviewToggled)),
    );
}

/// A sketch on the board's top plane, built entity by entity.
struct Sketch {
    data: SketchData,
    plane: PlaneId,
}

impl Sketch {
    fn new() -> Self {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        Self { data, plane }
    }

    fn add(&mut self, kind: EntityKind) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data.entities.push(Entity::new(id, self.plane, kind));
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

    /// A 12 x 3 slot along X from (`x`, `y`); its bottom line.
    fn slot(&mut self, x: f64, y: f64) -> SketchEntityId {
        let (a, b) = (self.point(x, y), self.point(x + 9.0, y));
        let (c, d) = (self.point(x + 9.0, y + 3.0), self.point(x, y + 3.0));
        let (right, left) = (self.point(x + 9.0, y + 1.5), self.point(x, y + 1.5));
        let bottom = self.line(a, b);
        for (center, start, end) in [(right, b, c), (left, d, a)] {
            self.add(EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw: true,
            });
        }
        self.line(c, d);
        bottom
    }

    fn get(&mut self, id: SketchEntityId) -> &mut Entity {
        self.data.entities.iter_mut().find(|e| e.id == id).unwrap()
    }

    fn cutout(&mut self, id: SketchEntityId) {
        self.get(id).board_cutout = Some(BoardCutoutAttr {
            edge_radius_expr: None,
            through: true,
        });
    }

    /// The 40 x 30 outline; every edge but the first is tagged, so the
    /// first edge's toggle makes the edit that solves.
    fn outline(&mut self) -> SketchEntityId {
        let edges = self.rect(0.0, 0.0, 40.0, 30.0);
        for &edge in &edges[1..] {
            self.get(edge).board_outline = true;
        }
        edges[0]
    }
}

/// A board with a placed U1, open, in sketch mode on `sketch`, solved
/// through one edit (`first` joins the outline).
fn sketching(sketch: SketchData, first: SketchEntityId) -> (Signex, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("temp dir");
    let path = tmp.path().join("rover.snxpcb");
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.outline = [(0.0, 0.0), (40.0, 0.0), (40.0, 25.0), (0.0, 25.0)]
        .into_iter()
        .map(|(x, y)| BoardPoint::new(x, y))
        .collect();
    board.footprints.push(
        serde_json::from_value(serde_json::json!({
            "uuid": U1,
            "reference": "U1",
            "position": {"x": 0.0, "y": 0.0},
            "layer": "TopCopper",
        }))
        .expect("a footprint"),
    );
    let text = SnxPcb::new(board).write_string().expect("encode");
    std::fs::write(&path, text).expect("write board");
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    send(&mut app, Message::File(FileMsg::Opened(Some(path))));
    send(
        &mut app,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::ModeToggled)),
    );
    let session = app.interaction_state.board_sketch.as_mut().expect("on");
    session.editor.primitive_mut().sketch = Some(sketch);
    panel(
        &mut app,
        PanelMsg::BoardSketch(BoardSketchAttrMsg::OutlineToggled {
            id: first,
            on: true,
        }),
    );
    (app, tmp)
}

fn props(app: &Signex) -> BoardSketchProps {
    app.document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .and_then(|fp| fp.board.clone())
        .expect("board sketch Properties")
}

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn panel_texts(app: &Signex) -> Vec<String> {
    let view = crate::panels::view_properties(&app.document_state.panel_ctx).boxed();
    layout_probe::lay_out_texts(view, DOCK)
        .into_iter()
        .map(|laid| plain(&laid.text))
        .collect()
}

fn canvas_texts(app: &mut Signex) -> Vec<String> {
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    let view = app.board_sketch_view(window).expect("the board sketch");
    layout_probe::lay_out_texts(view, Size::new(1400.0, 900.0))
        .into_iter()
        .map(|laid| plain(&laid.text))
        .collect()
}

/// Section headings render in capitals, so a heading matches either way.
fn has(texts: &[String], label: &str) -> bool {
    texts
        .iter()
        .any(|text| text == label || *text == label.to_uppercase())
}

/// The outline, a plated hole, a top-copper keepout, a slot, a plain
/// cutout and an anchor on U1: every kind of row.
fn every_target() -> (SketchData, SketchEntityId, SketchEntityId) {
    let mut s = Sketch::new();
    let first = s.outline();
    let center = s.point(5.0, 5.0);
    let hole = s.add(EntityKind::Circle {
        center,
        radius: 1.6,
    });
    s.get(hole).mounting_hole = Some(MountingHoleAttr {
        plated: true,
        ..MountingHoleAttr::default()
    });
    let keepout = s.rect(10.0, 2.0, 15.0, 7.0);
    s.get(keepout[0]).keepout = Some(KeepoutAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        kinds: KeepoutKinds::ALL_COPPER,
    });
    let slot = s.slot(20.0, 10.0);
    s.cutout(slot);
    s.get(slot).board_slot = Some(BoardSlotAttr::default());
    let plain = s.rect(30.0, 20.0, 35.0, 25.0);
    s.cutout(plain[0]);
    let anchor = s.point(25.0, 25.0);
    s.get(anchor).component_anchor = Some(ComponentAnchorAttr {
        footprint: Some(U1),
    });
    (s.data, first, hole)
}

#[test]
fn the_preview_is_the_bake_panel_and_no_card() {
    let (sketch, first, _) = every_target();
    let (mut app, _tmp) = sketching(sketch, first);
    assert_eq!(props(&app).bake, None, "closed until asked for");
    toggle_preview(&mut app);

    let texts = panel_texts(&app);
    assert!(has(&texts, "rover is unchanged until Bake"), "{texts:#?}");
    assert!(has(&texts, "Bake result"), "{texts:#?}");
    let canvas = canvas_texts(&mut app);
    for card in [
        "Bake result",
        "Construction and centerline entities never bake.",
    ] {
        assert!(!has(&canvas, card), "{card:?} over the canvas: {canvas:#?}");
    }
    assert!(
        has(&canvas, "Bake preview"),
        "the mode bar keeps its button"
    );

    // The mode bar's Bake preview button closes the panel.
    toggle_preview(&mut app);
    assert_eq!(props(&app).bake, None);
    assert!(!has(&panel_texts(&app), "rover is unchanged until Bake"));
}

#[test]
fn the_panel_lists_each_real_value_and_a_slot_once() {
    let (sketch, first, _) = every_target();
    let (mut app, _tmp) = sketching(sketch, first);
    toggle_preview(&mut app);
    let Some(BoardBakePreview::Writes(rows)) = props(&app).bake else {
        panic!("the sketch bakes: {:?}", props(&app).bake);
    };
    assert_eq!(rows.outline, Some(BakedOutline::Size(40.0, 30.0)));
    assert_eq!(rows.holes.len(), 1);
    let BakedHole { drill_mm, plated } = rows.holes[0];
    assert!((drill_mm - 3.2).abs() < 1e-6 && plated, "{:?}", rows.holes);
    assert_eq!(
        rows.keepouts,
        vec![vec![PcbLayer::Copper(CopperLayer::Top)]]
    );
    assert_eq!(rows.slots.len(), 1, "the slot is listed once");
    let (length, width) = rows.slots[0];
    assert!((length - 12.0).abs() < 1e-6 && (width - 3.0).abs() < 1e-6);
    assert_eq!(rows.cutouts, 1, "the slot is not a cutout too");
    assert_eq!(rows.anchors, vec!["U1".to_string()]);

    let texts = panel_texts(&app);
    for value in [
        "40.00 × 30.00 mm",
        "Ø 3.20 mm · plated",
        "On Top Layer",
        "12.00 × 3.00 · routed",
        "Moves U1",
    ] {
        assert!(
            texts.iter().any(|text| text.contains(value)),
            "{value:?} in {texts:#?}"
        );
    }
}

#[test]
fn the_panel_shows_the_solve_state() {
    let (sketch, first, _) = every_target();
    let (mut app, _tmp) = sketching(sketch, first);
    toggle_preview(&mut app);
    let state = app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .expect("the context")
        .sketch_state;
    let i18n = I18n::en_us();
    let pill = if state.tone == PillTone::Ok {
        t!(i18n, ids::EDITORS_FP_PROPS_DOF_ZERO)
    } else {
        t!(i18n, state.label)
    };
    let texts = panel_texts(&app);
    assert!(has(&texts, "Solve"), "{texts:#?}");
    assert!(has(&texts, &plain(&pill.into_string())), "{texts:#?}");
}

#[test]
fn an_entity_click_keeps_the_panel() {
    let (sketch, first, hole) = every_target();
    let (mut app, _tmp) = sketching(sketch, first);
    toggle_preview(&mut app);
    panel(&mut app, PanelMsg::FpEditorSelectSketchEntity { id: hole });
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    assert_eq!(
        session.editor.state.selected_sketch,
        Some(hole),
        "highlighted"
    );
    assert!(props(&app).bake.is_some(), "the preview stays open");
    let texts = panel_texts(&app);
    assert!(has(&texts, "rover is unchanged until Bake"), "{texts:#?}");
    assert!(has(&texts, "Bake result"), "{texts:#?}");
}

#[test]
fn a_failed_check_in_the_panel_selects_its_place() {
    let mut s = Sketch::new();
    let first = s.outline();
    let stray = s.rect(50.0, 50.0, 55.0, 55.0);
    s.cutout(stray[0]);
    let (mut app, _tmp) = sketching(s.data, first);
    toggle_preview(&mut app);
    let view = crate::panels::view_properties(&app.document_state.panel_ctx).boxed();
    let messages = layout_probe::messages_on_click(view, DOCK, "Cutout outside the outline");
    let select = messages
        .into_iter()
        .find(|msg| matches!(msg, PanelMsg::FpEditorSelectSketchEntity { .. }))
        .expect("the line selects its place");
    assert!(matches!(
        select,
        PanelMsg::FpEditorSelectSketchEntity { id } if id == stray[0]
    ));
    panel(&mut app, select);
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    assert_eq!(session.editor.state.selected_sketch, Some(stray[0]));
    assert!(props(&app).bake.is_some(), "the panel stays");
}

#[test]
fn a_refusal_and_close_gap_are_in_the_panel() {
    let mut s = Sketch::new();
    let p = [
        (0.0, 0.0),
        (40.0, 0.0),
        (40.0, 30.0),
        (0.0, 30.0),
        (0.0, 0.03),
    ]
    .map(|(x, y)| s.point(x, y));
    let edges: Vec<_> = (0..4).map(|i| s.line(p[i], p[i + 1])).collect();
    for &edge in &edges[1..] {
        s.get(edge).board_outline = true;
    }
    let (mut app, _tmp) = sketching(s.data, edges[0]);
    toggle_preview(&mut app);
    let Some(BoardBakePreview::Refused(refusal)) = props(&app).bake else {
        panic!("an open outline refuses: {:?}", props(&app).bake);
    };
    assert!(refusal.reason.is_some() && refusal.place.is_some());
    let offer = refusal.gap.expect("the gap is offered");
    assert!(offer.closes());

    let texts = panel_texts(&app);
    assert!(has(&texts, "Why it does not bake"), "{texts:#?}");
    let view = crate::panels::view_properties(&app.document_state.panel_ctx).boxed();
    let messages = layout_probe::messages_on_click(view, DOCK, "Close gap");
    let close = messages
        .into_iter()
        .find(|msg| {
            matches!(
                msg,
                PanelMsg::SketchFix(SketchFixMsg::CloseGapClicked { .. })
            )
        })
        .expect("Close gap is in the panel");
    panel(&mut app, close);
    assert!(
        matches!(props(&app).bake, Some(BoardBakePreview::Writes(_))),
        "the closed outline bakes: {:?}",
        props(&app).bake
    );
}
