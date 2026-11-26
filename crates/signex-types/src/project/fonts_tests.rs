//! #1178: the project's font list. Stored lazily under `fonts`, written only
//! when there is a font, kept byte for byte when it cannot be read, and the
//! stored paths that are read are checked, since a project arrives from
//! someone else.

use std::path::Path;

use serde_json::json;

use super::*;

const SHA: &str = "0f3a9c";

fn face(family: &str, style: &str) -> FontFace {
    FontFace {
        family: family.into(),
        style: style.into(),
    }
}

fn path(sha: &str) -> String {
    project_font_asset_path(sha, "otf")
}

fn project() -> ProjectData {
    let text = r#"{"name": "p", "dir": "", "schematic_root": null, "pcb_file": null}"#;
    parse_project_bytes(Path::new("/p/p.snxprj"), text.as_bytes()).expect("a project")
}

fn reread(data: &ProjectData) -> ProjectData {
    let bytes = encode_project(data).expect("encodes");
    parse_project_bytes(Path::new("/p/p.snxprj"), &bytes).expect("reads back")
}

#[test]
fn a_project_without_fonts_writes_no_key() {
    let data = project();
    let text = String::from_utf8(encode_project(&data).unwrap()).unwrap();
    assert!(!text.contains("\"fonts\""), "{text}");
    assert!(data.project_fonts().unwrap().is_empty());
}

#[test]
fn an_added_file_is_listed_under_its_family_and_round_trips() {
    let mut data = project();
    let touched = data
        .add_project_font_file(
            &path(SHA),
            &[face("Carrier Sans", "Bold")],
            FontEmbedding::Installable,
        )
        .unwrap();
    assert_eq!(touched, ["Carrier Sans"]);
    let back = reread(&data);
    assert_eq!(
        back.project_fonts_raw(),
        Some(&json!([{
            "family": "Carrier Sans",
            "files": [{"path": "assets/fonts/0f3a9c.otf", "style": "Bold",
                       "embedding": "installable"}]
        }]))
    );
    assert_eq!(back.project_fonts().unwrap(), data.project_fonts().unwrap());
}

#[test]
fn styles_of_one_family_group_and_a_second_add_of_a_file_lists_it_once() {
    let mut data = project();
    let regular = path("aa");
    let bold = path("bb");
    for (file, style) in [
        (&regular, "Regular"),
        (&bold, "Bold"),
        (&regular, "Regular"),
    ] {
        data.add_project_font_file(
            file,
            &[face("carrier sans", style)],
            FontEmbedding::Editable,
        )
        .unwrap();
    }
    data.add_project_font_file(
        &bold,
        &[face("Carrier Sans", "Bold")],
        FontEmbedding::Editable,
    )
    .unwrap();
    let fonts = data.project_fonts().unwrap();
    assert_eq!(fonts.len(), 1, "one family, compared without case");
    assert_eq!(
        fonts[0].family, "carrier sans",
        "the first spelling is kept"
    );
    let files: Vec<&str> = fonts[0].files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, [regular.as_str(), bold.as_str()]);
}

#[test]
fn a_collection_is_listed_under_each_of_its_families() {
    let mut data = project();
    let touched = data
        .add_project_font_file(
            &project_font_asset_path("cc", "ttc"),
            &[face("Serif A", "Regular"), face("Math A", "Regular")],
            FontEmbedding::Installable,
        )
        .unwrap();
    assert_eq!(touched, ["Serif A", "Math A"]);
    assert_eq!(data.project_fonts().unwrap().len(), 2);
}

#[test]
fn a_restricted_font_is_refused_and_nothing_changes() {
    let mut data = project();
    let refused = data.add_project_font_file(
        &path(SHA),
        &[face("House", "Regular")],
        FontEmbedding::Restricted,
    );
    assert_eq!(
        refused,
        Err(ProjectFontProblem::Restricted {
            family: "House".into()
        })
    );
    assert_eq!(data.project_fonts_raw(), None);
}

#[test]
fn a_path_outside_the_font_folder_or_a_nameless_font_is_refused() {
    let mut data = project();
    for bad in [
        "../secret.otf",
        "assets/fonts/../../secret.otf",
        "/etc/fonts/a.otf",
        "assets/fonts/a.exe",
        "assets/fonts/sub/a.otf",
        "assets/fonts/.otf",
        "assets\\fonts\\a.otf",
    ] {
        assert!(!is_project_font_asset_path(bad), "{bad}");
        assert!(matches!(
            data.add_project_font_file(bad, &[face("A", "")], FontEmbedding::Installable),
            Err(ProjectFontProblem::NotAnAssetPath { .. })
        ));
    }
    assert_eq!(
        data.add_project_font_file(&path(SHA), &[face("  ", "")], FontEmbedding::Installable),
        Err(ProjectFontProblem::NoFamily)
    );
    assert_eq!(data.project_fonts_raw(), None);
}

