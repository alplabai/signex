//! The example extension of `docs/EXTENSION_API.md`, "Walkthrough: an
//! example extension" (#2495). It uses only the public API a binary built
//! on `signex-app` sees, and adds one of each: a dock panel, a Tools menu
//! row and a catalog command the row runs. Each test drives one of them
//! through the application the way a user or a script would: the panel is
//! opened and its view's message reaches its own update hook, the menu row
//! is picked from the menu the window draws, and the command runs from a
//! window, headless, and through the one request path.
//!
//! The headless boot needs a process-wide config root override, so that
//! test runs in a child process of this binary ([`probe`]).

#![allow(
    clippy::disallowed_methods,
    reason = "the headless test re-runs its own binary as a child process"
)]

use std::sync::{Arc, Mutex};

use iced::{Element, Task, Widget as _};
use signex_app::app::{Message, Signex};
use signex_app::config_root::set_config_root_override;
use signex_app::dock::{PanelPosition, PanelSite};
use signex_app::extension::points::MenuRoot;
use signex_app::extension::{
    CommandFlags, CommandMetadata, Extension, ExtensionHost, ExtensionId, ExtensionMsg,
    ExtensionPoints, ExtensionView, ViewContext,
};
use signex_app::fonts::catalog::{FontSet, install_before_boot};
use signex_app::headless::{
    CommandArgs, CommandOutcome, CommandRequest, RefusalCause, RequestOutcome, RequestSurface,
};
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::model::{MenuEntry, menu_bar_model};
use signex_app::menu_bar::{MenuContext, MenuMessage};
use signex_app::panels::PanelKind;
use signex_app::task_driver::TaskDriver;
use signex_i18n::{MessageId, t};
use signex_types::project::{ErcRules, RuleProfile};
use signex_widgets::{button, text};

// --- The example extension -------------------------------------------------

/// The extension's name: every id it adds starts with `ext.example.`.
const NAME: &str = "example";
/// The dock panel's layout key, saved in the dock layout.
const NOTES: &str = "ext.example.notes";
/// The catalog command the menu row runs.
const SAY_HELLO: &str = "ext.example.say_hello";
/// The extension's own messages, in its own `.ftl` file. Each id starts with
/// `ext-example-`, so it cannot shadow a built-in message.
const MESSAGES: &str = include_str!("example_extension/en-US.ftl");

/// One of the extension's message ids, checked when the binary compiles.
const fn own(id: &'static str) -> MessageId {
    match MessageId::extension(id) {
        Some(id) => id,
        None => panic!("not an extension message id"),
    }
}

/// The panel's title, the button it draws, and the command's label.
const TITLE: MessageId = own("ext-example-notes-title");
const BODY: MessageId = own("ext-example-notes-body");
const LABEL: MessageId = own("ext-example-say-hello");
/// The extension's one preference, and its label and help on the
/// extension's Preferences page.
const GREETING: &str = "ext.example.greeting";
const GREETING_LABEL: MessageId = own("ext-example-greeting");
const GREETING_HELP: MessageId = own("ext-example-greeting-help");

/// The command's catalog row. It takes no operands, changes no document and
/// runs headless as well as from a window.
static SAY_HELLO_ROW: CommandMetadata = CommandMetadata {
    id: SAY_HELLO,
    category: "example",
    label: LABEL,
    flags: CommandFlags::RUNS_HEADLESS,
    ..CommandMetadata::DEFAULT
};

/// The extension's own message: the command ran. A past-tense event.
#[derive(Debug, Clone, PartialEq)]
struct HelloSaid;

/// The panel's own message: its button was pressed.
#[derive(Debug, Clone, PartialEq)]
struct NotesPressed;

/// What the extension saw, shared with the test.
#[derive(Debug, Default)]
struct Seen {
    /// `HelloSaid` messages the extension's `update` received.
    hellos: usize,
    /// `NotesPressed` messages the panel's `update` received.
    presses: usize,
}

type Shared = Arc<Mutex<Seen>>;

/// The panel: one button. The object is the panel's state.
struct Notes {
    owner: ExtensionId,
    seen: Shared,
}

