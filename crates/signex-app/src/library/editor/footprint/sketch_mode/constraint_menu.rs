//! One constraint menu (#188): a right-click on a constraint's glyph or
//! value opens it at the click, on the footprint editor and the board
//! sketch alike. Rows: Edit value, Make parameter and Convert to reference
//! (dimensions only; a reference dimension shows Make driving instead and
//! greys the value rows), Suppress, Select constrained entities, Show
//! conflicts (only when the constraint is in one), Delete. A pattern glyph
//! opens the same card with the pattern's rows: one Edit row per value,
//! then Delete pattern; a mirror glyph offers only Delete mirror. Greyed
//! rows carry a tooltip that says why. Drawn with the shared dropdown
//! renderer, over a click-outside layer that closes it.

use std::path::PathBuf;

use iced::widget::{Space, container, mouse_area, stack};
use iced::{Element, Length, Widget as _};
use signex_i18n::{Arg, I18n, MessageId, ids, t};
use signex_sketch::SketchData;
use signex_sketch::array::{ArrayId, ArrayKind};
use signex_sketch::constraint::{ConstraintKind, DimTarget};
use signex_sketch::id::ConstraintId;
use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar_dropdown::{self, DropdownEntry, DropdownItem, MenuMetrics};

use super::value_edit::{self, SketchCanvasMsg};
use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::FootprintEditorState;
use crate::library::editor::footprint::state::sketch_overlay::{
    MenuTarget, PatternField, ValueTarget,
};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// Convert to reference, Make driving and Delete mirror (#187, #188): each
/// one undo step, solved like every sketch edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SketchReferenceMsg {
    /// Convert to reference: the dimension now measures, never drives.
    ReferenceConverted(ConstraintId),
    /// Make driving: the reference dimension drives again.
    DrivingRestored(ConstraintId),
    /// Delete mirror, on a pattern mirror's glyph.
    MirrorDeleted(ArrayId),
}

/// What a menu row sends.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    Canvas(SketchCanvasMsg),
    Reference(SketchReferenceMsg),
}

impl From<SketchCanvasMsg> for MenuAction {
    fn from(msg: SketchCanvasMsg) -> Self {
        Self::Canvas(msg)
    }
}

impl From<SketchReferenceMsg> for MenuAction {
    fn from(msg: SketchReferenceMsg) -> Self {
        Self::Reference(msg)
    }
}

impl MenuAction {
    fn into_editor_msg(self) -> FootprintEditorMsg {
        match self {
            Self::Canvas(msg) => FootprintEditorMsg::SketchCanvas(msg),
            Self::Reference(msg) => FootprintEditorMsg::SketchReference(msg),
        }
    }
}

/// Why a row is greyed: the message, and the parameter it names if any.
#[derive(Debug, Clone, PartialEq)]
pub struct Why {
    pub id: MessageId,
    pub name: Option<String>,
}

impl Why {
    fn plain(id: MessageId) -> Self {
        Self { id, name: None }
    }
}

/// One row of a canvas menu, as data.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuRow {
    pub label: MessageId,
    /// What a click sends; `None` greys the row.
    pub action: Option<MenuAction>,
    /// The tooltip of a greyed row.
    pub why: Option<Why>,
    /// A right-aligned hint.
    pub hint: Option<MessageId>,
    /// A separator line goes above this row.
    pub separated: bool,
}

impl MenuRow {
    fn new(label: MessageId, action: impl Into<MenuAction>) -> Self {
        Self {
            label,
            action: Some(action.into()),
            why: None,
            hint: None,
            separated: false,
        }
    }

    fn greyed(label: MessageId, why: Why) -> Self {
        Self {
            label,
            action: None,
            why: Some(why),
            hint: None,
            separated: false,
        }
    }
}

/// The message `Make parameter` hands to the sketch parameters table, which
/// names the parameter and binds the dimension to it.
pub fn parameter_request(constraint: ConstraintId) -> FootprintEditorMsg {
    FootprintEditorMsg::SketchParameters(
        crate::library::editor::footprint::parameters_table::SketchParametersMsg::ParameterFromConstraintRequested {
            constraint,
        },
    )
}

