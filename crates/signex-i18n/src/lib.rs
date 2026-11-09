//! Message catalogs and locale-aware formatting for the Signex UI.
//!
//! The only crate that imports Fluent or ICU4X; a backend change touches
//! this crate alone. It is a UI-layer crate: the domain crates cannot depend
//! on it (`scripts/check_arch_guard.sh`).
//!
//! An [`I18n`] is plain data held in the app state, not a mutable global; the
//! one shared instance, [`I18n::en_us`], is a process-wide immutable fallback
//! bundle with no app state. [`t!`]
//! renders a message to [`Localized`], which is for display only; state and
//! files keep a [`MessageId`] and its arguments.

mod bundle;
mod catalog;
mod format;
mod i18n;
mod locale;
mod numbers;
mod text;
mod types;

pub use format::{DateLength, ListKind};
pub use i18n::{I18n, LanguageOption};
pub use locale::LanguagePref;
pub use types::{Arg, LocalNumber, Localized, MessageId, UserText};

/// Every message id, defined together with the [`ids::ALL`] list so a new
/// id cannot be left out of the consistency tests. This is the only non-test place
/// `MessageId::new` is called (it is crate-private).
macro_rules! define_ids {
    ($($name:ident = $id:literal;)+) => {
        /// The message ids; each lives in `locales/en-US/`.
        pub mod ids {
            use crate::MessageId;

            $(pub const $name: MessageId = MessageId::new($id);)+

            /// Every id above.
            #[cfg(test)]
            pub(crate) const ALL: &[MessageId] = &[$($name),+];
        }
    };
}

