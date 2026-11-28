//! End to end (#1178): the Fonts commands through the real `signex-cli`
//! binary. `project_add_font` copies a font into the project's
//! `assets/fonts/` and lists it, a font whose license forbids embedding is
//! refused with nothing copied, and a saved project reads back its fonts.

mod support;

use std::path::{Path, PathBuf};

use serde_json::json;
use support::{jsonl, pins_fixture, run_cli, temp_root};

const ROBOTO: &[u8] = include_bytes!("../../signex-app/assets/fonts/Roboto-Regular.ttf");

/// `font` with its `OS/2.fsType` set to `fs_type`.
fn with_fs_type(font: &[u8], fs_type: u16) -> Vec<u8> {
    let mut bytes = font.to_vec();
    let tables = usize::from(u16::from_be_bytes([bytes[4], bytes[5]]));
    let os2 = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|&record| &bytes[record..record + 4] == b"OS/2")
        .map(|record| {
            let at = &bytes[record + 8..record + 12];
            u32::from_be_bytes([at[0], at[1], at[2], at[3]]) as usize
        })
        .expect("the font has an OS/2 table");
    bytes[os2 + 8..os2 + 10].copy_from_slice(&fs_type.to_be_bytes());
    bytes
}

/// `bytes` written as `name` in the project's `incoming` folder, a folder the
/// run may read.
fn incoming(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let folder = dir.join("incoming");
    std::fs::create_dir_all(&folder).expect("incoming folder");
    let path = folder.join(name);
    std::fs::write(&path, bytes).expect("font file");
    path
}

fn font_files(dir: &Path) -> Vec<PathBuf> {
    match std::fs::read_dir(dir.join("assets").join("fonts")) {
        Ok(entries) => entries.map(|entry| entry.expect("entry").path()).collect(),
        Err(_) => Vec::new(),
    }
}

#[test]
fn a_font_is_copied_listed_and_saved_and_a_restricted_one_is_refused() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let free = incoming(&fx.dir, "House-Regular.ttf", ROBOTO);
    let locked = incoming(&fx.dir, "Locked.ttf", &with_fs_type(ROBOTO, 0x0002));
    let out = run_cli(
        &[
            "run",
            fx.project.to_str().unwrap(),
            "--save",
            "--keep-going",
        ],
        jsonl(&[
            json!({"id": "project_add_font",
                   "args": {"document": "demo.snxprj", "path": locked.to_str().unwrap()}}),
            json!({"id": "project_add_font",
                   "args": {"document": "demo.snxprj", "path": free.to_str().unwrap()}}),
            json!({"id": "project_set_font_license",
                   "args": {"document": "demo.snxprj", "family": "roboto", "value": "OFL-1.1"}}),
            json!({"id": "save_document", "args": {"document": "demo.snxprj"}}),
        ]),
    );
    let refused = &out.lines[0];
    assert_eq!(refused["status"], "refused", "{}", out.stdout);
    assert!(
        refused["reason"]
            .as_str()
            .is_some_and(|why| why.contains("does not allow embedding")),
        "{}",
        out.stdout
    );

    let added = &out.lines[1];
    assert_eq!(added["status"], "answered", "{}", out.stdout);
    assert_eq!(added["data"]["added"]["embedding"], "installable");
    assert_eq!(
        added["data"]["added"]["faces"],
        json!([{"family": "Roboto", "style": "Regular"}])
    );
    let path = added["data"]["added"]["path"].as_str().expect("a path");
    assert!(
        path.starts_with("assets/fonts/") && path.ends_with(".ttf"),
        "{path}"
    );
    assert_eq!(
        font_files(&fx.dir),
        [fx.dir.join(path)],
        "only the free font was copied"
    );
    assert_eq!(std::fs::read(fx.dir.join(path)).unwrap(), ROBOTO);

    assert_eq!(out.lines[2]["status"], "answered", "{}", out.stdout);
    assert!(out.lines[3]["status"] != "refused", "{}", out.stdout);
    let saved = signex_types::project::parse_project(&fx.project).expect("project");
    let family = saved.project_font_family("Roboto").expect("listed");
    assert_eq!(family.license.as_deref(), Some("OFL-1.1"));
    assert_eq!(family.files.len(), 1);
    assert_eq!(family.files[0].path, path);
}
