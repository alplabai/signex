//! Sheet entries (the pins on a sheet symbol's border) and the child sheet a
//! symbol stands for: pure domain rules, no file IO.
//!
//! The app collects the cursor and the name, asks here where the entry goes
//! and whether it is acceptable, and applies `Command::PlaceSheetPin`.
//! Creating the child file from a symbol is [`child_document_for_symbol`]'s
//! content plus the app's write; the write is not an edit and is not undone.

use signex_types::schematic::{
    ChildSheet, GRID_MM, HAlign, Label, LabelType, Point, SchematicSheet, SheetPin, VAlign,
    normalize_port_name,
};
use uuid::Uuid;

use crate::EngineError;
use crate::sheet::{lock_sheet_pin_to_child_edge, normalize_sheet_pin_direction};

/// How far outside a symbol's outline (mm) the cursor still picks it.
pub const SHEET_SYMBOL_REACH_MM: f64 = 2.0 * GRID_MM;

/// Distance from `p` to the outline of `symbol`; `0` on the outline, growing
/// both outside and inside.
fn distance_to_outline(symbol: &ChildSheet, p: Point) -> f64 {
    let (x0, y0) = (symbol.position.x, symbol.position.y);
    let (x1, y1) = (x0 + symbol.size.0, y0 + symbol.size.1);
    let inside = p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1;
    if inside {
        return (p.x - x0).min(x1 - p.x).min(p.y - y0).min(y1 - p.y);
    }
    let dx = (x0 - p.x).max(p.x - x1).max(0.0);
    let dy = (y0 - p.y).max(p.y - y1).max(0.0);
    dx.hypot(dy)
}

