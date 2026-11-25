//! What Auto Place weighs (#2941): the four priorities and their weights,
//! and the roles a part plays, read from its `Placement` property and its
//! designator.

use signex_types::pcb::Footprint;

/// One placement priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Parts that share nets sit close together.
    Connection,
    /// Hot parts keep the thermal gap from each other and from
    /// heat-sensitive parts.
    Thermal,
    /// Noisy parts keep the separation gap from sensitive parts.
    Separation,
    /// Connectors sit at the board edge.
    Edges,
}

impl Priority {
    pub const ALL: [Priority; 4] = [
        Priority::Connection,
        Priority::Thermal,
        Priority::Separation,
        Priority::Edges,
    ];

    /// The name a `weights` list uses.
    pub const fn token(self) -> &'static str {
        match self {
            Priority::Connection => "connection",
            Priority::Thermal => "thermal",
            Priority::Separation => "separation",
            Priority::Edges => "edges",
        }
    }
}

/// How much each priority counts. A weight is finite and not below zero;
/// zero turns the priority off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    pub connection: f64,
    pub thermal: f64,
    pub separation: f64,
    pub edges: f64,
}

impl Default for Weights {
    /// Every priority counts once.
    fn default() -> Self {
        Weights {
            connection: 1.0,
            thermal: 1.0,
            separation: 1.0,
            edges: 1.0,
        }
    }
}

impl Weights {
    pub fn of(&self, priority: Priority) -> f64 {
        match priority {
            Priority::Connection => self.connection,
            Priority::Thermal => self.thermal,
            Priority::Separation => self.separation,
            Priority::Edges => self.edges,
        }
    }

    fn set(&mut self, priority: Priority, weight: f64) {
        match priority {
            Priority::Connection => self.connection = weight,
            Priority::Thermal => self.thermal = weight,
            Priority::Separation => self.separation = weight,
            Priority::Edges => self.edges = weight,
        }
    }

    /// `connection:3,thermal:1,...` over the defaults: a priority left out
    /// keeps its default weight. An unknown name, a weight that is not a
    /// number, a negative one or a priority named twice is refused.
    pub fn parse(text: &str) -> Result<Weights, String> {
        let mut weights = Weights::default();
        let mut seen: Vec<Priority> = Vec::new();
        for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let Some((name, value)) = part.split_once(':') else {
                return Err(format!("`{part}` is not <priority>:<weight>"));
            };
            let name = name.trim();
            let Some(priority) = Priority::ALL.into_iter().find(|p| p.token() == name) else {
                return Err(format!(
                    "`{name}` is not a priority (connection, thermal, separation, edges)"
                ));
            };
            let weight = value
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|w| w.is_finite() && *w >= 0.0)
                .ok_or_else(|| format!("the weight of `{name}` must be a number from 0 up"))?;
            if seen.contains(&priority) {
                return Err(format!("`{name}` is named twice"));
            }
            seen.push(priority);
            weights.set(priority, weight);
        }
        Ok(weights)
    }
}

/// The footprint property whose value lists a part's placement roles,
/// comma or space separated: `hot`, `heat_sensitive`, `noisy`, `sensitive`,
/// `connector`. The key is matched without case.
pub const PLACEMENT_PROPERTY: &str = "Placement";

/// Designator prefixes that make a part a connector without the property.
const CONNECTOR_PREFIXES: [&str; 3] = ["J", "P", "CN"];

/// The roles a part plays in Auto Place.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartRoles {
    pub hot: bool,
    pub heat_sensitive: bool,
    pub noisy: bool,
    pub sensitive: bool,
    pub connector: bool,
}

impl PartRoles {
    pub fn of(fp: &Footprint) -> PartRoles {
        let mut roles = PartRoles::default();
        let tokens = fp
            .properties
            .iter()
            .filter(|p| p.key.trim().eq_ignore_ascii_case(PLACEMENT_PROPERTY))
            .flat_map(|p| {
                p.value
                    .split([',', ' ', ';'])
                    .map(|t| t.trim().to_ascii_lowercase())
                    .collect::<Vec<_>>()
            });
        for token in tokens {
            match token.as_str() {
                "hot" => roles.hot = true,
                "heat_sensitive" => roles.heat_sensitive = true,
                "noisy" => roles.noisy = true,
                "sensitive" => roles.sensitive = true,
                "connector" => roles.connector = true,
                _ => {}
            }
        }
        let prefix: String = fp
            .reference
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect::<String>()
            .to_ascii_uppercase();
        let numbered = fp.reference.len() > prefix.len();
        roles.connector |= numbered && CONNECTOR_PREFIXES.contains(&prefix.as_str());
        roles
    }

    /// Whether the two keep the thermal gap: two hot parts, or a hot part
    /// and a heat-sensitive one.
    pub fn thermal_pair(self, other: PartRoles) -> bool {
        (self.hot && (other.hot || other.heat_sensitive)) || (other.hot && self.heat_sensitive)
    }

    /// Whether the two keep the separation gap: a noisy part and a
    /// sensitive one.
    pub fn separation_pair(self, other: PartRoles) -> bool {
        (self.noisy && other.sensitive) || (self.sensitive && other.noisy)
    }
}
