//! Where a run works (#1924): through a running Signex that has the project
//! open, or headless, holding the project for the run. The registry is read
//! with the checks of `signex_app::headless::agent_client`; this only decides.

use std::path::{Path, PathBuf};

use signex_app::headless::agent_client::{self, GuiPeer, Hold, Lookup};

use crate::cli::Route;

pub enum Placement {
    /// Send the commands to this running app.
    Attach(GuiPeer),
    /// Run here. The hold is the project's registry entry and lock for the
    /// run, released when it drops. A headless run never goes without one.
    Headless(Hold),
}

/// Decide for `project` (canonical) from the registry of the user's runtime
/// directory.
pub fn place(route: Route, project: &Path) -> Result<Placement, String> {
    place_in(route, agent_client::runtime_dir(), project)
}

pub(crate) fn place_in(
    route: Route,
    dir: Result<PathBuf, String>,
    project: &Path,
) -> Result<Placement, String> {
    let name = project.display();
    let no_gui = |why: String| format!("--attach: no running Signex has {name} open ({why})");
    // What cannot be read cannot be trusted: a run that cannot tell who holds
    // the project does not go on without holding it.
    let (dir, found) = dir
        .and_then(|dir| agent_client::lookup(&dir, project).map(|found| (dir, found)))
        .map_err(|why| {
            if route == Route::Attach {
                no_gui(why)
            } else {
                format!("cannot look for a running Signex or a run that holds {name}: {why}")
            }
        })?;
    match found {
        Lookup::Gui(peer) if route == Route::Headless => Err(format!(
            "--headless: {name} is open in a running Signex (process {}); refusing to edit a \
             file that is open in a window. Leave out --headless to work through it",
            peer.pid
        )),
        Lookup::Gui(peer) => Ok(Placement::Attach(peer)),
        Lookup::GuiNotListening { pid } => Err(not_listening(&name, pid)),
        Lookup::Headless { pid } if route == Route::Attach => {
            Err(no_gui(format!("a headless run, process {pid}, holds it")))
        }
        Lookup::Headless { pid } => Err(format!(
            "the project is held by another signex-cli run (pid {pid}): {name}"
        )),
        found => {
            if let Lookup::Stale(stale) = found
                && let Err(why) = stale.remove()
            {
                eprintln!("[signex-cli] a stale entry for {name} was left in place: {why}");
            }
            if route == Route::Attach {
                return Err(no_gui("no live entry".into()));
            }
            held(route, &dir, project)
        }
    }
}

/// A Signex has the project open and does not listen: the run does not edit
/// the same files behind it, for any route.
fn not_listening(project: &impl std::fmt::Display, pid: u32) -> String {
    format!(
        "{project} is open in Signex (pid {pid}). To let an agent work on it, turn on \
         Preferences > Allow agent connections."
    )
}

/// Take the hold. If that fails under `Auto` because a running Signex
/// registered the project in between, attach to it; otherwise fail.
fn held(route: Route, dir: &Path, project: &Path) -> Result<Placement, String> {
    match agent_client::hold(dir, project) {
        Ok(hold) => Ok(Placement::Headless(hold)),
        Err(why) => match agent_client::lookup(dir, project) {
            Ok(Lookup::Gui(peer)) if route == Route::Auto => Ok(Placement::Attach(peer)),
            Ok(Lookup::GuiNotListening { pid }) => Err(not_listening(&project.display(), pid)),
            _ => Err(format!("cannot hold {}: {why}", project.display())),
        },
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;
    use std::os::unix::net::UnixListener;

    use super::*;

    /// A running app registered the project between the lookup and the hold:
    /// the hold is refused, and `Auto` attaches instead of going on without
    /// one. A forced `--headless` fails.
    #[test]
    fn a_hold_refused_for_a_running_app_attaches_under_auto_and_fails_otherwise() {
        let root = tempfile::Builder::new()
            .prefix("s")
            .tempdir_in("/tmp")
            .unwrap();
        let dir = root.path().join("signex-agent");
        for folder in [dir.clone(), dir.join("registry")] {
            std::fs::create_dir(&folder).unwrap();
            std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let project = root.path().join("P.snxprj");
        std::fs::write(&project, "{}").unwrap();
        let canonical = std::fs::canonicalize(&project).unwrap();
        let boot = "cccccccccccccccc";
        let socket = dir.join(format!("{boot}.sock"));
        let _listener = UnixListener::bind(&socket).unwrap();
        let entry = serde_json::json!({"format": 2, "kind": "gui", "project": canonical,
            "pid": std::process::id(), "socket": socket, "boot_id": boot});
        let file = agent_client::entry_path(&dir, &project).unwrap();
        std::fs::write(file, entry.to_string()).unwrap();

        assert!(matches!(
            held(Route::Auto, &dir, &project),
            Ok(Placement::Attach(_))
        ));
        let why = held(Route::Headless, &dir, &project).err().unwrap();
        assert!(why.contains("cannot hold"), "{why}");
    }
}