/// The name of a constraint kind in the menu header.
fn kind_name(kind: &ConstraintKind) -> MessageId {
    use ConstraintKind::*;
    match kind {
        Coincident { .. } => ids::SKETCH_CANVAS_KIND_COINCIDENT,
        PointOnLine { .. } => ids::SKETCH_CANVAS_KIND_POINT_ON_LINE,
        PointOnArc { .. } => ids::SKETCH_CANVAS_KIND_POINT_ON_ARC,
        Horizontal { .. } => ids::SKETCH_CANVAS_KIND_HORIZONTAL,
        Vertical { .. } => ids::SKETCH_CANVAS_KIND_VERTICAL,
        Parallel { .. } => ids::SKETCH_CANVAS_KIND_PARALLEL,
        Perpendicular { .. } => ids::SKETCH_CANVAS_KIND_PERPENDICULAR,
        DistancePtPt { .. } => ids::SKETCH_CANVAS_KIND_DISTANCE,
        DistancePtLine { .. } => ids::SKETCH_CANVAS_KIND_DISTANCE_TO_LINE,
        DistancePtCircle { .. } => ids::SKETCH_CANVAS_KIND_DISTANCE_TO_CIRCLE,
        Angle { .. } => ids::SKETCH_CANVAS_KIND_ANGLE,
        EqualLength { .. } => ids::SKETCH_CANVAS_KIND_EQUAL_LENGTH,
        EqualRadius { .. } => ids::SKETCH_CANVAS_KIND_EQUAL_RADIUS,
        TangentLineArc { .. } | TangentArcArc { .. } => ids::SKETCH_CANVAS_KIND_TANGENT,
        SymmetricAboutLine { .. } | SymmetricAboutPoint { .. } => ids::SKETCH_CANVAS_KIND_SYMMETRIC,
        Midpoint { .. } => ids::SKETCH_CANVAS_KIND_MIDPOINT,
        Fixed { .. } => ids::SKETCH_CANVAS_KIND_FIXED,
    }
}

/// The parameter a dimension's target is bound to, if any.
fn bound_name(kind: &ConstraintKind, sketch: &SketchData) -> Option<String> {
    match kind.dim_target()? {
        DimTarget::Expr(source) => value_edit::bound_parameter(source, sketch).map(str::to_owned),
        DimTarget::Literal(_) => None,
    }
}

/// The value rows of a driving dimension: Edit value, Make parameter,
/// Convert to reference.
fn driving_rows(id: ConstraintId, kind: &ConstraintKind, sketch: &SketchData) -> [MenuRow; 3] {
    let mut edit = MenuRow::new(
        ids::SKETCH_CANVAS_MENU_EDIT_VALUE,
        SketchCanvasMsg::ValueEditOpened(ValueTarget::Constraint(id)),
    );
    edit.hint = Some(ids::SKETCH_CANVAS_MENU_EDIT_VALUE_HINT);
    let make_parameter = match bound_name(kind, sketch) {
        Some(name) => MenuRow::greyed(
            ids::SKETCH_CANVAS_MENU_MAKE_PARAMETER,
            Why {
                id: ids::SKETCH_CANVAS_MENU_MAKE_PARAMETER_BOUND,
                name: Some(name),
            },
        ),
        None => MenuRow::new(
            ids::SKETCH_CANVAS_MENU_MAKE_PARAMETER,
            SketchCanvasMsg::ParameterRequested(id),
        ),
    };
    let convert = MenuRow::new(
        ids::SKETCH_CANVAS_MENU_CONVERT_REFERENCE,
        SketchReferenceMsg::ReferenceConverted(id),
    );
    [edit, make_parameter, convert]
}

/// The value rows of a reference dimension: Edit value and Make parameter
/// greyed (it measures), then Make driving.
fn driven_rows(id: ConstraintId) -> [MenuRow; 3] {
    let measures = || Why::plain(ids::SKETCH_CANVAS_MENU_REFERENCE_MEASURES);
    [
        MenuRow::greyed(ids::SKETCH_CANVAS_MENU_EDIT_VALUE, measures()),
        MenuRow::greyed(ids::SKETCH_CANVAS_MENU_MAKE_PARAMETER, measures()),
        MenuRow::new(
            ids::SKETCH_CANVAS_MENU_MAKE_DRIVING,
            SketchReferenceMsg::DrivingRestored(id),
        ),
    ]
}

