//! File ▸ New Project: where the project lives and the off-thread writes.
//!
//! The Save-As dialog yields `<parent>/<stem>.snxprj`; the project itself is
//! the folder `<parent>/<stem>/` holding `<stem>.snxprj` and `<stem>.snxsch`.
//! Writing into `<parent>` directly would turn a shared folder (a Desktop) into
//! the project directory (#1205).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use signex_library::vcs_target_guard::{KnownFolders, enclosing_project};

/// OS-generated files that do not make a folder "in use".
const OS_CLUTTER: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];

/// The `.snxprj` must end in `.snxprj`, have a parent and a UTF-8 stem.
pub(crate) fn new_project_parts(project_path: &Path) -> Result<(&Path, String)> {
    if project_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("snxprj"))
        != Some(true)
    {
        anyhow::bail!(
            "new project path must end in .snxprj (got {})",
            project_path.display()
        );
    }
    let dir = project_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("project path has no parent directory"))?;
    let stem = project_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("project path has no file stem"))?
        .to_string();
    Ok((dir, stem))
}

/// Where the `.snxprj` goes relative to the path the project was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum NewProjectLayout {
    /// A Save-As pick `<parent>/<stem>.snxprj`: the project is the folder
    /// `<parent>/<stem>/`, reusing a same-named empty `<parent>` (#1205).
    #[default]
    Picked,
    /// The dialog's "Create folder" on: always `<parent>/<stem>/<stem>.snxprj`.
    NamedFolder,
    /// The dialog's "Create folder" off: `<parent>/<stem>.snxprj` in a folder
    /// the user already made.
    InPlace,
}

/// What a New Project writes besides the blank project: where it goes and the
/// setup the dialog chose (#2232). The default is the plain pick.
#[derive(Debug, Clone, Default)]
pub(crate) struct NewProjectPlan {
    pub layout: NewProjectLayout,
    pub setup: Option<signex_types::project::ProjectSetup>,
}

/// Where a New Project lands: its folder, its `.snxprj`, and whether the
/// folder is already there (an empty one the user made).
pub(crate) struct NewProjectTarget {
    pub(crate) folder: PathBuf,
    pub(crate) project_path: PathBuf,
    folder_exists: bool,
    /// The user's same-named parent was not usable, so the project went into
    /// `<parent>/<stem>/` instead of the parent itself.
    nested_in_same_named_parent: bool,
}

/// Where a New Project chosen at `chosen` lands. Reads the disk, so it runs on
/// the blocking worker. Refuses a folder that is not empty, already holds a
/// project, or sits inside another project; never creates anything.
pub(crate) fn resolve_new_project(
    chosen: &Path,
    layout: NewProjectLayout,
) -> Result<NewProjectTarget> {
    resolve_new_project_with(chosen, &KnownFolders::from_system(), layout)
}

/// The `.snxprj` of project `stem` under `parent`: `<parent>/<stem>/<stem>.snxprj`
/// in its own folder, else `<parent>/<stem>.snxprj`. Reads no disk, so the
/// New Project dialog shows the path the resolver below lands on.
pub(crate) fn project_file_in(parent: &Path, stem: &str, own_folder: bool) -> PathBuf {
    let file = format!("{stem}.snxprj");
    if own_folder {
        parent.join(stem).join(file)
    } else {
        parent.join(file)
    }
}

fn resolve_in_place(parent: &Path, stem: &str, known: &KnownFolders) -> Result<NewProjectTarget> {
    if !parent.is_dir() {
        anyhow::bail!("{} is not a folder", parent.display());
    }
    if let Some(outer) = enclosing_project(parent, known) {
        anyhow::bail!(
            "{} is inside the project in {} — pick a location outside it",
            parent.display(),
            outer.display()
        );
    }
    // `enclosing_project` looks above the folder; the folder itself may be a
    // project already.
    if holds_a_project(parent)? {
        anyhow::bail!(
            "a project already exists in {} — open it instead of creating a new one over it",
            parent.display()
        );
    }
    let project_path = project_file_in(parent, stem, false);
    if project_path.exists() {
        anyhow::bail!("{} already exists", project_path.display());
    }
    Ok(NewProjectTarget {
        folder: parent.to_path_buf(),
        project_path,
        folder_exists: true,
        nested_in_same_named_parent: false,
    })
}

