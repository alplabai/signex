//! What a run may do (#1923): checked on every command before the app sees
//! it. Pure: nothing here touches the app, and nothing runs.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use signex_app::keymap::{AppCommandId, CommandMetadata, OperandKind, metadata_for};

use crate::capability::{Capability, capability_of};
use crate::catalog::GUI_ONLY_REASON;
use crate::confine::confine;
use crate::input::Limits;
use crate::protocol::Request;

#[derive(Debug, Clone)]
pub struct Policy {
    pub capabilities: BTreeSet<Capability>,
    /// Folders outside the project a run may name (`--allow-path`), resolved.
    pub allow_paths: Vec<PathBuf>,
    pub limits: Limits,
}

/// A request that passed every check: what to dispatch, with each path
/// operand replaced by the resolved path that was checked.
#[derive(Debug)]
pub struct Approved {
    pub command: AppCommandId,
    pub meta: &'static CommandMetadata,
    pub operands: Vec<(String, String)>,
}

impl Policy {
    /// Why `request` may not run, or what to run. `base` is the project
    /// folder a relative path is read against; `roots` are the folders a
    /// path may sit in (the project and its mounted libraries).
    pub fn authorize(
        &self,
        request: &Request,
        base: &Path,
        roots: &[PathBuf],
    ) -> Result<Approved, String> {
        let command = AppCommandId::new(request.id.as_str())?;
        let meta = metadata_for(&command)
            .ok_or_else(|| format!("unknown command `{}`", command.as_str()))?;
        if meta.flags.gui_only {
            return Err(format!("`{}` is {GUI_ONLY_REASON}", command.as_str()));
        }
        let needed = capability_of(meta);
        if !self.capabilities.contains(&needed) {
            return Err(needs_capability(command.as_str(), needed));
        }
        let allowed = self.allowed(roots);
        let writes = needed.writes_files();
        let operands = request
            .operands
            .iter()
            .map(|(name, text)| {
                if !is_path_operand(meta, name) {
                    return Ok((name.clone(), text.clone()));
                }
                let checked = self
                    .check_path(base, text, &allowed, writes)
                    .map_err(|why| format!("operand `{name}`: {why}"))?;
                Ok((name.clone(), checked))
            })
            .collect::<Result<_, String>>()?;
        Ok(Approved {
            command,
            meta,
            operands,
        })
    }

    /// The check again, right before the command is handed to the app: each
    /// path operand must still resolve to the very path that was approved
    /// (a link swapped in since then changes it, so a write target that was
    /// a plain file stays one). A same-user process racing symlinks between this
    /// check and the app's own open or write is out of scope (docs/CLI.md).
    pub fn recheck(&self, approved: &Approved, roots: &[PathBuf]) -> Result<(), String> {
        let allowed = self.allowed(roots);
        for (name, text) in &approved.operands {
            if !is_path_operand(approved.meta, name) {
                continue;
            }
            let now = confine(Path::new(text), &allowed)
                .map_err(|why| format!("operand `{name}`: {why}"))?;
            if now != Path::new(text) {
                return Err(format!(
                    "operand `{name}`: {text} now resolves to {}; it changed while the command was prepared",
                    now.display()
                ));
            }
        }
        Ok(())
    }

    /// The files a write command will write without being told (the active
    /// document, a dirty one, a project file): each must be a plain file or
    /// a new file inside the allowed folders, never a link and never one that
    /// resolves outside, exactly as for a path operand.
    pub fn check_write_targets(
        &self,
        targets: &[PathBuf],
        roots: &[PathBuf],
    ) -> Result<(), String> {
        let allowed = self.allowed(roots);
        for target in targets {
            refuse_link(target)
                .and_then(|()| confine(target, &allowed).map(|_| ()))
                .map_err(|why| format!("it would write {}: {why}", target.display()))?;
        }
        Ok(())
    }

    fn allowed(&self, roots: &[PathBuf]) -> Vec<PathBuf> {
        roots.iter().chain(&self.allow_paths).cloned().collect()
    }