impl ExtensionView for Notes {
    fn view<'a>(&'a self, cx: ViewContext<'a>) -> Element<'a, ExtensionMsg> {
        let pressed = ExtensionMsg::for_view(self.owner.clone(), NOTES, NotesPressed);
        button(text(t!(cx.i18n, BODY))).on_press(pressed).boxed()
    }

    fn update(&mut self, message: ExtensionMsg) -> Task<Message> {
        if message.payload::<NotesPressed>().is_some() {
            self.seen.lock().expect("seen").presses += 1;
        }
        Task::none()
    }
}

struct Example {
    seen: Shared,
}

impl Extension for Example {
    fn name(&self) -> &str {
        NAME
    }

    fn register(&mut self, points: &mut ExtensionPoints) {
        points
            .messages
            .add("en-US", MESSAGES)
            .expect("the messages register");
        let notes = Notes {
            owner: points.owner().clone(),
            seen: Arc::clone(&self.seen),
        };
        points
            .panels
            .add(NOTES, TITLE, PanelPosition::Right, notes)
            .expect("the panel registers");
        points
            .commands
            .add(&SAY_HELLO_ROW, |_args| Ok(HelloSaid))
            .expect("the command registers");
        points
            .menu_rows
            .add_menu_row(MenuRoot::Tools, LABEL, SAY_HELLO)
            .expect("the menu row registers");
        points
            .preferences
            .add(
                GREETING,
                GREETING_LABEL,
                Some(GREETING_HELP),
                serde_json::json!("hello"),
            )
            .expect("the preference registers");
    }

    fn update(&mut self, message: ExtensionMsg) -> Task<Message> {
        if message.payload::<HelloSaid>().is_some() {
            self.seen.lock().expect("seen").hellos += 1;
        }
        Task::none()
    }
}

/// The example installed alone, and what it sees.
fn example_host() -> (ExtensionHost, Shared) {
    let seen = Shared::default();
    let example = Example {
        seen: Arc::clone(&seen),
    };
    let host = ExtensionHost::install(vec![Box::new(example)]).expect("the example installs");
    (host, seen)
}

fn app_with_example() -> (Signex, Shared) {
    let (host, seen) = example_host();
    let (app, _boot) = Signex::new();
    (app.with_extensions(host), seen)
}

fn say_hello() -> AppCommandId {
    AppCommandId::new(SAY_HELLO).expect("an extension command id")
}

// --- Driving it ------------------------------------------------------------

#[test]
fn the_panel_opens_in_its_home_region_and_its_messages_reach_its_update() {
    let (mut app, seen) = app_with_example();
    let notes = app
        .extensions()
        .panel(NOTES)
        .expect("the panel is registered");
    // Its title is the example's own message, in the app's language.
    assert_eq!(
        app.ui_state.i18n.message(notes.title(), &[]).into_string(),
        "Notes"
    );
    assert!(
        app.document_view
            .dock
            .locate(PanelKind::Extension(notes))
            .is_none()
    );

    let _task = app.update(notes.open_message());
    assert!(matches!(
        app.document_view.dock.locate(PanelKind::Extension(notes)),
        Some(PanelSite::Docked(PanelPosition::Right, _))
    ));

    // The message the panel's button sends goes to the panel, not to the
    // extension.
    // Outside the crate an id comes only from install: the extension keeps
    // its own from `ExtensionPoints::owner`; a test reads it off the host.
    let owner = app
        .extensions()
        .ids()
        .find(|id| id.name() == NAME)
        .cloned()
        .expect("the example is installed");
    let pressed = ExtensionMsg::for_view(owner, NOTES, NotesPressed);
    let _task = app.update(Message::Extension(pressed));
    let seen = seen.lock().expect("seen");
    assert_eq!((seen.presses, seen.hellos), (1, 0));
}

