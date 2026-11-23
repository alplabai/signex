//! The Close Project confirm's texts (#2262): the intro counts every unsaved
//! row and names the project settings apart, each row says what it is, and
//! the kept documents say why they stay open.

use std::path::{Path, PathBuf};

use signex_i18n::{I18n, ids};

use super::{close_detail, kept_line, row_kind};

fn plain(text: String) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|n| PathBuf::from("/p").join(n)).collect()
}

#[test]
fn the_detail_counts_the_documents_and_names_the_project_settings_apart() {
    let en = I18n::en_us_ref();
    let detail = |names: &[&str], rows| plain(close_detail(en, "dcdc-buck", &paths(names), rows));
    assert_eq!(
        detail(
            &[
                "power.snxsch",
                "mcu.snxsch",
                "dcdc-buck.snxpcb",
                "dcdc-buck.snxprj"
            ],
            0
        ),
        "'dcdc-buck' has 3 unsaved documents and unsaved project settings."
    );
    assert_eq!(
        detail(&["power.snxsch", "dcdc-buck.snxprj"], 0),
        "'dcdc-buck' has 1 unsaved document and unsaved project settings."
    );
    assert_eq!(
        detail(&["dcdc-buck.snxprj"], 0),
        "'dcdc-buck' has unsaved project settings."
    );
    assert_eq!(
        detail(&["power.snxsch"], 0),
        "'dcdc-buck' has 1 unsaved document."
    );
    assert_eq!(
        detail(&["power.snxsch", "mcu.snxsch"], 0),
        "'dcdc-buck' has 2 unsaved documents."
    );
}

/// Library rows among them make the count "items", as before.
#[test]
fn library_rows_count_as_items() {
    let en = I18n::en_us_ref();
    let detail = |names: &[&str], rows| plain(close_detail(en, "P", &paths(names), rows));
    assert_eq!(detail(&["power.snxsch"], 1), "'P' has 2 unsaved items.");
    assert_eq!(detail(&[], 1), "'P' has 1 unsaved item.");
    assert_eq!(
        detail(&["P.snxprj"], 2),
        "'P' has 2 unsaved items and unsaved project settings."
    );
}

#[test]
fn each_row_says_what_it_is() {
    let en = I18n::en_us_ref();
    let kind = |name: &str| en.message_plain(row_kind(Path::new(name)), &[]);
    assert_eq!(kind("power.snxsch"), "Schematic");
    assert_eq!(kind("POWER.SNXSCH"), "Schematic");
    assert_eq!(kind("dcdc-buck.snxpcb"), "PCB");
    assert_eq!(kind("dcdc-buck.snxprj"), "Project settings");
    assert_eq!(kind("Stock.snxlib"), "Library");
    assert_eq!(kind("symbols/r.snxsym"), "Symbol");
    assert_eq!(kind("footprints/r0402.snxfpt"), "Footprint");
    assert_eq!(kind("notes.txt"), "File");
    assert_eq!(
        en.message_plain(ids::DIALOG_CLOSE_PROJECT_KIND_COMPONENT, &[]),
        "Component"
    );
}

fn app_closing(
    dirty: &[&str],
    sheet_refs: crate::app::state::sheet_refs::SheetRefs,
) -> crate::app::Signex {
    let (mut app, _boot) = crate::app::Signex::new();
    let project_id = app.document_state.mint_project_id();
    app.ui_state.project_close_confirm = Some(Box::new(crate::app::ProjectCloseConfirmState {
        project_id,
        project_name: "P".into(),
        dirty_paths: paths(dirty),
        dirty_rows: Vec::new(),
        kept_paths: Vec::new(),
        sheet_refs,
    }));
    app
}

/// The list shows each row's name and kind; Discard All stands alone at the
/// left of the footer, then Cancel and Save All on the right.
#[test]
fn rows_show_their_kind_and_discard_all_stands_at_the_left() {
    use crate::layout_probe::{find, lay_out_texts};
    let app = app_closing(&["a.snxsch", "P.snxprj"], Default::default());
    let texts = lay_out_texts(
        app.view_project_close_confirm_body(),
        iced::Size::new(900.0, 700.0),
    );
    for label in ["a.snxsch", "Schematic", "P.snxprj", "Project settings"] {
        find(&texts, label);
    }
    let x = |label| find(&texts, label).bounds.x;
    assert!(x("Discard All") < x("Cancel"), "{texts:#?}");
    assert!(x("Cancel") < x("Save All"), "{texts:#?}");
}

#[test]
fn kept_documents_say_why_they_stay_open() {
    let en = I18n::en_us_ref();
    let kept = |names: &[&str]| plain(kept_line(en, &paths(names)).into_string());
    assert_eq!(
        kept(&["Stock.snxlib"]),
        "Stock.snxlib stays open because another open project may use it."
    );
    assert_eq!(
        kept(&["a.snxsch", "b.snxsch"]),
        "a.snxsch, b.snxsch stay open because other open projects may use them."
    );
}
