//! mockup row 187 (Caner, 2026-11-20, decision 8): the footprint editor switches with
//! a two-state Footprint / Sketch segment; 3D is a separate toggle beside it
//! that returns to the mode it was entered from. Each click sends what the
//! editor bar's `ModeSwitch` says that control sends.

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::library::editor::footprint::EditorMode;
use signex_app::library::editor::footprint::ModeSwitch;
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_i18n::t;
use signex_library::{Footprint, FootprintFile};
use std::path::PathBuf;

struct Rig {
    app: Signex,
    path: PathBuf,
}

/// An empty footprint open in a footprint tab, in Footprint mode.
fn rig() -> Rig {
    let path = PathBuf::from("mode-switch.snxfpt");
    let editor = FootprintDocument::new(
        path.clone(),
        FootprintFile::from_footprint(Footprint::empty("mode-switch")),
    );
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "mode-switch".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    Rig { app, path }
}

impl Rig {
    fn editor(&self) -> &FootprintDocument {
        &self.app.document_state.footprint_editors[&self.path]
    }

    fn mode(&self) -> EditorMode {
        self.editor().state.mode
    }

    /// The switch as the editor bar draws it now.
    fn switch(&self) -> ModeSwitch {
        let state = &self.editor().state;
        ModeSwitch::of(state.mode, state.mode_before_3d)
    }

    fn send(&mut self, msg: FootprintEditorMsg) {
        drop(
            self.app
                .update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                    path: self.path.clone(),
                    msg: PrimitiveEdit::Footprint(msg),
                })),
        );
    }

    /// A click on the segment of `segment`.
    fn click_segment(&mut self, segment: EditorMode) {
        assert!(
            ModeSwitch::SEGMENTS.contains(&segment),
            "{segment:?} has no segment"
        );
        self.send(FootprintEditorMsg::SetMode(segment));
    }

    /// A click on the 3D toggle.
    fn click_3d(&mut self) {
        let target = self.switch().toggle_target;
        self.send(FootprintEditorMsg::SetMode(target));
    }

    /// `id` in en-US, without Fluent's isolation marks.
    fn english(&self, id: signex_i18n::MessageId) -> String {
        t!(self.app.ui_state.i18n, id)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    }
}

#[test]
fn clicking_sketch_enters_sketch_mode_and_footprint_leaves_it() {
    let mut rig = rig();
    assert_eq!(rig.mode(), EditorMode::Normal);
    assert!(rig.switch().is_lit(EditorMode::Normal));
    assert!(!rig.switch().is_3d_on);

    rig.click_segment(EditorMode::Sketch);
    assert_eq!(rig.mode(), EditorMode::Sketch);
    assert!(rig.switch().is_lit(EditorMode::Sketch));
    assert!(!rig.switch().is_lit(EditorMode::Normal));

    // Leaving through the switch keeps the exit behaviour: the bake
    // preview open in sketch mode closes.
    rig.send(FootprintEditorMsg::SketchBakePreviewToggled);
    assert!(rig.editor().state.sketch_exit.preview);
    rig.click_segment(EditorMode::Normal);
    assert_eq!(rig.mode(), EditorMode::Normal);
    assert!(rig.switch().is_lit(EditorMode::Normal));
    assert!(!rig.editor().state.sketch_exit.preview);
}

#[test]
fn three_d_from_sketch_then_three_d_off_returns_to_sketch() {
    let mut rig = rig();
    rig.click_segment(EditorMode::Sketch);

    rig.click_3d();
    assert_eq!(rig.mode(), EditorMode::View3d);
    let on = rig.switch();
    assert!(on.is_3d_on);
    assert_eq!(on.toggle_target, EditorMode::Sketch);
    assert!(
        on.is_lit(EditorMode::Sketch),
        "the Sketch segment stays lit"
    );

    rig.click_3d();
    assert_eq!(rig.mode(), EditorMode::Sketch);
    assert!(!rig.switch().is_3d_on);
}

#[test]
fn three_d_from_footprint_then_three_d_off_returns_to_footprint() {
    let mut rig = rig();
    rig.click_segment(EditorMode::Sketch);
    rig.click_segment(EditorMode::Normal);

    rig.click_3d();
    assert_eq!(rig.mode(), EditorMode::View3d);
    assert_eq!(rig.switch().toggle_target, EditorMode::Normal);
    assert!(rig.switch().is_lit(EditorMode::Normal));

    rig.click_3d();
    assert_eq!(rig.mode(), EditorMode::Normal);
    assert!(!rig.switch().is_3d_on);
}

#[test]
fn a_segment_click_in_3d_leaves_3d_for_that_mode() {
    let mut rig = rig();
    rig.click_3d();
    assert_eq!(rig.mode(), EditorMode::View3d);

    rig.click_segment(EditorMode::Sketch);
    assert_eq!(rig.mode(), EditorMode::Sketch);
    assert!(!rig.switch().is_3d_on);
}

#[test]
fn the_switch_reads_footprint_sketch_and_3d() {
    let rig = rig();
    let segments: Vec<String> = ModeSwitch::SEGMENTS
        .iter()
        .map(|m| rig.english(m.switch_label()))
        .collect();
    assert_eq!(segments, ["Footprint", "Sketch"]);
    assert_eq!(rig.english(EditorMode::View3d.switch_label()), "3D");
}
