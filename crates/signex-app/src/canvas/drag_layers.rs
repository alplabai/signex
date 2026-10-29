//! The two layers of a move-drag preview (#1335).
//!
//! Redrawing the whole sheet for every cursor step costs O(sheet). Instead
//! the sheet is split once per drag: the items the drag leaves alone are
//! drawn into a retained geometry, and each frame draws only the moving
//! items, shifted, on top of it.
//!
//! Intended change: a moving item always draws above every unmoved one, even
//! where the full redraw drew an unmoved item over it. What is drawn is
//! otherwise the same.
//!
//! The renderer lowers each item on its own (the item lists plus the shared
//! `lib_symbols`), so drawing the static sheet and the moving sheet separately
//! produces exactly the primitives of the full sheet.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use iced::widget::canvas;
use signex_types::schematic::{ChildSheet, SchDrawing, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

use crate::schematic_runtime::{ContentSceneCache, SceneInputs, SceneOverrides, ScreenTransform};
use signex_types::theme::CanvasColors;

/// Revision `0` means "unknown" (`SchematicRenderCache::revision`), which can
/// never match a cached one: the document may have changed under it.
pub(super) fn is_known(revision: u64) -> bool {
    revision != 0
}

/// What one drag frame draws on top of the static layer.
#[derive(Debug, Clone)]
pub(crate) struct DragFrame {
    /// The moving items, already shifted (or rubber-banded): the selection
    /// plus anything the drag changes. Shares `lib_symbols` with the sheet.
    pub moving: Rc<SchematicSheet>,
    /// The ids of the sheet items the static layer leaves out.
    pub skip: Rc<HashSet<Uuid>>,
}

fn drawing_id(drawing: &SchDrawing) -> Uuid {
    match drawing {
        SchDrawing::Line { uuid, .. }
        | SchDrawing::Rect { uuid, .. }
        | SchDrawing::Circle { uuid, .. }
        | SchDrawing::Arc { uuid, .. }
        | SchDrawing::Polyline { uuid, .. } => *uuid,
    }
}

/// The ids of the whole sheet items a selection moves: a symbol field drags
/// with its symbol's item, a sheet pin with its child sheet's.
pub(super) fn moved_item_ids(snap: &SchematicSheet, selection: &[SelectedItem]) -> HashSet<Uuid> {
    let mut ids: HashSet<Uuid> = selection.iter().map(|item| item.uuid).collect();
    if selection
        .iter()
        .any(|item| item.kind == SelectedKind::SheetPin)
    {
        let owner: HashMap<Uuid, Uuid> = snap
            .child_sheets
            .iter()
            .flat_map(|cs| cs.pins.iter().map(move |pin| (pin.uuid, cs.uuid)))
            .collect();
        for item in selection
            .iter()
            .filter(|i| i.kind == SelectedKind::SheetPin)
        {
            if let Some(child) = owner.get(&item.uuid) {
                ids.insert(*child);
            }
        }
    }
    ids
}

fn keep_where<T: Clone>(
    items: &[T],
    id: impl Fn(&T) -> Uuid,
    keep: &dyn Fn(Uuid) -> bool,
) -> Vec<T> {
    items
        .iter()
        .filter(|item| keep(id(item)))
        .cloned()
        .collect()
}

/// A sheet holding the items of `snap` whose id passes `keep`, sharing its
/// library symbols.
fn filtered(snap: &SchematicSheet, keep: &dyn Fn(Uuid) -> bool) -> SchematicSheet {
    SchematicSheet {
        uuid: snap.uuid,
        font_family: snap.font_family.clone(),
        lib_symbols: snap.lib_symbols.clone(),
        // A drawing keeps its placement in whichever layer it lands (#1583).
        drawing_placements: snap.drawing_placements.clone(),
        symbols: keep_where(&snap.symbols, |s| s.uuid, keep),
        wires: keep_where(&snap.wires, |w| w.uuid, keep),
        junctions: keep_where(&snap.junctions, |j| j.uuid, keep),
        labels: keep_where(&snap.labels, |l| l.uuid, keep),
        child_sheets: keep_where(&snap.child_sheets, |c| c.uuid, keep),
        no_connects: keep_where(&snap.no_connects, |n| n.uuid, keep),
        text_notes: keep_where(&snap.text_notes, |t| t.uuid, keep),
        buses: keep_where(&snap.buses, |b| b.uuid, keep),
        bus_entries: keep_where(&snap.bus_entries, |b| b.uuid, keep),
        drawings: keep_where(&snap.drawings, drawing_id, keep),
        no_erc_directives: keep_where(&snap.no_erc_directives, |d| d.uuid, keep),
        ..SchematicSheet::default()
    }
}

/// `snap` without the items in `skip`: the static layer.
pub(super) fn sheet_without(snap: &SchematicSheet, skip: &HashSet<Uuid>) -> SchematicSheet {
    filtered(snap, &|id| !skip.contains(&id))
}

/// Only the items of `snap` in `ids`, unshifted: what the drag lifts.
fn sheet_only(snap: &SchematicSheet, ids: &HashSet<Uuid>) -> SchematicSheet {
    filtered(snap, &|id| ids.contains(&id))
}

/// One item kind of [`split_ghost`]: base ids that are gone or differ from
/// the ghost join `skip`; ghost items that are new, changed or selected join
/// `moving`.
fn diff_kind<T: PartialEq + Clone>(
    base: &[T],
    ghost: &[T],
    id: impl Fn(&T) -> Uuid,
    selected: &HashSet<Uuid>,
    skip: &mut HashSet<Uuid>,
    moving: &mut Vec<T>,
) {
    let ghost_by_id: HashMap<Uuid, &T> = ghost.iter().map(|item| (id(item), item)).collect();
    let mut base_ids = HashSet::with_capacity(base.len());
    for item in base {
        let uuid = id(item);
        base_ids.insert(uuid);
        if ghost_by_id.get(&uuid).is_none_or(|g| *g != item) {
            skip.insert(uuid);
        }
    }
    for item in ghost {
        let uuid = id(item);
        if skip.contains(&uuid) || selected.contains(&uuid) || !base_ids.contains(&uuid) {
            moving.push(item.clone());
        }
    }
}

/// Split the rubber-banded `ghost` (the sheet the engine would commit) against
/// the `base` sheet: the moving items are every one the drag changes, adds or
/// selects, and the static layer leaves out the base items they replace.
pub(super) fn split_ghost(
    base: &SchematicSheet,
    ghost: &SchematicSheet,
    selected: &HashSet<Uuid>,
) -> DragFrame {
    let mut skip = selected.clone();
    let mut moving = SchematicSheet {
        uuid: ghost.uuid,
        font_family: ghost.font_family.clone(),
        lib_symbols: ghost.lib_symbols.clone(),
        drawing_placements: ghost.drawing_placements.clone(),
        ..SchematicSheet::default()
    };
    macro_rules! kind {
        ($field:ident, $id:expr) => {
            diff_kind(
                &base.$field,
                &ghost.$field,
                $id,
                selected,
                &mut skip,
                &mut moving.$field,
            )
        };
    }
    kind!(symbols, |s: &signex_types::schematic::Symbol| s.uuid);
    kind!(wires, |w: &signex_types::schematic::Wire| w.uuid);
    kind!(junctions, |j: &signex_types::schematic::Junction| j.uuid);
    kind!(labels, |l: &signex_types::schematic::Label| l.uuid);
    kind!(child_sheets, |c: &ChildSheet| c.uuid);
    kind!(no_connects, |n: &signex_types::schematic::NoConnect| n.uuid);
    kind!(text_notes, |t: &signex_types::schematic::TextNote| t.uuid);
    kind!(buses, |b: &signex_types::schematic::Bus| b.uuid);
    kind!(bus_entries, |b: &signex_types::schematic::BusEntry| b.uuid);
    kind!(drawings, drawing_id);
    kind!(
        no_erc_directives,
        |d: &signex_types::schematic::NoConnect| d.uuid
    );
    DragFrame {
        moving: Rc::new(moving),
        skip: Rc::new(skip),
    }
}

/// The items a drag lifted, found once per drag.
pub(super) struct Session {
    revision: u64,
    selection: Vec<SelectedItem>,
    /// Ids of the whole items the selection moves.
    pub ids: Rc<HashSet<Uuid>>,
    /// Those items, unshifted; each frame shifts a copy of just these.
    pub lifted: SchematicSheet,
}

/// What the static geometry was drawn for.
struct StaticKey {
    inputs: SceneInputs,
    revision: u64,
    skip: HashSet<Uuid>,
}

/// What a frame is drawn through: everything but the document that the static
/// geometry depends on.
pub(super) struct StaticView<'a> {
    pub transform: &'a ScreenTransform,
    pub colors: &'a CanvasColors,
    pub viewport: iced::Size,
    pub focus_set: Option<&'a HashSet<Uuid>>,
    pub overrides: SceneOverrides<'a>,
}