/// The rows of constraint `id` of `kind`. `in_conflict` adds Show
/// conflicts.
pub fn constraint_rows(
    id: ConstraintId,
    kind: &ConstraintKind,
    sketch: &SketchData,
    in_conflict: bool,
) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    if kind.dim_target().is_some() {
        if sketch.is_driven(id) {
            rows.extend(driven_rows(id));
        } else {
            rows.extend(driving_rows(id, kind, sketch));
        }
    }
    rows.push(MenuRow::greyed(
        ids::SKETCH_CANVAS_MENU_SUPPRESS,
        Why::plain(ids::SKETCH_CANVAS_MENU_SUPPRESS_WHY),
    ));
    rows.push(MenuRow::new(
        ids::SKETCH_CANVAS_MENU_SELECT_ENTITIES,
        SketchCanvasMsg::ConstrainedEntitiesSelected(id),
    ));
    if in_conflict {
        rows.push(MenuRow::new(
            ids::SKETCH_CANVAS_MENU_SHOW_CONFLICTS,
            SketchCanvasMsg::ConflictsShown(id),
        ));
    }
    let mut delete = MenuRow::new(
        ids::SKETCH_CANVAS_MENU_DELETE,
        SketchCanvasMsg::ConstraintDeleted(id),
    );
    delete.separated = true;
    rows.push(delete);
    rows
}

/// The Edit row of one pattern value.
fn field_label(field: PatternField) -> MessageId {
    match field {
        PatternField::Count => ids::SKETCH_CANVAS_MENU_EDIT_COUNT,
        PatternField::Columns => ids::SKETCH_CANVAS_MENU_EDIT_COLUMNS,
        PatternField::Rows => ids::SKETCH_CANVAS_MENU_EDIT_ROWS,
        PatternField::SpacingX => ids::SKETCH_CANVAS_MENU_EDIT_SPACING_X,
        PatternField::SpacingY => ids::SKETCH_CANVAS_MENU_EDIT_SPACING_Y,
        PatternField::Sweep => ids::SKETCH_CANVAS_MENU_EDIT_SWEEP,
    }
}

/// The rows of pattern `array`: one Edit row per value, then Delete.
pub fn pattern_rows(array: ArrayId, kind: &ArrayKind) -> Vec<MenuRow> {
    let mut rows: Vec<MenuRow> = PatternField::of(kind)
        .iter()
        .map(|&field| {
            MenuRow::new(
                field_label(field),
                SketchCanvasMsg::ValueEditOpened(ValueTarget::Pattern(array, field)),
            )
        })
        .collect();
    let mut delete = MenuRow::new(
        ids::SKETCH_CANVAS_MENU_DELETE_PATTERN,
        SketchCanvasMsg::PatternDeleted(array),
    );
    delete.separated = true;
    rows.push(delete);
    rows
}

/// The rows of pattern mirror `mirror`: Delete mirror only. Its values are
/// the mirrored pattern's, edited there.
pub fn mirror_rows(mirror: ArrayId) -> Vec<MenuRow> {
    vec![MenuRow::new(
        ids::SKETCH_CANVAS_MENU_DELETE_MIRROR,
        SketchReferenceMsg::MirrorDeleted(mirror),
    )]
}

