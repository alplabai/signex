//! End to end (#2968): `generate_block_diagram` and
//! `regenerate_block_diagram` through the real `signex-cli` binary. Generate
//! writes one linked block per sheet; regenerate after a sheet is added lists
//! exactly one new block, writes nothing until `apply=true`, and keeps the
//! existing positions.

mod support;

use std::path::Path;

use serde_json::{Value, json};
use signex_types::block_diagram::{LinkState, read_block_diagram};
use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet};
use support::{fixture, jsonl, run_cli, temp_root};

fn symbol(name: &str, file: &str, x: f64) -> ChildSheet {
    ChildSheet {
        uuid: uuid::Uuid::now_v7(),
        name: name.into(),
        filename: file.into(),
        position: Point::new(x, 20.0),
        size: (20.0, 15.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn save(sheet: SchematicSheet, path: &Path) {
    signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf()))
        .expect("engine")
        .save()
        .expect("write sheet");
}

/// The fixture's root sheet replaced by a top sheet that only places
/// `children`, each an empty sheet of its own.
fn write_hierarchy(dir: &Path, root: &Path, children: &[(&str, &str)]) {
    let mut top = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        ..Default::default()
    };
    for (i, (name, file)) in children.iter().enumerate() {
        top.child_sheets.push(symbol(name, file, 40.0 * i as f64));
        let child = dir.join(file);
        if !child.exists() {
            let sheet = SchematicSheet {
                uuid: uuid::Uuid::now_v7(),
                ..Default::default()
            };
            save(sheet, &child);
        }
    }
    if root.exists() {
        std::fs::remove_file(root).expect("the old root sheet");
    }
    save(top, root);
}

fn names(answer: &Value) -> Vec<String> {
    answer["diagram"]["blocks"]
        .as_array()
        .expect("blocks")
        .iter()
        .map(|b| b["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn generate_writes_one_linked_block_per_sheet_and_refuses_to_overwrite() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    write_hierarchy(
        &fx.dir,
        &fx.sheet,
        &[("Alpha", "a.snxsch"), ("Beta", "b.snxsch")],
    );
    let out = run_cli(
        &[
            "run",
            fx.project.to_str().unwrap(),
            "--save",
            "--keep-going",
        ],
        jsonl(&[
            json!({"id": "generate_block_diagram", "args": {"document": "demo.snxprj"}}),
            json!({"id": "generate_block_diagram", "args": {"document": "demo.snxprj"}}),
        ]),
    );
    let first = &out.lines[0];
    assert_eq!(first["status"], "answered", "{}", out.stdout);
    assert_eq!(first["data"]["summary"]["blocks"], 2, "{}", out.stdout);
    assert_eq!(names(&first["data"]), ["Alpha", "Beta"], "{}", out.stdout);

    let file = fx.dir.join("demo.snxblk");
    let read = read_block_diagram(&file).expect("the diagram reads back");
    let paths: Vec<&str> = read
        .diagram
        .blocks
        .iter()
        .map(|b| b.sheet.as_ref().expect("linked").path.as_str())
        .collect();
    assert_eq!(paths, ["a.snxsch", "b.snxsch"]);
    assert!(read.diagram.blocks.iter().all(|b| {
        b.sheet
            .as_ref()
            .is_some_and(|l| l.state == LinkState::Linked)
    }));

    let second = &out.lines[1];
    assert_eq!(second["status"], "refused", "{}", out.stdout);
    assert!(
        second["reason"]
            .as_str()
            .is_some_and(|why| why.contains("overwrite=true")),
        "{}",
        out.stdout
    );
}

#[test]
fn regenerate_lists_one_new_block_and_writes_only_with_apply() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    write_hierarchy(
        &fx.dir,
        &fx.sheet,
        &[("Alpha", "a.snxsch"), ("Beta", "b.snxsch")],
    );
    let project = fx.project.to_str().unwrap();
    let generate = json!({"id": "generate_block_diagram", "args": {"document": "demo.snxprj"}});
    let out = run_cli(&["run", project, "--save"], jsonl(&[generate]));
    assert_eq!(out.lines[0]["status"], "answered", "{}", out.stdout);
    let file = fx.dir.join("demo.snxblk");
    let before = read_block_diagram(&file).expect("generated").diagram;

    write_hierarchy(
        &fx.dir,
        &fx.sheet,
        &[
            ("Alpha", "a.snxsch"),
            ("Beta", "b.snxsch"),
            ("Gamma", "c.snxsch"),
        ],
    );
    let bytes = std::fs::read(&file).expect("diagram bytes");
    let regenerate = |apply: bool| {
        json!({"id": "regenerate_block_diagram",
               "args": {"document": "demo.snxprj", "apply": apply}})
    };
    let out = run_cli(
        &["run", project, "--save"],
        jsonl(&[regenerate(false), regenerate(true)]),
    );
    let listed = &out.lines[0];
    assert_eq!(listed["status"], "answered", "{}", out.stdout);
    let changes = listed["data"]["changes"].as_array().expect("changes");
    assert_eq!(changes.len(), 1, "{}", out.stdout);
    assert_eq!(changes[0]["change"], "block_added", "{}", out.stdout);
    assert_eq!(changes[0]["name"], "Gamma", "{}", out.stdout);
    assert_eq!(listed["data"]["written"], false, "{}", out.stdout);

    let applied = &out.lines[1];
    assert_eq!(applied["status"], "answered", "{}", out.stdout);
    assert_eq!(applied["data"]["written"], true, "{}", out.stdout);
    assert_ne!(std::fs::read(&file).expect("diagram bytes"), bytes);
    let after = read_block_diagram(&file).expect("regenerated").diagram;
    assert_eq!(after.blocks.len(), 3);
    for block in &before.blocks {
        let kept = after.block(block.id).expect("the block stays");
        assert_eq!(kept.position, block.position);
    }
}

#[test]
fn the_commands_need_the_save_capability() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    write_hierarchy(&fx.dir, &fx.sheet, &[("Alpha", "a.snxsch")]);
    let out = run_cli(
        &["run", fx.project.to_str().unwrap()],
        jsonl(&[json!({"id": "generate_block_diagram", "args": {"document": "demo.snxprj"}})]),
    );
    assert_eq!(out.lines[0]["status"], "refused", "{}", out.stdout);
    assert_eq!(
        out.lines[0]["reason"], "`generate_block_diagram` needs the `save` capability; pass --save",
        "{}",
        out.stdout
    );
    assert!(!fx.dir.join("demo.snxblk").exists());
}
