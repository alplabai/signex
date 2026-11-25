//! A sample extension that adds one of everything, for the tests of every
//! extension point (#2495): the dock panel [`NOTES`], the Start-screen
//! section [`WELCOME`], the command [`SAY_HELLO`] and a Tools row and a
//! Route-group Active Bar row that run it, and the preference [`GREETING`].
//! What it sees is
//! recorded in a shared [`Seen`].

use std::sync::{Arc, Mutex};

use iced::{Element, Task, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::{button, text};

use super::points::{MenuRoot, PcbBarGroup, StartScreenSlot};
use super::{
    Extension, ExtensionHost, ExtensionId, ExtensionMsg, ExtensionPoints, ExtensionView,
    PreferenceValues, ViewContext,
};
use crate::app::Message;
use crate::dock::PanelPosition;

/// The sample's dock panel key.
pub(crate) const NOTES: &str = "ext.sample.notes";
/// The sample's Start-screen section key.
pub(crate) const WELCOME: &str = "ext.sample.welcome";
/// The command its menu and Active Bar rows run.
pub(crate) const SAY_HELLO: &str = "ext.sample.say_hello";
/// That command's catalog row: it runs headless as well as in a window.
static SAY_HELLO_ROW: super::CommandMetadata = super::CommandMetadata {
    id: SAY_HELLO,
    category: "sample",
    flags: super::CommandFlags::RUNS_HEADLESS,
    ..super::CommandMetadata::DEFAULT
};

/// What the sample's command handler sends; the sample ignores it.
#[derive(Debug, Clone, PartialEq)]
struct HelloSaid;
/// Its one preference.
pub(crate) const GREETING: &str = "ext.sample.greeting";
/// The catalog message every sample label uses ("Wiki").
pub(crate) const LABEL: signex_i18n::MessageId = ids::PANELS_KIND_WIKI;
/// The catalog message of the button each sample surface draws.
pub(crate) const BODY: signex_i18n::MessageId = ids::PANELS_STUB_WIKI;

/// The sample's own message: a surface or the extension was pinged.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pinged(pub u32);

/// What the sample saw.
#[derive(Debug, Default)]
pub(crate) struct Seen {
    /// `Pinged` values the extension's own `update` received.
    pub extension: Vec<u32>,
    /// `Pinged` values the panel's `update` received.
    pub panel: Vec<u32>,
    /// `Pinged` values the section's `update` received.
    pub section: Vec<u32>,
    /// The values `preferences_loaded` received.
    pub preferences: Option<PreferenceValues>,
}

/// Which list a [`Surface`] records into.
#[derive(Clone, Copy)]
enum Which {
    Panel,
    Section,
}

/// The sample's panel or section: one button that pings it.
struct Surface {
    owner: ExtensionId,
    key: &'static str,
    which: Which,
    seen: Arc<Mutex<Seen>>,
}

impl ExtensionView for Surface {
    fn view<'a>(&'a self, cx: ViewContext<'a>) -> Element<'a, ExtensionMsg> {
        let ping = ExtensionMsg::for_view(self.owner.clone(), self.key, Pinged(1));
        button(text(t!(cx.i18n, BODY))).on_press(ping).boxed()
    }

    fn update(&mut self, message: ExtensionMsg) -> Task<Message> {
        if let Some(Pinged(n)) = message.payload::<Pinged>() {
            let mut seen = self.seen.lock().expect("seen");
            match self.which {
                Which::Panel => seen.panel.push(*n),
                Which::Section => seen.section.push(*n),
            }
        }
        Task::none()
    }
}

struct Sample {
    seen: Arc<Mutex<Seen>>,
}

impl Extension for Sample {
    fn name(&self) -> &str {
        "sample"
    }

    fn register(&mut self, points: &mut ExtensionPoints) {
        let surface = |key, which| Surface {
            owner: points.owner().clone(),
            key,
            which,
            seen: Arc::clone(&self.seen),
        };
        let panel = surface(NOTES, Which::Panel);
        let section = surface(WELCOME, Which::Section);
        points
            .panels
            .add(NOTES, LABEL, PanelPosition::Right, panel)
            .expect("the panel");
        points
            .start_screen
            .add(WELCOME, LABEL, StartScreenSlot::HomeEnd, section)
            .expect("the section");
        points
            .commands
            .add(&SAY_HELLO_ROW, |_args| Ok(HelloSaid))
            .expect("the command its rows run");
        points
            .menu_rows
            .add_menu_row(MenuRoot::Tools, LABEL, SAY_HELLO)
            .expect("the menu row");
        points
            .menu_rows
            .add_active_bar_row(PcbBarGroup::Route, LABEL, SAY_HELLO)
            .expect("the Active Bar row");
        points
            .preferences
            .add(GREETING, serde_json::json!("hi"))
            .expect("the preference");
    }

    fn update(&mut self, message: ExtensionMsg) -> Task<Message> {
        if let Some(Pinged(n)) = message.payload::<Pinged>() {
            self.seen.lock().expect("seen").extension.push(*n);
        }
        Task::none()
    }

    fn preferences_loaded(&mut self, values: &PreferenceValues) {
        self.seen.lock().expect("seen").preferences = Some(values.clone());
    }
}

/// The sample installed alone, and what it sees.
pub(crate) fn sample_host() -> (ExtensionHost, Arc<Mutex<Seen>>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let sample = Sample {
        seen: Arc::clone(&seen),
    };
    let host = ExtensionHost::install(vec![Box::new(sample)]).expect("the sample installs");
    (host, seen)
}

/// The sample's id.
pub(crate) fn sample_id() -> ExtensionId {
    ExtensionId::new("sample").expect("valid name")
}
