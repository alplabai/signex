//! The draft behind the New Project dialog (#2232): the fab tier card, its
//! copper layer count and every value, as one plain struct the dialog and
//! the `new_project_with_profile` command both drive. Tiers and layer
//! counts: `docs/PROJECT_SETUP.md`, "Profiles".

use super::rule_file::RuleTemplate;
use super::setup::{
    COPPER_LAYERS, DEFAULT_DESIGNATORS, PcbDefaults, PcbField, ProjectDefaults, SheetDefaults,
};
use super::user_profile::{
    PROJECT_PROFILE_TOKEN, ProfileRef, UserProfile, find_user_profile, resolve_profile,
};
use crate::designator::DesignatorStandard;
use crate::schematic::{SheetOrientation, SheetSize};

/// A starting point for the board rules: a fab tier (#2537), or Custom.
/// The copper layer count is a separate choice on every card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetupProfile {
    Standard,
    Advanced,
    Hdi,
    Custom,
}

/// Board thickness every tier starts from, in mm.
const TIER_THICKNESS_MM: f64 = 1.6;

/// The copper layer stepper on a card moves by this many layers.
pub const LAYER_STEP: u8 = 2;

/// The fewest copper layers the card stepper offers.
pub const MIN_STEPPED_LAYERS: u8 = 2;

impl SetupProfile {
    pub const ALL: [Self; 4] = [Self::Standard, Self::Advanced, Self::Hdi, Self::Custom];

    /// The fab tiers, in card order.
    pub const TIERS: [Self; 3] = [Self::Standard, Self::Advanced, Self::Hdi];

    pub const fn token(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Advanced => "advanced",
            Self::Hdi => "hdi",
            Self::Custom => "custom",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.token() == token)
    }

    /// The generic fab class this tier is; `None` for Custom.
    pub const fn fab_template(self) -> Option<FabTemplate> {
        match self {
            Self::Standard => Some(FabTemplate::Standard),
            Self::Advanced => Some(FabTemplate::Advanced),
            Self::Hdi => Some(FabTemplate::Hdi),
            Self::Custom => None,
        }
    }

    /// The tier card of a generic fab class.
    pub const fn from_fab(fab: FabTemplate) -> Self {
        match fab {
            FabTemplate::Standard => Self::Standard,
            FabTemplate::Advanced => Self::Advanced,
            FabTemplate::Hdi => Self::Hdi,
        }
    }

    /// The copper layer count the card's stepper starts on.
    pub const fn default_layers(self) -> u8 {
        match self {
            Self::Standard | Self::Custom => 4,
            Self::Advanced => 6,
            Self::Hdi => 8,
        }
    }

    /// `(clearance, track, via size, via drill)` in mm. Clearance and via
    /// are the fab tier's limits (`FabTemplate::capabilities`, the one tier
    /// table); the track is the card's own, never under the tier's floor.
    /// Custom starts from Standard's.
    pub const fn rule_values(self) -> (f64, f64, f64, f64) {
        let fab = match self.fab_template() {
            Some(fab) => fab,
            None => FabTemplate::Standard,
        };
        let c = fab.capabilities();
        (
            c.clearance,
            self.card_track_mm(),
            c.via_diameter,
            c.via_drill,
        )
    }

    /// The track width a card starts on, in mm (#2537).
    const fn card_track_mm(self) -> f64 {
        match self {
            Self::Standard | Self::Custom => 0.2,
            Self::Advanced => 0.125,
            Self::Hdi => 0.075,
        }
    }

    /// The profile's shipped rules on `layers` copper layers.
    pub fn rules_on(self, layers: u8) -> PcbDefaults {
        let (clearance, track, via, drill) = self.rule_values();
        PcbDefaults {
            copper_layers: layers,
            thickness_mm: TIER_THICKNESS_MM,
            clearance_mm: clearance,
            track_width_mm: track,
            via_diameter_mm: via,
            via_drill_mm: drill,
        }
    }

    /// The profile's shipped rules on its default layer count; Custom on
    /// `custom_layers`.
    pub fn pcb(self, custom_layers: u8) -> PcbDefaults {
        let layers = match self {
            Self::Custom => custom_layers,
            tier => tier.default_layers(),
        };
        self.rules_on(layers)
    }
}