define_ids! {
    DRC_RUN = "drc-run";
    DRC_HEADING = "drc-heading";
    DRC_DEFAULT_CLEARANCE = "drc-default-clearance";
    DRC_IDLE = "drc-idle";
    DRC_RUNNING = "drc-running";
    DRC_CLEAN = "drc-clean";
    DRC_STALE = "drc-stale";
    DRC_SUMMARY = "drc-summary";
    DRC_SKIPPED = "drc-skipped";
    DRC_MORE = "drc-more";
    DRC_KIND_CLEARANCE = "drc-kind-clearance";
    DRC_KIND_SHORT = "drc-kind-short";
    DRC_KIND_UNCONNECTED = "drc-kind-unconnected";
    DRC_DETAIL_CLEARANCE = "drc-detail-clearance";
    DRC_DETAIL_SHORT = "drc-detail-short";
    DRC_DETAIL_UNCONNECTED = "drc-detail-unconnected";
    DRC_LOCATION = "drc-location";
    DRC_LAYER_TOP = "drc-layer-top";
    DRC_LAYER_BOTTOM = "drc-layer-bottom";
    DRC_LAYER_INNER = "drc-layer-inner";
    LANGUAGE_SYSTEM_DEFAULT = "language-system-default";
    MENU_FILE = "menu-file";
    PROPERTIES_LIBRARY_OUT_OF_DATE = "properties-library-out-of-date";
    PROPERTIES_LIBRARY_SYMBOL_MISSING = "properties-library-symbol-missing";
    PROPERTIES_LIBRARY_UPDATE = "properties-library-update";
    PROPERTIES_LIBRARY_UPDATE_TIP = "properties-library-update-tip";
    PROPERTIES_LIBRARY_OLDER_INSTANCES = "properties-library-older-instances";
    PROPERTIES_LIBRARY_ONLY_THIS_ONE = "properties-library-only-this-one";
    PROPERTIES_LIBRARY_ONLY_THIS_ONE_TIP = "properties-library-only-this-one-tip";
    PREFERENCES_AGENT_CONNECTIONS_LABEL = "preferences-agent-connections-label";
    PREFERENCES_AGENT_CONNECTIONS_HELP = "preferences-agent-connections-help";
    PCB_LAYERS_TITLE = "pcb-layers-title";
    PCB_LAYERS_EMPTY = "pcb-layers-empty";
    PCB_LAYERS_SHOW_ALL = "pcb-layers-show-all";
    PCB_LAYERS_COPPER = "pcb-layers-copper";
    PCB_LAYERS_TECHNICAL = "pcb-layers-technical";
    PCB_LAYERS_MECHANICAL = "pcb-layers-mechanical";
    PCB_STATUS_LAYER = "pcb-status-layer";
    PCB_LAYER_TOP_COPPER = "pcb-layer-top-copper";
    PCB_LAYER_BOTTOM_COPPER = "pcb-layer-bottom-copper";
    PCB_LAYER_INNER_COPPER = "pcb-layer-inner-copper";
    PCB_LAYER_MECHANICAL = "pcb-layer-mechanical";
    PCB_LAYER_EDGE_CUTS = "pcb-layer-edge-cuts";
    PCB_LAYER_MARGIN = "pcb-layer-margin";
    PCB_LAYER_SILKSCREEN_TOP = "pcb-layer-silkscreen-top";
    PCB_LAYER_SILKSCREEN_BOTTOM = "pcb-layer-silkscreen-bottom";
    PCB_LAYER_SOLDER_MASK_TOP = "pcb-layer-solder-mask-top";
    PCB_LAYER_SOLDER_MASK_BOTTOM = "pcb-layer-solder-mask-bottom";
    PCB_LAYER_PASTE_TOP = "pcb-layer-paste-top";
    PCB_LAYER_PASTE_BOTTOM = "pcb-layer-paste-bottom";
    PCB_LAYER_ADHESIVE_TOP = "pcb-layer-adhesive-top";
    PCB_LAYER_ADHESIVE_BOTTOM = "pcb-layer-adhesive-bottom";
    PCB_LAYER_FAB_TOP = "pcb-layer-fab-top";
    PCB_LAYER_FAB_BOTTOM = "pcb-layer-fab-bottom";
    PCB_LAYER_ASSEMBLY_TOP = "pcb-layer-assembly-top";
    PCB_LAYER_ASSEMBLY_BOTTOM = "pcb-layer-assembly-bottom";
    PCB_LAYER_COURTYARD_TOP = "pcb-layer-courtyard-top";
    PCB_LAYER_COURTYARD_BOTTOM = "pcb-layer-courtyard-bottom";
    PCB_LAYER_BODY_3D_TOP = "pcb-layer-body-3d-top";
    PCB_LAYER_BODY_3D_BOTTOM = "pcb-layer-body-3d-bottom";
    PCB_LAYER_KEEPOUT_TOP = "pcb-layer-keepout-top";
    PCB_LAYER_KEEPOUT_BOTTOM = "pcb-layer-keepout-bottom";
    PCB_LAYER_FIDUCIAL_TOP = "pcb-layer-fiducial-top";
    PCB_LAYER_FIDUCIAL_BOTTOM = "pcb-layer-fiducial-bottom";
    PCB_LAYER_DESIGNATOR_TOP = "pcb-layer-designator-top";
    PCB_LAYER_DESIGNATOR_BOTTOM = "pcb-layer-designator-bottom";
    PCB_LAYER_DIMENSIONS = "pcb-layer-dimensions";
    PCB_LAYER_NOTES = "pcb-layer-notes";
    PCB_LAYER_DRILL_DRAWING = "pcb-layer-drill-drawing";
    PCB_LAYER_ECO_1 = "pcb-layer-eco-1";
    PCB_LAYER_ECO_2 = "pcb-layer-eco-2";
    PCB_LAYERS_HIDE = "pcb-layers-hide";
    PCB_LAYERS_SHOW = "pcb-layers-show";
}

/// Render a message: `t!(i18n, ids::MENU_FILE)` or, with arguments,
/// `t!(i18n, ID, "count" => 3_i64, "name" => "R1")`.
#[macro_export]
macro_rules! t {
    ($i18n:expr, $id:expr) => {
        $i18n.message($id, &[])
    };
    ($i18n:expr, $id:expr, $($key:literal => $value:expr),+ $(,)?) => {
        $i18n.message($id, &[$(($key, $crate::Arg::from($value))),+])
    };
}

#[cfg(test)]
mod consistency_tests;
#[cfg(test)]
mod format_tests;
#[cfg(test)]
mod tests;