    fn check_path(
        &self,
        base: &Path,
        text: &str,
        allowed: &[PathBuf],
        writes: bool,
    ) -> Result<String, String> {
        let given = Path::new(text);
        let joined = if given.is_absolute() {
            given.to_path_buf()
        } else {
            base.join(given)
        };
        if writes {
            refuse_link(&joined)?;
        }
        let resolved = confine(&joined, allowed)?;
        // A file that is not there yet is a write target; only a read is sized.
        if let Ok(meta) = fs::metadata(&resolved)
            && meta.is_file()
            && meta.len() > self.limits.max_file_bytes
        {
            return Err(format!(
                "{} is {} bytes, over the limit of {} (--max-file-bytes)",
                resolved.display(),
                meta.len(),
                self.limits.max_file_bytes
            ));
        }
        resolved
            .to_str()
            .map(str::to_string)
            .ok_or_else(|| format!("{} is not valid UTF-8", resolved.display()))
    }
}

fn is_path_operand(meta: &CommandMetadata, name: &str) -> bool {
    meta.operands
        .iter()
        .any(|operand| operand.name == name && operand.kind == OperandKind::Path)
}

/// A write goes to a real file: through a link it could land anywhere the
/// link is later pointed.
fn refuse_link(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(format!(
            "{} is a symbolic link; writing through a link is refused",
            path.display()
        )),
        _ => Ok(()),
    }
}

