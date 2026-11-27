//! #2598: a footprint sketch pour's Corners and Corner size, edited in the
//! Pour form. Each edit is one undo step, the bake shapes the pour's
//! outline, and a pour with corners other than Sharp saves as `snxfpt/1.5`.

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::{FpSubformMsg, PanelMsg, pour_corner_size_buffer_key};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::attr::{PourAttr, PourFillType, ThermalRelief};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::zone_pour_shape::PourCorners;
use std::path::PathBuf;

fn open_editor(stem: &str, footprint: Footprint) -> (Signex, PathBuf) {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(footprint));
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path)
}

fn subform(app: &mut Signex, msg: FpSubformMsg) {
    drop(app.update(Message::Dock(DockMessage::Panel(PanelMsg::FpSubform(msg)))));
}

fn editor<'a>(app: &'a Signex, path: &PathBuf) -> &'a FootprintDocument {
    app.document_state.footprint_editors.get(path).unwrap()
}

fn pour<'a>(app: &'a Signex, path: &PathBuf, id: SketchEntityId) -> &'a PourAttr {
    let sketch = editor(app, path).primitive().sketch.as_ref().unwrap();
    let entity = sketch.entities.iter().find(|e| e.id == id).unwrap();
    entity.pour.as_ref().unwrap()
}

/// The points of the one pour the footprint bakes.
fn baked_outline(app: &Signex, path: &PathBuf) -> Vec<[f64; 2]> {
    let pours = &editor(app, path).primitive().pours;
    assert_eq!(pours.len(), 1, "one pour bakes");
    pours[0].boundary.points.clone()
}

/// A 4 mm square of lines whose first line carries a Sharp pour.
fn pour_footprint() -> (Footprint, SketchEntityId) {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let corners = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
    let points: Vec<SketchEntityId> = corners
        .iter()
        .map(|&(x, y)| {
            let id = SketchEntityId::new();
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Point { x, y }));
            id
        })
        .collect();
    let lines: Vec<SketchEntityId> = (0..4)
        .map(|i| {
            let id = SketchEntityId::new();
            let (start, end) = (points[i], points[(i + 1) % 4]);
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Line { start, end }));
            id
        })
        .collect();
    let first = sketch.entities.iter_mut().find(|e| e.id == lines[0]);
    first.unwrap().pour = Some(PourAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        net: None,
        fill_type: PourFillType::Solid,
        thermal_relief: ThermalRelief::default(),
        clearance_expr: None,
        min_thickness_expr: None,
        priority: 0,
        corners: PourCorners::Sharp,
        corner_size: 0.0,
    });
    let mut footprint = Footprint::empty("pour");
    footprint.sketch = Some(sketch);
    (footprint, lines[0])
}

fn size(app: &mut Signex, id: SketchEntityId, value: &str) {
    subform(
        app,
        FpSubformMsg::PourCornerSizeEdited {
            id,
            value: value.into(),
        },
    );
}

#[test]
fn rounded_corners_shape_the_baked_pour_and_save_as_snxfpt_1_5() {
    let (footprint, id) = pour_footprint();
    let (mut app, path) = open_editor("pour-corners", footprint);
    assert_eq!(editor(&app, &path).file.format_token(), "snxfpt/1.1");

    let corners = PourCorners::Rounded;
    subform(&mut app, FpSubformMsg::PourCornersPicked { id, corners });
    size(&mut app, id, "0.5");

    let attr = pour(&app, &path, id);
    assert_eq!(
        (attr.corners, attr.corner_size),
        (PourCorners::Rounded, 0.5)
    );
    assert!(editor(&app, &path).is_dirty());
    assert_eq!(editor(&app, &path).history.undo_len(), 2, "one step each");
    let rounded = baked_outline(&app, &path);
    assert!(rounded.len() > 4, "the corners are arcs: {rounded:?}");
    let at_origin = |p: &[f64; 2]| p[0].abs() < 1e-6 && p[1].abs() < 1e-6;
    assert!(!rounded.iter().any(at_origin), "{rounded:?}");
    assert_eq!(editor(&app, &path).file.format_token(), "snxfpt/1.5");

    let text = editor(&app, &path).file.to_toml_string().unwrap();
    let back = FootprintFile::from_toml_str(&text).unwrap();
    assert_eq!(back.format_token(), "snxfpt/1.5");

    // Undo twice: Sharp again and the lower token.
    let doc = app.document_state.footprint_editors.get_mut(&path).unwrap();
    doc.undo();
    doc.undo();
    assert_eq!(pour(&app, &path, id).corners, PourCorners::Sharp);
    assert_eq!(editor(&app, &path).file.format_token(), "snxfpt/1.1");
}

