//! Where a run works (#1924): through a running Signex that has the project
//! open, or headless, holding the project for the run. The registry is read
//! with the checks of `signex_app::headless::agent_client`; this only decides.

use std::path::{Path, PathBuf};

use signex_app::headless::agent_client::{self, GuiPeer, Hold, Lookup};

use crate::cli::Route;

pub enum Placement {
    /// Send the commands to this running app.
    Attach(GuiPeer),
    /// Run here. The hold is the project's registry entry for the run, removed
    /// when it drops; `None` when another run already holds the project or
    /// the entry could not be written (said on stderr).
    Headless(Option<Hold>),
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
    let looked = dir.and_then(|dir| agent_client::lookup(&dir, project).map(|found| (dir, found)));
    let (dir, found) = match looked {
        Ok(found) => found,
        Err(why) if route == Route::Attach => return Err(no_gui(why)),
        // Headless goes on as it always has; the project just is not held.
        Err(why) => {
            eprintln!("[signex-cli] cannot look for a running Signex ({why}); not holding {name}");
            return Ok(Placement::Headless(None));
        }
    };
    match found {
        Lookup::Gui(peer) if route == Route::Headless => Err(format!(
            "--headless: {name} is open in a running Signex (process {}); refusing to edit a \
             file that is open in a window. Leave out --headless to work through it",
            peer.pid
        )),
        Lookup::Gui(peer) => Ok(Placement::Attach(peer)),
        Lookup::Headless { pid } if route == Route::Attach => {
            Err(no_gui(format!("a headless run, process {pid}, holds it")))
        }
        Lookup::Headless { pid } => {
            eprintln!(
                "[signex-cli] {name} is held by a headless run (process {pid}); not holding it again"
            );
            Ok(Placement::Headless(None))
        }
        found => {
            if let Lookup::Stale(stale) = found
                && let Err(why) = stale.remove()
            {
                eprintln!("[signex-cli] a stale entry for {name} was left in place: {why}");
            }
            if route == Route::Attach {
                return Err(no_gui("no live entry".into()));
            }
            Ok(Placement::Headless(held(&dir, project)))
        }
    }
}

fn held(dir: &Path, project: &Path) -> Option<Hold> {
    match agent_client::hold(dir, project) {
        Ok(hold) => Some(hold),
        Err(why) => {
            eprintln!(
                "[signex-cli] could not hold {}: {why}; a Signex opened meanwhile will not know",
                project.display()
            );
            None
        }
    }
}
