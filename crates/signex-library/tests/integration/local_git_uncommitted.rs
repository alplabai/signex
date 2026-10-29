//! #858 -- a save whose file write succeeded is a successful save in every
//! write path, even when the git commit after it fails. The change is kept,
//! the adapter reports it as uncommitted, and the next successful commit (or
//! an explicit `commit_pending`) records it. Nothing is lost from the working
//! tree or the index, whatever happened to HEAD or `.git/index.lock` meanwhile.

#![cfg(feature = "local-git")]

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use signex_library::adapter::{CommitOutcome, LibraryAdapter, LibraryError};
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::{
    Body3D, BodyShape, Footprint, LayerId, Pad, PadKind, PadShape, Polygon, SimKind, SimModel,
    Symbol,
};
use uuid::Uuid;

use super::local_git_rollback::{class, head_message, head_oid, init, row};

const TABLE: &str = "R";

struct Lib {
    _dir: tempfile::TempDir,
    file: PathBuf,
    root: PathBuf,
    lib_id: Uuid,
    adapter: LocalGitAdapter,
}

impl Lib {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let (file, lib_id, adapter) = init(&dir);
        let root = file.parent().unwrap().to_path_buf();
        Self {
            _dir: dir,
            file,
            root,
            lib_id,
            adapter,
        }
    }

    fn seed_row(&self) -> signex_library::component::ComponentRow {
        let seed = row("R-1", self.lib_id);
        self.adapter
            .insert_row(TABLE, seed.clone(), "seed")
            .unwrap();
        self.adapter.read_row(TABLE, seed.row_id.into()).unwrap()
    }
}

/// Make the next commit fail at once: a branch-ref lock a crashed git would
/// have left (older than the stale threshold, so no 5 s wait). Remove the
/// returned path to repair.
fn fail_commits(root: &Path) -> PathBuf {
    let head = std::fs::read_to_string(root.join(".git").join("HEAD")).unwrap();
    let branch_ref = head.trim().strip_prefix("ref: ").unwrap().to_string();
    let lock = root.join(".git").join(format!("{branch_ref}.lock"));
    let file = std::fs::File::create(&lock).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(60))
        .unwrap();
    lock
}

/// The bytes HEAD holds for `rel`, `None` when HEAD has no such file.
fn head_bytes(root: &Path, rel: &str) -> Option<Vec<u8>> {
    let repo = git2::Repository::open(root).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let entry = tree.get_path(Path::new(rel)).ok()?;
    Some(repo.find_blob(entry.id()).unwrap().content().to_vec())
}

fn assert_head_holds_disk(root: &Path, paths: &[String]) {
    assert!(!paths.is_empty());
    for rel in paths {
        let disk = std::fs::read(root.join(rel)).ok();
        assert_eq!(
            head_bytes(root, rel),
            disk,
            "HEAD differs from disk at {rel}"
        );
    }
}

fn footprint(name: &str) -> Footprint {
    let now = chrono::Utc::now();
    Footprint {
        uuid: Uuid::now_v7(),
        name: name.into(),
        anchor: [0.0, 0.0],
        pads: vec![Pad {
            number: "1".into(),
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            size: [1.0, 1.4],
            position: [0.0, 0.0],
            rotation: 0.0,
            layers: vec![LayerId::new("F.Cu")],
            drill: None,
            solder_mask_margin: None,
            paste_margin: None,
            ..Pad::default()
        }],
        courtyard: Polygon::default(),
        silk_f: Vec::new(),
        silk_b: Vec::new(),
        fab_f: Vec::new(),
        fab_b: Vec::new(),
        body_3d: Body3D {
            shape: BodyShape::Extrude,
            height_mm: 1.6,
            offset_z_mm: 0.0,
            top_color: [0.1, 0.1, 0.1, 1.0],
            side_color: [0.2, 0.2, 0.2, 1.0],
            outline: None,
        },
        step_attachment: None,
        pcb_params: Default::default(),
        version: "0.0.1".into(),
        released: false,
        created: now,
        updated: now,
        schema_version: 2,
        sketch: None,
        load_repairs: Vec::new(),
        pours: Vec::new(),
        keepouts: Vec::new(),
        cutouts: Vec::new(),
        v_scores: Vec::new(),
        mask_openings: Vec::new(),
        mask_excludes: Vec::new(),
        paste_apertures: Vec::new(),
        description: String::new(),
        default_designator: String::new(),
        component_type: signex_library::primitive::footprint::ComponentType::Standard,
        height_mm: None,
    }
}

