//! The New Project dialog's draft (#2232): what the user typed and picked,
//! and the rules that decide whether Create is available. Pure data; the
//! disk check of the target lives in `target.rs`.

use std::path::{Path, PathBuf};

use signex_types::project::{PcbField, PcbValueError, ProjectSetup, SetupProfile, UserProfile};
use signex_types::schematic::SheetSize;

use super::NewProjectMsg;
use super::target::{TargetProblem, TargetStatus};

/// Characters a project name cannot hold: they are separators or reserved on
/// some platform's file systems.
const FORBIDDEN: [char; 9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// Device names Windows refuses as a file name, whatever the extension.
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Why a project name is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NameProblem {
    Empty,
    Invalid,
}

/// Check a project name as it would become a file stem.
pub(crate) fn check_name(name: &str) -> Result<&str, NameProblem> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameProblem::Empty);
    }
    let stem = name.split('.').next().unwrap_or(name);
    if name
        .chars()
        .any(|c| c.is_control() || FORBIDDEN.contains(&c))
        || name.ends_with('.')
        || RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem))
    {
        return Err(NameProblem::Invalid);
    }
    Ok(name)
}

/// The rule fields as typed. A value that does not parse keeps its text
/// and shows its reason; the setup keeps the last good value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuleDrafts {
    pub clearance: String,
    pub track: String,
    pub via: String,
    pub drill: String,
    pub thickness: String,
}

fn mm_text(value: f64) -> String {
    format!("{value}")
}

impl RuleDrafts {
    pub(crate) fn of(setup: &ProjectSetup) -> Self {
        Self {
            clearance: mm_text(setup.pcb.clearance_mm),
            track: mm_text(setup.pcb.track_width_mm),
            via: mm_text(setup.pcb.via_diameter_mm),
            drill: mm_text(setup.pcb.via_drill_mm),
            thickness: mm_text(setup.pcb.thickness_mm),
        }
    }

    pub(crate) fn text(&self, field: PcbField) -> &str {
        match field {
            PcbField::Clearance => &self.clearance,
            PcbField::TrackWidth => &self.track,
            PcbField::ViaSize => &self.via,
            PcbField::ViaDrill => &self.drill,
            PcbField::Thickness => &self.thickness,
            PcbField::CopperLayers => "",
        }
    }

    fn slot(&mut self, field: PcbField) -> Option<&mut String> {
        match field {
            PcbField::Clearance => Some(&mut self.clearance),
            PcbField::TrackWidth => Some(&mut self.track),
            PcbField::ViaSize => Some(&mut self.via),
            PcbField::ViaDrill => Some(&mut self.drill),
            PcbField::Thickness => Some(&mut self.thickness),
            PcbField::CopperLayers => None,
        }
    }

    /// Why `field`'s text is not usable, if it is not.
    pub(crate) fn problem(&self, field: PcbField) -> Option<PcbValueError> {
        let value = match field.parse(self.text(field)) {
            Ok(value) => value,
            Err(e) => return Some(e),
        };
        if field == PcbField::ViaDrill {
            let via = PcbField::ViaSize.parse(&self.via).ok()?;
            if value >= via {
                return Some(PcbValueError::DrillNotSmallerThanVia);
            }
        }
        None
    }
}

/// The rule fields in the order the dialog lists them.
pub(crate) const RULE_FIELDS: [PcbField; 5] = [
    PcbField::Thickness,
    PcbField::Clearance,
    PcbField::TrackWidth,
    PcbField::ViaSize,
    PcbField::ViaDrill,
];

/// Everything the open New Project dialog holds.
#[derive(Debug, Clone)]
pub struct NewProjectState {
    pub(crate) name: String,
    pub(crate) location: String,
    pub(crate) make_folder: bool,
    pub(crate) setup: ProjectSetup,
    pub(crate) customize_open: bool,
    pub(crate) drafts: RuleDrafts,
    /// Why the last imported rule file was not used.
    pub(crate) import_error: Option<super::RuleReadError>,
    /// The `.snxprj` a Create is writing; Create is off meanwhile.
    pub(crate) creating: Option<PathBuf>,
    /// Why the last Create failed, shown in the dialog.
    pub(crate) create_error: Option<String>,
    /// A dropdown list is open.
    pub(crate) dropdown_open: bool,
    pub(crate) target: TargetStatus,
    /// Bumped by every edit that changes the target; a check result for an
    /// older value is dropped.
    pub(crate) generation: u64,
    /// The Save as profile name field, while open.
    pub(crate) save_profile_name: Option<String>,
    /// Why the last Save as profile was refused.
    pub(crate) save_profile_problem: Option<signex_types::project::ProfileNameProblem>,
}

