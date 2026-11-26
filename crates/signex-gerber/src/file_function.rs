//! The X2 `.FileFunction` value: what an image or drill file is for. The
//! same words the Signex exporter writes (`signex_output` `fab::layers`
//! and `fab::drill`).

/// A board face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    Top,
    Bottom,
}

/// Where a copper layer sits in the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CopperSide {
    Top,
    Inner,
    Bottom,
}

/// What a drill file's holes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DrillKind {
    Pth,
    Npth,
    Blind,
    Buried,
}

/// One `.FileFunction` value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FileFunction {
    /// `Copper,L2,Inr`: `number` is the 1-based position, L1 the top.
    Copper {
        number: u32,
        side: CopperSide,
    },
    Soldermask(Side),
    Paste(Side),
    Legend(Side),
    /// `Profile,NP` or `Profile,P`.
    Profile {
        plated: bool,
    },
    /// `Plated,1,2,PTH`, `NonPlated,1,2,NPTH`, `Plated,1,2,Blind`.
    Drill {
        plated: bool,
        from: u32,
        to: u32,
        kind: DrillKind,
    },
    /// Any other value, verbatim.
    Other(String),
}

impl FileFunction {
    /// Read a value. A known function with extra trailing fields (the
    /// copper type `Plane`, the drill form `Drill`) reads as that
    /// function; anything not understood is kept whole as `Other`.
    pub fn parse(value: &str) -> FileFunction {
        let fields: Vec<&str> = value.split(',').map(str::trim).collect();
        known(&fields).unwrap_or_else(|| FileFunction::Other(value.to_string()))
    }

    /// The canonical X2 value; `parse` reads it back to the same function.
    pub fn as_x2(&self) -> String {
        match self {
            FileFunction::Copper { number, side } => {
                let word = match side {
                    CopperSide::Top => "Top",
                    CopperSide::Inner => "Inr",
                    CopperSide::Bottom => "Bot",
                };
                format!("Copper,L{number},{word}")
            }
            FileFunction::Soldermask(side) => format!("Soldermask,{}", side_word(*side)),
            FileFunction::Paste(side) => format!("Paste,{}", side_word(*side)),
            FileFunction::Legend(side) => format!("Legend,{}", side_word(*side)),
            FileFunction::Profile { plated } => {
                format!("Profile,{}", if *plated { "P" } else { "NP" })
            }
            FileFunction::Drill {
                plated,
                from,
                to,
                kind,
            } => {
                let plating = if *plated { "Plated" } else { "NonPlated" };
                let kind = match kind {
                    DrillKind::Pth => "PTH",
                    DrillKind::Npth => "NPTH",
                    DrillKind::Blind => "Blind",
                    DrillKind::Buried => "Buried",
                };
                format!("{plating},{from},{to},{kind}")
            }
            FileFunction::Other(value) => value.clone(),
        }
    }

    /// The face of a copper, mask, paste or legend image; `None` for an
    /// inner layer and every other function.
    pub fn side(&self) -> Option<Side> {
        match self {
            FileFunction::Copper { side, .. } => match side {
                CopperSide::Top => Some(Side::Top),
                CopperSide::Bottom => Some(Side::Bottom),
                CopperSide::Inner => None,
            },
            FileFunction::Soldermask(side)
            | FileFunction::Paste(side)
            | FileFunction::Legend(side) => Some(*side),
            FileFunction::Profile { .. } | FileFunction::Drill { .. } | FileFunction::Other(_) => {
                None
            }
        }
    }

    pub fn is_copper(&self) -> bool {
        matches!(self, FileFunction::Copper { .. })
    }
}

fn side_word(side: Side) -> &'static str {
    match side {
        Side::Top => "Top",
        Side::Bottom => "Bot",
    }
}

fn parse_side(word: &str) -> Option<Side> {
    match word {
        "Top" => Some(Side::Top),
        "Bot" => Some(Side::Bottom),
        _ => None,
    }
}