fn resolve_new_project_with(
    chosen: &Path,
    known: &KnownFolders,
    layout: NewProjectLayout,
) -> Result<NewProjectTarget> {
    let (parent, stem) = new_project_parts(chosen)?;
    if layout == NewProjectLayout::InPlace {
        return resolve_in_place(parent, &stem, known);
    }
    // ASCII case-insensitive on every volume: on a case-sensitive one this
    // only reuses a `foo` folder for project `Foo`, which is harmless.
    let parent_is_the_project_folder = layout == NewProjectLayout::Picked
        && parent
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(&stem));
    // The user already made `<stem>/` and picked inside it: use it as is
    // rather than nesting `<stem>/<stem>/`.
    if parent_is_the_project_folder && holds_a_project(parent)? {
        anyhow::bail!(
            "a project already exists in {} — open it instead of creating a new one over it",
            parent.display()
        );
    }
    let reuse_parent = parent_is_the_project_folder && is_unused(parent)?;
    let project_path = project_file_in(parent, &stem, !reuse_parent);
    let folder = project_path.parent().unwrap_or(parent).to_path_buf();
    if let Some(outer) = enclosing_project(&folder, known) {
        anyhow::bail!(
            "{} would be inside the project in {} — pick a location outside it",
            folder.display(),
            outer.display()
        );
    }
    let folder_exists = folder.exists();
    if folder_exists {
        if !folder.is_dir() {
            anyhow::bail!("{} exists and is not a folder", folder.display());
        }
        if holds_a_project(&folder)? {
            anyhow::bail!(
                "a project already exists in {} — open it instead of creating a new one over it",
                folder.display()
            );
        }
        if !is_unused(&folder)? {
            anyhow::bail!(
                "the folder {} is not empty — pick another name or location",
                folder.display()
            );
        }
    }
    Ok(NewProjectTarget {
        folder,
        project_path,
        folder_exists,
        nested_in_same_named_parent: parent_is_the_project_folder && !reuse_parent,
    })
}

fn entries_of(dir: &Path) -> Result<Vec<std::ffi::OsString>> {
    std::fs::read_dir(dir)
        .with_context(|| format!("read folder {}", dir.display()))?
        .map(|e| {
            e.map(|e| e.file_name())
                .with_context(|| format!("read folder {}", dir.display()))
        })
        .collect()
}

/// No entries other than OS clutter.
fn is_unused(dir: &Path) -> Result<bool> {
    Ok(entries_of(dir)?
        .iter()
        .all(|name| OS_CLUTTER.iter().any(|c| name.as_os_str() == *c)))
}

fn holds_a_project(dir: &Path) -> Result<bool> {
    Ok(entries_of(dir)?.iter().any(|name| {
        Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("snxprj"))
    }))
}

/// What one New Project call put on disk, so a failure can take it back.
#[derive(Default)]
struct Created {
    folder: bool,
    files: Vec<PathBuf>,
}

impl Created {
    /// Remove what this call created: its own files, then the folder with a
    /// non-recursive `remove_dir` when this call made it (it fails safely if
    /// something else landed there). Best effort; a leftover would make a
    /// retry refuse.
    fn roll_back(&self, dir: &Path) {
        for file in &self.files {
            if let Err(e) = std::fs::remove_file(file) {
                crate::diagnostics::log_warning(format!(
                    "[new project] could not remove {} after a failure: {e}",
                    file.display()
                ));
            }
        }
        if self.folder
            && let Err(e) = std::fs::remove_dir(dir)
        {
            crate::diagnostics::log_warning(format!(
                "[new project] could not remove {} after a failure: {e}",
                dir.display()
            ));
        }
    }
}

/// Create the project folder, the empty `.snxprj` marker (`parse_project` is
/// directory-driven and ignores its content) and the blank companion
/// `<stem>.snxsch`, then the managed `.gitignore` block (#1117). Returns the
/// `.snxprj` actually written. A failure removes what this call created.
/// Runs on a blocking worker, never in `update`.
#[cfg(test)]
pub(super) fn write_new_project_files(chosen: &Path, seed: &RuleSeed) -> Result<PathBuf> {
    write_new_project_files_with(chosen, seed, &NewProjectPlan::default())
}

