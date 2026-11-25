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
Start-screen section, a preference) is `ext.<name>.` followed by exactly one
snake_case segment, for example `ext.sample.say_hello`; `ext.sample.say.hello`
is refused (Caner, 2026-11-25; docs/DECISIONS.md). Built-in ids never start with `ext.`, so an extension id cannot
collide with a built-in one or with another extension's.
`ExtensionId::prefix` gives the prefix and `ExtensionId::owns` checks an id
against it. One function holds the whole rule,
`signex_commands::is_extension_id` (re-exported as
`signex_app::extension::is_extension_id`): `AppCommandId::new`, every
extension point and the dock layout's kept keys all read it. Built-in command
ids stay locked by `scripts/catalog-ids.txt`; extension ids are not listed
there.

An `ExtensionId` has no public constructor: install makes one per extension,
and the extension receives its own from `ExtensionPoints::owner` in
`register` (or `PreferenceValues::owner`). So an extension addresses
messages to itself only and cannot pose as another.

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
| Catalog commands | `commands: CatalogCommands` | Seen by the application core: names no iced type | open, see "Catalog commands" |
| Dock panels | `panels: Panels` | GUI adapter, outside `app/core/` | open |
| Menu bar and Active Bar rows | `menu_rows: MenuRows` | GUI adapter, outside `app/core/` | open |
| Start-screen sections | `start_screen: StartScreenSections` | GUI adapter, outside `app/core/` | open |
| Preferences | `preferences: Preferences` | GUI adapter, outside `app/core/` | open |
| Message files | `messages: Messages` | GUI adapter, outside `app/core/` | open, see "Messages an extension ships" |

A new point gets a file of its own with its entry type and its `add` method,
and a line under "Changes by version". The rules each point follows are set
in #2495:

- **Catalog commands.** A command has a core handler or is GUI-only, as the
  core's rules say for every command. Open; see "Catalog commands" below.
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
application, its own messages included (see "Messages an extension ships");
sizes come from the theme and the font size, never fixed pixels.

Every `add` checks the id against the owner's `ext.<name>.` prefix and the
id rule (see "Ids") and refuses one added twice. Every point refuses with one
type, `RegisterError`. A refusal is also recorded, and install then refuses
the whole set with `InstallError::Refused`, naming the extension and the
first refusal, so a binary never runs with half an extension.

### Views: panels and sections

A panel and a Start-screen section are drawn by an `ExtensionView`, an object
that is the surface's state:

| Method | Called | Default |
|---|---|---|
| `view(&self, cx: ViewContext) -> Element<ExtensionMsg>` | While the surface is on screen. Pure: no IO, no mutation. | none, required |
| `update(&mut self, message: ExtensionMsg) -> Task<Message>` | For each message addressed to the surface's key. | does nothing |

`ViewContext` carries the theme tokens and the language the window draws in.
A message the view sends is made with `ExtensionMsg::for_view(owner, key,
payload)`; the update loop hands it to that surface's `update`. A key the
extension did not register sends it to the extension's own `update`. A panel
and a section of one extension share one key space.

### Dock panels

```rust
let notes = points.panels.add("ext.sample.notes", title_id, PanelPosition::Right, NotesView::default())?;
```

`add` returns the panel's `ExtensionPanelKind`: its layout key, its title
(a catalog `MessageId`) and its home region. The dock holds it as
`PanelKind::Extension(kind)`, so a docked, floating or detached extension
panel behaves like a built-in one: tabs, collapse, close, drag and the
layout's change detection. `ExtensionPanelKind::open_message` is the message
that opens it, docked in its home region when it is not open yet; an
extension returns it from `update` as `Task::done(kind.open_message())`.

Every installed extension's panels are listed in the status bar's panel
list, in an Extensions group after the built-in groups, and in the View menu,
in a View ▸ Extensions submenu after the built-in panel rows, each row
checked while its panel is open (`docs/DECISIONS.md`, 2026-11-25). The
command palette offers "Open Panel: <title>" for them too. With no extension
installed neither group is shown, so the Community list and menu are the
built-in ones, row for row.

A saved layout (`prefs.json` and the workspace layouts) stores the panel
under its key. A key in some extension's namespace that no installed
extension registered is kept at its place among the region's entries and
written back unchanged, so a layout saved by a binary with more extensions
survives a run without them (`docs/DECISIONS.md`, 2026-11-25). Once the
extension is installed, the key loads as its panel at that place. A key
outside the `ext.` namespace that names no panel is still dropped, as before.

