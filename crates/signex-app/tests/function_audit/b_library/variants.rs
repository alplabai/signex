//! m32, Variant Manager. Per the 2026-11-19 decision the section adds,
//! renames, removes and picks variants and reads a Fitted / Not fitted
//! matrix; changing a cell, Duplicate, Set as default, Settings, Compare and
//! Open in BOM Studio are recorded there as not built and are not tested.

use std::path::PathBuf;

use signex_app::app::{
    ContextMenuMsg, EditMsg, FileMsg, Message, ProjectId, ProjectMsg, ProjectSection,
    ProjectTabMsg, ProjectTreeAction, Signex,
};
use signex_output::VariantMatrix;
use signex_types::format::SnxSchematic;
use signex_types::project::{VariantProblem, parse_project};
use signex_types::schematic::{Point, SchematicSheet, Symbol};
use tempfile::TempDir;
use uuid::Uuid;

use super::support::{fresh_app, load_project, project_data};
use crate::settle_support::settle;

struct Rig {
    app: Signex,
    id: ProjectId,
    prj: PathBuf,
    _tmp: TempDir,
}

fn part(reference: &str, x: f64) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        position: Point::new(x, 0.0),
        in_bom: true,
        on_board: true,
        ..Default::default()
    }
}

/// A saved two-part project with its Variants section in front.
fn rig() -> Rig {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![part("R1", 0.0), part("R2", 20.0)],
        ..Default::default()
    };
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(dir.join("root.snxsch"), text).expect("write sheet");
    let prj = dir.join("Proj.snxprj");
    let data = project_data(&dir, &["root.snxsch"]);
    let mut app = fresh_app();
    let id = load_project(&mut app, &prj, &data);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::OpenProjectOptions(vec![id.to_string()]),
    )));
    let mut rig = Rig {
        app,
        id,
        prj,
        _tmp: tmp,
    };
    rig.tab(ProjectTabMsg::SectionSelected {
        project_id: id,
        section: ProjectSection::Variants,
    });
    rig
}

impl Rig {
    fn tab(&mut self, msg: ProjectTabMsg) {
        settle(&mut self.app, Message::Project(ProjectMsg::Tab(msg)));
    }

    fn definitions(&self) -> Vec<String> {
        self.app.document_state.projects[0]
            .data
            .variant_definitions
            .clone()
    }

    fn active(&self) -> Option<String> {
        self.app.document_state.projects[0]
            .data
            .active_variant
            .clone()
    }

    fn state(&self) -> &signex_app::app::ProjectTabState {
        self.app
            .document_state
            .project_tabs
            .get(&self.id)
            .expect("the tab state")
    }

    fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.prj)
    }

    fn undo(&mut self) {
        settle(&mut self.app, Message::Edit(EditMsg::Undo));
    }

    /// `+`, then type `name` over the new row and press Enter.
    fn add(&mut self, name: &str) {
        let id = self.id;
        self.tab(ProjectTabMsg::NewVariantRequested { project_id: id });
        let from = self.definitions().last().cloned().expect("a new variant");
        self.tab(ProjectTabMsg::VariantRenameEdited {
            project_id: id,
            from,
            text: name.to_string(),
        });
        self.tab(ProjectTabMsg::VariantRenameSubmitted { project_id: id });
    }

    fn matrix(&self) -> Option<&VariantMatrix> {
        self.state()
            .variant_parts
            .as_deref()
            .and_then(|read| read.as_ref().ok())
    }
}

#[test]
fn new_variant_adds_a_row_selects_it_and_opens_its_name() {
    let mut r = rig();
    assert!(!r.dirty(), "the project opens clean");
    let id = r.id;

    r.tab(ProjectTabMsg::NewVariantRequested { project_id: id });

    let names = r.definitions();
    assert_eq!(names.len(), 1);
    assert_eq!(r.state().variant_selected.as_ref(), names.first());
    assert!(r.state().variant_rename.is_some(), "its name is open");
    assert!(r.dirty(), "a new variant is an unsaved project edit");
}

#[test]
fn undo_takes_a_new_variant_back_and_clears_the_dirty_mark() {
    let mut r = rig();
    let id = r.id;
    r.tab(ProjectTabMsg::NewVariantRequested { project_id: id });

    r.undo();

    assert!(r.definitions().is_empty());
    assert!(!r.dirty(), "back at the saved state");
}

#[test]
fn a_variant_is_on_disk_only_after_save() {
    let mut r = rig();
    r.add("Lite");
    assert!(
        parse_project(&r.prj)
            .expect("parse")
            .variant_definitions
            .is_empty(),
        "nothing is written until the project is saved"
    );

    settle(&mut r.app, Message::File(FileMsg::Save));

    assert_eq!(
        parse_project(&r.prj).expect("parse").variant_definitions,
        vec!["Lite".to_string()]
    );
    assert!(!r.dirty());
}