/// The sheet symbol of `sheet` whose outline is nearest to `p`, when `p` is
/// on it, within [`SHEET_SYMBOL_REACH_MM`] outside it, or inside it.
pub fn sheet_symbol_at(sheet: &SchematicSheet, p: Point) -> Option<&ChildSheet> {
    sheet
        .child_sheets
        .iter()
        .filter(|symbol| {
            let (x0, y0) = (symbol.position.x, symbol.position.y);
            let inside =
                p.x >= x0 && p.x <= x0 + symbol.size.0 && p.y >= y0 && p.y <= y0 + symbol.size.1;
            inside || distance_to_outline(symbol, p) <= SHEET_SYMBOL_REACH_MM
        })
        .min_by(|a, b| {
            distance_to_outline(a, p)
                .partial_cmp(&distance_to_outline(b, p))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// A new, hand-placed entry on `symbol`: `cursor` snapped to the nearest
/// edge (kept off the corners), with the rotation that edge needs. `None`
/// takes the direction the edge suggests: input on the left, output on the
/// right, bidirectional on top and bottom.
pub fn new_sheet_entry(
    symbol: &ChildSheet,
    cursor: Point,
    name: &str,
    direction: Option<&str>,
) -> SheetPin {
    let mut pin = SheetPin {
        uuid: Uuid::now_v7(),
        name: name.trim().to_string(),
        direction: String::new(),
        position: cursor,
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    };
    lock_sheet_pin_to_child_edge(
        &mut pin,
        0.0,
        0.0,
        symbol.position.x,
        symbol.position.y,
        symbol.size.0,
        symbol.size.1,
    );
    pin.direction = match direction {
        Some(direction) => normalize_sheet_pin_direction(direction),
        None if pin.rotation == 0.0 => "input",
        None if pin.rotation == 180.0 => "output",
        None => "bidirectional",
    }
    .to_string();
    pin
}

/// `P1`, `P2`, ... : the first name no entry of `symbol` uses yet.
pub fn unused_entry_name(symbol: &ChildSheet) -> String {
    (1..)
        .map(|n| format!("P{n}"))
        .find(|name| {
            !symbol
                .pins
                .iter()
                .any(|pin| normalize_port_name(&pin.name) == Some(name.as_str()))
        })
        .expect("an unbounded range has an unused name")
}

/// `Command::PlaceSheetPin`'s rule: a named entry, once per name on a symbol.
pub(crate) fn validated_new_pin(symbol: &ChildSheet, pin: &SheetPin) -> Result<(), EngineError> {
    let Some(name) = normalize_port_name(&pin.name) else {
        return Err(EngineError::EmptySheetEntryName);
    };
    let taken = symbol
        .pins
        .iter()
        .any(|p| normalize_port_name(&p.name) == Some(name));
    if taken {
        return Err(EngineError::DuplicateSheetEntryName(name.to_string()));
    }
    Ok(())
}

/// The child document for `symbol`: `blank` plus one Hierarchical label per
/// entry on the symbol, same name and direction, stacked down the left side.
/// The parent's pins and these labels then agree without a reconcile.
pub fn child_document_for_symbol(mut blank: SchematicSheet, symbol: &ChildSheet) -> SchematicSheet {
    let row = 4.0 * GRID_MM;
    let mut seen: Vec<&str> = Vec::new();
    for pin in &symbol.pins {
        let Some(name) = normalize_port_name(&pin.name) else {
            continue;
        };
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        blank.labels.push(Label {
            unknown_columns: Default::default(),
            uuid: Uuid::now_v7(),
            text: name.to_string(),
            position: Point::new(
                20.0 * GRID_MM,
                20.0 * GRID_MM + row * (seen.len() - 1) as f64,
            ),
            rotation: 0.0,
            label_type: LabelType::Hierarchical,
            shape: normalize_sheet_pin_direction(&pin.direction).to_string(),
            font_size: 1.27,
            font_family: None,
            justify: HAlign::default(),
            justify_v: VAlign::default(),
            locked: false,
        });
    }
    blank
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Engine};

    fn symbol() -> ChildSheet {
        crate::child_sheet_from_corners(Point::new(100.0, 50.0), Point::new(130.0, 80.0), "sheet1")
            .expect("big enough")
    }

    fn sheet_with(symbol: ChildSheet) -> SchematicSheet {
        SchematicSheet {
            child_sheets: vec![symbol],
            ..SchematicSheet::default()
        }
    }

    #[test]
    fn an_entry_snaps_to_the_nearest_edge_with_that_edges_rotation() {
        let s = symbol();
        let left = new_sheet_entry(&s, Point::new(98.0, 60.0), "A", Some("input"));
        assert_eq!((left.position.x, left.rotation), (100.0, 0.0));
        let right = new_sheet_entry(&s, Point::new(133.0, 60.0), "B", Some("output"));
        assert_eq!((right.position.x, right.rotation), (130.0, 180.0));
        let top = new_sheet_entry(&s, Point::new(115.0, 48.0), "C", Some("passive"));
        assert_eq!((top.position.y, top.rotation), (50.0, 270.0));
        let bottom = new_sheet_entry(&s, Point::new(115.0, 83.0), "D", Some("bogus"));
        assert_eq!((bottom.position.y, bottom.rotation), (80.0, 90.0));
        assert_eq!(bottom.direction, "bidirectional", "unknown direction");
        assert!(!left.auto_generated);
    }

    #[test]
    fn the_default_direction_follows_the_edge_and_names_are_unused() {
        let mut s = symbol();
        assert_eq!(
            new_sheet_entry(&s, Point::new(98.0, 60.0), "A", None).direction,
            "input"
        );
        assert_eq!(
            new_sheet_entry(&s, Point::new(133.0, 60.0), "A", None).direction,
            "output"
        );
        assert_eq!(
            new_sheet_entry(&s, Point::new(115.0, 48.0), "A", None).direction,
            "bidirectional"
        );
        assert_eq!(unused_entry_name(&s), "P1");
        s.pins.push(new_sheet_entry(
            &s.clone(),
            Point::new(98.0, 60.0),
            "P1",
            None,
        ));
        assert_eq!(unused_entry_name(&s), "P2");
    }

    #[test]
    fn an_entry_stays_off_the_corners() {
        let s = symbol();
        let pin = new_sheet_entry(&s, Point::new(100.0, 50.0), "A", Some("input"));
        assert!(pin.position.y >= 50.0 + GRID_MM);
    }

    #[test]
    fn the_symbol_under_the_cursor_is_the_nearest_one_in_reach() {
        let near = symbol();
        let far = crate::child_sheet_from_corners(
            Point::new(200.0, 50.0),
            Point::new(230.0, 80.0),
            "sheet2",
        )
        .expect("big enough");
        let mut sheet = sheet_with(near.clone());
        sheet.child_sheets.push(far.clone());
        assert_eq!(
            sheet_symbol_at(&sheet, Point::new(98.0, 60.0)).map(|s| s.uuid),
            Some(near.uuid)
        );
        assert_eq!(
            sheet_symbol_at(&sheet, Point::new(215.0, 65.0)).map(|s| s.uuid),
            Some(far.uuid)
        );
        assert!(sheet_symbol_at(&sheet, Point::new(160.0, 65.0)).is_none());
    }

    #[test]
    fn placing_an_entry_is_one_undo_step_and_refuses_a_repeated_name() {
        let s = symbol();
        let id = s.uuid;
        let mut engine = Engine::new(sheet_with(s.clone())).expect("engine");
        let pin = new_sheet_entry(&s, Point::new(98.0, 60.0), "CLK", Some("input"));
        engine
            .execute(Command::PlaceSheetPin { sheet_id: id, pin })
            .expect("placed");
        assert_eq!(engine.document().child_sheets[0].pins.len(), 1);
        let again = new_sheet_entry(&s, Point::new(98.0, 66.0), " CLK ", Some("input"));
        let err = engine
            .execute(Command::PlaceSheetPin {
                sheet_id: id,
                pin: again,
            })
            .unwrap_err();
        assert!(matches!(err, EngineError::DuplicateSheetEntryName(_)));
        let blank = new_sheet_entry(&s, Point::new(98.0, 66.0), "  ", Some("input"));
        assert!(matches!(
            engine.execute(Command::PlaceSheetPin {
                sheet_id: id,
                pin: blank
            }),
            Err(EngineError::EmptySheetEntryName)
        ));
        engine.undo().expect("undo").expect("one step");
        assert!(engine.document().child_sheets[0].pins.is_empty());
    }

    #[test]
    fn the_child_document_has_one_hierarchical_label_per_entry() {
        let mut s = symbol();
        s.pins.push(new_sheet_entry(
            &s.clone(),
            Point::new(98.0, 60.0),
            "CLK",
            Some("input"),
        ));
        s.pins.push(new_sheet_entry(
            &s.clone(),
            Point::new(133.0, 60.0),
            "OUT",
            Some("output"),
        ));
        let child = child_document_for_symbol(SchematicSheet::default(), &s);
        let ports = crate::exposed_sheet_ports(&child);
        let got: Vec<(&str, &str)> = ports
            .iter()
            .map(|p| (p.name.as_str(), p.direction.as_str()))
            .collect();
        assert_eq!(got, [("CLK", "input"), ("OUT", "output")]);
        assert!(
            child
                .labels
                .iter()
                .all(|l| l.label_type == LabelType::Hierarchical)
        );
    }
}