### Menu bar and Active Bar rows

```rust
points.menu_rows.add_menu_row(MenuRoot::Tools, label_id, "ext.sample.say_hello")?;
points.menu_rows.add_active_bar_row(PcbBarGroup::Route, label_id, "ext.sample.say_hello")?;
```

A menu row goes at the end of its root menu, after every built-in row and
below one separator; an Active Bar row goes at the end of a PCB Active Bar
tool group's dropdown, below one separator. The command must be the
extension's own (`ext.<name>.<snake_case>`, which `AppCommandId` accepts),
and one it added with `CatalogCommands::add`, before or after the row:
install refuses a row whose command was never added
(`RegisterError::NotRegistered`).
A click sends `MenuMessage::Command` (an Active Bar row: the group's
`CommandPicked`), the message a built-in row sends, so the command runs
through `dispatch_command`, the Command Log records it, the row greys by the
same gate, and its shortcut text comes from the active keymap. With no row
added, every menu and dropdown is the built-in one.

### Start-screen sections

```rust
points.start_screen.add("ext.sample.welcome", heading_id, StartScreenSlot::HomeEnd, WelcomeView)?;
```

The one slot in version 1, `StartScreenSlot::HomeEnd`, is on the Home page
below the reference design and the project lists. Each section is drawn under
its heading, in install order. With no section added the slot adds nothing to
the page.

### Preferences

```rust
points.preferences.add("ext.sample.greeting", serde_json::json!("hi"))?;
```

The values live in `prefs.json`, in its `extensions` object, under their full
keys (`{"extensions": {"ext.sample.greeting": "hello"}}`). At launch, after
install and before the first window, `Extension::preferences_loaded` receives
the value of each key the extension added (the stored one, else its default);
a stored key it did not add is not read. The extension keeps those
`PreferenceValues` to save later: `values.save(key, value)` returns the `Task`
that writes one value on a blocking worker, in the order the saves were made
(the quit path waits for it), and answers the extension with a
`PreferenceSaved` payload (the key, the value and whether the write landed).
It refuses a key outside the extension's prefix (`NotOwned`) and a key it did
not add with `Preferences::add` (`NotRegistered`).

Each installed extension that added preferences gets a Preferences page of
its own, named after it, under an Extensions group after the built-in pages
(`docs/DECISIONS.md`, 2026-11-25). The page is built from the same building
blocks as the built-in pages: a heading, a section and one setting row per
preference, with the preference's key, a pill naming the kind of value it
holds (text, number, on or off, list, group or empty) and its value. It shows
the value loaded at launch, or the last one a save landed for; it holds no
draft, so its footer offers Close. The values stay in the one `extensions`
object of `prefs.json`.

### Messages an extension ships

```rust
points.messages.add("en-US", include_str!("en-US.ftl"))?;
let title = points.messages.id("ext-sample-notes-title").expect("an own id");
points.panels.add("ext.sample.notes", title, PanelPosition::Right, NotesView::default())?;
```

An extension may ship its own `.ftl` files, one per locale
(`docs/DECISIONS.md`, 2026-11-25). Every message id a file defines starts
with `ext-<name>-` (a term: `-ext-<name>-`), so it cannot shadow a built-in
message or another extension's; no built-in id starts with `ext-`. Install
refuses a file that does not parse (`RegisterError::InvalidMessages`) or that
defines any other id (`NotOwned`). `Messages::id` hands out a `MessageId` for
the extension's own ids only; `MessageId::extension` is the `const` form, for
a catalog row in a `static`, and accepts nothing outside `ext-`.

The files join the app's catalog when the extensions are installed and every
time the language changes, so a panel title, a row label, a section heading
or a command label that names one follows the app language like a built-in
message. A message the active language's files lack falls back to the en-US
file, as a built-in message falls back to en-US; the pseudolocale reads the
en-US files.

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

An extension command takes the same path: the request reaches
`dispatch_command` (or `dispatch_command_headless`), which hands an installed
`ext.` id to the extension's handler.

The agent socket's window-layout commands (Tile and Untile) run through the
`Agent` runner too: they name no document, so their `if_revision` is refused
as for any request without one.