/// The refusal for a command whose capability the run does not hold.
pub fn needs_capability(command: &str, needed: Capability) -> String {
    let how = if needed == Capability::Save {
        "pass --save".to_string()
    } else {
        format!("pass --allow {needed}")
    };
    format!("`{command}` needs the `{needed}` capability; {how}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confine::canonical;
    use crate::protocol::request_from_args;

    struct Fixture {
        _guard: tempfile::TempDir,
        root: PathBuf,
    }

    fn fixture() -> Fixture {
        let guard = tempfile::tempdir().expect("temp dir");
        let root = canonical(guard.path()).expect("canonical");
        Fixture {
            _guard: guard,
            root,
        }
    }

    fn policy(caps: &[Capability]) -> Policy {
        Policy {
            capabilities: caps.iter().copied().collect(),
            allow_paths: Vec::new(),
            limits: Limits::default(),
        }
    }

    fn request(id: &str, args: &[&str]) -> Request {
        let args: Vec<String> = args.iter().map(ToString::to_string).collect();
        request_from_args(id, &args).unwrap()
    }

    fn authorize(policy: &Policy, fx: &Fixture, request: &Request) -> Result<Approved, String> {
        policy.authorize(request, &fx.root, std::slice::from_ref(&fx.root))
    }

    const DEFAULT: [Capability; 2] = Capability::DEFAULT;

    #[test]
    fn an_edit_passes_the_default_policy_and_a_save_names_its_capability() {
        let fx = fixture();
        let ok = authorize(&policy(&DEFAULT), &fx, &request("undo", &[])).unwrap();
        assert_eq!(ok.command.as_str(), "undo");
        let err = authorize(&policy(&DEFAULT), &fx, &request("save_document", &[])).unwrap_err();
        assert_eq!(
            err,
            "`save_document` needs the `save` capability; pass --save"
        );
        let err =
            authorize(&policy(&DEFAULT), &fx, &request("report_manager_bom", &[])).unwrap_err();
        assert_eq!(
            err,
            "`report_manager_bom` needs the `export` capability; pass --allow export"
        );
        let held = policy(&[Capability::Read, Capability::Edit, Capability::Save]);
        assert!(authorize(&held, &fx, &request("save_document", &[])).is_ok());
    }

    #[test]
    fn an_edit_needs_edit_and_a_read_does_not() {
        let fx = fixture();
        let read_only = policy(&[Capability::Read]);
        let err = authorize(&read_only, &fx, &request("undo", &[])).unwrap_err();
        assert!(err.contains("needs the `edit` capability"), "{err}");
        assert!(authorize(&read_only, &fx, &request("select_all", &[])).is_ok());
    }

    #[test]
    fn unknown_malformed_and_gui_only_commands_are_refused_with_the_reason() {
        let fx = fixture();
        let all = policy(&Capability::ALL);
        let unknown = authorize(&all, &fx, &request("zq_no_such_command", &[])).unwrap_err();
        assert_eq!(unknown, "unknown command `zq_no_such_command`");
        let bad = authorize(&all, &fx, &request("Not An Id", &[])).unwrap_err();
        assert!(bad.contains("snake_case"), "{bad}");
        let gui = authorize(&all, &fx, &request("toggle_properties_panel", &[])).unwrap_err();
        assert!(gui.contains(GUI_ONLY_REASON), "{gui}");
    }

    #[test]
    fn a_path_operand_is_resolved_against_the_project_and_confined() {
        let fx = fixture();
        let all = policy(&Capability::ALL);
        std::fs::write(fx.root.join("a.snxsch"), "x").unwrap();
        let relative = authorize(&all, &fx, &request("open_document", &["path=a.snxsch"])).unwrap();
        let want = fx.root.join("a.snxsch").to_str().unwrap().to_string();
        assert_eq!(relative.operands, [("path".to_string(), want)]);

        let other = fixture();
        let outside = format!("path={}", other.root.join("b.snxsch").display());
        let err = authorize(&all, &fx, &request("open_document", &[&outside])).unwrap_err();
        assert!(
            err.starts_with("operand `path`:") && err.contains("--allow-path"),
            "{err}"
        );
        let climbing = request("open_document", &["path=../elsewhere.snxsch"]);
        assert!(authorize(&all, &fx, &climbing).is_err());
    }

    #[test]
    fn allow_path_opens_one_more_folder_and_no_other() {
        let fx = fixture();
        let other = fixture();
        let third = fixture();
        let mut all = policy(&Capability::ALL);
        all.allow_paths.push(other.root.clone());
        let allowed = format!("path={}", other.root.join("b.snxsch").display());
        assert!(authorize(&all, &fx, &request("save_document_as", &[&allowed])).is_ok());
        let denied = format!("path={}", third.root.join("c.snxsch").display());
        assert!(authorize(&all, &fx, &request("save_document_as", &[&denied])).is_err());
    }

    #[test]
    fn a_file_over_the_size_limit_is_refused_but_a_missing_one_is_a_write_target() {
        let fx = fixture();
        let mut all = policy(&Capability::ALL);
        all.limits.max_file_bytes = 3;
        std::fs::write(fx.root.join("big.snxsch"), "12345").unwrap();
        std::fs::write(fx.root.join("small.snxsch"), "123").unwrap();
        let err =
            authorize(&all, &fx, &request("open_document", &["path=big.snxsch"])).unwrap_err();
        assert!(err.contains("5 bytes, over the limit of 3"), "{err}");
        assert!(authorize(&all, &fx, &request("open_document", &["path=small.snxsch"])).is_ok());
        assert!(
            authorize(
                &all,
                &fx,
                &request("save_document_as", &["path=new.snxsch"])
            )
            .is_ok()
        );
    }

    #[test]
    fn an_operand_that_is_not_a_path_passes_through_unchanged() {
        let fx = fixture();
        let ok = authorize(
            &policy(&DEFAULT),
            &fx,
            &request("place_net_label", &["at=1,2", "text=../../etc"]),
        )
        .unwrap();
        assert_eq!(
            ok.operands,
            [
                ("at".to_string(), "1,2".to_string()),
                ("text".to_string(), "../../etc".to_string())
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_project_is_refused_and_a_link_is_never_a_write_target() {
        let fx = fixture();
        let outside = fixture();
        let all = policy(&Capability::ALL);
        std::fs::write(outside.root.join("x.snxsch"), "x").unwrap();
        std::fs::write(fx.root.join("real.snxsch"), "x").unwrap();
        std::os::unix::fs::symlink(outside.root.join("x.snxsch"), fx.root.join("out.snxsch"))
            .unwrap();
        std::os::unix::fs::symlink(fx.root.join("real.snxsch"), fx.root.join("in.snxsch")).unwrap();

        for id in ["open_document", "save_document_as"] {
            let err = authorize(&all, &fx, &request(id, &["path=out.snxsch"])).unwrap_err();
            assert!(
                err.contains("outside the project") || err.contains("symbolic link"),
                "{err}"
            );
        }
        // A link that stays inside is read through, but never written through.
        assert!(authorize(&all, &fx, &request("open_document", &["path=in.snxsch"])).is_ok());
        let err =
            authorize(&all, &fx, &request("save_document_as", &["path=in.snxsch"])).unwrap_err();
        assert!(err.contains("writing through a link is refused"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_path_swapped_for_a_link_after_approval_is_caught_by_the_recheck() {
        let fx = fixture();
        let outside = fixture();
        let all = policy(&Capability::ALL);
        let roots = std::slice::from_ref(&fx.root);
        let target = fx.root.join("a.snxsch");
        std::fs::write(&target, "x").unwrap();
        std::fs::write(outside.root.join("secret.snxsch"), "x").unwrap();

        let read = authorize(&all, &fx, &request("open_document", &["path=a.snxsch"])).unwrap();
        assert!(
            all.recheck(&read, roots).is_ok(),
            "unchanged: still approved"
        );
        let write = authorize(&all, &fx, &request("save_document_as", &["path=a.snxsch"])).unwrap();
        assert!(all.recheck(&write, roots).is_ok());

        // The file is replaced by a link to somewhere else.
        std::fs::remove_file(&target).unwrap();
        std::os::unix::fs::symlink(outside.root.join("secret.snxsch"), &target).unwrap();
        let err = all.recheck(&read, roots).unwrap_err();
        assert!(err.contains("outside the project"), "{err}");
        assert!(all.recheck(&write, roots).is_err());

        // A link that stays inside changes the resolved path, so it is caught too.
        std::fs::remove_file(&target).unwrap();
        std::fs::write(fx.root.join("b.snxsch"), "x").unwrap();
        std::os::unix::fs::symlink(fx.root.join("b.snxsch"), &target).unwrap();
        let err = all.recheck(&read, roots).unwrap_err();
        assert!(
            err.contains("changed while the command was prepared"),
            "{err}"
        );
    }

    #[test]
    fn implicit_write_targets_inside_the_roots_pass_and_outside_ones_are_refused() {
        let fx = fixture();
        let outside = fixture();
        let all = policy(&Capability::ALL);
        let roots = std::slice::from_ref(&fx.root);
        let inside = fx.root.join("a.snxsch");
        std::fs::write(&inside, "x").unwrap();
        let fresh = fx.root.join("new.snxsch");
        assert!(all.check_write_targets(&[inside, fresh], roots).is_ok());
        assert!(all.check_write_targets(&[], roots).is_ok());

        let err = all
            .check_write_targets(&[outside.root.join("b.snxsch")], roots)
            .unwrap_err();
        assert!(
            err.starts_with("it would write") && err.contains("outside the project"),
            "{err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_implicit_write_target_that_is_a_link_is_refused_even_inside_the_roots() {
        let fx = fixture();
        let outside = fixture();
        let all = policy(&Capability::ALL);
        let roots = std::slice::from_ref(&fx.root);
        std::fs::write(fx.root.join("real.snxsch"), "x").unwrap();
        std::fs::write(outside.root.join("x.snxsch"), "x").unwrap();
        let to_inside = fx.root.join("in.snxsch");
        let to_outside = fx.root.join("out.snxsch");
        std::os::unix::fs::symlink(fx.root.join("real.snxsch"), &to_inside).unwrap();
        std::os::unix::fs::symlink(outside.root.join("x.snxsch"), &to_outside).unwrap();
        for link in [to_inside, to_outside] {
            let err = all.check_write_targets(&[link], roots).unwrap_err();
            assert!(err.contains("writing through a link is refused"), "{err}");
        }
    }

    #[test]
    fn the_recheck_passes_an_approval_with_no_path_operand() {
        let fx = fixture();
        let all = policy(&Capability::ALL);
        let approved = authorize(&all, &fx, &request("undo", &[])).unwrap();
        assert!(
            all.recheck(&approved, std::slice::from_ref(&fx.root))
                .is_ok()
        );
    }
}
