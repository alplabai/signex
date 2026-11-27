//! #2688 (Caner delegated 2026-11-27): a linear pattern has a Mask field like
//! a polar one. The Properties edit writes the pattern's depopulation mask
//! and keeps its suppression list, clearing an empty mask leaves no
//! depopulation, and the bake drops the masked instances while the others
//! keep their numbers (the polar rule: numbering follows the index).

use std::collections::HashMap;
use std::path::PathBuf;

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::{ArrayParamField, PanelMsg};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::solver::Solver;

/// An open footprint editor whose sketch has a pad and a 5-long linear
/// pattern of it.
fn app_with_linear(stem: &str) -> (Signex, PathBuf, ArrayId) {
    let plane = PlaneId::new();
    let source = SketchEntityId::new();
    let mut point = Entity::new(source, plane, EntityKind::Point { x: 0.0, y: 0.0 });
    point.pad = Some(PadAttr {
        number: "1".into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: "0.3mm".into(),
        size_y_expr: "0.3mm".into(),
        ..PadAttr::default()
    });
    let array = ArrayId::new();
    let mut fp = Footprint::empty(stem);
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![point],
        arrays: vec![Array {
            id: array,
            kind: ArrayKind::Linear {
                source,
                count_expr: "5".into(),
                dx_expr: "1mm".into(),
                dy_expr: "0mm".into(),
                depopulation: None,
            },
            numbering: NumberingScheme::default(),
        }],
        ..SketchData::default()
    });
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem.to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path, array)
}

fn sketch(app: &Signex, path: &PathBuf) -> SketchData {
    app.document_state.footprint_editors[path].file.footprints[0]
        .sketch
        .clone()
        .expect("the sketch")
}

fn send(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn edit_mask(app: &mut Signex, array_id: ArrayId, value: &str) {
    send(
        app,
        PanelMsg::FpEditorEditArrayParam {
            array_id,
            field: ArrayParamField::MaskExpr,
            value: value.into(),
        },
    );
}

fn baked_numbers(sketch: &SketchData) -> Vec<String> {
    let solved = Solver::default()
        .solve(sketch, &HashMap::new())
        .expect("the pattern solves");
    let (mut pads, mut warnings) = (Vec::new(), Vec::new());
    signex_bake::bake_pads(sketch, &solved, &HashMap::new(), &mut pads, &mut warnings).unwrap();
    signex_bake::bake_arrays(sketch, &solved, &HashMap::new(), &mut pads, &mut warnings).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    pads.into_iter().map(|p| p.number).collect()
}

#[test]
fn a_linear_mask_drops_its_instances_and_numbering_follows_the_index() {
    let (mut app, path, array) = app_with_linear("linear-mask");
    edit_mask(&mut app, array, "i != 1 && i != 3");

    let sketch = sketch(&app, &path);
    let mask = sketch.arrays[0]
        .kind
        .depopulation()
        .map(|d| d.mask_expr.clone());
    assert_eq!(mask.as_deref(), Some("i != 1 && i != 3"));
    assert_eq!(baked_numbers(&sketch), ["1", "3", "5"]);
}

#[test]
fn a_linear_mask_edit_keeps_the_suppression_list_and_an_empty_one_clears() {
    let (mut app, path, array) = app_with_linear("linear-mask-list");
    send(
        &mut app,
        PanelMsg::FpEditorToggleArrayInstance {
            array_id: array,
            i: 4,
            j: 0,
            value: false,
        },
    );
    edit_mask(&mut app, array, "i != 1");
    let kept = sketch(&app, &path);
    assert!(kept.arrays[0].kind.is_instance_suppressed(4, 0));
    assert_eq!(baked_numbers(&kept), ["1", "3", "4"]);

    edit_mask(&mut app, array, "");
    send(
        &mut app,
        PanelMsg::FpEditorToggleArrayInstance {
            array_id: array,
            i: 4,
            j: 0,
            value: true,
        },
    );
    let cleared = sketch(&app, &path);
    assert_eq!(
        cleared.arrays[0].kind.depopulation(),
        None,
        "no depopulation left"
    );
    assert_eq!(baked_numbers(&cleared), ["1", "2", "3", "4", "5"]);
}
