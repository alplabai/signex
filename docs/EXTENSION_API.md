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
| Dock panels | `panels: Panels` | GUI adapter, outside `app/core/` | open |
| Menu bar and Active Bar rows | `menu_rows: MenuRows` | GUI adapter, outside `app/core/` | open |
| Start-screen sections | `start_screen: StartScreenSections` | GUI adapter, outside `app/core/` | open |
| Preferences | `preferences: Preferences` | GUI adapter, outside `app/core/` | open |

An empty point has no entry type yet; the change that opens it adds its entry
type and its `add` method in its file and a line under "Changes by version".
The rules each point follows are set in #2495:

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

Every `add` checks the id against the owner's `ext.<name>.` prefix and
refuses one added twice (`RegisterError`). A refusal is also recorded, and
install then refuses the whole set with `InstallError::Refused`, so a binary
never runs with half an extension.

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
extension returns it from `update` as `Task::done(kind.open_message())`. An
extension panel is not listed in the built-in panel list.

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
extension's own (`ext.<name>.<snake_case>`, which `AppCommandId` accepts).
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
a stored key it did not add is not read. `save_preference(owner, key, value)`
returns the `Task` that writes one value on a blocking worker, in the order
the saves were made (the quit path waits for it), and answers the owner with a
`PreferenceSaved` payload. It refuses a key outside the owner's prefix.

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
- Dock panels (`Panels::add`, `ExtensionPanelKind`, `PanelKind::Extension`),
  menu and Active Bar rows (`MenuRows::add_menu_row`, `add_active_bar_row`,
  `MenuRoot`), Start-screen sections (`StartScreenSections::add`,
  `StartScreenSlot::HomeEnd`) and preferences (`Preferences::add`,
  `PreferenceValues`, `save_preference`, `PreferenceSaved`); the
  `ExtensionView` trait and `ViewContext`; `ExtensionMsg::for_view`;
  `Extension::preferences_loaded`; `RegisterError` and
  `InstallError::Refused`; `ExtensionHost::{panel, menu_rows,
  active_bar_rows, load_preferences}`; `AppCommandId` accepts
  `ext.<name>.<local>` ids.

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
- **Extension text.** A panel title, a row label and a section heading are a
  catalog `MessageId`, and only the built-in catalog makes one. Should an
  extension ship `.ftl` messages of its own, and how are they merged?
- **Panel list.** An extension panel opens only through its own message and
  a saved layout; the built-in panel list and View ▸ Panels do not list it.
  Should they, and under which group?
- **Which bars.** Rows go to the end of a root menu and of a PCB Active Bar
  tool group's dropdown. A group with one row (Text) shows no dropdown, so a
  row added there is not reachable; the schematic Active Bar and the context
  menus take no rows. Should rows carry a position, and reach those bars?
- **Slots.** The Start screen has one slot, at the end of Home. Are more
  wanted (the Projects or Libraries page, beside the hero)?
- **Kept keys and workspace layouts.** Applying a workspace layout replaces
  the dock, so the keys the old arrangement kept are not carried over; a
  layout saved while keys were kept keeps them. Should applying a layout
  carry the current kept keys over?
- **Preference storage.** Values sit in one `extensions` object of
  `prefs.json`, under their full keys. Is that the place, or should each
  extension get a file of its own? Extension preferences show no row in the
  Preferences dialog; should the dialog get a page per extension?
