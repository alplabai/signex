//! The Designators and Local nets format menus. A pick writes the project's
//! channel naming as one step of the project's General history, like the
//! annotation choices of the Project tab; Save writes the `.snxprj`.

use signex_types::project::ChannelNaming;
use signex_types::schematic::check_format;

use super::state::NamingMenu;
use crate::app::Signex;
use crate::panels::compiled_design::NamingTarget;

/// The preset formats of each menu, in menu order.
pub(crate) const DESIGNATOR_PRESETS: [&str; 4] = [
    "${Designator}_${Channel}",
    "${Channel}_${Designator}",
    "${Designator}${ChannelAlpha}",
    "${Designator}_${Index}",
];
pub(crate) const NET_PRESETS: [&str; 4] = [
    "${Net}_${Channel}",
    "${Channel}_${Net}",
    "${Net}${ChannelAlpha}",
    "${Net}_${Index}",
];

impl Signex {
    /// The active project's channel naming (the default outside a project).
    pub(crate) fn channel_naming(&self) -> ChannelNaming {
        self.compile_target()
            .and_then(|id| self.document_state.project_by_id(id))
            .map(|project| project.data.schematic_design.channel_naming.clone())
            .unwrap_or_default()
    }

    /// The active project's `schematic_design` section cannot be read, so
    /// its channel naming cannot be edited.
    pub(super) fn channel_naming_is_locked(&self) -> bool {
        self.compile_target()
            .and_then(|id| self.document_state.project_by_id(id))
            .is_some_and(|project| project.data.has_unreadable_schematic_design())
    }

    pub(super) fn toggle_naming_menu(&mut self, target: NamingTarget) {
        let store = &mut self.ui_state.compiled_design;
        let was_open = store
            .naming_menu
            .as_ref()
            .is_some_and(|m| m.target == target);
        store.naming_menu = (!was_open).then(|| NamingMenu {
            target,
            custom: String::new(),
        });
    }

    /// A preset was picked: write it and close the menu.
    pub(super) fn pick_naming(&mut self, target: NamingTarget, format: String) {
        self.ui_state.compiled_design.naming_menu = None;
        self.write_channel_naming(target, format);
    }

    /// Enter in the Custom format… field: a valid format is written and the
    /// menu closes; a refused one stays with its reason under the field.
    pub(super) fn submit_custom_naming(&mut self) {
        let Some(menu) = self.ui_state.compiled_design.naming_menu.clone() else {
            return;
        };
        if check_format(&menu.custom).is_err() {
            return;
        }
        self.ui_state.compiled_design.naming_menu = None;
        self.write_channel_naming(menu.target, menu.custom);
    }

    fn write_channel_naming(&mut self, target: NamingTarget, format: String) {
        let Some(project) = self.compile_target() else {
            return;
        };
        if self.refuse_schematic_design_edit(project) {
            return;
        }
        let mut naming = self.channel_naming();
        match target {
            NamingTarget::Designators => naming.designator_format = format,
            NamingTarget::Nets => naming.net_format = format,
        }
        let command = signex_engine::ProjectSectionCommand::SetChannelNaming { naming };
        if let Err(problem) = self.apply_project_section(project, &command) {
            crate::diagnostics::log_warning(
                signex_i18n::t!(
                    self.ui_state.i18n,
                    signex_i18n::ids::PANELS_COMPILED_DESIGN_NAMING_REFUSED,
                    "why" => problem.to_string()
                )
                .into_string(),
            );
        }
    }
}
