//! The report's sections and the options of a run (mockup row 201): the
//! template, document number and revision, the ten sections, and whether the
//! checks run now or the last results are used.

/// One section of the report, in report order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Section {
    Summary,
    Requirements,
    Architecture,
    Decisions,
    Bom,
    Checks,
    Simulation,
    Outputs,
    OpenItems,
    Changelog,
}

impl Section {
    /// Every section, in report order.
    pub const ALL: [Section; 10] = [
        Section::Summary,
        Section::Requirements,
        Section::Architecture,
        Section::Decisions,
        Section::Bom,
        Section::Checks,
        Section::Simulation,
        Section::Outputs,
        Section::OpenItems,
        Section::Changelog,
    ];

    /// The name the command line and the JSON answer use: `summary`,
    /// `requirements`, `architecture`, `decisions`, `bom`, `checks`,
    /// `simulation`, `outputs`, `open-items`, `changelog`.
    pub fn token(self) -> &'static str {
        match self {
            Section::Summary => "summary",
            Section::Requirements => "requirements",
            Section::Architecture => "architecture",
            Section::Decisions => "decisions",
            Section::Bom => "bom",
            Section::Checks => "checks",
            Section::Simulation => "simulation",
            Section::Outputs => "outputs",
            Section::OpenItems => "open-items",
            Section::Changelog => "changelog",
        }
    }

    /// The section named `token`; exact and lower case.
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|section| section.token() == token)
    }

    /// The heading the report prints. The report is a project document,
    /// written in English like the other generated reports.
    pub fn heading(self) -> &'static str {
        match self {
            Section::Summary => "Summary",
            Section::Requirements => "Requirements and how each was met",
            Section::Architecture => "Architecture",
            Section::Decisions => "Key decisions",
            Section::Bom => "Bill of materials summary",
            Section::Checks => "Check results",
            Section::Simulation => "Simulation results",
            Section::Outputs => "Outputs produced",
            Section::OpenItems => "Open items",
            Section::Changelog => "Change log",
        }
    }
}

/// The sections a comma-separated list names (`requirements,checks`), in
/// report order and each once; `all` names every one. Refused when the list
/// is empty or names a section that does not exist.
pub fn parse_sections(text: &str) -> Result<Vec<Section>, String> {
    let names: Vec<&str> = text
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        return Err("name at least one section".to_string());
    }
    if names == ["all"] {
        return Ok(Section::ALL.to_vec());
    }
    let mut picked = Vec::new();
    for name in names {
        let Some(section) = Section::parse(name) else {
            let known: Vec<&str> = Section::ALL.iter().map(|s| s.token()).collect();
            return Err(format!(
                "`{name}` is not a section; the sections are {}, or all",
                known.join(", ")
            ));
        };
        picked.push(section);
    }
    picked.sort();
    picked.dedup();
    Ok(picked)
}

/// Where the check results come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChecksSource {
    /// Run ERC, DRC and the requirement checks now (drawn as the default).
    #[default]
    RunNow,
    /// Use the newest runs the app keeps for the project.
    LastResults,
}

impl ChecksSource {
    pub const ALL: [ChecksSource; 2] = [ChecksSource::RunNow, ChecksSource::LastResults];

    /// `run` or `last`.
    pub fn token(self) -> &'static str {
        match self {
            ChecksSource::RunNow => "run",
            ChecksSource::LastResults => "last",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|source| source.token() == token)
    }
}

/// The page layout of the report. One is drawn: Design report (A4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Template {
    #[default]
    DesignReportA4,
}

impl Template {
    pub const ALL: [Template; 1] = [Template::DesignReportA4];

    /// `design-report-a4`.
    pub fn token(self) -> &'static str {
        match self {
            Template::DesignReportA4 => "design-report-a4",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|template| template.token() == token)
    }
}

/// What one run of Generate makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateOptions {
    /// The sections, in report order.
    pub sections: Vec<Section>,
    pub template: Template,
    /// The report's document number (`DN-0043`), printed in the footer.
    pub document: Option<String>,
    /// The report's own revision (`A`), printed in the footer.
    pub revision: Option<String>,
    pub checks: ChecksSource,
    /// The conditions that make the run's status `fail` (`--fail-on`).
    pub fail_on: Vec<super::FailOn>,
}

impl Default for GenerateOptions {
    /// Every section, the A4 design report, no document number or revision,
    /// checks run now: the options as drawn.
    fn default() -> Self {
        Self {
            sections: Section::ALL.to_vec(),
            template: Template::default(),
            document: None,
            revision: None,
            checks: ChecksSource::default(),
            fail_on: Vec::new(),
        }
    }
}

impl GenerateOptions {
    /// Is `section` in the report?
    pub fn has(&self, section: Section) -> bool {
        self.sections.contains(&section)
    }

    /// The same options with `section` in the report if it was not, out of
    /// it if it was; the sections stay in report order.
    pub fn toggled(mut self, section: Section) -> Self {
        if self.has(section) {
            self.sections.retain(|s| *s != section);
        } else {
            self.sections.push(section);
            self.sections.sort();
        }
        self
    }
}