/// `layers` moved by `delta` on the card stepper: even counts from
/// [`MIN_STEPPED_LAYERS`] to 32. An odd count steps on to the next even one
/// in the direction moved.
pub fn stepped_layers(layers: u8, delta: i8) -> u8 {
    let mut next = i16::from(layers) + i16::from(delta);
    if next % i16::from(LAYER_STEP) != 0 {
        next += i16::from(delta.signum());
    }
    let max = i16::from(*COPPER_LAYERS.end());
    u8::try_from(next.clamp(i16::from(MIN_STEPPED_LAYERS), max)).unwrap_or(MIN_STEPPED_LAYERS)
}

/// A generic fabrication capability class: the minimums of the base DRC
/// rules a fab of that class can make (`fab_tier.rs`). No fabricator is
/// named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FabTemplate {
    Standard,
    Advanced,
    Hdi,
}

impl FabTemplate {
    pub const ALL: [Self; 3] = [Self::Standard, Self::Advanced, Self::Hdi];

    /// The stable id (`standard`, `advanced`, `hdi`) a preset, an override
    /// and a script name the tier by.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Advanced => "advanced",
            Self::Hdi => "hdi",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.token() == token)
    }
}

/// Everything the dialog decides.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSetup {
    pub profile: SetupProfile,
    /// The tier whose `fab_*` rules the new project gets, when the Fab tier
    /// field picked one.
    pub fab: Option<FabTemplate>,
    /// An imported rule file is applied.
    pub imported: bool,
    pub pcb: PcbDefaults,
    pub sheet: SheetDefaults,
    pub designators: DesignatorStandard,
    /// The user profile picked, by id; `None` while a built-in card is.
    pub user_profile: Option<String>,
    /// Each card's stepper, in [`SetupProfile::ALL`] order; `None` shows
    /// the profile's own count.
    card_layers: [Option<u8>; 4],
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

const fn card_slot(profile: SetupProfile) -> usize {
    match profile {
        SetupProfile::Standard => 0,
        SetupProfile::Advanced => 1,
        SetupProfile::Hdi => 2,
        SetupProfile::Custom => 3,
    }
}