/// The drag session and the retained geometry of the static layer.
#[derive(Default)]
pub(super) struct DragLayers {
    session: Option<Session>,
    key: Option<StaticKey>,
    pub static_cache: canvas::Cache,
    /// The static layer as a scene, for a GPU-mounted sheet: built once per
    /// drag and again on zoom, a pan past its margin, a new revision or a new
    /// skip set. Its generation holds in between, so the GPU uploads it once.
    static_scene: ContentSceneCache,
    /// The revision and skip set `static_scene` was built for.
    scene_key: Option<(u64, HashSet<Uuid>)>,
    #[cfg(test)]
    builds: u32,
}

impl DragLayers {
    /// The session for this document revision and selection, found (one pass
    /// over the sheet) only when none matches.
    pub fn session_for(
        &mut self,
        snap: &SchematicSheet,
        revision: u64,
        selection: &[SelectedItem],
    ) -> &Session {
        let fits = self.session.as_ref().is_some_and(|s| {
            is_known(revision) && s.revision == revision && s.selection == selection
        });
        if !fits {
            let ids = moved_item_ids(snap, selection);
            self.session = Some(Session {
                revision,
                selection: selection.to_vec(),
                lifted: sheet_only(snap, &ids),
                ids: Rc::new(ids),
            });
        }
        self.session.as_ref().expect("session was just set")
    }