impl NewProjectState {
    /// A fresh dialog on the Standard profile. Nothing is on disk yet.
    pub(crate) fn new(location: String) -> Self {
        let setup = ProjectSetup::default();
        Self {
            name: String::new(),
            location,
            make_folder: true,
            drafts: RuleDrafts::of(&setup),
            setup,
            customize_open: false,
            import_error: None,
            creating: None,
            create_error: None,
            dropdown_open: false,
            target: TargetStatus::Unchecked,
            generation: 0,
            save_profile_name: None,
            save_profile_problem: None,
        }
    }

    pub(crate) fn name_problem(&self) -> Option<NameProblem> {
        check_name(&self.name).err()
    }

    /// The location as a path, `None` while empty.
    pub(crate) fn location_path(&self) -> Option<PathBuf> {
        let t = self.location.trim();
        (!t.is_empty()).then(|| PathBuf::from(t))
    }

    /// The `.snxprj` Create would write: with the folder,
    /// `<loc>/<name>/<name>.snxprj`, without it `<loc>/<name>.snxprj`.
    #[cfg(test)]
    pub(crate) fn project_file(&self) -> Option<PathBuf> {
        let name = check_name(&self.name).ok()?;
        let location = self.location_path()?;
        Some(project_file_in(&location, name, self.make_folder))
    }

    /// The label of the folder checkbox: tracks the name as it is typed.
    pub(crate) fn folder_label_name(&self) -> &str {
        self.name.trim()
    }

    pub(crate) fn rules_valid(&self) -> bool {
        RULE_FIELDS
            .iter()
            .all(|f| self.drafts.problem(*f).is_none())
    }

    /// The one inline reason Create is off, if any: a name or location
    /// problem, or what the disk check found.
    pub(crate) fn blocked_by(&self) -> Option<Blocker> {
        if self.name.trim().is_empty() {
            return Some(Blocker::Name(NameProblem::Empty));
        }
        if let Some(p) = self.name_problem() {
            return Some(Blocker::Name(p));
        }
        match &self.target {
            TargetStatus::Problem(p) => Some(Blocker::Target(p.clone())),
            _ => None,
        }
    }

    /// Create is available: a valid name, a checked and clear target, and
    /// valid rule values.
    pub(crate) fn can_create(&self) -> bool {
        self.blocked_by().is_none()
            && self.creating.is_none()
            && self.rules_valid()
            && matches!(self.target, TargetStatus::Clear)
    }

    /// Whether the target must be checked on disk again.
    pub(crate) fn wants_target_check(&self) -> bool {
        self.name_problem().is_none()
    }

    pub(crate) fn edit_rule(&mut self, field: PcbField, text: String) {
        if let Some(slot) = self.drafts.slot(field) {
            *slot = text;
        }
        if let Ok(v) = field.parse(self.drafts.text(field)) {
            match field {
                PcbField::Clearance => self.setup.pcb.clearance_mm = v,
                PcbField::TrackWidth => self.setup.pcb.track_width_mm = v,
                PcbField::ViaSize => self.setup.pcb.via_diameter_mm = v,
                PcbField::ViaDrill => self.setup.pcb.via_drill_mm = v,
                PcbField::Thickness => self.setup.pcb.thickness_mm = v,
                PcbField::CopperLayers => {}
            }
        }
    }

    /// Show the setup's rule values in the fields again (after a profile,
    /// template or file changed them).
    pub(crate) fn refresh_drafts(&mut self) {
        self.drafts = RuleDrafts::of(&self.setup);
    }

    pub(crate) fn pick_profile(&mut self, profile: SetupProfile) {
        self.setup.pick_profile(profile);
        self.import_error = None;
        self.refresh_drafts();
        if profile == SetupProfile::Custom {
            self.customize_open = true;
        }
    }

