# Start screen: the Libraries page

The Libraries page lists the libraries Signex loads for every project. This
note covers its Version control column (#2591). Code: `start_screen/libraries_page.rs` (view),
`app/dispatch/library/start_libraries.rs` (actions),
`signex_library::library_vcs` (the state of one folder).

## Version control column (#2591)

| Value | When |
|---|---|
| `—` | The Stock library. It is never read. |
| `git` | The library folder (the one holding the `.snxlib`) has a `.git` and nothing below applies. |
| `off` | The folder has no `.git`. `off` is a link that opens the library's Enable Version Control, the dialog the project tree's library menu opens. |
| needs attention + reason | The folder has a `.git`, and `git` is not on `PATH`; or the library uses Git LFS (`.gitattributes` has `filter=lfs`) and `git-lfs` is not on `PATH`; or the repository cannot be read; or it has uncommitted changes. One reason shows, in that order, as text under the pill. |
| empty | Not read yet. |

The state is read on a blocking worker (`read_start_library_vcs`), never in
`view` or `update`, and lands in `StartScreenState::library_vcs`, keyed by
folder. It is read when the Libraries page opens and again after Enable
Version Control finishes while the page shows. Each read carries a
generation; an older read that lands late is dropped. A library added while
the page shows has no state until the page opens again.