    /// The lifted items of the current drag, if one is under way.
    pub fn lifted(&self) -> Option<&SchematicSheet> {
        self.session.as_ref().map(|s| &s.lifted)
    }

    /// Rekey the retained static geometry for `view`, `revision` and `skip`,
    /// dropping it when any of them changed. For callers that only want the
    /// retained geometry current, not the answer [`Self::static_is_stale`]
    /// gives.
    pub fn refresh_static_key(
        &mut self,
        view: &StaticView<'_>,
        revision: u64,
        skip: &HashSet<Uuid>,
    ) {
        self.static_is_stale(view, revision, skip);
    }

    /// Whether the static geometry must be drawn again for `view` and this
    /// document `revision` and `skip` set. When it must, the retained
    /// geometry is dropped and the build is counted.
    pub fn static_is_stale(
        &mut self,
        view: &StaticView<'_>,
        revision: u64,
        skip: &HashSet<Uuid>,
    ) -> bool {
        let fresh = self.key.as_ref().is_some_and(|key| {
            is_known(revision)
                && key.revision == revision
                && key.skip == *skip
                && key.inputs.fits_in_place(
                    view.transform,
                    view.colors,
                    view.viewport,
                    view.focus_set,
                    view.overrides,
                )
        });
        if fresh {
            return false;
        }
        self.key = Some(StaticKey {
            inputs: SceneInputs::capture(
                view.transform,
                view.colors,
                view.viewport,
                view.focus_set,
                view.overrides,
            ),
            revision,
            skip: skip.clone(),
        });
        self.static_cache.clear();
        #[cfg(test)]
        {
            self.builds += 1;
        }
        true
    }