fn sim(name: &str) -> SimModel {
    SimModel {
        uuid: Uuid::now_v7(),
        name: name.into(),
        kind: SimKind::Spice3,
        body: ".SUBCKT TEST IN OUT\n.ENDS".into(),
        default_node_map: Default::default(),
        version: "0.0.1".into(),
        released: false,
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
    }
}

/// One write path with its commit failing: it returns `Ok`, leaves the file
/// holding the new bytes, and the adapter lists the change as uncommitted.
/// Then, with git repaired, the next successful save commits both changes.
fn check_path(name: &str, lib: &Lib, write: impl FnOnce(&Lib) -> Result<(), LibraryError>) {
    let head_before = head_oid(&lib.root);
    let lock = fail_commits(&lib.root);
    write(lib).unwrap_or_else(|e| panic!("{name}: a failed commit must not fail the save: {e}"));
    std::fs::remove_file(&lock).unwrap();

    assert_eq!(head_oid(&lib.root), head_before, "{name}: HEAD moved");
    let pending = lib
        .adapter
        .uncommitted_changes()
        .unwrap_or_else(|| panic!("{name}: the save is not reported as uncommitted"));
    assert!(
        pending.reason.contains("older than"),
        "{name}: {}",
        pending.reason
    );
    // The file holds the new bytes: it differs from HEAD's copy.
    assert!(
        pending
            .paths
            .iter()
            .any(|rel| head_bytes(&lib.root, rel) != std::fs::read(lib.root.join(rel)).ok()),
        "{name}: no pending path differs from HEAD: {:?}",
        pending.paths
    );

    // The next successful save commits both changes.
    lib.adapter
        .add_library_class(
            class(&format!("next-{}", Uuid::now_v7().simple())),
            "next save",
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(lib.adapter.uncommitted_changes(), None, "{name}");
    assert_ne!(head_oid(&lib.root), head_before, "{name}: no new commit");
    assert_eq!(head_message(&lib.root).trim(), "next save", "{name}");
    let mut paths = pending.paths;
    paths.push(lib.file.file_name().unwrap().to_string_lossy().into_owned());
    assert_head_holds_disk(&lib.root, &paths);
}

#[test]
fn a_footprint_save_with_a_failed_commit_is_a_save() {
    let lib = Lib::new();
    check_path("save_footprint", &lib, |l| {
        l.adapter.save_footprint(footprint("SOIC-8"), "add")
    });
}

#[test]
fn a_sim_save_with_a_failed_commit_is_a_save() {
    let lib = Lib::new();
    check_path("save_sim", &lib, |l| {
        l.adapter.save_sim(sim("LM358"), "add")
    });
}

#[test]
fn a_symbol_save_with_a_failed_commit_is_a_save() {
    let lib = Lib::new();
    check_path("save_symbol", &lib, |l| {
        l.adapter.save_symbol(Symbol::empty("R"), "add")
    });
}

#[test]
fn a_symbol_edit_in_an_existing_container_with_a_failed_commit_is_a_save() {
    let lib = Lib::new();
    let mut symbol = Symbol::empty("R");
    lib.adapter.save_symbol(symbol.clone(), "add").unwrap();
    symbol.name = "R-renamed".into();
    check_path("save_symbol (edit)", &lib, |l| {
        l.adapter.save_symbol(symbol, "edit")
    });
}

#[test]
fn an_external_change_commit_that_fails_is_reported_not_raised() {
    let lib = Lib::new();
    let note = lib.root.join("symbols").join("note.txt");
    std::fs::create_dir_all(note.parent().unwrap()).unwrap();
    std::fs::write(&note, "edited outside").unwrap();
    let lock = fail_commits(&lib.root);
    let outcome = lib.adapter.commit_external_change(&note, "edit").unwrap();
    std::fs::remove_file(lock).unwrap();
    assert!(
        matches!(&outcome, CommitOutcome::NotCommitted { reason } if reason.contains("older than")),
        "{outcome:?}"
    );
    assert_eq!(std::fs::read(&note).unwrap(), b"edited outside");
    assert_eq!(
        lib.adapter.uncommitted_changes().unwrap().paths,
        vec!["symbols/note.txt".to_string()]
    );
    lib.adapter
        .add_library_class(class("x"), "next save")
        .unwrap();
    assert_head_holds_disk(&lib.root, &["symbols/note.txt".to_string()]);
}

#[test]
fn a_locked_file_rewrite_with_a_failed_commit_is_a_save() {
    let lib = Lib::new();
    let target = lib.root.join("footprints").join("pair.snxfpt");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    check_path("rewrite_file_locked", &lib, |l| {
        let written = l
            .adapter
            .rewrite_file_locked(&target, "merge", &mut |_| Ok(Some(b"merged".to_vec())))?;
        assert!(written);
        assert_eq!(std::fs::read(&target).unwrap(), b"merged");
        Ok(())
    });
}

#[test]
fn row_inserts_updates_and_deletes_with_a_failed_commit_are_saves() {
    let lib = Lib::new();
    let seed = lib.seed_row();
    check_path("insert_row", &lib, |l| {
        l.adapter.insert_row(TABLE, row("R-2", l.lib_id), "insert")
    });
    let mut edited = seed.clone();
    edited.description = "edited".into();
    check_path("update_row", &lib, |l| {
        l.adapter.update_row(TABLE, edited, "edit").map(drop)
    });
    let current = lib.adapter.read_row(TABLE, seed.row_id.into()).unwrap();
    check_path("delete_row", &lib, |l| {
        l.adapter
            .delete_row(TABLE, seed.row_id.into(), current.server_revision, "delete")
    });
    assert!(lib.adapter.read_row(TABLE, seed.row_id.into()).is_err());
}

#[test]
fn a_batch_row_update_with_a_failed_commit_is_a_save() {
    let lib = Lib::new();
    let seed = lib.seed_row();
    check_path("update_rows", &lib, |l| {
        let targets = [(TABLE.to_string(), seed.row_id.into())];
        let outcomes = l.adapter.update_rows(
            &targets,
            &mut |row| {
                row.description = "batch".into();
                true
            },
            "batch",
        )?;
        assert!(matches!(outcomes[0].1, Ok(true)));
        Ok(())
    });
    let stored = lib.adapter.read_row(TABLE, seed.row_id.into()).unwrap();
    assert_eq!(stored.description, "batch");
}

#[test]
fn table_and_class_writes_with_a_failed_commit_are_saves() {
    let lib = Lib::new();
    check_path("create_empty_table", &lib, |l| {
        l.adapter.create_empty_table("caps", "create")
    });
    check_path("rename_table", &lib, |l| {
        l.adapter.rename_table("caps", "capacitors", "rename")
    });
    check_path("delete_empty_table", &lib, |l| {
        l.adapter.delete_empty_table("capacitors", "delete")
    });
    check_path("add_library_class", &lib, |l| {
        l.adapter.add_library_class(class("res"), "add")
    });
    check_path("rename_library_class", &lib, |l| {
        l.adapter
            .rename_library_class("res", class("resistor"), "rename")
    });
    check_path("update_library_classes", &lib, |l| {
        l.adapter
            .update_library_classes(vec![class("a"), class("b")], "set")
    });
    check_path("remove_library_class", &lib, |l| {
        l.adapter.remove_library_class("a", "remove")
    });
}

#[test]
fn a_rename_of_the_library_file_with_a_failed_commit_still_moves_the_adapter() {
    let mut lib = Lib::new();
    lib.seed_row();
    let renamed = lib.root.join("Renamed.snxlib");
    std::fs::rename(&lib.file, &renamed).unwrap();
    let lock = fail_commits(&lib.root);
    lib.adapter
        .relocate_library_file(&renamed)
        .expect("the file is already renamed; a failed commit must not undo that");
    std::fs::remove_file(lock).unwrap();
    assert_eq!(lib.adapter.library_file_path(), Some(renamed.as_path()));
    let pending = lib.adapter.uncommitted_changes().unwrap();
    assert_eq!(pending.paths, vec!["Rb.snxlib", "Renamed.snxlib"]);

    lib.adapter.add_library_class(class("x"), "next").unwrap();
    assert_eq!(head_bytes(&lib.root, "Rb.snxlib"), None, "old name removed");
    assert_head_holds_disk(&lib.root, &["Renamed.snxlib".to_string()]);
}

#[test]
fn commit_pending_commits_by_itself_and_reports_a_second_failure() {
    let lib = Lib::new();
    let lock = fail_commits(&lib.root);
    lib.adapter.add_library_class(class("res"), "add").unwrap();
    let outcome = lib.adapter.commit_pending("retry").unwrap();
    assert!(
        matches!(outcome, CommitOutcome::NotCommitted { .. }),
        "{outcome:?}"
    );
    assert!(lib.adapter.uncommitted_changes().is_some());

    std::fs::remove_file(lock).unwrap();
    assert_eq!(
        lib.adapter.commit_pending("retry").unwrap(),
        CommitOutcome::Committed
    );
    assert_eq!(lib.adapter.uncommitted_changes(), None);
    assert_eq!(head_message(&lib.root).trim(), "retry");
    assert_head_holds_disk(&lib.root, &["Rb.snxlib".to_string()]);
    assert_eq!(
        lib.adapter.commit_pending("again").unwrap(),
        CommitOutcome::NothingToCommit
    );
}

/// A stale `.git/index.lock` gives the same result in every path: the commit
/// does not need the index, so the save is committed at once and the lock is
/// left alone.
#[test]
fn a_stale_index_lock_gives_the_same_result_in_every_path() {
    let lib = Lib::new();
    let seed = lib.seed_row();
    let lock = lib.root.join(".git").join("index.lock");
    let stale = std::fs::File::create(&lock).unwrap();
    stale
        .set_modified(SystemTime::now() - Duration::from_secs(60))
        .unwrap();
    drop(stale);

    let started = std::time::Instant::now();
    let note = lib.root.join("note.txt");
    std::fs::write(&note, "n").unwrap();
    let writes: Vec<NamedWrite> = vec![
        (
            "save_footprint",
            Box::new(|l| {
                l.adapter
                    .save_footprint(footprint("F"), "fp")
                    .map(|()| outcome_of(l))
            }),
        ),
        (
            "save_sim",
            Box::new(|l| l.adapter.save_sim(sim("S"), "sim").map(|()| outcome_of(l))),
        ),
        (
            "save_symbol",
            Box::new(|l| {
                l.adapter
                    .save_symbol(Symbol::empty("Y"), "sym")
                    .map(|()| outcome_of(l))
            }),
        ),
        (
            "commit_external_change",
            Box::new({
                let note = note.clone();
                move |l| l.adapter.commit_external_change(&note, "ext")
            }),
        ),
        (
            "add_library_class",
            Box::new(|l| {
                l.adapter
                    .add_library_class(class("c"), "cls")
                    .map(|()| outcome_of(l))
            }),
        ),
        (
            "insert_row",
            Box::new(|l| {
                l.adapter
                    .insert_row(TABLE, row("R-9", l.lib_id), "ins")
                    .map(|()| outcome_of(l))
            }),
        ),
        (
            "update_rows",
            Box::new(move |l| {
                let targets = [(TABLE.to_string(), seed.row_id.into())];
                l.adapter
                    .update_rows(
                        &targets,
                        &mut |r| {
                            r.description = "d".into();
                            true
                        },
                        "batch",
                    )
                    .map(|_| outcome_of(l))
            }),
        ),
    ];
    for (name, write) in writes {
        let head_before = head_oid(&lib.root);
        let outcome = write(&lib).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(outcome, CommitOutcome::Committed, "{name}");
        assert_ne!(head_oid(&lib.root), head_before, "{name}: not committed");
        assert_eq!(lib.adapter.uncommitted_changes(), None, "{name}");
    }
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert!(lock.exists(), "Signex never removes a lock it did not make");
    // Every working file is what HEAD holds.
    for rel in ["note.txt", "Rb.snxlib"] {
        assert_head_holds_disk(&lib.root, &[rel.to_string()]);
    }
}

/// A named write path of the stale-lock test.
type NamedWrite = (
    &'static str,
    Box<dyn Fn(&Lib) -> Result<CommitOutcome, LibraryError>>,
);

/// The outcome of the save that just ran, read off the adapter: committed
/// when nothing is waiting.
fn outcome_of(lib: &Lib) -> CommitOutcome {
    match lib.adapter.uncommitted_changes() {
        None => CommitOutcome::Committed,
        Some(p) => CommitOutcome::NotCommitted { reason: p.reason },
    }
}

/// A second adapter on the same repository (another window, another process)
/// commits while the first one's save is still uncommitted. Neither commit
/// reverts the other and the first adapter's retry builds on the moved HEAD.
#[test]
fn a_second_adapter_committing_meanwhile_loses_no_change() {
    let lib = Lib::new();
    let other = LocalGitAdapter::open(&lib.file).unwrap();

    let lock = fail_commits(&lib.root);
    lib.adapter
        .save_symbol(Symbol::empty("A"), "add A")
        .unwrap();
    std::fs::remove_file(lock).unwrap();
    let pending = lib.adapter.uncommitted_changes().unwrap();
    let symbol_rel = pending
        .paths
        .iter()
        .find(|p| p.starts_with("symbols/"))
        .expect("the symbol file is pending")
        .clone();

    other
        .add_library_class(class("from-other"), "other adds")
        .unwrap();
    assert_eq!(head_message(&lib.root).trim(), "other adds");
    assert_eq!(head_bytes(&lib.root, &symbol_rel), None, "not in HEAD yet");

    assert_eq!(
        lib.adapter.commit_pending("retry").unwrap(),
        CommitOutcome::Committed
    );
    assert_head_holds_disk(&lib.root, &[symbol_rel, "Rb.snxlib".to_string()]);
    let lf = std::fs::read_to_string(&lib.file).unwrap();
    assert!(
        lf.contains("from-other"),
        "the other adapter's class was lost"
    );
}

/// #893 part 2: commits that do not run under a library write lock
/// (`commit_external_change`, the project adapter) from several adapters on
/// one repository take turns on the repository commit lock, so every commit
/// lands and none fails.
#[test]
fn adapters_committing_to_one_repository_concurrently_all_land() {
    use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
    const COMMITS: u32 = 60;
    let lib = Lib::new();
    let root = lib.root.clone();
    let names = ["a.txt", "b.txt", "c.txt", "d.txt"];
    for name in names {
        std::fs::write(root.join(name), "0").unwrap();
    }
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(names.len()));
    let writers: Vec<_> = names
        .into_iter()
        .enumerate()
        .map(|(n, name)| {
            let (file, root, barrier) = (lib.file.clone(), root.clone(), barrier.clone());
            std::thread::spawn(move || {
                let library = LocalGitAdapter::open(&file).unwrap();
                let project = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
                barrier.wait();
                for i in 1..=COMMITS {
                    std::fs::write(root.join(name), i.to_string()).unwrap();
                    if n % 2 == 0 {
                        let outcome = library
                            .commit_external_change(&root.join(name), &format!("{name} {i}"))
                            .unwrap();
                        assert!(outcome.is_recorded(), "{name} {i}: {outcome:?}");
                    } else {
                        project
                            .commit_path(Path::new(name), &format!("{name} {i}"))
                            .unwrap_or_else(|e| panic!("{name} {i}: {e}"));
                    }
                }
                library.uncommitted_changes()
            })
        })
        .collect();
    for writer in writers {
        assert_eq!(writer.join().unwrap(), None);
    }
    for name in names {
        assert_eq!(
            head_bytes(&root, name).unwrap(),
            COMMITS.to_string().into_bytes(),
            "{name}"
        );
    }
}

