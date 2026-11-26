//! The extension module: names, install, and routing of `Message::Extension`
//! to its owner alone (#2495).

use std::sync::{Arc, Mutex};

use iced::Task;

use super::{
    API_VERSION, Extension, ExtensionHost, ExtensionId, ExtensionMsg, ExtensionPoints, InstallError,
};
use crate::app::{Message, Signex};

/// A sample extension's own message type.
#[derive(Debug, Clone, PartialEq)]
struct Pinged(u32);

/// What a [`Recorder`] saw: its owner id at register, then one entry per
/// message, the `Pinged` value or `None` for another payload type.
#[derive(Debug, Default)]
struct Seen {
    registered_as: Vec<String>,
    messages: Vec<Option<u32>>,
}

struct Recorder {
    name: &'static str,
    seen: Arc<Mutex<Seen>>,
}

impl Extension for Recorder {
    fn name(&self) -> &str {
        self.name
    }

    fn register(&mut self, points: &mut ExtensionPoints) {
        let mut seen = self.seen.lock().expect("seen");
        seen.registered_as.push(points.owner().name().to_owned());
    }

    fn update(&mut self, message: ExtensionMsg) -> Task<Message> {
        let ping = message.payload::<Pinged>().map(|p| p.0);
        self.seen.lock().expect("seen").messages.push(ping);
        Task::none()
    }
}

fn recorder(name: &'static str) -> (Box<dyn Extension>, Arc<Mutex<Seen>>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let extension = Recorder {
        name,
        seen: Arc::clone(&seen),
    };
    (Box::new(extension), seen)
}

/// A name that registers nothing and handles nothing, for install checks.
struct Named(&'static str);

impl Extension for Named {
    fn name(&self) -> &str {
        self.0
    }
}

fn id(name: &str) -> ExtensionId {
    ExtensionId::new(name).expect("valid name")
}

fn messages(seen: &Arc<Mutex<Seen>>) -> Vec<Option<u32>> {
    seen.lock().expect("seen").messages.clone()
}

#[test]
fn the_api_version_is_one() {
    assert_eq!(API_VERSION, 1);
}

#[test]
fn a_name_is_a_lowercase_identifier() {
    for name in ["sample", "s", "sample_2", "a1"] {
        assert_eq!(id(name).name(), name);
    }
    for name in [
        "", "Sample", "2nd", "_sample", "sam-ple", "sam.ple", "sample ", "ü",
    ] {
        assert!(ExtensionId::new(name).is_err(), "{name:?} was accepted");
    }
}

#[test]
fn an_id_owns_only_ids_under_its_own_prefix() {
    let sample = id("sample");
    assert_eq!(sample.prefix(), "ext.sample.");
    assert!(sample.owns("ext.sample.say_hello"));
    assert!(sample.owns("ext.sample.panel.notes"));
    assert!(!sample.owns("ext.sample."));
    assert!(!sample.owns("ext.sample"));
    assert!(!sample.owns("ext.samples.say_hello"));
    assert!(!sample.owns("ext.other.say_hello"));
    assert!(!sample.owns("sample.say_hello"));
    assert!(!sample.owns("say_hello"));
}

#[test]
fn install_refuses_an_invalid_or_repeated_name() {
    let invalid = ExtensionHost::install(vec![Box::new(Named("Bad"))]);
    assert!(matches!(invalid, Err(InstallError::InvalidName(_))));

    let repeated = ExtensionHost::install(vec![Box::new(Named("one")), Box::new(Named("one"))]);
    assert_eq!(
        repeated.err(),
        Some(InstallError::DuplicateName("one".to_owned()))
    );
}

#[test]
fn install_registers_each_extension_once_under_its_own_id() {
    let (first, first_seen) = recorder("first");
    let (second, second_seen) = recorder("second");
    let host = ExtensionHost::install(vec![first, second]).expect("install");

    assert_eq!(first_seen.lock().expect("seen").registered_as, ["first"]);
    assert_eq!(second_seen.lock().expect("seen").registered_as, ["second"]);
    let ids: Vec<&str> = host.ids().map(ExtensionId::name).collect();
    assert_eq!(ids, ["first", "second"]);
    let owners: Vec<&str> = host.points().map(|p| p.owner().name()).collect();
    assert_eq!(owners, ["first", "second"]);
    assert!(host.points().all(ExtensionPoints::is_empty));
}

/// The Community binary installs no extension: the model it boots holds none,
/// an empty install changes nothing, and a stray extension message is dropped.
#[test]
fn a_model_booted_without_extensions_has_none_and_drops_extension_messages() {
    let (app, _boot) = Signex::new();
    assert!(app.extensions().is_empty());

    let host = ExtensionHost::install(Vec::new()).expect("empty install");
    assert!(host.is_empty());
    let mut app = app.with_extensions(host);
    assert!(app.extensions().is_empty());

    let _task = app.update(Message::Extension(ExtensionMsg::new(
        id("sample"),
        Pinged(1),
    )));
    assert!(app.extensions().is_empty());
}

#[test]
fn an_extension_message_reaches_its_owner_and_no_other_extension() {
    let (first, first_seen) = recorder("first");
    let (second, second_seen) = recorder("second");
    let host = ExtensionHost::install(vec![first, second]).expect("install");
    let (app, _boot) = Signex::new();
    let mut app = app.with_extensions(host);

    let _task = app.update(Message::Extension(ExtensionMsg::new(
        id("first"),
        Pinged(7),
    )));
    assert_eq!(messages(&first_seen), [Some(7)]);
    assert!(messages(&second_seen).is_empty());

    // Another payload type still reaches the owner, which cannot read it as
    // its own.
    let _task = app.update(Message::Extension(ExtensionMsg::new(id("second"), "text")));
    assert_eq!(messages(&first_seen), [Some(7)]);
    assert_eq!(messages(&second_seen), [None]);

    // A built-in message and one for an id nobody installed reach no one.
    let _task = app.update(Message::noop());
    let _task = app.update(Message::Extension(ExtensionMsg::new(
        id("third"),
        Pinged(9),
    )));
    assert_eq!(messages(&first_seen), [Some(7)]);
    assert_eq!(messages(&second_seen), [None]);
}

#[test]
fn a_payload_reads_back_only_as_its_own_type() {
    let message = ExtensionMsg::new(id("sample"), Pinged(3));
    assert_eq!(message.extension().name(), "sample");
    assert_eq!(message.payload::<Pinged>(), Some(&Pinged(3)));
    assert_eq!(message.payload::<u32>(), None);
    // A clone shares the payload; the log line names the extension.
    let copy = message.clone();
    assert_eq!(copy.payload::<Pinged>(), Some(&Pinged(3)));
    assert!(format!("{copy:?}").contains("sample"));
}