#[test]
fn a_chamfer_cuts_the_baked_corners_by_the_size() {
    let (footprint, id) = pour_footprint();
    let (mut app, path) = open_editor("pour-chamfer", footprint);
    let corners = PourCorners::Chamfer;
    subform(&mut app, FpSubformMsg::PourCornersPicked { id, corners });
    size(&mut app, id, "1");
    let points = baked_outline(&app, &path);
    let has = |x: f64, y: f64| {
        points
            .iter()
            .any(|p| (p[0] - x).abs() < 1e-6 && (p[1] - y).abs() < 1e-6)
    };
    assert!(
        has(1.0, 0.0) && has(0.0, 1.0) && has(3.0, 4.0),
        "{points:?}"
    );
    assert!(!has(0.0, 0.0), "{points:?}");
}

#[test]
fn a_size_that_is_no_number_or_negative_is_kept_refused() {
    let (footprint, id) = pour_footprint();
    let (mut app, path) = open_editor("pour-size-typing", footprint);
    let corners = PourCorners::Rounded;
    subform(&mut app, FpSubformMsg::PourCornersPicked { id, corners });
    let key = pour_corner_size_buffer_key(id);
    for refused in ["-", "-0.5", "abc"] {
        size(&mut app, id, refused);
        assert_eq!(pour(&app, &path, id).corner_size, 0.0, "{refused}");
        let typed = editor(&app, &path).state.numeric_buffers.get(&key);
        assert_eq!(typed.map(String::as_str), Some(refused));
    }
    assert_eq!(editor(&app, &path).history.undo_len(), 1, "the pick only");

    size(&mut app, id, "0.25");
    assert_eq!(pour(&app, &path, id).corner_size, 0.25);
    assert!(!editor(&app, &path).state.numeric_buffers.contains_key(&key));
}

#[test]
fn picking_the_corners_the_pour_has_records_no_step() {
    let (footprint, id) = pour_footprint();
    let (mut app, path) = open_editor("pour-same-corners", footprint);
    let corners = PourCorners::Sharp;
    subform(&mut app, FpSubformMsg::PourCornersPicked { id, corners });
    assert_eq!(editor(&app, &path).history.undo_len(), 0);
}

/// A pour switched from Rounded back to Sharp drops its size too, so it
/// writes what a pour that never had corners writes: no `corners` or
/// `corner_size` key, and the lower token.
#[test]
fn switching_back_to_sharp_drops_the_size_and_writes_no_corner_keys() {
    let (footprint, id) = pour_footprint();
    let (mut app, path) = open_editor("pour-sharp-again", footprint);
    let corners = PourCorners::Rounded;
    subform(&mut app, FpSubformMsg::PourCornersPicked { id, corners });
    size(&mut app, id, "0.5");
    let corners = PourCorners::Sharp;
    subform(&mut app, FpSubformMsg::PourCornersPicked { id, corners });

    let attr = pour(&app, &path, id);
    assert_eq!((attr.corners, attr.corner_size), (PourCorners::Sharp, 0.0));
    assert_eq!(editor(&app, &path).file.format_token(), "snxfpt/1.1");
    let after = editor(&app, &path).file.to_toml_string().unwrap();
    assert!(!after.contains("corner_size"), "{after}");
    assert!(!after.contains("corners"), "{after}");
}
