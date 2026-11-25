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

/// Where a laid-out text is, by its words.
fn at(texts: &[LaidOutText], label: &str) -> iced::Rectangle {
    texts
        .iter()
        .find(|t| t.text == label)
        .unwrap_or_else(|| panic!("{label:?} missing: {texts:#?}"))
        .bounds
}

/// Mockup row 190: with the bake preview open, Properties holds the Bake
/// panel: the Roles section, the preview card (what Bake writes), then Back
/// to sketch and Bake last. The card is not drawn over the canvas.
#[test]
fn an_open_footprint_preview_shows_its_card_in_properties() {
    use crate::library::editor::footprint::sketch_mode::BakePreview;
    use crate::library::editor::standalone::sketch_exit::FootprintBakeCard;
    use signex_types::sketch::baked_kinds::BakedKind;

    let app = app();
    let mut fp = sketch_ctx(&app, SketchHost::Footprint);
    fp.footprint_bake = Some(FootprintBakeCard {
        preview: BakePreview::Counts(vec![(BakedKind::Courtyard, 1)]),
        open_end: None,
        solved: true,
        warnings: Vec::new(),
    });
    let i18n = I18n::en_us();
    let ctx = &app.document_state.panel_ctx;
    let col = super::super::body::view_body(
        &fp,
        PanelPalette::from_tokens(&ctx.tokens),
        &ctx.tokens,
        &ctx.collapsed_sections,
        signex_types::coord::Unit::Mm,
        &i18n,
    );
    let texts = cleaned(lay_out_texts(col.boxed(), DOCK));
    for label in [
        "ROLES",
        "Profile 1 · 4 edges",
        "BAKE PREVIEW",
        "Bake result",
        "Back to sketch",
        "Bake",
    ] {
        assert!(has(&texts, label), "{label:?} missing: {texts:#?}");
    }
    // The panel's title is "Bake" too; the button is the last one.
    let result = at(&texts, "Bake result");
    let bake = texts
        .iter()
        .rev()
        .find(|t| t.text == "Bake")
        .expect("the Bake button")
        .bounds;
    assert!(
        bake.y > result.y,
        "Bake {bake:?} is not under the card {result:?}"
    );
    assert!(
        at(&texts, "Profile 1 · 4 edges").y < result.y,
        "Roles is not above the card"
    );
}

/// The Properties context after a Bake of the fixture's two profiles: Pads
/// mode, Baked · 8 pads.
fn baked_ctx(app: &Signex) -> FootprintEditorPanelContext {
    use crate::library::editor::footprint::sketch_mode::BakeOutcome;
    use signex_types::sketch::baked_kinds::BakedKind;

    let mut fp = sketch_ctx(app, SketchHost::Footprint);
    fp.mode_kind = FootprintModeKind::Pads;
    fp.bake_outcome = Some(BakeOutcome {
        name: "QFN-16".into(),
        written: vec![(BakedKind::Pads, 8), (BakedKind::Courtyard, 1)],
        pads: 8,
    });
    fp
}

/// The outcome's texts, laid out with `collapsed` as the panel's set.
fn outcome_texts(
    app: &Signex,
    fp: &FootprintEditorPanelContext,
    collapsed: &crate::panels::CollapsedSections,
) -> Vec<LaidOutText> {
    let i18n = I18n::en_us();
    let mut cx = cx(app, fp, &i18n);
    cx.collapsed = collapsed;
    let outcome = fp.bake_outcome.as_ref().expect("an outcome");
    let col = super::super::bake_outcome::view_bake_outcome(Column::new(), cx, outcome);
    cleaned(lay_out_texts(col.boxed(), DOCK))
}

/// Mockup row 190, the baked state: a Pads ↔ Sketch section that explains
/// the links and says "No pad selected." until a pad is clicked, then the
/// Roles and Bake result sections, closed, then Edit in Sketch.
#[test]
fn after_bake_the_pad_links_explain_themselves_and_roles_and_result_start_closed() {
    let app = app();
    let fp = baked_ctx(&app);
    let texts = outcome_texts(&app, &fp, &Default::default());
    for label in [
        "Baked · 8 pads",
        "PADS ↔ SKETCH",
        "Each pad keeps a link to the sketch entity that made it. Click a pad on the canvas.",
        "No pad selected.",
        "ROLES",
        "BAKE RESULT",
        "Edit in Sketch ▸",
    ] {
        assert!(has(&texts, label), "{label:?} missing: {texts:#?}");
    }
    let (link, roles, result) = (
        at(&texts, "PADS ↔ SKETCH"),
        at(&texts, "ROLES"),
        at(&texts, "BAKE RESULT"),
    );
    assert!(link.y < roles.y && roles.y < result.y, "{texts:#?}");
    assert!(
        !has(&texts, "Profile 1 · 4 edges"),
        "Roles is open: {texts:#?}"
    );
    assert!(
        !has(&texts, "Click a pad"),
        "the old one-row link: {texts:#?}"
    );
}

/// Opened, Roles lists each profile with the role it baked as and Bake
/// result each target with its count.
#[test]
fn the_opened_outcome_sections_list_roles_and_counts() {
    let app = app();
    let fp = baked_ctx(&app);
    let open: crate::panels::CollapsedSections = [
        super::super::bake_outcome::ROLES_KEY,
        super::super::bake_outcome::RESULT_KEY,
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let texts = outcome_texts(&app, &fp, &open);
    for label in ["Profile 1 · 4 edges", "Profile 2 · 4 edges"] {
        assert!(has(&texts, label), "{label:?} missing: {texts:#?}");
    }
    for (key, count) in [("PADS", "8"), ("COURTYARD", "1")] {
        let key_at = at(&texts, key);
        let on_its_row = texts.iter().any(|t| {
            t.text == count
                && t.bounds.x > key_at.x
                && t.bounds.y < key_at.y + key_at.height
                && key_at.y < t.bounds.y + t.bounds.height
        });
        assert!(
            on_its_row,
            "{count:?} is not on the {key:?} row: {texts:#?}"
        );
    }
}

/// Mockup row 190, the baked state with a pad clicked: "Pad 3 ← <source>"
/// and Show in Sketch on one row, without the section title repeated.
#[test]
fn a_clicked_pad_names_its_source_with_show_in_sketch_on_its_row() {
    use crate::library::editor::footprint::PadSketchSource;

    let app = app();
    let fp = baked_ctx(&app);
    let i18n = I18n::en_us();
    let source = PadSketchSource {
        edges: 4,
        instance: None,
    };
    let line = super::super::bake_outcome::pad_link_row(2, "3", Some(source), cx(&app, &fp, &i18n));
    let texts = cleaned(lay_out_texts(line, DOCK));
    let pad = at(&texts, "Pad 3 ← Profile of 4 edges");
    let show = at(&texts, "Show in Sketch");
    assert!(
        show.x > pad.x && show.y < pad.y + pad.height && pad.y < show.y + show.height,
        "Show in Sketch is not on the pad's row: {texts:#?}"
    );
    assert!(
        !texts.iter().any(|t| t.text.starts_with("Pads ↔ Sketch")),
        "the section title is repeated on the row: {texts:#?}"
    );
}