**Documented exception: a waiting request's second run.** A command that
waits for its own work (an edit that reads the disk, a check run, a command
that waits for a project read) answers `Waiting { ticket }`, and when that
work lands it runs again outside `run_command_request`, in
`app/agent/deferred.rs`. That second run meets the same checks again first:
the agent gate (connections on, the session token unchanged, the document
one of an open project's) and the request's `if_revision`. A request whose
document moved meanwhile is refused with the revision reason, nothing runs
and no edit-journal line is written, as at request time.

## Catalog commands

An extension adds a command with one catalog row and one handler:

```rust
use signex_app::extension::{
    CommandFlags, CommandMetadata, Extension, ExtensionMsg, ExtensionPoints,
};

static SAY_HELLO: CommandMetadata = CommandMetadata {
    id: "ext.sample.say_hello",
    category: "sample",
    flags: CommandFlags::RUNS_HEADLESS,
    ..CommandMetadata::DEFAULT
};

#[derive(Debug)]
struct HelloSaid;

impl Extension for Sample {
    fn name(&self) -> &str {
        "sample"
    }

    fn register(&mut self, points: &mut ExtensionPoints) {
        let added = points.commands.add(&SAY_HELLO, |_args| Ok(HelloSaid));
        debug_assert!(added.is_ok(), "{added:?}");
    }

    fn update(&mut self, message: ExtensionMsg) -> iced::Task<signex_app::app::Message> {
        if message.payload::<HelloSaid>().is_some() { /* ... */ }
        iced::Task::none()
    }
}
```

- **One command path.** A command runs through `dispatch_command`, as a
  built-in one does, from every surface that names it by id: a key binding,
  a menu row and a headless run (`Signex::dispatch_command_headless`). Each
  run writes its Command Log row. Its handler is called with the run's
  `CommandArgs` and returns the payload of the `ExtensionMsg` its
  extension's `update` then receives, or a `Translatable` reason, which is
  reported as `CommandOutcome::Disabled` and reaches no `update`.
- **The handler is UI-free.** It reads only the arguments, names no iced type
  and no GUI state; the core sees it with the row (ADR-0006). Work beyond
  making the message is the job of `update`, as for any other message.
- **Headless parity.** The row decides it, as for a built-in row: exactly one
  of `CommandFlags::GUI_ONLY` and `CommandFlags::RUNS_HEADLESS`. A headless
  run refuses a GUI-only command with the reason every headless surface
  gives (`` `<id>` only works in a window ``) and runs the other.
- **What `add` refuses**, with a `RegisterError`:
  - `ShadowsBuiltIn`: the id is a built-in id, or its `<command>` part is
    one (`ext.sample.undo`);
  - `NotOwned`: the id is not under the extension's own `ext.<name>.`
    prefix;
  - `InvalidId`: the `<command>` part is not one snake_case segment
    (`AppCommandId::new`);
  - `Duplicate`: the extension already added the id;
  - `HeadlessUndecided`: the flags set neither or both of `gui_only` and
    `runs_headless`;
  - `TakesOperands` and `MutatesDocument`: this version gives an extension
    command no operands and no document access.

  A refused row is not added, and install then refuses the whole set with
  `InstallError::Refused`, naming the extension and the first refusal.
- **Ids.** `AppCommandId` accepts `ext.<name>.<command>` next to the built-in
  snake_case ids. Built-in ids never start with `ext.` (a test in
  `signex-commands` pins it) and stay locked by `scripts/catalog-ids.txt`;
  extension ids are never listed there. `Signex::command_metadata` finds a
  row of either kind.
- **Not on the agent socket.** The agent socket refuses every `ext.` id,
  installed or not, with "extension commands are not open to the agent
  socket in version 1".
- **The palette and the keyboard shortcuts.** An installed extension's
  command is listed in the command palette and in the keyboard shortcuts
  (the Preferences page and the Help sheet) as "<extension>: <label>", under
  its row's category, with its shortcut (`docs/DECISIONS.md`, 2026-11-25).
  It can be bound to a key like a built-in command: a custom profile's
  binding names the `ext.` id, the binding is saved under that id in
  `keyboard_shortcuts.toml`, and the key runs the command through
  `dispatch_command`. A binding to an extension command that no installed
  extension adds stays in its profile (the TOML parses ext ids) and is
  listed as "<id> (unavailable: extension not installed)"; its key does
  nothing.
- **Not listed elsewhere.** `docs/COMMANDS.md` and the `signex commands`
  output describe the Community build and list built-in commands only, and
  `signex-cli` installs no extension, so it refuses an `ext.` id as an
  unknown command.

### Behaviour changes

- `AppCommandId` now accepts `ext.<name>.<command>` ids. Where an id is read
  from a file (a keymap profile's TOML, a saved layout or any other saved
  data that holds a command id), such an id used to be refused when the file
  was parsed; it now parses, and is refused later as an unknown command when
  no installed extension added it. A malformed `ext.` id is still refused at
  parse, with a message that names the `ext.<name>.<command>` form.

## Walkthrough: an example extension

`crates/signex-app/tests/example_extension.rs` is a whole extension written
against the public API only, as a binary built on `signex-app` would write
it, and runs under `cargo test --workspace`. It adds one dock panel, one
Tools menu row and one catalog command, and its tests drive each one.

**1. Name and ids.** The extension is named `example`, so each id it adds
is `ext.example.` and one snake_case segment: the panel key
`ext.example.notes` and the command `ext.example.say_hello`.

**2. The command's row and message.** The command is a `static`
`CommandMetadata` with no operands, no document access and
`CommandFlags::RUNS_HEADLESS`, so it runs from a window and headless alike.
Its handler returns the past-tense payload `HelloSaid`.

**3. The panel.** `Notes` implements `ExtensionView`: `view` draws one
button whose press is `ExtensionMsg::for_view(owner, NOTES, NotesPressed)`,
and `update` counts the presses. The object is the panel's state.

**4. Its own messages.** `tests/example_extension/en-US.ftl` holds the
panel's title, the button's text and the command's label, each under
`ext-example-`. The ids are `const` (`MessageId::extension`), so the
command's `static` row names its label directly.

**5. Registering.** `Extension::register` adds the message file, then all
three:

```rust
points.messages.add("en-US", MESSAGES)?;
points.panels.add(NOTES, TITLE, PanelPosition::Right, notes)?;
points.commands.add(&SAY_HELLO_ROW, |_args| Ok(HelloSaid))?;
points.menu_rows.add_menu_row(MenuRoot::Tools, LABEL, SAY_HELLO)?;
```

A test installs the extension with `ExtensionHost::install` and hands the
host to `Signex::new()` through `with_extensions`; a binary passes the same
`Vec<Box<dyn Extension>>` to `launch::run`.

**6. Driving it.**

- *Open the panel.* `app.extensions().panel(NOTES)` gives its
  `ExtensionPanelKind`, titled "Notes" from the example's own file;
  `app.update(kind.open_message())` docks it in its home region, the right
  dock. A `Message::Extension` addressed to the
  panel's key reaches `Notes::update`, not the extension's own `update`.
  The test reads the extension's id off `app.extensions().ids()`, since
  outside the crate an id comes only from install.
- *Pick the menu row.* `menu_bar_model` built with
  `MenuContext::default().with_extension_rows(app.extensions().menu_rows())`
  in the app's language, the model the window draws, ends Tools with a
  separator and the row, "Say hello". The
  row's `MenuMessage::Command` is what a click sends; `app.update` runs the
  command through `dispatch_command`, and the extension receives `HelloSaid`.
- *Run the command through the request path.*
  `run_command_request(RequestSurface::Gui { window: None }, request)`
  answers `RequestOutcome::Ran`. An id in the extension's namespace that it
  did not add is refused with `RefusalCause::NoSuchCommand`.
- *Run it headless.* In a child process with its own config root override
  (the override is process-wide), `Signex::boot_headless`, then
  `with_extensions`, then both `dispatch_command_headless` and
  `run_command_request(RequestSurface::Headless, request)` answer `Ran`.

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

- The `Extension` trait (`name`, `register`, `update`), `ExtensionId` (no
  public constructor), `ExtensionMsg`, `ExtensionHost`, `InstallError`, `launch::run` and
  `Signex::with_extensions`.
- `Message::Extension`, routed to the owning extension alone.
- `ExtensionPoints` with six points: catalog commands, dock panels,
  menu rows, Start-screen sections, preferences and message files.
- An extension's own `.ftl` files: `Messages::{add, id, prefix, files}`,
  `MessageId::extension`, `ExtensionMessages`, `RegisterError::InvalidMessages`
  (`signex_i18n::{ExtensionMessages, ftl_entry_ids}`,
  `I18n::with_extension_messages`).
- Extension panels in the panel list and under View ▸ Extensions
  (`ExtensionHost::panels`, `MenuContext::with_extension_panels`,
  `MenuMessage::OpenExtensionPanel`, `PanelGroup::Extensions`); extension
  commands in the palette and the keyboard shortcuts
  (`ExtensionHost::command_names`, `keymap::ExtensionCommandNames`,
  `KeymapEditorModel::rows_in_with`, `build_sections_with`); a Preferences
  page per extension (`ExtensionHost::{preference_pages, preference_page,
  name_at}`, `PreferencePage`, `PrefNav::Extension`).
- Catalog commands: `CatalogCommands::add`, `ExtensionCommand`,
  `ExtensionHost::{command, commands}`, `Signex::command_metadata`, and
  `AppCommandId` accepting `ext.<name>.<command>`
  (`AppCommandId::{is_extension, extension_local}`,
  `signex_commands::{EXTENSION_ID_PREFIX, is_extension_id,
  is_extension_name}`, `is_extension_id` re-exported from
  `signex_app::extension`).
- Dock panels (`Panels::add`, `ExtensionPanelKind`, `PanelKind::Extension`),
  menu and Active Bar rows (`MenuRows::add_menu_row`, `add_active_bar_row`,
  `MenuRoot`), Start-screen sections (`StartScreenSections::add`,
  `StartScreenSlot::HomeEnd`) and preferences (`Preferences::add`,
  `PreferenceValues` with `owner` and `save`, `PreferenceSaved`); the
  `ExtensionView` trait and `ViewContext`; `ExtensionMsg::for_view`;
  `Extension::preferences_loaded`; `ExtensionHost::{panel, menu_rows,
  active_bar_rows, load_preferences}`.
- One refusal type for every point, `RegisterError` (with `NotRegistered`
  for a row whose command, or a saved preference whose key, was never
  added), and `InstallError::Refused`.
- `MenuContext::with_extension_rows`, so code outside the crate builds the
  menu model a window draws; and the example extension
  (`crates/signex-app/tests/example_extension.rs`).
- One request path: `Signex::run_command_request`, `CommandRequest`,
  `RequestSurface`, `RequestOutcome` and `RefusalCause`, with `if_revision`.

## Implementation notes (version 1 defaults)

What version 1 does where the decisions above leave a choice. Each is the
conservative default; a later version may change it, under the versioning
rule.

- **Stability.** Version 1 promises the versioning rule above and nothing
  more.
- **What an extension can read.** `update` receives only its own message; an
  extension cannot read the documents, the selection or other application
  state.
- **Boot work.** Install returns no `Task`, so an extension starts no
  background work at boot.
- **Headless runs.** `headless::boot_headless` and `signex-cli` install no
  extension; a headless caller installs a set with `with_extensions`.
- **Bench mode.** The pan/zoom bench (`SIGNEX_BENCH_PAN_ZOOM`) installs the
  binary's extensions too.
- **Install order.** Extensions register in the order the binary lists them,
  and their rows, panels, sections and pages appear in that order; an
  addition carries no position of its own.
- **Name rules.** A name is a lowercase ASCII identifier; no name is
  reserved.
- **Which bars.** Rows go to the end of a root menu and of a PCB Active Bar
  tool group's dropdown. A group with one row (Text) shows no dropdown, so a
  row added there is not reachable; the schematic Active Bar and the context
  menus take no rows.
- **Slots.** The Start screen has one slot, at the end of Home.
- **Kept keys and workspace layouts.** Applying a workspace layout replaces
  the dock, so the keys the old arrangement kept are not carried over; a
  layout saved while keys were kept keeps them.
- **Preference pages are read-only.** An extension's page shows its values;
  the extension changes them with `PreferenceValues::save`. Its preferences
  carry no label message, so a row shows the preference's key.
- **`if_revision` in `signex-cli`.** Only the agent socket takes
  `if_revision`; the `signex-cli` request line does not.
- **One revision.** `if_revision` checks the one document named by
  `document`; `apply_edits` keeps its own `expect_revisions` for more.
- **A revision refusal writes no journal line**, because nothing ran.
- **The GUI's revision refusal.** No GUI surface sets `if_revision`; a GUI
  refusal for the revision would add no Command Log row and log no Messages
  line.
- **A command id that reads as a built-in one** (`ext.sample.undo`) is
  refused as well as `undo`, so a palette or log line never shows two
  commands with one name.
- **Operands and document access.** An extension command takes no operands
  and cannot mutate a document.
- **Enablement.** A row's `enable` predicate is carried, not evaluated, as for
  every built-in row.
- **Extension commands on the agent socket** are refused.
- **Product name.** A binary built on `signex-app` still shows "Signex" in
  the window title, the help and version text and the catalog's
  `-brand-name` term; the extension API sets no product name.