fn known(fields: &[&str]) -> Option<FileFunction> {
    match *fields {
        ["Copper", layer, side, ..] => {
            let number = layer
                .strip_prefix('L')?
                .parse::<u32>()
                .ok()
                .filter(|n| *n > 0)?;
            let side = match side {
                "Top" => CopperSide::Top,
                "Inr" => CopperSide::Inner,
                "Bot" => CopperSide::Bottom,
                _ => return None,
            };
            Some(FileFunction::Copper { number, side })
        }
        ["Soldermask", side] => parse_side(side).map(FileFunction::Soldermask),
        ["Paste", side] => parse_side(side).map(FileFunction::Paste),
        ["Legend", side] => parse_side(side).map(FileFunction::Legend),
        ["Profile", "NP"] => Some(FileFunction::Profile { plated: false }),
        ["Profile", "P"] => Some(FileFunction::Profile { plated: true }),
        [plating @ ("Plated" | "NonPlated"), from, to, kind, ..] => {
            let kind = match kind {
                "PTH" => DrillKind::Pth,
                "NPTH" => DrillKind::Npth,
                "Blind" => DrillKind::Blind,
                "Buried" => DrillKind::Buried,
                _ => return None,
            };
            Some(FileFunction::Drill {
                plated: plating == "Plated",
                from: from.parse::<u32>().ok()?,
                to: to.parse::<u32>().ok()?,
                kind,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exporter_words_read_back() {
        assert_eq!(
            FileFunction::parse("Copper,L2,Inr"),
            FileFunction::Copper {
                number: 2,
                side: CopperSide::Inner
            }
        );
        assert_eq!(
            FileFunction::parse("Soldermask,Bot"),
            FileFunction::Soldermask(Side::Bottom)
        );
        assert_eq!(
            FileFunction::parse("Profile,NP"),
            FileFunction::Profile { plated: false }
        );
        assert_eq!(
            FileFunction::parse("NonPlated,1,4,NPTH"),
            FileFunction::Drill {
                plated: false,
                from: 1,
                to: 4,
                kind: DrillKind::Npth
            }
        );
        assert_eq!(
            FileFunction::parse("Vcut"),
            FileFunction::Other("Vcut".into())
        );
    }

    #[test]
    fn extra_fields_keep_the_function_and_junk_stays_verbatim() {
        assert!(FileFunction::parse("Copper,L3,Inr,Plane").is_copper());
        assert_eq!(
            FileFunction::parse("Plated,1,2,PTH,Drill"),
            FileFunction::parse("Plated,1,2,PTH")
        );
        assert_eq!(
            FileFunction::parse("Copper,L0,Top"),
            FileFunction::Other("Copper,L0,Top".into())
        );
        assert_eq!(
            FileFunction::parse("Legend,Left"),
            FileFunction::Other("Legend,Left".into())
        );
    }

    #[test]
    fn every_function_round_trips_through_its_x2_value() {
        let all = [
            FileFunction::Copper {
                number: 1,
                side: CopperSide::Top,
            },
            FileFunction::Copper {
                number: 3,
                side: CopperSide::Inner,
            },
            FileFunction::Copper {
                number: 4,
                side: CopperSide::Bottom,
            },
            FileFunction::Soldermask(Side::Top),
            FileFunction::Paste(Side::Bottom),
            FileFunction::Legend(Side::Top),
            FileFunction::Profile { plated: true },
            FileFunction::Profile { plated: false },
            FileFunction::Drill {
                plated: true,
                from: 1,
                to: 2,
                kind: DrillKind::Blind,
            },
            FileFunction::Drill {
                plated: true,
                from: 2,
                to: 3,
                kind: DrillKind::Buried,
            },
            FileFunction::Other("Keepout,Top".into()),
        ];
        for function in all {
            assert_eq!(FileFunction::parse(&function.as_x2()), function);
        }
        assert_eq!(
            FileFunction::Copper {
                number: 1,
                side: CopperSide::Top
            }
            .side(),
            Some(Side::Top)
        );
        assert_eq!(
            FileFunction::Copper {
                number: 2,
                side: CopperSide::Inner
            }
            .side(),
            None
        );
        assert_eq!(FileFunction::Paste(Side::Bottom).side(), Some(Side::Bottom));
        assert_eq!(FileFunction::Profile { plated: false }.side(), None);
    }
}