/// The New Project writes for a dialog-made plan: the layout, and the
/// setup stored in the `.snxprj` and applied to the first sheet.
pub(crate) fn write_new_project_files_with(
    chosen: &Path,
    seed: &RuleSeed,
    plan: &NewProjectPlan,
) -> Result<PathBuf> {
    let (_, stem) = new_project_parts(chosen)?;
    let target = resolve_new_project(chosen, plan.layout)?;
    let mut created = Created::default();
    let result = write_into(&target, &stem, &mut created, plan.setup.as_ref(), &|dir| {
        signex_types::project_vcs_files::ensure_project_gitignore(dir)
    });
    if result.is_err() {
        created.roll_back(&target.folder);
    }
    if result.is_ok() {
        let tier = plan.setup.as_ref().and_then(|setup| setup.fab);
        seed_project_rules(&target.project_path, seed, tier);
    }
    if result.is_ok() && target.nested_in_same_named_parent {
        crate::diagnostics::log_info(format!(
            "[new project] {} was not empty, so the project was created in {}",
            target
                .folder
                .parent()
                .map_or_else(String::new, |p| p.display().to_string()),
            target.folder.display()
        ));
    }
    result.map(|()| target.project_path)
}

/// Where a new project's rule set comes from (#1608). Captured on the UI
/// thread; the worker reads the profile file.
#[derive(Debug, Clone, Default)]
pub(crate) struct RuleSeed {
    /// The profile library folder, `None` when the platform has none.
    pub dir: Option<PathBuf>,
    /// The profile the user chose as the default, if any.
    pub selected: Option<String>,
    /// Profile writes still queued when New Project was chosen. The worker runs
    /// them before it reads the library, so an edit made a moment ago is in
    /// the profile the new project copies.
    pub pending: Vec<crate::app::rule_profile_ui::Dispatch>,
    /// This machine's Annotate undo preference, stored in the new project as
    /// its own choice (#1692); `None` stores none.
    pub annotate_undo: Option<crate::annotate_undo::AnnotateUndo>,
    /// Preferences ▸ PCB Defaults, stored in the new project as its own
    /// dimension defaults (#2868); `None` stores none.
    pub dimension: Option<signex_types::pcb_dimension_style::DimensionDefaults>,
}

/// Store a copy of the default rule profile in the new `.snxprj`, with its
/// provenance. The project already exists, so a failure here is reported and
/// the project is left as created: it then uses the machine's rules until
/// the user stores some. A fab tier picked in the setup adds its `fab_*`
/// rules (#2311) and its min slot (mockup row 179d) to the copy.
fn seed_project_rules(
    project_path: &Path,
    seed: &RuleSeed,
    tier: Option<signex_types::project::FabTemplate>,
) {
    if let Some(dir) = seed.dir.as_deref()
        && let Err(why) = crate::app::rule_profile_ui::run_all(dir, &seed.pending)
    {
        crate::diagnostics::log_warning(format!(
            "[new project] a queued profile change was not saved first: {why}"
        ));
    }
    let (mut rules, mut notes) =
        crate::rule_profiles::rules_for_new_project(seed.dir.as_deref(), seed.selected.as_deref());
    if let Some(tier) = tier {
        rules.set_min_slot(Some(tier.capabilities().min_slot));
        match signex_drc_dsl::table::presets::with_tier(&rules.drc, tier) {
            Ok(drc) => rules.drc = drc,
            Err(why) => notes.push(format!(
                "the {} fab tier rules were not added, the profile's rule text has an error: {why}",
                tier.token()
            )),
        }
    }
    for note in notes {
        crate::diagnostics::log_warning(format!("[new project] {note}"));
    }
    let stored = signex_types::project::parse_project(project_path).and_then(|mut data| {
        data.rules = Some(rules);
        if let Some(choice) = seed.annotate_undo {
            data.set_annotate_undo(Some(choice));
        }
        if let Some(dimension) = &seed.dimension {
            data.project_defaults.dimension = Some(dimension.clone());
        }
        data.document_id
            .get_or_insert_with(signex_types::document_id::DocumentId::new);
        signex_types::project::write_project(project_path, &data)
    });
    if let Err(e) = stored {
        crate::diagnostics::log_warning(format!(
            "[new project] the default rule profile was not stored in {}: {e}",
            project_path.display()
        ));
    }
}

