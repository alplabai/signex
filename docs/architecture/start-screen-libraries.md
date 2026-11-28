# Start screen: the Libraries page

The Libraries page lists the libraries Signex loads for every project. This
note covers its Version control column (#2591) and Remove from Signex
(#2592). Code: `start_screen/libraries_page.rs` (view),
`app/dispatch/library/start_libraries.rs` (actions),
`signex_library::library_vcs` (the state of one folder).

## Version control column (#2591)

| Value | When |
|---|---|
| `—` | The Stock library. It is never read. |
| `git` | The library folder (the one holding the `.snxlib`) has a `.git` and nothing below applies. |
| `off` | The folder has no `.git`. `off` is a link that opens the library's Enable Version Control, the dialog the project tree's library menu opens. |
| needs attention + reason | The folder has a `.git`, and `git` is not on `PATH`; or the library uses Git LFS (`.gitattributes` has `filter=lfs`) and `git-lfs` is not on `PATH`; or the repository cannot be read; or it has uncommitted changes. One reason shows, in that order, as text under the pill. |
| `…` | Not read yet, so a pending read never looks like no version control. |

The state is read on a blocking worker (`read_start_library_vcs`), never in
`view` or `update`, and lands in `StartScreenState::library_vcs`, keyed by
folder. It is read when the Libraries page opens, and again while the page shows
after Enable Version Control finishes or Add library… adds a library. Each read carries a
generation; an older read that lands late is dropped.

The app checks the `.git` entry (`std::fs::symlink_metadata`) and passes its
presence to `read_library_vcs_state`, since the library crate does no file
IO. Only a missing `.git` reads `off`. A `.git` git cannot open (HEAD,
objects or refs missing) reads needs attention, "cannot be read", so its row
never offers Enable Version Control, which refuses a folder that has a
`.git` (#3235).

## Remove from Signex (#2592)

A Global or Installed row has a ⋯ button; its menu holds Remove from
Signex. Stock and a project's own libraries have no ⋯ button.

1. Remove from Signex opens a confirm card, before any Close Library prompt, (the notice card with
   `NoticeAction::RemoveLibraryFromSignex`). It says the files stay on disk
   and shows the path. Cancel changes nothing.
2. Remove from Signex on the card adds the library to
   `StartScreenState::removing_libraries` and runs the existing Close Library: it closes at once, waits for the
   library's writes, or first asks about unsaved edits (Save All / Discard
   All / Cancel). Its open Library Browser and Component Preview tabs close
   with it.
3. Once the library is unmounted (`close_tabs_of_unmounted_libraries`), it
   leaves the session's Installed list, and a Global library's entry leaves
   `global_libraries.toml` on a worker (`global_prefs::remove_path`). A
   failed write keeps the entry, so the library loads again at the next
   start, and the save-failure notice says so. Several removals can wait at
   once (one library's close waits for its writes while another is removed):
   each library leaves the registry when its own close finishes (#3235).

Cancel in the close prompt, a failed Save All, or a deferred close stopped by
new edits drops the pending removal, and so does opening the library again
while the removal waits (`LibraryState::note_reopened`, drained in
`finish_update`): nothing is removed, and a later plain
Close Library does not remove it either. Nothing on disk is deleted. A
project that lists the library keeps its own reference. There is no undo;
Add library… adds it back.