#[test]
fn renaming_the_active_variant_keeps_it_active_under_the_new_name() {
    let mut r = rig();
    let id = r.id;
    r.add("Lite");
    r.tab(ProjectTabMsg::ActiveVariantPicked {
        project_id: id,
        name: Some("Lite".to_string()),
    });

    r.tab(ProjectTabMsg::VariantRenameStarted {
        project_id: id,
        name: "Lite".to_string(),
    });
    r.tab(ProjectTabMsg::VariantRenameEdited {
        project_id: id,
        from: "Lite".to_string(),
        text: "Mini".to_string(),
    });
    r.tab(ProjectTabMsg::VariantRenameSubmitted { project_id: id });

    assert_eq!(r.definitions(), vec!["Mini".to_string()]);
    assert_eq!(r.active().as_deref(), Some("Mini"));
    assert_eq!(r.state().variant_selected.as_deref(), Some("Mini"));
}

#[test]
fn a_name_another_variant_holds_is_refused_and_stays_in_the_box() {
    let mut r = rig();
    let id = r.id;
    r.add("Lite");
    r.add("Pro");

    r.tab(ProjectTabMsg::VariantRenameEdited {
        project_id: id,
        from: "Pro".to_string(),
        text: "lite".to_string(),
    });
    r.tab(ProjectTabMsg::VariantRenameSubmitted { project_id: id });

    assert_eq!(
        r.definitions(),
        vec!["Lite".to_string(), "Pro".to_string()],
        "names are unique ignoring case"
    );
    assert!(matches!(
        r.state().variant_problem,
        Some(VariantProblem::Taken(_))
    ));
    assert_eq!(
        r.state().variant_rename.as_ref().map(|d| d.text.as_str()),
        Some("lite"),
        "the refused name stays typed"
    );
}

#[test]
fn base_cannot_be_deleted_or_renamed() {
    let mut r = rig();
    let id = r.id;

    r.tab(ProjectTabMsg::VariantRemoved {
        project_id: id,
        name: "Base".to_string(),
    });
    assert!(r.state().variant_problem.is_some(), "the delete is refused");

    r.tab(ProjectTabMsg::VariantRenameStarted {
        project_id: id,
        name: "Base".to_string(),
    });
    r.tab(ProjectTabMsg::VariantRenameEdited {
        project_id: id,
        from: "Base".to_string(),
        text: "Main".to_string(),
    });
    r.tab(ProjectTabMsg::VariantRenameSubmitted { project_id: id });

    assert!(r.state().variant_problem.is_some(), "the rename is refused");
    assert!(r.definitions().is_empty());
    assert_eq!(r.active(), None);
    assert!(!r.dirty(), "a refused edit changes nothing");
}

#[test]
fn clicking_a_variant_selects_it_without_making_it_active() {
    let mut r = rig();
    let id = r.id;
    r.add("Lite");
    settle(&mut r.app, Message::File(FileMsg::Save));

    r.tab(ProjectTabMsg::VariantSelected {
        project_id: id,
        name: Some("Lite".to_string()),
    });

    assert_eq!(r.state().variant_selected.as_deref(), Some("Lite"));
    assert_eq!(r.active(), None, "Base stays active");
    assert!(!r.dirty(), "selecting is not an edit");
}

#[test]
fn removing_the_active_variant_makes_base_active_and_undo_restores_it() {
    let mut r = rig();
    let id = r.id;
    r.add("Lite");
    r.tab(ProjectTabMsg::ActiveVariantPicked {
        project_id: id,
        name: Some("Lite".to_string()),
    });

    r.tab(ProjectTabMsg::VariantRemoved {
        project_id: id,
        name: "Lite".to_string(),
    });
    assert!(r.definitions().is_empty());
    assert_eq!(r.active(), None, "Base is active again");

    r.undo();
    assert_eq!(r.definitions(), vec!["Lite".to_string()]);
    assert_eq!(r.active().as_deref(), Some("Lite"));
}

#[test]
fn the_parts_filter_and_only_rows_that_differ_are_view_state() {
    let mut r = rig();
    let id = r.id;

    r.tab(ProjectTabMsg::VariantFilterEdited {
        project_id: id,
        text: "R1".to_string(),
    });
    r.tab(ProjectTabMsg::VariantDifferingToggled { project_id: id });

    assert_eq!(r.state().variant_filter, "R1");
    assert!(r.state().variants_only_differ);
    assert!(!r.dirty(), "a filter is not a document edit");
}

#[test]
fn the_matrix_lists_each_part_as_fitted_in_base_and_a_new_variant() {
    let mut r = rig();
    let id = r.id;
    r.add("Lite");

    r.tab(ProjectTabMsg::VariantPartsReadRequested { project_id: id });

    let matrix = r.matrix().expect("the parts were read");
    assert_eq!(matrix.variants, vec!["Lite".to_string()]);
    let designators: Vec<&str> = matrix.rows.iter().map(|p| p.designator.as_str()).collect();
    assert_eq!(designators, ["R1", "R2"]);
    for row in &matrix.rows {
        assert_eq!(row.fitted, vec![true, true], "{}", row.designator);
        assert!(!row.differs_at(1), "{} matches Base", row.designator);
    }
}
