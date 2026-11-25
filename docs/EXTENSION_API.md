# Extension API

`signex-app` is a library as well as the Community binary. Another binary can
link it, install its own **extensions** and run the same application with
them. This page is the contract for those extensions (#2495, part of #2326).

The code is `crates/signex-app/src/extension/` (`signex_app::extension`) and
`crates/signex-app/src/launch.rs` (`signex_app::launch`).

## What it is, and what it is not

- **In-process and compile-time.** An extension is trusted Rust code, linked
  into the binary at build time and installed once, at startup, before the
  first window opens.
- **Not the plugin system.** The Plugin System band in `docs/ROADMAP.md`
  (sandboxed modules loaded while the app runs) and slice S-P in ADR-0006 load
  untrusted code at run time. This API does not. A later plugin host can
  register through the same extension points, so the two do not compete.
- **The Community binary installs nothing.** Its `main` calls
  `signex_app::launch::run(Vec::new())`; with no extension installed every
  menu, command, panel, keymap and golden fixture is unchanged.

## Running with extensions

```rust
use signex_app::extension::Extension;

struct Sample;

impl Extension for Sample {
    fn name(&self) -> &str {
        "sample"
    }
}

fn main() -> iced::Result {
    signex_app::launch::run(vec![Box::new(Sample)])
}
```

`launch::run` does everything the Community `main` does (command-line
arguments, logging, fonts, the window) and installs the extensions before the
first window. Install checks every name and refuses the whole set when one is
invalid or two are the same: the process prints the reason and exits with
status 2, as for a bad command-line argument. A binary never runs with half
its extensions.

A test or a binary with its own runtime can install a set itself with
`ExtensionHost::install` and hand it to a model with `Signex::with_extensions`.

## The `Extension` trait

| Method | When it is called | Default |
|---|---|---|
| `name(&self) -> &str` | At install. | none, required |
| `register(&mut self, points: &mut ExtensionPoints)` | Once, at install, in the order the binary listed the extensions. | adds nothing |
| `update(&mut self, message: ExtensionMsg) -> Task<Message>` | For each `Message::Extension` addressed to this extension, and no other message. | does nothing |

The extension object is its state. It lives in the application model, and
after install only the update loop calls into it, so `update` is its one
mutation site, as for every other part of the application (ADR-0001). Long or
blocking work goes in the returned `Task`, never inline. An extension is
`Send + 'static`.

## Ids

An extension's name starts with a lowercase ASCII letter and holds only
lowercase ASCII letters, digits and `_`. It is unique among the installed
extensions.

Every id an extension adds (a catalog command, a panel key, a menu row, a
Start-screen section, a preference) starts with `ext.<name>.`, for example
`ext.sample.say_hello`. Built-in ids never start with `ext.`, so an extension
id cannot collide with a built-in one or with another extension's.
`ExtensionId::prefix` gives the prefix and `ExtensionId::owns` checks an id
against it. Built-in command ids stay locked by `scripts/catalog-ids.txt`;
extension ids are not listed there.

## Messages

The root `Message` has one variant for every extension, `Message::Extension(ExtensionMsg)`.
An `ExtensionMsg` carries the id of the extension it is for and a payload of
that extension's own message type:

```rust
let message = ExtensionMsg::new(points.owner().clone(), SamplePinged(3));
// in Sample::update:
if let Some(SamplePinged(n)) = message.payload::<SamplePinged>() { /* ... */ }
```

The update loop hands the message to the one installed extension of that id.
A message for an id that is not installed is dropped with a log line. Name
payload variants as past-tense events, as the rest of the application does
(`FileLoaded`, not `LoadFile`). A payload is `Debug + Send + Sync + 'static`.

## Extension points

`register` fills one `ExtensionPoints` per extension, kept apart from every
other extension's. Each point is a field with a file of its own under
`crates/signex-app/src/extension/points/`.

| Point | Field | Boundary (ADR-0006) | Status in version 1 |
|---|---|---|---|
| Catalog commands | `commands: CatalogCommands` | Seen by the application core: names no iced type | empty |
| Dock panels | `panels: Panels` | GUI adapter, outside `app/core/` | empty |
| Menu bar and Active Bar rows | `menu_rows: MenuRows` | GUI adapter, outside `app/core/` | empty |
| Start-screen sections | `start_screen: StartScreenSections` | GUI adapter, outside `app/core/` | empty |
| Preferences | `preferences: Preferences` | GUI adapter, outside `app/core/` | empty |

An empty point has no entry type yet; the change that opens it adds its entry
type and its `add` method in its file and a line under "Changes by version".
The rules each point will follow are set in #2495:

- **Catalog commands.** A command has a core handler or is GUI-only, as the
  core's rules say for every command.
- **Dock panels.** A panel has a stable layout key, a title id, a home dock
  region and its view and update hooks. A saved layout that names a panel key
  no installed extension registered keeps that key (`docs/DECISIONS.md`,
  2026-11-25).
- **Menu and Active Bar rows.** A row runs a command by id through the same
  path as a built-in row, so one command keeps one behaviour on every
  surface. Built-in rows keep their order.
- **Start-screen sections.** One named slot; with no extension installed the
  Start screen renders nothing there.

Text an extension shows goes through the `.ftl` catalogs like the rest of the
application; sizes come from the theme and the font size, never fixed pixels.

## One request path

Every surface runs a catalog command through one function,
`Signex::run_command_request(surface, request)` in
`crates/signex-app/src/app/command/request/` (also exported from
`signex_app::headless`). An extension that runs a command from its own code
uses it too, so its commands run as a menu row or a key runs them.

- **`CommandRequest`**: the command id, its `CommandArgs` and an optional
  `if_revision`.
- **`RequestSurface`**: `Gui { window }` (menu rows, keys, the palette and
  context-menu rows, through `dispatch_command`), `Headless` (`signex-cli`,
  through `dispatch_command_headless`) or `Agent` (the agent socket, once a
  request has passed the channel's token, allowlist and document checks; only
  the agent channel can make this one).
- **`RequestOutcome`**: `Answered(data)`, `Ran`, `Refused { reason, cause }`,
  `Failed(reason)`, or `Waiting { ticket }` for a command that waits for its
  own file reads and answers later. `RefusalCause` says why nothing ran:
  `NoSuchCommand`, `NotAvailable` or `Revision`.

Each surface keeps its own checks in front of the function and its own runner
behind it. What the function adds for all of them is the revision check and
one reading of the outcome. `headless::outcome_verdict` is that reading as the
`status`, `reason` and `data` of a reply line.

**`if_revision`.** A request with `if_revision = N` runs only while the
document its `document` operand names is still at revision `N`, the revision
`read_document` reports. If the document moved, or has no revision (a
library file, a project), or the request names no `document`, the request is
refused with `RefusalCause::Revision` before any runner sees it, and nothing
changes. On the agent socket `if_revision` is a top-level request field
(`docs/AGENT_SOCKET.md`). `apply_edits` keeps its own `expect_revision`
operand, which also checks the other sheets its edits name.

After its tasks settle, a request can leave an error notice.
`RequestOutcome::after_notice` turns a `Ran`, or a `NotAvailable` refusal the
app explained in a notice, into `Failed`, as the headless runner reports it.

## Versioning

`signex_app::extension::API_VERSION` is the version of this API. It goes up by
one in the release that changes the API in a way an extension written for the
previous version might not compile against, or might behave differently with.
Adding to the API (a new point, a new method with a default) does not raise it.
Each version has a section below, and the release's changelog names the
change.

An extension that wants to fail its build on a version it was not written for
can check the constant at compile time:

```rust
const _: () = assert!(signex_app::extension::API_VERSION == 1);
```

## Changes by version

### Version 1

- The `Extension` trait (`name`, `register`, `update`), `ExtensionId`,
  `ExtensionMsg`, `ExtensionHost`, `InstallError`, `launch::run` and
  `Signex::with_extensions`.
- `Message::Extension`, routed to the owning extension alone.
- `ExtensionPoints` with five empty points: catalog commands, dock panels,
  menu rows, Start-screen sections and preferences.
- One request path: `Signex::run_command_request`, `CommandRequest`,
  `RequestSurface`, `RequestOutcome` and `RefusalCause`, with `if_revision`.

## Open questions

Choices made in version 1 for the maintainers to confirm or change:

- **How stable is this API?** Version 1 promises the versioning rule above. Is
  the API stable across releases from now on, or is nothing promised before
  Signex 1.0?
- **What an extension can read.** `update` receives only its own message; it
  cannot read the documents, the selection or the other application state.
  Should it get read access, and through which part (the core's `query`, or a
  GUI context)?
- **Boot work.** Install cannot return a `Task`, so an extension cannot start
  background work at boot. Should `register` (or a separate hook) return one?
- **Headless runs.** `headless::boot_headless` and `signex-cli` install no
  extension. Should a headless run take the same set, so an extension's core
  commands run from a script?
- **Bench mode.** The pan/zoom bench (`SIGNEX_BENCH_PAN_ZOOM`) installs the
  binary's extensions too. Should it run without them?
- **Install order.** Extensions register in the order the binary lists them.
  Is that the order their rows, panels and sections appear in, or should each
  addition carry its own position?
- **Name rules.** A name is a lowercase ASCII identifier. Should names allow
  more (`-`, upper case), or be reserved in a list?
- **`if_revision` in `signex-cli`.** The agent socket takes `if_revision`;
  the `signex-cli` request line does not yet, so a headless run cannot ask
  for it. Should the CLI line take it as a top-level field too?
- **One revision or many.** `if_revision` checks the one document named by
  `document`. Should it also take a map of documents to revisions, as
  `apply_edits` takes `expect_revisions`?
- **A revision refusal in the journal.** A request refused by `if_revision`
  writes no edit-journal line, because nothing ran. Should it be recorded?
- **The GUI's revision refusal.** No GUI surface sets `if_revision`. A GUI
  refusal for the revision would add no Command Log row and log no
  Messages line. Is that right for an extension's row?