    /// Step the copper layer count; the profile becomes Custom and the
    /// customize section opens, as the stepper is in it.
    pub(crate) fn step_layers(&mut self, delta: i8) {
        let next = i16::from(self.setup.pcb.copper_layers) + i16::from(delta);
        let next = u8::try_from(next.clamp(1, 32)).unwrap_or(1);
        self.setup.set_layers(next);
        self.customize_open = true;
    }
}

impl NewProjectState {
    /// Apply a message that edits the setup (profile, rules, sheet),
    /// the part the dialog shares with the Project tab. `false` for any other
    /// message, which is left to the caller.
    pub(crate) fn apply_setup(&mut self, msg: NewProjectMsg, users: &[UserProfile]) -> bool {
        use NewProjectMsg as M;
        match msg {
            M::ProfilePicked(p) => self.pick_profile(p),
            M::UserProfilePicked(id) => {
                let Some(profile) = users.iter().find(|p| p.id == id) else {
                    return true;
                };
                self.setup.pick_user_profile(profile);
                self.import_error = None;
                self.refresh_drafts();
            }
            M::SaveProfileOpened => {
                let name = self
                    .setup
                    .user_profile
                    .as_ref()
                    .and_then(|id| users.iter().find(|p| p.id == *id))
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                self.save_profile_name = Some(name);
                self.save_profile_problem = None;
            }
            M::SaveProfileNameEdited(name) => {
                self.save_profile_name = Some(name);
                self.save_profile_problem = None;
            }
            M::SaveProfileCancelled => {
                self.save_profile_name = None;
                self.save_profile_problem = None;
            }
            M::SheetOrientationPicked(orientation) => {
                let (size, _) = SheetSize::parse(&self.setup.sheet.paper_size);
                self.setup.sheet.paper_size = size.token(orientation);
                self.dropdown_open = false;
            }
            M::CustomizeToggled => self.customize_open = !self.customize_open,
            M::FabPicked(fab) => {
                self.setup.apply_fab(fab);
                self.import_error = None;
                self.refresh_drafts();
            }
            M::RulesFileRead(Some(Ok(template))) => {
                self.setup.apply_template(&template);
                self.import_error = None;
                self.refresh_drafts();
            }
            M::RulesFileRead(Some(Err(why))) => self.import_error = Some(why),
            M::RulesFileRead(None) => {}
            M::LayersStepped(delta) => self.step_layers(delta),
            M::RuleEdited(field, text) => self.edit_rule(field, text),
            // A pick closes its list; `on_close` only fires on an outside
            // click, so the pick clears the open flag itself.
            M::SheetSizePicked(size) => {
                // The size keeps the orientation already chosen.
                let (_, orientation) = SheetSize::parse(&self.setup.sheet.paper_size);
                self.setup.sheet.paper_size = SheetSize::parse(&size).0.token(orientation);
                self.dropdown_open = false;
            }
            M::TitleBlockPicked(on) => {
                self.setup.sheet.show_title_block = on;
                self.dropdown_open = false;
            }
            M::DesignatorsPicked(standard) => {
                self.setup.designators = standard;
                self.dropdown_open = false;
            }
            M::DropdownToggled(open) => self.dropdown_open = open,
            _ => return false,
        }
        true
    }

    /// A draft for the Project tab, from what the project stores; the card
    /// its stored profile id names is picked when the user has it.
    pub(crate) fn of_project(
        data: &signex_types::project::ProjectData,
        users: &[UserProfile],
    ) -> Self {
        let mut state = Self::of_setup(
            ProjectSetup::for_project(&data.project_defaults, data.designator_standard(), users)
                .setup,
        );
        state.customize_open = true;
        state
    }

    /// A draft holding `setup`, for a dialog with no name or location.
    pub(crate) fn of_setup(setup: ProjectSetup) -> Self {
        let mut state = Self::new(String::new());
        state.setup = setup;
        state.refresh_drafts();
        state
    }
}

/// `<loc>/<name>/<name>.snxprj` or `<loc>/<name>.snxprj`.
pub(crate) fn project_file_in(location: &Path, name: &str, make_folder: bool) -> PathBuf {
    let file = format!("{name}.snxprj");
    if make_folder {
        location.join(name).join(file)
    } else {
        location.join(file)
    }
}

/// Why Create is off, for the inline message under the fields.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Blocker {
    Name(NameProblem),
    Target(TargetProblem),
}
