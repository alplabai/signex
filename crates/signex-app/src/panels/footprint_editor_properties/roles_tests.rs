//! Bake as on a board sketch (mockup row 179, DECISIONS 2026-11-24): each
//! closed profile with its picker and no Bake buttons; the footprint's Roles
//! section keeps its own.

use std::path::PathBuf;

use iced::Widget as _;
use iced::widget::Column;
use signex_i18n::I18n;
use signex_library::{Footprint, FootprintFile};
use signex_types::sketch::SketchHost;

use super::super::body::Ctx;
use super::{view_board_bake_as, view_roles};
use crate::app::{FootprintDocument, Signex, TabInfo, TabKind};
use crate::layout_probe::{LaidOutText, lay_out_texts};
use crate::library::editor::footprint::sketch_mode::roles::{
    board_profile_rows, fixtures::rect_and_slot, profile_role_rows,
};
use crate::panels::{FootprintEditorPanelContext, FootprintModeKind, PanelPalette};

const PATH: &str = "bake-as.snxfpt";
const DOCK: iced::Size = iced::Size::new(300.0, 4000.0);

fn app() -> Signex {
    let path = PathBuf::from(PATH);
    let file = FootprintFile::from_footprint(Footprint::empty("board"));
    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "board".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = 0;
    app.refresh_panel_ctx();
    app
}

/// The panel context in Sketch mode on `host`, listing the fixture's two
/// profiles as that host lists them.
fn sketch_ctx(app: &Signex, host: SketchHost) -> FootprintEditorPanelContext {
    let mut fp = app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_deref()
        .cloned()
        .expect("the footprint editor panel context is built");
    let f = rect_and_slot();
    fp.mode_kind = FootprintModeKind::Sketch;
    fp.sketch_host = host;
    fp.profile_roles = match host {
        SketchHost::Board => board_profile_rows(&f.sketch, Some(&f.solve)),
        SketchHost::Footprint => profile_role_rows(&f.sketch, host),
    };
    fp
}

fn cx<'a, 'i>(app: &'a Signex, fp: &'a FootprintEditorPanelContext, i18n: &'i I18n) -> Ctx<'a, 'i> {
    let ctx = &app.document_state.panel_ctx;
    Ctx {
        fp,
        palette: PanelPalette::from_tokens(&ctx.tokens),
        tokens: &ctx.tokens,
        collapsed: &ctx.collapsed_sections,
        unit: signex_types::coord::Unit::Mm,
        i18n,
    }
}

fn cleaned(texts: Vec<LaidOutText>) -> Vec<LaidOutText> {
    texts
        .into_iter()
        .map(|mut t| {
            t.text = t.text.replace(['\u{2068}', '\u{2069}'], "");
            t
        })
        .collect()
}

fn bake_as_texts(app: &Signex, fp: &FootprintEditorPanelContext) -> Vec<LaidOutText> {
    let i18n = I18n::en_us();
    let col = view_board_bake_as(Column::new(), cx(app, fp, &i18n));
    cleaned(lay_out_texts(col.boxed(), DOCK))
}

fn roles_texts(app: &Signex, fp: &FootprintEditorPanelContext) -> Vec<LaidOutText> {
    let i18n = I18n::en_us();
    let col = view_roles(Column::new(), cx(app, fp, &i18n));
    cleaned(lay_out_texts(col.boxed(), DOCK))
}

fn has(texts: &[LaidOutText], label: &str) -> bool {
    texts.iter().any(|t| t.text == label)
}

#[test]
fn a_board_lists_each_closed_profile_under_bake_as_without_bake_buttons() {
    let app = app();
    let fp = sketch_ctx(&app, SketchHost::Board);
    let texts = bake_as_texts(&app, &fp);
    for label in ["BAKE AS", "Profile 1 · 4 edges", "Profile 2 · 4 edges"] {
        assert!(has(&texts, label), "{label:?} missing: {texts:#?}");
    }
    for label in ["Bake", "Bake preview", "ROLES"] {
        assert!(!has(&texts, label), "{label:?} shown: {texts:#?}");
    }
    let roles = roles_texts(&app, &fp);
    assert!(
        roles.is_empty(),
        "no footprint Roles on a board: {roles:#?}"
    );
}

#[test]
fn the_footprint_roles_section_keeps_its_bake_buttons_and_no_bake_as() {
    let app = app();
    let fp = sketch_ctx(&app, SketchHost::Footprint);
    let texts = roles_texts(&app, &fp);
    for label in ["ROLES", "Profile 1 · 4 edges", "Bake", "Bake preview"] {
        assert!(has(&texts, label), "{label:?} missing: {texts:#?}");
    }
    let board = bake_as_texts(&app, &fp);
    assert!(board.is_empty(), "no Bake as on a footprint: {board:#?}");
}