#[test]
fn the_menu_row_ends_tools_and_picking_it_runs_the_command() {
    let (mut app, seen) = app_with_example();
    // The menu in the app's language, which carries the example's messages.
    let mut ctx = MenuContext::default()
        .with_extension_rows(app.extensions().menu_rows())
        .with_extension_panels(app.extensions().panels());
    ctx.i18n = Arc::clone(&app.ui_state.i18n);
    let menu = menu_bar_model(&ctx);
    let tools = MenuRoot::ALL
        .iter()
        .position(|root| *root == MenuRoot::Tools)
        .expect("Tools is a root menu");
    let [.., MenuEntry::Separator, MenuEntry::Item(row)] = menu.menus[tools].entries.as_slice()
    else {
        panic!("Tools ends with a separator and the example's row");
    };
    assert_eq!(row.label, "Say hello");
    let picked = row.dispatches().cloned().expect("the row is live");
    assert!(matches!(&picked, MenuMessage::Command(id) if id.as_str() == SAY_HELLO));

    let _task = app.update(Message::Menu(picked));
    assert_eq!(seen.lock().expect("seen").hellos, 1);
}

#[test]
fn the_preference_page_names_the_setting_by_its_own_label_and_help() {
    let (app, _seen) = app_with_example();
    let page = app
        .extensions()
        .preference_pages()
        .find(|page| page.name == NAME)
        .expect("the example's Preferences page");
    assert_eq!(page.values.get(GREETING), Some(&serde_json::json!("hello")));
    let texts = page.preferences.texts(GREETING).expect("the preference");
    let shown = |id: MessageId| app.ui_state.i18n.message(id, &[]).into_string();
    assert_eq!(shown(texts.label), "Greeting");
    assert_eq!(
        texts.help.map(shown).as_deref(),
        Some("The word the example says hello with.")
    );
}

#[test]
fn the_command_runs_from_a_window_through_the_request_path() {
    let (mut app, seen) = app_with_example();
    let request = CommandRequest::new(say_hello(), CommandArgs::none());
    let (outcome, _task) = app.run_command_request(RequestSurface::Gui { window: None }, request);
    assert_eq!(outcome, RequestOutcome::Ran);
    assert_eq!(seen.lock().expect("seen").hellos, 1);

    // An id in the example's namespace that it did not add is no command.
    let missing = AppCommandId::new("ext.example.missing").expect("an extension command id");
    let request = CommandRequest::new(missing, CommandArgs::none());
    let (outcome, _task) = app.run_command_request(RequestSurface::Gui { window: None }, request);
    assert!(
        matches!(
            outcome,
            RequestOutcome::Refused {
                cause: RefusalCause::NoSuchCommand,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(seen.lock().expect("seen").hellos, 1);
}

#[test]
fn the_command_runs_headless_and_through_the_request_path() {
    run_probe("headless");
}

// --- The headless child process ---------------------------------------------

const PROBE_ENV: &str = "SIGNEX_EXAMPLE_EXTENSION_PROBE";

fn run_probe(scenario: &str) {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["probe", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, scenario)
        .output()
        .expect("run the probe");
    assert!(
        output.status.success(),
        "scenario {scenario} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn probe() {
    let Ok(scenario) = std::env::var(PROBE_ENV) else {
        return;
    };
    match scenario.as_str() {
        "headless" => headless(),
        other => panic!("unknown scenario {other}"),
    }
}

/// Boot headless as `signex-cli` does, install the example, and run its
/// command by id: directly, then as a headless request.
fn headless() {
    let isolated = tempfile::tempdir().expect("override root");
    set_config_root_override(isolated.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let rules = RuleProfile::new("Headless", "1", ErcRules::default());
    let (booted, boot) = Signex::boot_headless(Vec::new(), &rules).expect("boot");
    let (host, seen) = example_host();
    let mut app = Box::new((*booted).with_extensions(host));
    let mut driver = TaskDriver::new();
    driver.settle(&mut app, boot).expect("boot settled");

    let (outcome, task) = app.dispatch_command_headless(&say_hello(), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::Ran);
    driver.settle(&mut app, task).expect("the run settled");

    let request = CommandRequest::new(say_hello(), CommandArgs::none());
    let (outcome, task) = app.run_command_request(RequestSurface::Headless, request);
    assert_eq!(outcome, RequestOutcome::Ran);
    driver.settle(&mut app, task).expect("the request settled");
    assert_eq!(seen.lock().expect("seen").hellos, 2);
}