/// The first content of a dialog-made `.snxprj`: the project as the folder
/// shows it, with the chosen defaults and designator standard and its id,
/// and the picked fab tier's router tool (mockup row 179d). Part of the creation: a
/// failure takes the project back.
fn write_setup(project_path: &Path, setup: &signex_types::project::ProjectSetup) -> Result<()> {
    let mut data = signex_types::project::parse_project_bytes(project_path, b"")
        .with_context(|| format!("read the new project {}", project_path.display()))?;
    data.project_defaults = setup.defaults();
    if let Some(tier) = setup.fab {
        data.set_router_tool_diameter(Some(tier.capabilities().router_tool_diameter));
    }
    data.annotation = Some(signex_types::project::AnnotationSettings {
        designator_standard: Some(setup.designators),
        ..Default::default()
    });
    data.document_id
        .get_or_insert_with(signex_types::document_id::DocumentId::new);
    signex_types::project::write_project(project_path, &data)
        .with_context(|| format!("write project file {}", project_path.display()))
}

/// `ensure_gitignore` writes the managed `.gitignore` block; it is a
/// parameter so a failing writer can be tested.
fn write_into(
    target: &NewProjectTarget,
    stem: &str,
    created: &mut Created,
    setup: Option<&signex_types::project::ProjectSetup>,
    ensure_gitignore: &dyn Fn(
        &Path,
    ) -> Result<(), signex_types::project_vcs_files::ManagedFileError>,
) -> Result<()> {
    let dir = &target.folder;
    if !target.folder_exists {
        // `create_dir`, not `_all`: fail if the folder appeared since the
        // check instead of writing into someone else's folder.
        std::fs::create_dir(dir)
            .with_context(|| format!("create project directory {}", dir.display()))?;
        created.folder = true;
    }
    // `create_new` never truncates: a racing writer makes this fail rather
    // than clobber its file.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target.project_path)
        .with_context(|| format!("write project file {}", target.project_path.display()))?;
    created.files.push(target.project_path.clone());

    let sch_path = dir.join(format!("{stem}.snxsch"));
    let mut sheet = super::blank_schematic_sheet();
    if let Some(setup) = setup {
        setup.sheet.apply(&mut sheet);
    }
    let serialised = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .context("serialise blank schematic")?;
    // No-clobber publish; a schematic that appeared meanwhile is not ours.
    super::super::dock::project_navigation::add_io::create_noclobber(&sch_path, |f| {
        std::io::Write::write_all(f, serialised.as_bytes())
    })
    .map_err(|e| anyhow::anyhow!("write blank schematic {}: {e}", sch_path.display()))?;
    created.files.push(sch_path);
    if let Some(setup) = setup {
        write_setup(&target.project_path, setup)?;
    }
    // The project exists now; a `.gitignore` problem is reported but never
    // fails the creation.
    if let Err(e) = ensure_gitignore(dir) {
        crate::diagnostics::log_warning(format!("[git] .gitignore not updated: {e}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_gitignore_is_logged_and_the_project_is_still_created() {
        let _ = crate::diagnostics::init_logging();
        let tmp = tempfile::tempdir().unwrap();
        let chosen = tmp.path().join("GI.snxprj");
        let target =
            resolve_new_project_with(&chosen, &KnownFolders::default(), NewProjectLayout::Picked)
                .unwrap();
        let mut created = Created::default();

        let result = write_into(&target, "GI", &mut created, None, &|_| {
            Err(
                signex_types::project_vcs_files::ManagedFileError::NotRegularFile {
                    path: ".gitignore".into(),
                },
            )
        });

        assert!(result.is_ok(), "{result:?}");
        assert!(target.folder.join("GI.snxprj").is_file());
        assert!(target.folder.join("GI.snxsch").is_file());
        assert!(
            crate::diagnostics::recent_entries()
                .iter()
                .any(|e| e.message.starts_with("[git] .gitignore not updated")),
            "the failure must reach the Messages panel"
        );
    }

    fn library_with_company() -> (tempfile::TempDir, signex_types::project::RuleProfile) {
        let lib = tempfile::tempdir().unwrap();
        let mut erc = signex_types::project::ErcRules::default();
        erc.severity.insert("unused_pin".into(), "off".into());
        let profile = signex_types::project::RuleProfile::new("Company", "4", erc);
        std::fs::write(lib.path().join("company.snxrules"), profile.to_json_bytes()).unwrap();
        (lib, profile)
    }

    #[test]
    fn a_new_project_stores_a_copy_of_the_selected_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let (lib, profile) = library_with_company();
        let seed = RuleSeed {
            dir: Some(lib.path().to_path_buf()),
            selected: Some("Company".into()),
            pending: Vec::new(),
            annotate_undo: None,
            dimension: None,
        };

        let written = write_new_project_files(&tmp.path().join("Seeded.snxprj"), &seed).unwrap();
        let data = signex_types::project::parse_project(&written).unwrap();
        let rules = data.rules.expect("the new project stores rules");
        assert_eq!(rules.erc, profile.erc);
        let reference = rules.profile.expect("provenance");
        assert_eq!(
            (
                reference.name.as_str(),
                reference.source.as_str(),
                reference.version.as_str()
            ),
            ("Company", "user", "4")
        );
        assert_eq!(reference.sha256, profile.content_sha256());
        assert!(written.parent().unwrap().join("Seeded.snxsch").is_file());

        // A copy, not a link: editing the profile later leaves the project's
        // rules as they were, and the recorded hash shows the drift.
        let mut edited = profile.clone();
        edited
            .erc
            .severity
            .insert("unused_pin".into(), "error".into());
        std::fs::write(lib.path().join("company.snxrules"), edited.to_json_bytes()).unwrap();
        let again = signex_types::project::parse_project(&written).unwrap();
        assert_eq!(again.rules.unwrap().erc, profile.erc);
    }

    #[test]
    fn a_new_project_with_no_profile_chosen_gets_the_shipped_one() {
        let tmp = tempfile::tempdir().unwrap();
        let seed = RuleSeed {
            dir: Some(tmp.path().join("no-library-yet")),
            selected: None,
            pending: Vec::new(),
            annotate_undo: None,
            dimension: None,
        };
        let written = write_new_project_files(&tmp.path().join("Plain.snxprj"), &seed).unwrap();
        let rules = signex_types::project::parse_project(&written)
            .unwrap()
            .rules
            .expect("rules stored");
        let reference = rules.profile.expect("provenance");
        assert_eq!(
            (reference.name.as_str(), reference.source.as_str()),
            ("Signex Default", "builtin")
        );
        assert_eq!(
            rules.erc,
            signex_erc::project_rules::profile::builtin_profile().erc
        );
    }

    #[test]
    fn a_chosen_profile_that_is_gone_does_not_stop_the_project_being_created() {
        let _ = crate::diagnostics::init_logging();
        let tmp = tempfile::tempdir().unwrap();
        let lib = tempfile::tempdir().unwrap();
        let seed = RuleSeed {
            dir: Some(lib.path().to_path_buf()),
            selected: Some("Missing".into()),
            pending: Vec::new(),
            annotate_undo: None,
            dimension: None,
        };
        let written = write_new_project_files(&tmp.path().join("Fallback.snxprj"), &seed).unwrap();
        let rules = signex_types::project::parse_project(&written)
            .unwrap()
            .rules
            .unwrap();
        assert_eq!(rules.profile.unwrap().name, "Signex Default");
        assert!(
            crate::diagnostics::recent_entries().iter().any(
                |e| e.message.contains("\"Missing\"") && e.message.starts_with("[new project]")
            ),
            "the fallback must reach the Messages panel"
        );
    }

    #[test]
    fn a_new_project_copies_the_machine_rules_including_an_edit_still_queued() {
        use crate::app::rule_profile_ui::{ProfileWriteQueue, QueuedWrite};
        let tmp = tempfile::tempdir().unwrap();
        let lib = tempfile::tempdir().unwrap();
        // The machine's rules on disk, then an edit that has not been written.
        crate::rule_profiles::write_user_default(lib.path(), {
            let mut erc = signex_types::project::ErcRules::default();
            erc.severity.insert("unused_pin".into(), "error".into());
            erc
        })
        .unwrap();
        let mut queue = ProfileWriteQueue::default();
        let mut edited = signex_types::project::ErcRules::default();
        edited.severity.insert("unused_pin".into(), "off".into());
        let dispatch = queue
            .submit("User Default", QueuedWrite::UserDefault(edited))
            .unwrap();
        let seed = RuleSeed {
            dir: Some(lib.path().to_path_buf()),
            selected: None,
            pending: queue.pending(),
            annotate_undo: None,
            dimension: None,
        };

        let written = write_new_project_files(&tmp.path().join("Queued.snxprj"), &seed).unwrap();
        let rules = signex_types::project::parse_project(&written)
            .unwrap()
            .rules
            .expect("rules stored");
        assert_eq!(
            rules.erc.severity["unused_pin"], "off",
            "the queued edit is in the copy"
        );

        // The queue's own worker then finds its write already done.
        assert_eq!(
            dispatch.run(lib.path()).unwrap(),
            crate::app::rule_profile_ui::WriteOutcome::Skipped
        );
    }

    #[test]
    fn a_rollback_removes_our_files_and_the_folder_this_call_created() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P");
        std::fs::create_dir(&dir).unwrap();
        let ours = dir.join("P.snxprj");
        std::fs::write(&ours, b"").unwrap();
        let created = Created {
            folder: true,
            files: vec![ours],
        };
        created.roll_back(&dir);
        assert!(!dir.exists());
    }

    #[test]
    fn a_rollback_never_deletes_a_foreign_file_that_appeared() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P");
        std::fs::create_dir(&dir).unwrap();
        let ours = dir.join("P.snxprj");
        std::fs::write(&ours, b"").unwrap();
        std::fs::write(dir.join("notes.txt"), b"theirs").unwrap();
        let created = Created {
            folder: true,
            files: vec![ours.clone()],
        };
        created.roll_back(&dir);
        assert!(!ours.exists());
        assert_eq!(std::fs::read(dir.join("notes.txt")).unwrap(), b"theirs");
    }

    #[test]
    fn a_rollback_keeps_a_folder_the_user_made() {
        let tmp = tempfile::tempdir().unwrap();
        let ours = tmp.path().join("P.snxprj");
        std::fs::write(&ours, b"").unwrap();
        std::fs::write(tmp.path().join(".DS_Store"), b"x").unwrap();
        let created = Created {
            folder: false,
            files: vec![ours.clone()],
        };
        created.roll_back(tmp.path());
        assert!(!ours.exists());
        assert!(tmp.path().join(".DS_Store").exists());
    }

    #[test]
    fn a_loose_project_on_the_desktop_does_not_block_new_projects_there() {
        let home = tempfile::tempdir().unwrap();
        let desktop = home.path().join("Desktop");
        std::fs::create_dir_all(&desktop).unwrap();
        std::fs::write(desktop.join("Untitled.snxprj"), b"").unwrap();
        let known = KnownFolders {
            home: Some(home.path().to_path_buf()),
            shared: vec![desktop.clone()],
        };
        for chosen in [
            desktop.join("Foo.snxprj"),
            desktop.join("work").join("X.snxprj"),
        ] {
            let target =
                resolve_new_project_with(&chosen, &known, NewProjectLayout::Picked).unwrap();
            assert!(target.folder.ends_with(chosen.file_stem().unwrap()));
        }
    }

    #[test]
    fn without_its_own_folder_a_project_is_not_created_in_a_project_folder() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Old.snxprj"), b"").unwrap();
        let err = resolve_new_project_with(
            &tmp.path().join("New.snxprj"),
            &KnownFolders::default(),
            NewProjectLayout::InPlace,
        );
        assert!(
            err.is_err_and(|e| e.to_string().contains("a project already exists")),
            "the location is itself a project"
        );
    }

    #[test]
    fn the_setup_is_in_the_first_write_of_the_project_file() {
        let tmp = tempfile::tempdir().unwrap();
        let target = resolve_new_project_with(
            &tmp.path().join("First.snxprj"),
            &KnownFolders::default(),
            NewProjectLayout::NamedFolder,
        )
        .unwrap();
        let mut setup = signex_types::project::ProjectSetup::default();
        setup.set_layers(6);
        let mut created = Created::default();
        write_into(&target, "First", &mut created, Some(&setup), &|_| Ok(())).unwrap();
        let data = signex_types::project::parse_project(&target.project_path).unwrap();
        assert_eq!(data.project_defaults, setup.defaults());
        assert!(data.designator_standard().is_some());
        assert!(
            data.rules.is_none(),
            "the rules are added after, best effort"
        );
    }

    #[test]
    fn a_project_folder_still_blocks_a_new_project_inside_it() {
        let tmp = tempfile::tempdir().unwrap();
        let alpha = tmp.path().join("Alpha");
        std::fs::create_dir(&alpha).unwrap();
        std::fs::write(alpha.join("Alpha.snxprj"), b"").unwrap();
        let err = resolve_new_project_with(
            &alpha.join("Beta.snxprj"),
            &KnownFolders::default(),
            NewProjectLayout::Picked,
        );
        assert!(err.is_err_and(|e| e.to_string().contains("inside the project")));
    }

    /// #1692: this machine's Annotate undo preference seeds a new project as
    /// the project's own choice.
    #[test]
    fn a_new_project_stores_the_annotate_undo_preference() {
        let tmp = tempfile::tempdir().unwrap();
        let seed = RuleSeed {
            dir: Some(tmp.path().join("no-library-yet")),
            selected: None,
            pending: Vec::new(),
            annotate_undo: Some(crate::annotate_undo::AnnotateUndo::PerSheet),
            dimension: None,
        };
        let written = write_new_project_files(&tmp.path().join("Seeded.snxprj"), &seed).unwrap();
        let data = signex_types::project::parse_project(&written).unwrap();
        assert_eq!(
            data.annotate_undo(),
            Some(crate::annotate_undo::AnnotateUndo::PerSheet)
        );
    }

    /// #2868 (Caner 2026-11-24): a new project stores the Preferences
    /// dimension defaults as its own, with or without a dialog setup.
    #[test]
    fn a_new_project_stores_the_preferences_dimension_defaults() {
        use signex_types::pcb_dimension_style::{ArrowStyle, DimensionDefaults};
        use signex_types::project::ProjectSetup;
        let tmp = tempfile::tempdir().unwrap();
        let mut dimension = DimensionDefaults::default();
        dimension.style.arrow_style = ArrowStyle::Filled;
        dimension.precision = 3;
        let seed = RuleSeed {
            dir: Some(tmp.path().join("no-library-yet")),
            dimension: Some(dimension.clone()),
            ..RuleSeed::default()
        };
        let plain = write_new_project_files(&tmp.path().join("Plain.snxprj"), &seed).unwrap();
        let data = signex_types::project::parse_project(&plain).unwrap();
        assert_eq!(data.project_defaults.dimension, Some(dimension.clone()));

        let plan = NewProjectPlan {
            layout: NewProjectLayout::NamedFolder,
            setup: Some(ProjectSetup::default()),
        };
        let other = tempfile::tempdir().unwrap();
        let chosen = other.path().join("Setup.snxprj");
        let written = write_new_project_files_with(&chosen, &seed, &plan).unwrap();
        let data = signex_types::project::parse_project(&written).unwrap();
        assert_eq!(data.project_defaults.dimension, Some(dimension));
        assert!(data.project_defaults.pcb.is_some(), "the setup is kept");
    }

    /// #2311: a fab tier picked in the setup adds its `fab_*` rules to the
    /// stored rules, beside the profile's, and they read back as written.
    #[test]
    fn a_new_project_with_a_fab_tier_stores_its_fab_rules() {
        use signex_types::project::{FabTemplate, ProjectSetup};
        let tmp = tempfile::tempdir().unwrap();
        let seed = RuleSeed {
            dir: Some(tmp.path().join("no-library-yet")),
            ..RuleSeed::default()
        };
        let mut setup = ProjectSetup::default();
        setup.apply_fab(FabTemplate::Advanced);
        let plan = NewProjectPlan {
            layout: NewProjectLayout::NamedFolder,
            setup: Some(setup),
        };
        let written =
            write_new_project_files_with(&tmp.path().join("Fab.snxprj"), &seed, &plan).unwrap();
        let data = signex_types::project::parse_project(&written).unwrap();
        let rules = data.rules.clone().expect("rules stored");
        let shipped = signex_erc::project_rules::profile::builtin_profile();
        assert_eq!(rules.erc, shipped.erc, "the profile's rules are kept");
        let expected =
            signex_drc_dsl::table::presets::with_tier(&shipped.drc, FabTemplate::Advanced).unwrap();
        assert_eq!(rules.drc, expected);
        let dsl = rules.drc.dsl.unwrap_or_default();
        assert!(
            dsl.contains("rule fab_clearance on clearance require clearance >= 0.1mm then error"),
            "{dsl}"
        );
        // Saved again, the project reads back the same.
        signex_types::project::write_project(&written, &data).unwrap();
        let again = signex_types::project::parse_project(&written).unwrap();
        assert_eq!(again.rules, data.rules);

        // No tier picked: no fab rule.
        let plain = NewProjectPlan {
            layout: NewProjectLayout::NamedFolder,
            setup: Some(ProjectSetup::default()),
        };
        let written =
            write_new_project_files_with(&tmp.path().join("Plain.snxprj"), &seed, &plain).unwrap();
        let rules = signex_types::project::parse_project(&written)
            .unwrap()
            .rules;
        let dsl = rules.and_then(|r| r.drc.dsl).unwrap_or_default();
        assert!(!dsl.contains("fab_"), "{dsl}");
    }

    /// mockup row 179d: a fab tier picked in the setup stores its router tool in the
    /// project; no tier stores none.
    #[test]
    fn a_new_project_with_a_fab_tier_stores_its_router_tool() {
        use signex_types::project::{FabTemplate, ProjectSetup};
        let tmp = tempfile::tempdir().unwrap();
        let seed = RuleSeed {
            dir: Some(tmp.path().join("no-library-yet")),
            ..RuleSeed::default()
        };
        for (tier, diameter) in [
            (FabTemplate::Standard, 2.0),
            (FabTemplate::Advanced, 1.6),
            (FabTemplate::Hdi, 1.0),
        ] {
            let mut setup = ProjectSetup::default();
            setup.apply_fab(tier);
            let plan = NewProjectPlan {
                layout: NewProjectLayout::NamedFolder,
                setup: Some(setup),
            };
            let name = format!("{tier:?}.snxprj");
            let written =
                write_new_project_files_with(&tmp.path().join(name), &seed, &plan).unwrap();
            let data = signex_types::project::parse_project(&written).unwrap();
            assert_eq!(data.router_tool_diameter(), Some(diameter), "{tier:?}");
        }
        let plain = NewProjectPlan {
            layout: NewProjectLayout::NamedFolder,
            setup: Some(ProjectSetup::default()),
        };
        let written =
            write_new_project_files_with(&tmp.path().join("Plain.snxprj"), &seed, &plain).unwrap();
        let data = signex_types::project::parse_project(&written).unwrap();
        assert_eq!(data.router_tool_diameter(), None);
    }

    /// mockup row 179d: a fab tier picked in the setup stores its min slot
    /// with the rules, at rules schema 2; no tier stores none.
    #[test]
    fn a_new_project_with_a_fab_tier_stores_its_min_slot_with_the_rules() {
        use signex_types::project::{FabTemplate, ProjectSetup, RULES_SCHEMA_V2};
        let tmp = tempfile::tempdir().unwrap();
        let seed = RuleSeed {
            dir: Some(tmp.path().join("no-library-yet")),
            ..RuleSeed::default()
        };
        for (tier, min_slot) in [
            (FabTemplate::Standard, 0.8),
            (FabTemplate::Advanced, 0.6),
            (FabTemplate::Hdi, 0.5),
        ] {
            let mut setup = ProjectSetup::default();
            setup.apply_fab(tier);
            let plan = NewProjectPlan {
                layout: NewProjectLayout::NamedFolder,
                setup: Some(setup),
            };
            let name = format!("Slot{tier:?}.snxprj");
            let written =
                write_new_project_files_with(&tmp.path().join(name), &seed, &plan).unwrap();
            let data = signex_types::project::parse_project(&written).unwrap();
            let rules = data.rules.expect("rules stored");
            assert_eq!(rules.min_slot(), Some(min_slot), "{tier:?}");
            assert_eq!(rules.schema, Some(RULES_SCHEMA_V2), "{tier:?}");
        }
        let plain = NewProjectPlan {
            layout: NewProjectLayout::NamedFolder,
            setup: Some(ProjectSetup::default()),
        };
        let written =
            write_new_project_files_with(&tmp.path().join("SlotPlain.snxprj"), &seed, &plain)
                .unwrap();
        let data = signex_types::project::parse_project(&written).unwrap();
        assert_eq!(data.rules.and_then(|rules| rules.min_slot()), None);
    }
}