/// The header and rows of the open menu; `None` when no menu is open or
/// its constraint, pattern or mirror is gone. A mirror glyph opens through
/// the pattern target, its id being a pattern-object id.
fn open_menu(
    state: &FootprintEditorState,
    sketch: &SketchData,
    i18n: &I18n,
) -> Option<(String, Vec<MenuRow>)> {
    match state.sketch_overlay.menu?.target {
        MenuTarget::Constraint(id) => {
            let c = sketch.constraints.iter().find(|c| c.id == id)?;
            let in_conflict = value_edit::conflicting_constraints(state).contains(&id);
            let kind = t!(i18n, kind_name(&c.kind)).into_string();
            let header = match bound_name(&c.kind, sketch) {
                Some(name) => i18n.message_plain(
                    ids::SKETCH_CANVAS_MENU_HEADER_BOUND,
                    &[("kind", Arg::from(kind)), ("name", Arg::from(name))],
                ),
                None => kind,
            };
            Some((header, constraint_rows(id, &c.kind, sketch, in_conflict)))
        }
        MenuTarget::Pattern(array) => {
            if sketch.pattern_mirrors.iter().any(|m| m.id == array) {
                let header = t!(i18n, ids::SKETCH_CANVAS_PATTERN_MENU_MIRROR).into_string();
                return Some((header, mirror_rows(array)));
            }
            let a = sketch.arrays.iter().find(|a| a.id == array)?;
            let header = match a.kind {
                ArrayKind::Linear { .. } => ids::SKETCH_CANVAS_PATTERN_MENU_LINEAR,
                ArrayKind::Grid { .. } => ids::SKETCH_CANVAS_PATTERN_MENU_GRID,
                ArrayKind::Polar { .. } => ids::SKETCH_CANVAS_PATTERN_MENU_POLAR,
            };
            Some((t!(i18n, header).into_string(), pattern_rows(array, &a.kind)))
        }
    }
}

fn fp(path: PathBuf, msg: FootprintEditorMsg) -> LibraryMessage {
    LibraryMessage::PrimitiveEditorEvent {
        path,
        msg: PrimitiveEdit::Footprint(msg),
    }
}

/// The open menu as dropdown entries.
pub fn entries(
    state: &FootprintEditorState,
    sketch: &SketchData,
    path: PathBuf,
    i18n: &I18n,
) -> Option<Vec<DropdownEntry<LibraryMessage>>> {
    let (header, rows) = open_menu(state, sketch, i18n)?;
    let mut entries = vec![DropdownEntry::Header(header)];
    for row in rows {
        if row.separated {
            entries.push(DropdownEntry::Separator);
        }
        let label = t!(i18n, row.label).into_string();
        let mut item = match row.action {
            Some(action) => DropdownItem::new(label, fp(path.clone(), action.into_editor_msg())),
            None => DropdownItem::passive(label).disabled(true),
        };
        if let Some(why) = row.why {
            let args: Vec<(&str, Arg)> = why
                .name
                .map(|name| ("name", Arg::from(name)))
                .into_iter()
                .collect();
            item = item.tooltip(i18n.message_plain(why.id, &args));
        }
        if let Some(hint) = row.hint {
            item = item.shortcut(t!(i18n, hint).into_string());
        }
        entries.push(DropdownEntry::Item(item));
    }
    Some(entries)
}

