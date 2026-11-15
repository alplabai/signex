//! The draft behind the New Project dialog (#2232): profile, fab template
//! and every value, as one plain struct the dialog and the
//! `new_project_with_profile` command both drive.

use super::rule_file::RuleTemplate;
use super::setup::{
    COPPER_LAYERS, DEFAULT_DESIGNATORS, PcbDefaults, PcbField, ProjectDefaults, SheetDefaults,
};
use super::user_profile::{
    PROJECT_PROFILE_TOKEN, ProfileRef, UserProfile, find_user_profile, resolve_profile,
};
use crate::designator::DesignatorStandard;
use crate::schematic::{SheetOrientation, SheetSize};

/// A starting point for the board rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetupProfile {
    Hobby,
    Standard,
    Hdi,
    Custom,
}

impl SetupProfile {
    pub const ALL: [Self; 4] = [Self::Hobby, Self::Standard, Self::Hdi, Self::Custom];

    pub const fn token(self) -> &'static str {
        match self {
            Self::Hobby => "hobby",
            Self::Standard => "standard",
            Self::Hdi => "hdi",
            Self::Custom => "custom",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.token() == token)
    }

    /// The rules of this profile. `Custom` starts from the standard values
    /// on `custom_layers` copper layers.
    pub fn pcb(self, custom_layers: u8) -> PcbDefaults {
        let (layers, thickness, clearance, track, via, drill) = match self {
            Self::Hobby => (2, 1.6, 0.2, 0.25, 0.6, 0.3),
            Self::Standard => (4, 1.6, 0.15, 0.2, 0.5, 0.25),
            Self::Hdi => (6, 1.2, 0.1, 0.1, 0.3, 0.15),
            Self::Custom => (custom_layers, 1.6, 0.15, 0.2, 0.5, 0.25),
        };
        PcbDefaults {
            copper_layers: layers,
            thickness_mm: thickness,
            clearance_mm: clearance,
            track_width_mm: track,
            via_diameter_mm: via,
            via_drill_mm: drill,
        }
    }
}

/// A generic fabrication capability class: the minimum clearance, track and
/// via sizes a fab of that class can make. No fabricator is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FabTemplate {
    Standard,
    Advanced,
    Hdi,
}

impl FabTemplate {
    pub const ALL: [Self; 3] = [Self::Standard, Self::Advanced, Self::Hdi];

    /// `(clearance, track, via size, via drill)` in mm.
    pub const fn minimums(self) -> (f64, f64, f64, f64) {
        match self {
            Self::Standard => (0.15, 0.15, 0.5, 0.25),
            Self::Advanced => (0.1, 0.1, 0.4, 0.2),
            Self::Hdi => (0.075, 0.075, 0.3, 0.15),
        }
    }
}

/// Where the stepper starts when the user first picks Custom.
const CUSTOM_START_LAYERS: u8 = 8;

/// Everything the dialog decides.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSetup {
    pub profile: SetupProfile,
    /// The generic class last applied, if any.
    pub fab: Option<FabTemplate>,
    /// An imported rule file is applied.
    pub imported: bool,
    pub pcb: PcbDefaults,
    pub sheet: SheetDefaults,
    pub designators: DesignatorStandard,
    /// The user profile picked, by id; `None` while a built-in card is.
    pub user_profile: Option<String>,
    custom_layers: u8,
}

/// A project's setup as a dialog starts from it, and the user profile it
/// names that is not in the user's settings, if any.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSetup {
    pub setup: ProjectSetup,
    pub missing_profile: Option<String>,
}

impl Default for ProjectSetup {
    fn default() -> Self {
        Self::from_profile(SetupProfile::Standard)
    }
}

impl ProjectSetup {
    pub fn from_profile(profile: SetupProfile) -> Self {
        Self {
            profile,
            fab: None,
            imported: false,
            pcb: profile.pcb(CUSTOM_START_LAYERS),
            sheet: SheetDefaults::default(),
            designators: DEFAULT_DESIGNATORS,
            user_profile: None,
            custom_layers: CUSTOM_START_LAYERS,
        }
    }

    /// The draft of a user profile: its rules, sheet and designators.
    pub fn from_user_profile(profile: &UserProfile) -> Self {
        let mut setup = Self::default();
        setup.pick_user_profile(profile);
        setup
    }

    /// Pick a user profile card: its values replace the rules, the sheet
    /// and the designator standard.
    pub fn pick_user_profile(&mut self, profile: &UserProfile) {
        self.pcb = profile.pcb.clone();
        self.sheet = profile.sheet.clone();
        self.designators = profile.designators;
        self.fab = None;
        self.imported = false;
        self.select_user(&profile.id);
    }

