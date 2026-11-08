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
        let allowed: Vec<PathBuf> = roots.iter().chain(&self.allow_paths).cloned().collect();
        let operands = request
            .operands
            .iter()
            .map(|(name, text)| {
                let is_path = meta
                    .operands
                    .iter()
                    .any(|operand| operand.name == name && operand.kind == OperandKind::Path);
                if !is_path {
                    return Ok((name.clone(), text.clone()));
                }
                let checked = self
                    .check_path(base, text, &allowed)
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

    fn check_path(&self, base: &Path, text: &str, allowed: &[PathBuf]) -> Result<String, String> {
        let given = Path::new(text);
        let resolved = if given.is_absolute() {
            confine(given, allowed)?
        } else {
            confine(&base.join(given), allowed)?
        };
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
}
