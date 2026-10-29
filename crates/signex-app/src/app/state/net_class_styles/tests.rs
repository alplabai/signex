use std::path::PathBuf;

use signex_types::net_class::NetLineStyle;
use signex_types::schematic::{
    HAlign, Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point,
    SchematicSheet, Symbol, VAlign, Wire,
};

use super::*;

/// A one-pin part. The netlist only forms a net where a pin lands, so a
/// labelled wire alone would resolve to no net at all.
fn one_pin_part() -> LibSymbol {
    LibSymbol {
        id: "R".to_string(),
        reference: String::new(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn sheet_with_net(wire_id: Uuid, name: &str) -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert("R".to_string(), one_pin_part());
    sheet.symbols.push(Symbol {
        uuid: Uuid::from_u128(98),
        lib_id: "R".to_string(),
        reference: "R1".to_string(),
        position: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet.wires.push(Wire {
        uuid: wire_id,
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet.labels.push(Label {
        uuid: Uuid::from_u128(99),
        text: name.to_string(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
        locked: false,
    });
    sheet
}

fn dashed_class(name: &str, net: &str) -> NetClass {
    let mut class = NetClass::named(name);
    class.nets.push(net.to_string());
    class.style.line = NetLineStyle::Dashed;
    class
}

const PROJECT: ProjectId = ProjectId(1);
const OTHER: ProjectId = ProjectId(2);

fn key(connectivity: u64, classes: &[NetClass]) -> ProjectKey {
    ProjectKey {
        connectivity,
        pages: vec![PathBuf::from("A.snxsch")],
        root: PathBuf::from("A.snxsch"),
        fold: CaseFold::default(),
        classes: classes.to_vec(),
    }
}

/// What the worker returns for `project` at `key`: the project netlist (here
/// the sheet's own) joined with the key's class table.
fn resolved(
    project: ProjectId,
    key: ProjectKey,
    netlist: &signex_types::net::Netlist,
) -> ResolvedProject {
    let (styles, nets) = if key.classes.is_empty() {
        Default::default()
    } else {
        (
            signex_net::element_class_styles(netlist, &key.classes),
            signex_net::element_net_names(netlist),
        )
    };
    ResolvedProject {
        project,
        key,
        styles,
        nets,
        warnings: Vec::new(),
    }
}

/// Ask for `key` and, if the cache wants it computed, compute and apply it.
fn resolve_and_apply(
    cache: &mut NetClassStyleCache,
    project: ProjectId,
    key: ProjectKey,
    sheet: &SchematicSheet,
) -> Applied {
    if !cache.want(project, &key) {
        return Applied::default();
    }
    let netlist = signex_net::build_netlist(sheet);
    cache.apply(&ResolvedStyles {
        projects: vec![resolved(project, key, &netlist)],
    })
}

#[test]
fn a_member_net_wire_gets_the_class_style() {
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VCC");
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();

    let applied = resolve_and_apply(&mut cache, PROJECT, key(1, &classes), &sheet);

    assert!(applied.styles_changed);
    assert_eq!(cache.styles()[&wire_id].line, NetLineStyle::Dashed);
    assert_eq!(cache.net_of(PROJECT, wire_id), Some("VCC"));
}

#[test]
fn the_revision_moves_with_the_style_map_and_only_then() {
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VCC");
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    let start = cache.revision();

    resolve_and_apply(&mut cache, PROJECT, key(1, &classes), &sheet);
    let styled = cache.revision();
    assert_ne!(styled, start, "the map gained a style");

    // Same styles under a new connectivity stamp: the map did not change.
    resolve_and_apply(&mut cache, PROJECT, key(2, &classes), &sheet);
    assert_eq!(cache.revision(), styled, "an equal map keeps the revision");

    cache.retain_projects(&[]);
    assert_ne!(
        cache.revision(),
        styled,
        "dropping the project empties the map"
    );
}

#[test]
fn no_classes_leaves_the_cache_empty_and_reports_no_change() {
    let sheet = sheet_with_net(Uuid::from_u128(1), "VCC");
    let mut cache = NetClassStyleCache::default();

    let applied = resolve_and_apply(&mut cache, PROJECT, key(1, &[]), &sheet);

    assert!(!applied.styles_changed);
    assert!(cache.is_empty());
}

#[test]
fn a_new_connectivity_revision_with_a_renamed_net_drops_the_style() {
    let wire_id = Uuid::from_u128(1);
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    resolve_and_apply(
        &mut cache,
        PROJECT,
        key(1, &classes),
        &sheet_with_net(wire_id, "VCC"),
    );

    let applied = resolve_and_apply(
        &mut cache,
        PROJECT,
        key(2, &classes),
        &sheet_with_net(wire_id, "SDA"),
    );

    assert!(applied.styles_changed);
    assert!(cache.styles().is_empty());
    assert_eq!(cache.net_of(PROJECT, wire_id), Some("SDA"));
}

#[test]
fn an_unchanged_key_is_not_resolved_again() {
    let wire_id = Uuid::from_u128(1);
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    resolve_and_apply(
        &mut cache,
        PROJECT,
        key(1, &classes),
        &sheet_with_net(wire_id, "VCC"),
    );

    assert!(
        !cache.want(PROJECT, &key(1, &classes)),
        "the key is the contract: the cached resolution stands"
    );
    assert!(cache.styles().contains_key(&wire_id));
}

#[test]
fn a_key_already_in_flight_is_not_requested_twice() {
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();

    assert!(cache.want(PROJECT, &key(1, &classes)));
    assert!(!cache.want(PROJECT, &key(1, &classes)));
}

#[test]
fn each_part_of_the_key_decides_when_to_resolve_again() {
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    let sheet = sheet_with_net(Uuid::from_u128(1), "VCC");
    resolve_and_apply(&mut cache, PROJECT, key(1, &classes), &sheet);

    let mut dotted = dashed_class("Power", "VCC");
    dotted.style.line = NetLineStyle::Dotted;
    assert!(cache.want(PROJECT, &key(1, &[dotted])), "class table edit");

    let mut pages = key(1, &classes);
    pages.pages.clear();
    assert!(cache.want(PROJECT, &pages), "a page was removed");

    let mut root = key(1, &classes);
    root.root = PathBuf::from("B.snxsch");
    assert!(cache.want(PROJECT, &root), "the root sheet changed");
}

#[test]
fn a_project_that_is_no_longer_an_input_leaves_the_cache() {
    let wire_id = Uuid::from_u128(1);
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    resolve_and_apply(
        &mut cache,
        PROJECT,
        key(1, &classes),
        &sheet_with_net(wire_id, "VCC"),
    );

    let changed = cache.retain_projects(&[]);

    assert!(changed);
    assert!(cache.styles().is_empty());
    assert_eq!(cache.net_of(PROJECT, wire_id), None);
}

#[test]
fn the_class_table_joins_the_project_netlist_not_one_sheet() {
    // The element is named `VIN` on its own sheet; the project netlist says
    // the net is `VCC`. Only the project netlist decides.
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VIN");
    let mut project_netlist = signex_net::build_netlist(&sheet);
    project_netlist.nets[0].name = "VCC".to_string();
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    let k = key(1, &classes);
    cache.want(PROJECT, &k);

    cache.apply(&ResolvedStyles {
        projects: vec![resolved(PROJECT, k, &project_netlist)],
    });

    assert_eq!(cache.styles()[&wire_id].line, NetLineStyle::Dashed);
    assert_eq!(cache.net_of(PROJECT, wire_id), Some("VCC"));
}

#[test]
fn a_late_answer_for_a_project_that_moved_on_is_dropped() {
    let wire_id = Uuid::from_u128(1);
    let classes = [dashed_class("Power", "VCC")];
    let sheet = sheet_with_net(wire_id, "VCC");
    let netlist = signex_net::build_netlist(&sheet);
    let mut cache = NetClassStyleCache::default();
    let (older, newer) = (key(1, &classes), key(2, &classes));
    cache.want(PROJECT, &older);
    cache.want(PROJECT, &newer);

    let applied = cache.apply(&ResolvedStyles {
        projects: vec![resolved(PROJECT, older, &netlist)],
    });

    assert_eq!(applied, Applied::default());
    assert!(cache.is_empty(), "the stale answer must not be stored");
}

#[test]
fn a_late_answer_survives_edits_to_another_project() {
    let wire_id = Uuid::from_u128(1);
    let classes = [dashed_class("Power", "VCC")];
    let sheet = sheet_with_net(wire_id, "VCC");
    let netlist = signex_net::build_netlist(&sheet);
    let mut cache = NetClassStyleCache::default();
    let slow = key(1, &classes);
    cache.want(PROJECT, &slow);
    // Another project is edited again and again while PROJECT is resolving.
    for revision in 1..=5 {
        cache.want(OTHER, &key(revision, &classes));
    }

    let applied = cache.apply(&ResolvedStyles {
        projects: vec![resolved(PROJECT, slow, &netlist)],
    });

    assert!(
        applied.styles_changed,
        "PROJECT did not change, so it lands"
    );
    assert!(cache.styles().contains_key(&wire_id));
}

#[test]
fn going_back_to_the_stored_key_cancels_the_answer_in_flight() {
    let wire_id = Uuid::from_u128(1);
    let classes = [dashed_class("Power", "VCC")];
    let netlist = signex_net::build_netlist(&sheet_with_net(wire_id, "VCC"));
    let mut cache = NetClassStyleCache::default();
    let stored = key(1, &classes);
    cache.want(PROJECT, &stored);
    cache.apply(&ResolvedStyles {
        projects: vec![resolved(PROJECT, stored.clone(), &netlist)],
    });
    let left = key(2, &classes);
    cache.want(PROJECT, &left);

    assert!(!cache.want(PROJECT, &stored), "already held");
    let applied = cache.apply(&ResolvedStyles {
        projects: vec![resolved(PROJECT, left, &netlist)],
    });

    assert_eq!(applied, Applied::default());
}

#[test]
fn a_failed_key_is_not_asked_again_until_the_key_moves() {
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    let k = key(1, &classes);
    assert!(cache.want(PROJECT, &k));

    cache.give_up(&[(PROJECT, k.clone())]);

    assert!(!cache.want(PROJECT, &k), "same failing key: not retried");
    let moved = key(2, &classes);
    assert!(cache.want(PROJECT, &moved), "moved key: asked");
    cache.give_up(&[(PROJECT, moved)]);
    assert!(cache.want(PROJECT, &k), "back to the old key after a move");
}

#[test]
fn a_failed_task_forgets_only_its_own_projects() {
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    let k = key(1, &classes);
    cache.want(PROJECT, &k);
    cache.want(OTHER, &k);

    cache.give_up(&[(PROJECT, k.clone())]);

    assert!(!cache.want(PROJECT, &k), "failed key not retried");
    assert!(!cache.want(OTHER, &k), "still in flight");
}

#[test]
fn a_failure_for_a_key_the_project_left_does_not_block_the_newer_key() {
    let wire_id = Uuid::from_u128(1);
    let classes = [dashed_class("Power", "VCC")];
    let netlist = signex_net::build_netlist(&sheet_with_net(wire_id, "VCC"));
    let mut cache = NetClassStyleCache::default();
    let (k1, k2) = (key(1, &classes), key(2, &classes));
    cache.want(PROJECT, &k1);
    cache.want(PROJECT, &k2);

    cache.give_up(&[(PROJECT, k1)]);
    let applied = cache.apply(&ResolvedStyles {
        projects: vec![resolved(PROJECT, k2, &netlist)],
    });

    assert!(applied.styles_changed, "the answer for K2 is still wanted");
    assert!(cache.styles().contains_key(&wire_id));
}

#[test]
fn a_warning_is_reported_once_until_it_changes() {
    let wire_id = Uuid::from_u128(1);
    let classes = [dashed_class("Power", "VCC")];
    let netlist = signex_net::build_netlist(&sheet_with_net(wire_id, "VCC"));
    let mut cache = NetClassStyleCache::default();
    let mut answer = |revision: u64| {
        let k = key(revision, &classes);
        cache.want(PROJECT, &k);
        let mut project = resolved(PROJECT, k, &netlist);
        project.warnings = vec!["Netlist: sheet 'c.snxsch' does not parse".to_string()];
        cache.apply(&ResolvedStyles {
            projects: vec![project],
        })
    };

    assert_eq!(answer(1).warnings.len(), 1);
    assert!(answer(2).warnings.is_empty(), "same warning, same project");
}

fn project_with(classes: Vec<NetClass>) -> LoadedProject {
    use signex_types::project::ProjectData;
    use std::collections::HashMap;

    LoadedProject {
        id: PROJECT,
        path: PathBuf::from("P.snxprj"),
        data: ProjectData {
            name: "P".to_string(),
            description: None,
            dir: String::new(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: classes,
            rules: None,
            annotation: None,
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    }
}

#[test]
fn the_gate_is_open_only_when_a_sheet_or_a_project_changed() {
    let mut project = project_with(vec![dashed_class("Power", "VCC")]);
    let mut cache = NetClassStyleCache::default();

    assert!(cache.needs_refresh(7, std::slice::from_ref(&project)));
    assert!(
        !cache.needs_refresh(7, std::slice::from_ref(&project)),
        "nothing changed, as on a pointer move"
    );
    assert!(
        cache.needs_refresh(8, std::slice::from_ref(&project)),
        "a sheet's connectivity changed"
    );
    project.data.net_classes[0].style.line = NetLineStyle::Dotted;
    assert!(
        cache.needs_refresh(8, std::slice::from_ref(&project)),
        "the class table was edited"
    );
    assert!(!cache.needs_refresh(8, std::slice::from_ref(&project)));
    project.data.schematic_root = Some("B.snxsch".to_string());
    assert!(
        cache.needs_refresh(8, std::slice::from_ref(&project)),
        "the root sheet was changed"
    );
    assert!(!cache.needs_refresh(8, std::slice::from_ref(&project)));
    project.case_fold = CaseFold::from_probe(!project.case_fold.fold_case);
    assert!(
        cache.needs_refresh(8, std::slice::from_ref(&project)),
        "the volume probe moved the case fold, so the stitch is stale"
    );
    assert!(cache.needs_refresh(8, &[]), "the project was closed");
}
