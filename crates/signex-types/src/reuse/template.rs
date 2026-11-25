//! What a new sheet or project takes from its template (#1283). A sheet takes
//! the template's frame: paper size and font, border and title block, page
//! settings, title block values, sheet parameters, and its drawings and text
//! with new ids. Parts, wires and labels stay behind: a circuit fragment is a
//! snippet. A project takes the settings its template sets, and its first
//! sheet takes the frame of the template's first sheet.

use std::collections::{BTreeMap, HashMap};

use uuid::Uuid;

use crate::object_id::IdSource;
use crate::project::ProjectData;
use crate::schematic::SchematicSheet;

/// Put the frame of `template` on `sheet`, the new sheet, replacing its own
/// frame. The drawings and text notes get ids from `ids`.
pub fn apply_sheet_template(
    template: &SchematicSheet,
    sheet: &mut SchematicSheet,
    ids: &mut dyn IdSource,
) {
    sheet.paper_size = template.paper_size.clone();
    sheet.font_family = template.font_family.clone();
    sheet.frame = template.frame;
    sheet.page = template.page;
    sheet.title_block = template.title_block.clone();
    sheet.variables = template.variables.clone();
    let mut renamed: HashMap<Uuid, Uuid> = HashMap::new();
    sheet.drawings = template
        .drawings
        .iter()
        .map(|drawing| {
            let mut drawing = drawing.clone();
            let id = ids.next_id();
            renamed.insert(drawing.uuid(), id);
            drawing.set_uuid(id);
            drawing
        })
        .collect();
    sheet.drawing_placements = template
        .drawing_placements
        .iter()
        .filter_map(|(old, placement)| renamed.get(old).map(|new| (*new, *placement)))
        .collect::<BTreeMap<_, _>>();
    sheet.text_notes = template
        .text_notes
        .iter()
        .map(|note| {
            let mut note = note.clone();
            note.uuid = ids.next_id();
            note
        })
        .collect();
}

/// Put the settings `template` sets on `data`, a new project: its parameters
/// (over any of the same name), net classes, designator standard, schematic
/// defaults, channel naming, harness and differential pair classes, and the
/// sheet, board and profile defaults it sets. Everything it leaves unset
/// stays as the new project has it; its sheets, rules, variants and
/// libraries are not taken.
pub fn apply_project_template(template: &ProjectData, data: &mut ProjectData) {
    data.parameters.extend(
        template
            .parameters
            .iter()
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    if !template.net_classes.is_empty() {
        data.net_classes = template.net_classes.clone();
    }
    if template.annotation.is_some() {
        data.annotation = template.annotation.clone();
    }
    if !template.schematic_defaults.is_empty() {
        data.schematic_defaults = template.schematic_defaults.clone();
    }
    if !template.schematic_design.is_default() {
        data.schematic_design = template.schematic_design.clone();
    }
    let defaults = &template.project_defaults;
    let into = &mut data.project_defaults;
    if defaults.pcb.is_some() {
        into.pcb = defaults.pcb.clone();
    }
    if defaults.sheet.is_some() {
        into.sheet = defaults.sheet.clone();
    }
    if defaults.profile.is_some() {
        into.profile = defaults.profile.clone();
    }
    if defaults.dimension.is_some() {
        into.dimension = defaults.dimension.clone();
    }
}