impl ProjectSetup {
    /// The draft on `profile`'s shipped values and default layer count.
    pub fn from_profile(profile: SetupProfile) -> Self {
        Self {
            profile,
            fab: None,
            imported: false,
            pcb: profile.rules_on(profile.default_layers()),
            sheet: SheetDefaults::default(),
            designators: DEFAULT_DESIGNATORS,
            user_profile: None,
            card_layers: [None; 4],
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

    /// The layer count `profile`'s card shows: the one stepped to, else the
    /// profile's own (an edited built-in's, else the shipped default).
    pub fn card_layers(&self, profile: SetupProfile, users: &[UserProfile]) -> u8 {
        self.card_layers[card_slot(profile)].unwrap_or_else(|| match profile {
            SetupProfile::Custom => profile.default_layers(),
            tier => tier.values(users).copper_layers,
        })
    }

    /// Pick a profile card. A tier takes its rules (an edited built-in's
    /// override, else the shipped table) on the card's layer count; Custom
    /// keeps the rules shown and takes its card's layer count.
    pub fn pick_profile(&mut self, profile: SetupProfile, users: &[UserProfile]) {
        let layers = self.card_layers(profile, users);
        self.user_profile = None;
        self.profile = profile;
        if profile != SetupProfile::Custom {
            self.fab = None;
            self.imported = false;
            self.pcb = profile.values(users);
        }
        self.pcb.copper_layers = layers;
    }

    /// The stepper on `profile`'s card moved by `delta` layers: that card is
    /// picked on the new count.
    pub fn step_card_layers(&mut self, profile: SetupProfile, delta: i8, users: &[UserProfile]) {
        let layers = stepped_layers(self.card_layers(profile, users), delta);
        self.card_layers[card_slot(profile)] = Some(layers);
        self.pick_profile(profile, users);
    }

    /// Set the copper layer count, clamped to what a board can have. The
    /// picked card keeps its tier: the layer count is its own choice.
    pub fn set_layers(&mut self, layers: u8) {
        let layers = layers.clamp(*COPPER_LAYERS.start(), *COPPER_LAYERS.end());
        if self.user_profile.is_none() {
            self.card_layers[card_slot(self.profile)] = Some(layers);
        }
        self.pcb.copper_layers = layers;
    }

    /// Step the copper layer count as the card stepper does.
    pub fn step_layers(&mut self, delta: i8) {
        self.set_layers(stepped_layers(self.pcb.copper_layers, delta));
    }

    /// The Fab tier field picked `fab`: its card, the tier's rules on the
    /// layer count shown, and the tier's `fab_*` rules in the new project.
    pub fn apply_fab(&mut self, fab: FabTemplate) {
        let layers = self.pcb.copper_layers;
        let (clearance, track, via, drill) = SetupProfile::from_fab(fab).rule_values();
        self.set_rules(clearance, track, via, drill);
        self.user_profile = None;
        self.profile = SetupProfile::from_fab(fab);
        self.set_layers(layers);
        self.fab = Some(fab);
        self.imported = false;
    }

    /// Fill the rules from an imported rule file: they are no tier's, so
    /// the Custom card is picked.
    pub fn apply_template(&mut self, template: &RuleTemplate) {
        self.set_rules(
            template.clearance_mm,
            template.track_width_mm,
            template.via_diameter_mm,
            template.via_drill_mm,
        );
        self.user_profile = None;
        self.profile = SetupProfile::Custom;
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

    /// The defaults to store in the new `.snxprj`. The dimension defaults
    /// are not part of a setup: they come from Preferences (#2868).
    pub fn defaults(&self) -> ProjectDefaults {
        ProjectDefaults {
            pcb: Some(self.pcb.clone()),
            sheet: Some(self.sheet.clone()),
            profile: Some(self.profile_id()),
            dimension: None,
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
                Some(ProfileRef::BuiltIn(profile) | ProfileRef::Edited { profile, .. }) => {
                    if defaults.pcb.is_none() {
                        setup.pick_profile(profile, users);
                    } else {
                        setup.profile = profile;
                        setup.user_profile = None;
                        setup.card_layers[card_slot(profile)] = Some(setup.pcb.copper_layers);
                    }
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
            setup.pick_profile(builtin, users);
            return Ok(setup);
        }
        if let Some(user) = find_user_profile(token, users) {
            let mut setup = project.setup.clone();
            setup.pick_user_profile(user);
            return Ok(setup);
        }
        Err(format!(
            "profile `{token}` is not project, standard, advanced, hdi, custom or a saved profile"
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

    /// The draft for defaults a project already stores: the tier whose
    /// shipped rules they match on their layer count (else Custom), and the
    /// stored sheet and designator standard. A project that stores none
    /// starts on Standard.
    pub fn from_stored(
        defaults: &ProjectDefaults,
        designators: Option<DesignatorStandard>,
    ) -> Self {
        let mut setup = Self::default();
        if let Some(pcb) = &defaults.pcb {
            setup.profile = SetupProfile::TIERS
                .into_iter()
                .find(|p| p.rules_on(pcb.copper_layers) == *pcb)
                .unwrap_or(SetupProfile::Custom);
            setup.card_layers[card_slot(setup.profile)] = Some(pcb.copper_layers);
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
    /// `title_block` (`true`/`false`), `designators` (`ieee315`,
    /// `din40719_2`, `iec81346_2`, `iec81346_2_full`) and `fab` (`standard`,
    /// `advanced`, `hdi`: the tier's rules, and its `fab_*` rules in the new
    /// project). Keys apply in written order.
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
            "fab" => {
                let tier = FabTemplate::from_token(value).ok_or_else(|| {
                    format!("override `fab`: {value} is not standard, advanced or hdi")
                })?;
                self.apply_fab(tier);
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
                            "override `designators`: {value} is not ieee315, din40719_2, \
                             iec81346_2 or iec81346_2_full"
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
        DesignatorStandard::Iec81346_2Full => "iec81346_2_full",
    }
}