    fn select_user(&mut self, id: &str) {
        self.profile = SetupProfile::Custom;
        self.custom_layers = self.pcb.copper_layers;
        self.user_profile = Some(id.to_string());
    }

    /// The id of the picked profile: the user profile's, else the built-in
    /// token.
    pub fn profile_id(&self) -> String {
        self.user_profile
            .clone()
            .unwrap_or_else(|| self.profile.token().to_string())
    }

    /// Whether the built-in card `profile` is the one picked.
    pub fn is_builtin_picked(&self, profile: SetupProfile) -> bool {
        self.user_profile.is_none() && self.profile == profile
    }

    /// Pick a profile card. A fixed profile resets the rules and layers to
    /// its values; Custom keeps what is set.
    pub fn pick_profile(&mut self, profile: SetupProfile) {
        self.user_profile = None;
        self.profile = profile;
        if profile == SetupProfile::Custom {
            self.pcb.copper_layers = self.custom_layers;
        } else {
            self.fab = None;
            self.imported = false;
            self.pcb = profile.pcb(self.custom_layers);
        }
    }

    /// The layer count the Custom card shows and uses.
    pub fn custom_layers(&self) -> u8 {
        self.custom_layers
    }

    /// Set the copper layer count; the profile becomes Custom.
    pub fn set_layers(&mut self, layers: u8) {
        let layers = layers.clamp(*COPPER_LAYERS.start(), *COPPER_LAYERS.end());
        self.custom_layers = layers;
        self.pcb.copper_layers = layers;
        self.profile = SetupProfile::Custom;
        self.user_profile = None;
    }

    /// Fill clearance, track and via sizes from a generic class.
    pub fn apply_fab(&mut self, fab: FabTemplate) {
        let (clearance, track, via, drill) = fab.minimums();
        self.set_rules(clearance, track, via, drill);
        self.fab = Some(fab);
        self.imported = false;
    }

    /// Fill the rules from an imported rule file.
    pub fn apply_template(&mut self, template: &RuleTemplate) {
        self.set_rules(
            template.clearance_mm,
            template.track_width_mm,
            template.via_diameter_mm,
            template.via_drill_mm,
        );
        if let Some(layers) = template.copper_layers {
            self.set_layers(layers);
        }
        if let Some(thickness) = template.thickness_mm {
            self.pcb.thickness_mm = thickness;
        }
        self.fab = None;
        self.imported = true;
    }

    fn set_rules(&mut self, clearance: f64, track: f64, via: f64, drill: f64) {
        self.pcb.clearance_mm = clearance;
        self.pcb.track_width_mm = track;
        self.pcb.via_diameter_mm = via;
        self.pcb.via_drill_mm = drill;
    }

    /// The defaults to store in the new `.snxprj`.
    pub fn defaults(&self) -> ProjectDefaults {
        ProjectDefaults {
            pcb: Some(self.pcb.clone()),
            sheet: Some(self.sheet.clone()),
            profile: Some(self.profile_id()),
        }
    }

    /// The draft a dialog of `defaults`' project starts from. The stored
    /// values win; the stored profile id picks the card. A part the project
    /// does not store comes from the named profile. A user profile that is
    /// not in `users` leaves the stored values (else Standard) and is
    /// returned so the caller can say so.
    pub fn for_project(
        defaults: &ProjectDefaults,
        designators: Option<DesignatorStandard>,
        users: &[UserProfile],
    ) -> ResolvedSetup {
        let mut setup = Self::from_stored(defaults, designators);
        let mut missing_profile = None;
        match defaults.profile.as_deref() {
            None => {}
            Some(id) => match resolve_profile(id, users) {
                Some(ProfileRef::BuiltIn(profile)) if defaults.pcb.is_none() => {
                    setup.pick_profile(profile);
                }
                Some(ProfileRef::BuiltIn(profile)) => {
                    setup.profile = profile;
                    setup.user_profile = None;
                }
                Some(ProfileRef::User(user)) => {
                    if defaults.pcb.is_none() {
                        setup.pcb = user.pcb.clone();
                    }
                    if defaults.sheet.is_none() {
                        setup.sheet = user.sheet.clone();
                    }
                    if designators.is_none() {
                        setup.designators = user.designators;
                    }
                    setup.select_user(&user.id);
                }
                None => missing_profile = Some(id.to_string()),
            },
        }
        ResolvedSetup {
            setup,
            missing_profile,
        }
    }