/// Hold the repository commit lock of the repo at `root` from this thread,
/// as another adapter or Signex process would while it commits.
fn hold_commit_lock(root: &Path) -> std::fs::File {
    let lock = root.join(".git").join("signex").join("commit.lock");
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    let handle = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock)
        .unwrap();
    handle.lock().unwrap();
    handle
}

/// #893 part 2: a commit that does not hold the library write lock waits for
/// another committer of the repository instead of racing it.
#[test]
fn a_library_commit_waits_for_another_committer_of_the_repository() {
    let lib = Lib::new();
    let note = lib.root.join("note.txt");
    std::fs::write(&note, "n").unwrap();
    let held = hold_commit_lock(&lib.root);
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        drop(held);
    });
    let started = std::time::Instant::now();
    let outcome = lib.adapter.commit_external_change(&note, "note").unwrap();
    let waited = started.elapsed();
    release.join().unwrap();
    assert_eq!(outcome, CommitOutcome::Committed);
    assert!(
        waited >= Duration::from_millis(350),
        "the commit did not wait: {waited:?}"
    );
}

/// The project adapter's commit (which also follows `restore_at`) takes the
/// same turn.
#[test]
fn a_project_commit_waits_for_another_committer_of_the_repository() {
    use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
    let lib = Lib::new();
    let project = LocalGitProjectAdapter::open_or_init(lib.root.clone()).unwrap();
    std::fs::write(lib.root.join("doc.txt"), "d").unwrap();
    let held = hold_commit_lock(&lib.root);
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        drop(held);
    });
    let started = std::time::Instant::now();
    project.commit_path(Path::new("doc.txt"), "doc").unwrap();
    let waited = started.elapsed();
    release.join().unwrap();
    assert!(
        waited >= Duration::from_millis(350),
        "the commit did not wait: {waited:?}"
    );
    assert_eq!(head_bytes(&lib.root, "doc.txt").unwrap(), b"d");
}