#[test]
fn the_asset_path_and_the_extension_are_lower_case() {
    assert_eq!(
        project_font_asset_path("ab12", "OTF"),
        "assets/fonts/ab12.otf"
    );
    assert_eq!(project_font_extension("House-Bold.TTF"), Some("ttf"));
    assert_eq!(project_font_extension("House.ttc"), Some("ttc"));
    assert_eq!(project_font_extension("House.woff2"), None);
    assert_eq!(project_font_extension("House"), None);
}

#[test]
fn a_license_is_set_cleared_and_capped() {
    let mut data = project();
    data.add_project_font_file(&path(SHA), &[face("House", "")], FontEmbedding::Installable)
        .unwrap();
    data.set_project_font_license("house", " OFL-1.1 ").unwrap();
    assert_eq!(
        data.project_font_family("HOUSE")
            .unwrap()
            .license
            .as_deref(),
        Some("OFL-1.1")
    );
    let long = "x".repeat(MAX_FONT_LICENSE_CHARS + 1);
    assert_eq!(
        data.set_project_font_license("House", &long),
        Err(ProjectFontProblem::LicenseTooLong)
    );
    data.set_project_font_license("House", "  ").unwrap();
    assert_eq!(data.project_font_family("House").unwrap().license, None);
    assert_eq!(
        data.set_project_font_license("Other", "MIT"),
        Err(ProjectFontProblem::NoSuchFamily {
            family: "Other".into()
        })
    );
}

#[test]
fn removing_the_last_family_drops_the_key() {
    let mut data = project();
    data.add_project_font_file(&path(SHA), &[face("House", "")], FontEmbedding::Installable)
        .unwrap();
    data.remove_project_font("House").unwrap();
    assert_eq!(data.project_fonts_raw(), None);
    assert!(matches!(
        data.remove_project_font("House"),
        Err(ProjectFontProblem::NoSuchFamily { .. })
    ));
}

#[test]
fn keys_of_a_newer_build_survive_an_edit() {
    let text = r#"{"name": "p", "dir": "", "schematic_root": null, "pcb_file": null,
        "fonts": [{"family": "House", "future": 1,
                   "files": [{"path": "assets/fonts/aa.otf", "embedding": "editable",
                              "hinting": "full"}]}]}"#;
    let mut data = parse_project_bytes(Path::new("/p/p.snxprj"), text.as_bytes()).unwrap();
    data.set_project_font_license("House", "OFL-1.1").unwrap();
    let raw = reread(&data).project_fonts_raw().cloned().unwrap();
    assert_eq!(raw[0]["future"], json!(1));
    assert_eq!(raw[0]["files"][0]["hinting"], json!("full"));
    assert_eq!(raw[0]["license"], json!("OFL-1.1"));
}

#[test]
fn an_unreadable_list_is_kept_as_written_and_refuses_edits() {
    let text = r#"{"name": "p", "dir": "", "schematic_root": null, "pcb_file": null,
        "fonts": {"House": "assets/fonts/aa.otf"}}"#;
    let mut data = parse_project_bytes(Path::new("/p/p.snxprj"), text.as_bytes()).unwrap();
    assert_eq!(data.project_fonts(), Err(ProjectFontProblem::Unreadable));
    assert_eq!(
        data.add_project_font_file(&path(SHA), &[face("A", "")], FontEmbedding::Installable),
        Err(ProjectFontProblem::Unreadable)
    );
    assert_eq!(
        reread(&data).project_fonts_raw(),
        Some(&json!({"House": "assets/fonts/aa.otf"}))
    );
}

#[test]
fn the_strictest_permission_of_a_collection_wins() {
    use FontEmbedding::*;
    assert_eq!(Installable.strictest(PreviewAndPrint), PreviewAndPrint);
    assert_eq!(Restricted.strictest(Editable), Restricted);
    assert_eq!(Editable.strictest(Installable), Editable);
    assert!(PreviewAndPrint.allows_project_copy());
    assert!(!Restricted.allows_project_copy());
}