/// The open menu over `editor`'s canvas, at the click, with the layer that
/// closes it on a click outside. Its width is measured in `ui_family`, the
/// configured UI font, and it moves only as far as needed to stay inside
/// the canvas. An empty layer when no menu is open, so the stack it is
/// pushed on keeps its shape.
pub fn overlay<'a>(
    editor: &FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    ui_family: &str,
) -> Element<'a, LibraryMessage> {
    let open = editor
        .state
        .sketch_overlay
        .menu
        .zip(editor.primitive().sketch.as_ref())
        .and_then(|(menu, sketch)| {
            entries(&editor.state, sketch, editor.path.clone(), i18n).map(|e| (menu, e))
        });
    let Some((menu, entries)) = open else {
        return Space::new().boxed();
    };
    let width = MenuMetrics::GRAPHITE.fitted_width(&entries, f32::MAX, ui_family);
    let card = active_bar_dropdown::view(entries, tokens, Some(width));
    let close = fp(
        editor.path.clone(),
        FootprintEditorMsg::SketchCanvas(SketchCanvasMsg::MenuClosed),
    );
    let backstop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(close.clone())
    .on_right_press(close);
    let anchored = crate::app::view::AnchoredMenu::new(card, (menu.x, menu.y));
    stack![backstop, anchored.boxed()].boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::id::SketchEntityId;

    fn labels(rows: &[MenuRow]) -> Vec<MessageId> {
        rows.iter().map(|r| r.label).collect()
    }

    fn distance(target: DimTarget) -> ConstraintKind {
        ConstraintKind::DistancePtPt {
            p1: SketchEntityId::new(),
            p2: SketchEntityId::new(),
            target,
        }
    }

    /// A sketch holding dimension `id` of `kind`, driven when `driven`.
    fn holding(id: ConstraintId, kind: ConstraintKind, driven: bool) -> SketchData {
        let mut sketch = SketchData {
            constraints: vec![signex_sketch::constraint::Constraint { id, kind }],
            ..SketchData::default()
        };
        if driven {
            assert!(sketch.set_driven(id, true));
        }
        sketch
    }

    #[test]
    fn a_dimension_gets_the_value_rows_and_a_relation_does_not() {
        let id = ConstraintId::new();
        let sketch = SketchData::default();
        let dim = constraint_rows(id, &distance(DimTarget::Literal(0.7)), &sketch, false);
        assert_eq!(
            labels(&dim),
            [
                ids::SKETCH_CANVAS_MENU_EDIT_VALUE,
                ids::SKETCH_CANVAS_MENU_MAKE_PARAMETER,
                ids::SKETCH_CANVAS_MENU_CONVERT_REFERENCE,
                ids::SKETCH_CANVAS_MENU_SUPPRESS,
                ids::SKETCH_CANVAS_MENU_SELECT_ENTITIES,
                ids::SKETCH_CANVAS_MENU_DELETE,
            ]
        );
        let horizontal = ConstraintKind::Horizontal {
            line: SketchEntityId::new(),
        };
        assert_eq!(
            labels(&constraint_rows(id, &horizontal, &sketch, false)),
            [
                ids::SKETCH_CANVAS_MENU_SUPPRESS,
                ids::SKETCH_CANVAS_MENU_SELECT_ENTITIES,
                ids::SKETCH_CANVAS_MENU_DELETE,
            ]
        );
    }

    #[test]
    fn show_conflicts_appears_only_for_a_constraint_in_conflict() {
        let id = ConstraintId::new();
        let sketch = SketchData::default();
        let kind = distance(DimTarget::Literal(1.0));
        let has = |rows: Vec<MenuRow>| {
            rows.iter()
                .any(|r| r.label == ids::SKETCH_CANVAS_MENU_SHOW_CONFLICTS)
        };
        assert!(!has(constraint_rows(id, &kind, &sketch, false)));
        assert!(has(constraint_rows(id, &kind, &sketch, true)));
    }

    #[test]
    fn greyed_rows_say_why_and_send_nothing() {
        let id = ConstraintId::new();
        let rows = constraint_rows(
            id,
            &distance(DimTarget::Literal(1.0)),
            &SketchData::default(),
            true,
        );
        for row in &rows {
            assert_eq!(
                row.action.is_none(),
                row.why.is_some(),
                "a greyed row has a tooltip and only a greyed row: {row:?}"
            );
        }
        let greyed: Vec<MessageId> = rows
            .iter()
            .filter(|r| r.action.is_none())
            .map(|r| r.label)
            .collect();
        assert_eq!(greyed, [ids::SKETCH_CANVAS_MENU_SUPPRESS]);
        let delete = rows.last().expect("rows");
        assert_eq!(
            delete.action,
            Some(MenuAction::Canvas(SketchCanvasMsg::ConstraintDeleted(id)))
        );
        assert!(delete.separated);
    }

    #[test]
    fn convert_to_reference_is_offered_on_a_driving_dimension() {
        let id = ConstraintId::new();
        let kind = distance(DimTarget::Literal(1.0));
        let sketch = holding(id, kind.clone(), false);
        let rows = constraint_rows(id, &kind, &sketch, false);
        let convert = rows
            .iter()
            .find(|r| r.label == ids::SKETCH_CANVAS_MENU_CONVERT_REFERENCE)
            .expect("row");
        assert_eq!(
            convert.action,
            Some(MenuAction::Reference(
                SketchReferenceMsg::ReferenceConverted(id)
            ))
        );
        assert!(
            rows.iter()
                .all(|r| r.label != ids::SKETCH_CANVAS_MENU_MAKE_DRIVING)
        );
    }

    #[test]
    fn a_reference_dimension_offers_make_driving_and_greys_its_value_rows() {
        let id = ConstraintId::new();
        let kind = distance(DimTarget::Literal(1.0));
        let sketch = holding(id, kind.clone(), true);
        let rows = constraint_rows(id, &kind, &sketch, false);
        assert_eq!(
            labels(&rows),
            [
                ids::SKETCH_CANVAS_MENU_EDIT_VALUE,
                ids::SKETCH_CANVAS_MENU_MAKE_PARAMETER,
                ids::SKETCH_CANVAS_MENU_MAKE_DRIVING,
                ids::SKETCH_CANVAS_MENU_SUPPRESS,
                ids::SKETCH_CANVAS_MENU_SELECT_ENTITIES,
                ids::SKETCH_CANVAS_MENU_DELETE,
            ]
        );
        for row in &rows[..2] {
            assert!(row.action.is_none(), "{row:?}");
            assert_eq!(
                row.why,
                Some(Why::plain(ids::SKETCH_CANVAS_MENU_REFERENCE_MEASURES))
            );
        }
        assert_eq!(
            rows[2].action,
            Some(MenuAction::Reference(SketchReferenceMsg::DrivingRestored(
                id
            )))
        );
    }

    #[test]
    fn make_parameter_is_greyed_when_the_value_already_is_one() {
        let mut sketch = SketchData::default();
        sketch.parameters.insert("pitch", "0.65mm");
        let rows = constraint_rows(
            ConstraintId::new(),
            &distance(DimTarget::Expr("pitch".into())),
            &sketch,
            false,
        );
        let make = rows
            .iter()
            .find(|r| r.label == ids::SKETCH_CANVAS_MENU_MAKE_PARAMETER)
            .expect("row");
        assert!(make.action.is_none());
        assert_eq!(
            make.why.as_ref().and_then(|w| w.name.as_deref()),
            Some("pitch")
        );
    }

    #[test]
    fn a_pattern_menu_edits_each_value_then_deletes() {
        let array = ArrayId::new();
        let kind = ArrayKind::Polar {
            source: SketchEntityId::new(),
            center: SketchEntityId::new(),
            count_expr: "8".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: None,
        };
        let rows = pattern_rows(array, &kind);
        assert_eq!(
            labels(&rows),
            [
                ids::SKETCH_CANVAS_MENU_EDIT_COUNT,
                ids::SKETCH_CANVAS_MENU_EDIT_SWEEP,
                ids::SKETCH_CANVAS_MENU_DELETE_PATTERN,
            ]
        );
        assert_eq!(
            rows[0].action,
            Some(MenuAction::Canvas(SketchCanvasMsg::ValueEditOpened(
                ValueTarget::Pattern(array, PatternField::Count)
            )))
        );
    }

    #[test]
    fn a_mirror_menu_only_deletes_the_mirror() {
        let mirror = ArrayId::new();
        let rows = mirror_rows(mirror);
        assert_eq!(labels(&rows), [ids::SKETCH_CANVAS_MENU_DELETE_MIRROR]);
        assert_eq!(
            rows[0].action,
            Some(MenuAction::Reference(SketchReferenceMsg::MirrorDeleted(
                mirror
            )))
        );
    }

    #[test]
    fn the_rendered_menu_carries_its_tooltips() {
        let mut state = FootprintEditorState::empty();
        let id = ConstraintId::new();
        let sketch = holding(id, distance(DimTarget::Literal(1.0)), false);
        state.sketch_overlay.menu = Some(
            crate::library::editor::footprint::state::sketch_overlay::CanvasMenu {
                target: MenuTarget::Constraint(id),
                x: 0.0,
                y: 0.0,
            },
        );
        let i18n = I18n::new(&signex_i18n::LanguagePref::Locale("en-US".into()));
        let entries = entries(&state, &sketch, PathBuf::from("t"), &i18n).expect("open");
        let items: Vec<&DropdownItem<LibraryMessage>> = entries
            .iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) => Some(item),
                _ => None,
            })
            .collect();
        for item in &items {
            assert_eq!(item.disabled, item.tooltip.is_some(), "{}", item.label);
        }
        assert!(matches!(&entries[0], DropdownEntry::Header(h) if h == "Distance"));
    }
}