    /// The static layer's scene and generation for a GPU-mounted sheet: `snap`
    /// without `skip`, built only when the cached scene no longer fits.
    pub fn static_scene(
        &mut self,
        snap: &SchematicSheet,
        revision: u64,
        skip: &HashSet<Uuid>,
        view: &StaticView<'_>,
    ) -> (
        Option<std::sync::Arc<signex_gfx::scene::Scene>>,
        Option<u64>,
    ) {
        let current = self
            .scene_key
            .as_ref()
            .is_some_and(|(rev, set)| is_known(revision) && *rev == revision && set == skip);
        if !current {
            self.static_scene.invalidate();
            self.scene_key = Some((revision, skip.clone()));
        }
        let scene = self
            .static_scene
            .scene_for_with(
                || std::borrow::Cow::Owned(sheet_without(snap, skip)),
                view.transform,
                view.colors,
                iced::Rectangle::with_size(view.viewport),
                view.focus_set,
                view.overrides,
            )
            .cloned();
        (scene, self.static_scene.generation())
    }

    /// Whether a drag is being drawn in layers.
    pub fn is_active(&self) -> bool {
        self.session.is_some() || self.key.is_some() || self.scene_key.is_some()
    }

    /// Drop the session and the retained geometry (the drag ended or was
    /// cancelled, or the document changed).
    pub fn clear(&mut self) {
        self.session = None;
        self.key = None;
        self.static_cache.clear();
        self.static_scene.invalidate();
        self.scene_key = None;
    }

    /// How many times the static layer was (re)drawn.
    #[cfg(test)]
    pub fn builds(&self) -> u32 {
        self.builds
    }
}

#[cfg(test)]
mod tests {
    use super::super::drag_preview::GhostState;
    use super::super::shift_selection::shift_snapshot_for_selection;
    use super::*;
    use crate::schematic_runtime::{
        SceneOverrides, WireClassStyles, WireStyling, build_renderer_snapshot,
    };
    use signex_renderer::schematic::SchematicSnapshot as Primitives;
    use signex_types::schematic::{
        FillType, Graphic, HAlign, Junction, LibGraphic, LibPin, LibSymbol, Pin, PinDirection,
        PinShapeStyle, Point, Symbol, TextProp, VAlign, Wire,
    };
    use signex_types::theme::{Color, ThemeId, canvas_colors};

    fn lib_body(id: &str, half: f64) -> LibSymbol {
        let mut lib: LibSymbol =
            serde_json::from_value(serde_json::json!({ "id": id })).expect("lib symbol");
        lib.graphics.push(LibGraphic {
            unit: 0,
            body_style: 1,
            graphic: Graphic::Rectangle {
                start: Point::new(-half, -half),
                end: Point::new(half, half),
                width: 0.0,
                fill: FillType::Background,
            },
        });
        lib.pins.push(LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(-half, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        });
        lib
    }

    fn symbol(lib_id: &str, x: f64, y: f64) -> Symbol {
        Symbol {
            uuid: Uuid::now_v7(),
            lib_id: lib_id.into(),
            reference: "U1".into(),
            ref_text: Some(TextProp {
                position: Point::new(x, y - 8.0),
                rotation: 0.0,
                font_size: 1.27,
                justify_h: HAlign::Left,
                justify_v: VAlign::Top,
                hidden: false,
                font_family: None,
            }),
            position: Point::new(x, y),
            ..Symbol::default()
        }
    }