    /// The setup a command's `profile` operand names: a built-in token, a
    /// user profile by id or name, or `project` for `project`'s own
    /// (`for_project`). `Err` says why the token is refused.
    pub fn for_command(
        token: &str,
        project: &ResolvedSetup,
        users: &[UserProfile],
    ) -> Result<Self, String> {
        let token = token.trim();
        if token == PROJECT_PROFILE_TOKEN {
            return Ok(project.setup.clone());
        }
        if let Some(builtin) = SetupProfile::from_token(token) {
            let mut setup = project.setup.clone();
            setup.pick_profile(builtin);
            return Ok(setup);
        }
        if let Some(user) = find_user_profile(token, users) {
            let mut setup = project.setup.clone();
            setup.pick_user_profile(user);
            return Ok(setup);
        }
        Err(format!(
            "profile `{token}` is not project, hobby, standard, hdi, custom or a saved profile"
        ))
    }

    /// The preview's sheet line: the paper and its orientation, such as
    /// `A3 landscape`.
    pub fn sheet_line(&self) -> String {
        let (size, orientation) = SheetSize::parse(&self.sheet.paper_size);
        let word = match orientation {
            SheetOrientation::Landscape => "landscape",
            SheetOrientation::Portrait => "portrait",
        };
        format!("{} {word}", size.token(SheetOrientation::Landscape))
    }

    /// The draft for defaults a project already stores: the profile whose
    /// rules they match (else Custom), and the stored sheet and designator
    /// standard. A project that stores none starts on Standard.
    pub fn from_stored(
        defaults: &ProjectDefaults,
        designators: Option<DesignatorStandard>,
    ) -> Self {
        let mut setup = Self::default();
        if let Some(pcb) = &defaults.pcb {
            setup.custom_layers = pcb.copper_layers;
            setup.profile = [
                SetupProfile::Hobby,
                SetupProfile::Standard,
                SetupProfile::Hdi,
            ]
            .into_iter()
            .find(|p| p.pcb(pcb.copper_layers) == *pcb)
            .unwrap_or(SetupProfile::Custom);
            setup.pcb = pcb.clone();
        }
        if let Some(sheet) = &defaults.sheet {
            setup.sheet = sheet.clone();
        }
        if let Some(standard) = designators {
            setup.designators = standard;
        }
        setup
    }

    /// Apply `key=value,key=value` overrides on top of the profile, the form
    /// the `new_project_with_profile` command takes. Keys: `layers`,
    /// `thickness`, `clearance`, `track`, `via`, `drill` (mm), `sheet`,
    /// `title_block` (`true`/`false`) and `designators` (`ieee315`,
    /// `din40719_2`, `iec81346_2`).
    pub fn apply_overrides(&mut self, text: &str) -> Result<(), String> {
        for pair in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| format!("override `{pair}` is not key=value"))?;
            self.apply_override(key.trim(), value.trim())?;
        }
        match self.pcb.problems().first() {
            Some((field, why)) => Err(format!("{} {why}", field.key())),
            None => Ok(()),
        }
    }

    fn apply_override(&mut self, key: &str, value: &str) -> Result<(), String> {
        let mm = |field: PcbField| {
            field
                .parse(value)
                .map_err(|why| format!("override `{key}`: {value} {why}"))
        };
        match key {
            "layers" => {
                let n: u8 = value
                    .parse()
                    .map_err(|_| format!("override `layers`: {value} is not a whole number"))?;
                if !COPPER_LAYERS.contains(&n) {
                    return Err(format!("override `layers`: {n} must be 1 to 32"));
                }
                self.set_layers(n);
            }
            "thickness" => self.pcb.thickness_mm = mm(PcbField::Thickness)?,
            "clearance" => self.pcb.clearance_mm = mm(PcbField::Clearance)?,
            "track" => self.pcb.track_width_mm = mm(PcbField::TrackWidth)?,
            "via" => self.pcb.via_diameter_mm = mm(PcbField::ViaSize)?,
            "drill" => self.pcb.via_drill_mm = mm(PcbField::ViaDrill)?,
            "sheet" => {
                if !SheetSize::parse(value).0.is_recognized() {
                    return Err(format!("override `sheet`: {value} is not a sheet size"));
                }
                self.sheet.paper_size = value.to_string();
            }
            "title_block" => {
                self.sheet.show_title_block = value
                    .parse()
                    .map_err(|_| format!("override `title_block`: {value} is not true or false"))?;
            }
            "designators" => {
                self.designators = DesignatorStandard::ALL
                    .into_iter()
                    .find(|d| designator_token(*d) == value)
                    .ok_or_else(|| {
                        format!(
                            "override `designators`: {value} is not ieee315, din40719_2 or \
                             iec81346_2"
                        )
                    })?;
            }
            other => return Err(format!("unknown override `{other}`")),
        }
        Ok(())
    }
}

/// The word the command uses for a standard.
pub const fn designator_token(standard: DesignatorStandard) -> &'static str {
    match standard {
        DesignatorStandard::Ieee315 => "ieee315",
        DesignatorStandard::Din40719_2 => "din40719_2",
        DesignatorStandard::Iec81346_2 => "iec81346_2",
    }
}