    fn wire(x0: f64, y0: f64, x1: f64, y1: f64) -> Wire {
        Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(x0, y0),
            end: Point::new(x1, y1),
            ..Wire::default()
        }
    }

    fn label(x: f64, y: f64) -> signex_types::schematic::Label {
        serde_json::from_value(serde_json::json!({
            "uuid": Uuid::now_v7(),
            "text": "N",
            "position": { "x": x, "y": y },
            "label_type": "net",
        }))
        .expect("label")
    }

    /// Two overlapping symbols (the first is moved), wires, a junction, a
    /// label and a drawing; returns the sheet and the selection.
    fn mixed_sheet() -> (SchematicSheet, Vec<SelectedItem>) {
        let mut sheet = SchematicSheet::default();
        sheet.lib_symbols.insert("A".into(), lib_body("A", 10.0));
        sheet.lib_symbols.insert("B".into(), lib_body("B", 15.0));
        let moved = symbol("A", 50.0, 50.0);
        let covering = symbol("B", 50.0, 50.0);
        let moved_wire = wire(0.0, 0.0, 10.0, 0.0);
        let still_wire = wire(0.0, 20.0, 10.0, 20.0);
        let moved_junction = Junction {
            uuid: Uuid::now_v7(),
            position: Point::new(5.0, 0.0),
            ..Junction::default()
        };
        let still_junction = Junction {
            uuid: Uuid::now_v7(),
            position: Point::new(5.0, 20.0),
            ..Junction::default()
        };
        let moved_label = label(2.0, 2.0);
        let still_label = label(2.0, 22.0);
        let drawing = SchDrawing::Line {
            uuid: Uuid::now_v7(),
            start: Point::new(0.0, 30.0),
            end: Point::new(5.0, 31.0),
            width: 0.0,
            stroke_color: None,
            locked: false,
        };
        let selection = vec![
            SelectedItem::new(moved.uuid, SelectedKind::Symbol),
            SelectedItem::new(moved_wire.uuid, SelectedKind::Wire),
            SelectedItem::new(moved_junction.uuid, SelectedKind::Junction),
            SelectedItem::new(moved_label.uuid, SelectedKind::Label),
            SelectedItem::new(drawing_id(&drawing), SelectedKind::Drawing),
        ];
        sheet.symbols.extend([moved, covering]);
        sheet.wires.extend([moved_wire, still_wire]);
        sheet.junctions.extend([moved_junction, still_junction]);
        sheet.labels.extend([moved_label, still_label]);
        sheet.drawings.push(drawing);
        (sheet, selection)
    }

    /// Every primitive of a build as one sorted bag of strings per kind, so
    /// two builds compare as multisets (z-order ignored).
    fn bags(p: &Primitives) -> Vec<Vec<String>> {
        fn bag<T: std::fmt::Debug>(items: &[T]) -> Vec<String> {
            let mut out: Vec<String> = items.iter().map(|i| format!("{i:?}")).collect();
            out.sort();
            out
        }
        let mut colors: Vec<String> = p
            .wire_color_overrides
            .iter()
            .map(|e| format!("{e:?}"))
            .collect();
        colors.sort();
        let mut styles: Vec<String> = p
            .wire_line_styles
            .iter()
            .map(|e| format!("{e:?}"))
            .collect();
        styles.sort();
        vec![
            bag(&p.wires),
            bag(&p.junctions),
            bag(&p.arcs),
            bag(&p.polygons),
            bag(&p.labels),
            bag(&p.pin_texts),
            bag(&p.reference_value_texts),
            bag(&p.parameter_texts),
            colors,
            styles,
        ]
    }

    fn merged(a: Vec<Vec<String>>, b: Vec<Vec<String>>) -> Vec<Vec<String>> {
        a.into_iter()
            .zip(b)
            .map(|(mut x, y)| {
                x.extend(y);
                x.sort();
                x
            })
            .collect()
    }

    struct Setup {
        overrides: HashMap<Uuid, Color>,
        classes: WireClassStyles,
        focus: HashSet<Uuid>,
    }

    fn build(sheet: &SchematicSheet, setup: &Setup) -> Primitives {
        let transform = ScreenTransform {
            offset_x: 0.0,
            offset_y: 0.0,
            scale: 4.0,
        };
        let bounds = iced::Rectangle::new(
            iced::Point::new(-10_000.0, -10_000.0),
            iced::Size::new(20_000.0, 20_000.0),
        );
        build_renderer_snapshot(
            sheet,
            &transform,
            &canvas_colors(ThemeId::Signex),
            bounds,
            Some(&setup.focus),
            SceneOverrides {
                wire_styling: Some(WireStyling {
                    color_overrides: &setup.overrides,
                    class_styles: &setup.classes,
                    class_revision: 0,
                }),
                designators: None,
                special_strings: None,
            },
        )
    }

    /// Net-colour overrides on a moving wire, a moving junction and a still
    /// wire, and a focus set naming one moving and one still item.
    fn setup_for(sheet: &SchematicSheet) -> Setup {
        let red = Color::new(255, 0, 0, 255);
        Setup {
            overrides: HashMap::from([
                (sheet.wires[0].uuid, red),
                (sheet.wires[1].uuid, red),
                (sheet.junctions[0].uuid, red),
            ]),
            classes: WireClassStyles::new(),
            focus: HashSet::from([sheet.symbols[0].uuid, sheet.wires[1].uuid]),
        }
    }

    #[test]
    fn the_layers_hold_exactly_the_primitives_of_the_shifted_sheet() {
        let (sheet, selection) = mixed_sheet();
        let setup = setup_for(&sheet);
        let (dx, dy) = (7.5, -3.0);

        let old = build(
            &shift_snapshot_for_selection(&sheet, &selection, dx, dy),
            &setup,
        );

        let mut layers = DragLayers::default();
        let session = layers.session_for(&sheet, 1, &selection);
        let moving = shift_snapshot_for_selection(&session.lifted, &selection, dx, dy);
        let still = sheet_without(&sheet, &session.ids);
        let new = merged(bags(&build(&still, &setup)), bags(&build(&moving, &setup)));

        assert_eq!(new, bags(&old));
        assert!(!new[0].is_empty() && !new[1].is_empty() && !new[3].is_empty());
    }

    #[test]
    fn the_moving_layer_holds_only_the_selected_items() {
        let (sheet, selection) = mixed_sheet();
        let mut layers = DragLayers::default();
        let session = layers.session_for(&sheet, 1, &selection);
        let lifted = &session.lifted;
        assert_eq!(lifted.symbols.len(), 1);
        assert_eq!(lifted.wires.len(), 1);
        assert_eq!(lifted.junctions.len(), 1);
        assert_eq!(lifted.labels.len(), 1);
        assert_eq!(lifted.drawings.len(), 1);
        assert_eq!(lifted.symbols[0].uuid, sheet.symbols[0].uuid);
        // The lifted items share the library symbols, not copy them.
        assert!(lifted.lib_symbols.shares_storage_with(&sheet.lib_symbols));

        let still = sheet_without(&sheet, &session.ids);
        assert_eq!(still.symbols.len(), 1);
        assert_eq!(still.symbols[0].uuid, sheet.symbols[1].uuid);
        assert_eq!(still.wires.len(), 1);
        assert!(still.drawings.is_empty());
    }

    #[test]
    fn a_stretched_wire_and_its_new_bend_are_moving_and_the_far_wire_is_not() {
        let mut sheet = SchematicSheet::default();
        sheet.lib_symbols.insert("A".into(), lib_body("A", 5.0));
        let resistor = symbol("A", 50.0, 50.0);
        let attached = wire(30.0, 50.0, 45.0, 50.0);
        let far = wire(0.0, 0.0, 10.0, 0.0);
        let selection = vec![SelectedItem::new(resistor.uuid, SelectedKind::Symbol)];
        sheet.symbols.push(resistor);
        sheet.wires.extend([attached.clone(), far.clone()]);

        // Along the wire the attached end stretches; across it a bend is added.
        for (dx, dy) in [(5.0, 0.0), (0.0, 10.0)] {
            let mut ghosts = GhostState::default();
            let ghost = ghosts.refresh(&sheet, 1, &selection, dx, dy);
            let frame = split_ghost(&sheet, &ghost, &moved_item_ids(&sheet, &selection));

            assert!(!frame.skip.contains(&far.uuid), "{dx},{dy}");
            assert!(frame.moving.wires.iter().all(|w| w.uuid != far.uuid));
            let changed = ghost.wires.iter().any(|w| {
                w.uuid == attached.uuid && (w.start != attached.start || w.end != attached.end)
            });
            assert_eq!(frame.skip.contains(&attached.uuid), changed, "{dx},{dy}");
            assert!(
                ghost.wires.len() == sheet.wires.len() || !frame.moving.wires.is_empty(),
                "a new bend wire is a moving item"
            );
            if dy != 0.0 {
                assert_eq!(ghost.wires.len(), 3, "the bend wire was added");
            } else {
                assert!(changed, "the attached wire was stretched");
            }
            check_equivalent(&sheet, &ghost, &frame, &attached, &far);
        }
    }

    fn check_equivalent(
        sheet: &SchematicSheet,
        ghost: &SchematicSheet,
        frame: &DragFrame,
        attached: &Wire,
        far: &Wire,
    ) {
        let setup = Setup {
            overrides: HashMap::from([(attached.uuid, Color::new(0, 255, 0, 255))]),
            classes: WireClassStyles::new(),
            focus: HashSet::from([far.uuid]),
        };
        let still = sheet_without(sheet, &frame.skip);
        let new = merged(
            bags(&build(&still, &setup)),
            bags(&build(&frame.moving, &setup)),
        );
        assert_eq!(new, bags(&build(ghost, &setup)));
    }

    #[test]
    fn a_zero_delta_ghost_still_carries_the_selection() {
        let (sheet, selection) = mixed_sheet();
        let ids = moved_item_ids(&sheet, &selection);
        let frame = split_ghost(&sheet, &sheet, &ids);
        assert_eq!(frame.moving.symbols.len(), 1);
        assert_eq!(frame.moving.wires.len(), 1);
        assert_eq!(*frame.skip, ids);
    }

    #[test]
    fn a_moved_item_draws_after_an_overlapping_unmoved_one() {
        // Intended change (#1335): the full redraw drew the symbol that comes
        // later in the sheet over the earlier one; the layers draw the moving
        // symbol over everything unmoved.
        let (sheet, selection) = mixed_sheet();
        let setup = setup_for(&sheet);
        let width = |p: &signex_renderer::schematic::PolygonInput| {
            let xs = p.vertices.iter().map(|v| v[0]);
            xs.clone().fold(f32::MIN, f32::max) - xs.fold(f32::MAX, f32::min)
        };
        let order = |p: &Primitives| -> Vec<bool> {
            // true = the moved symbol's 20 mm body, false = the covering 30 mm one
            p.polygons
                .iter()
                .map(width)
                .filter(|w| (*w - 20.0).abs() < 0.5 || (*w - 30.0).abs() < 0.5)
                .map(|w| (w - 20.0).abs() < 0.5)
                .collect()
        };

        let old = order(&build(&sheet, &setup));
        assert_eq!(
            old,
            vec![true, false],
            "sheet order: the covering symbol is on top"
        );

        let mut layers = DragLayers::default();
        let session = layers.session_for(&sheet, 1, &selection);
        let still = build(&sheet_without(&sheet, &session.ids), &setup);
        let moving = build(
            &shift_snapshot_for_selection(&session.lifted, &selection, 1.0, 1.0),
            &setup,
        );
        let layered: Vec<bool> = order(&still).into_iter().chain(order(&moving)).collect();
        assert_eq!(layered, vec![false, true], "the moving symbol is on top");
    }

    #[test]
    fn the_session_is_found_once_per_revision_and_selection() {
        let (sheet, selection) = mixed_sheet();
        let mut layers = DragLayers::default();
        let first = layers.session_for(&sheet, 1, &selection).ids.clone();
        let again = layers.session_for(&sheet, 1, &selection).ids.clone();
        assert!(Rc::ptr_eq(&first, &again), "no new pass over the sheet");
        let edited = layers.session_for(&sheet, 2, &selection).ids.clone();
        assert!(!Rc::ptr_eq(&first, &edited), "an edit finds it again");
        let fewer = layers.session_for(&sheet, 2, &selection[..1]).ids.clone();
        assert_eq!(fewer.len(), 1);
    }

    #[test]
    fn the_static_layer_is_drawn_once_across_frames_and_again_on_a_view_change() {
        let colors = canvas_colors(ThemeId::Signex);
        let transform = |scale: f32, offset_x: f32| ScreenTransform {
            offset_x,
            offset_y: 0.0,
            scale,
        };
        let skip: HashSet<Uuid> = HashSet::from([Uuid::now_v7()]);
        let mut layers = DragLayers::default();
        let stale =
            |layers: &mut DragLayers, t: &ScreenTransform, rev: u64, skip: &HashSet<Uuid>| {
                layers.static_is_stale(
                    &StaticView {
                        transform: t,
                        colors: &colors,
                        viewport: iced::Size::new(800.0, 600.0),
                        focus_set: None,
                        overrides: SceneOverrides::default(),
                    },
                    rev,
                    skip,
                )
            };

        let base = transform(4.0, 10.0);
        for frame in 0..25 {
            assert_eq!(
                stale(&mut layers, &base, 1, &skip),
                frame == 0,
                "frame {frame}"
            );
        }
        assert_eq!(layers.builds(), 1, "25 frames, one build");

        assert!(
            stale(&mut layers, &transform(8.0, 10.0), 1, &skip),
            "a zoom"
        );
        assert!(stale(&mut layers, &transform(8.0, 40.0), 1, &skip), "a pan");
        assert!(
            stale(&mut layers, &transform(8.0, 40.0), 2, &skip),
            "an edit"
        );
        assert!(
            stale(&mut layers, &transform(8.0, 40.0), 2, &HashSet::new()),
            "a different lifted set"
        );
        assert_eq!(layers.builds(), 5);
    }

    #[test]
    fn the_end_or_cancel_of_a_drag_drops_the_session_and_the_static_layer() {
        let (sheet, selection) = mixed_sheet();
        let colors = canvas_colors(ThemeId::Signex);
        let mut layers = DragLayers::default();
        assert!(!layers.is_active());
        layers.session_for(&sheet, 1, &selection);
        layers.static_is_stale(
            &StaticView {
                transform: &ScreenTransform {
                    offset_x: 0.0,
                    offset_y: 0.0,
                    scale: 1.0,
                },
                colors: &colors,
                viewport: iced::Size::new(10.0, 10.0),
                focus_set: None,
                overrides: SceneOverrides::default(),
            },
            1,
            &HashSet::new(),
        );
        assert!(layers.is_active() && layers.lifted().is_some());

        layers.clear();
        assert!(!layers.is_active());
        assert!(layers.lifted().is_none());
    }

    #[test]
    fn a_document_change_drops_the_drag_layers() {
        let (sheet, selection) = mixed_sheet();
        let mut slot = super::super::CanvasSlot::new();
        slot.drag_layers
            .get_mut()
            .session_for(&sheet, 1, &selection);
        assert!(slot.drag_layers.borrow().is_active());
        slot.clear_content_cache();
        assert!(!slot.drag_layers.borrow().is_active());
    }
}
