//! Extended commands (`%...%`): format, units, apertures and macros, load
//! polarity, X2 attributes, step and repeat, block apertures, and the
//! deprecated image commands. A deprecated command at its identity value
//! (`%IPPOS*%`, `%MIA0B0*%`, `%LMN*%`, `%LR0*%`, ...) changes nothing and
//! is accepted; any other value, and `%LM`/`%LR`/`%LS` transforms, are
//! reported as unsupported and the objects drawn untransformed.

use pcb_geom::PointNm;

use super::macros::parse_macro;
use super::state::{Reader, StepRepeat, Template};
use crate::image::{ApertureDef, Polarity, Units};
use crate::number::{CoordFormat, Omitted, decimal_to_nm};
use crate::problem::ProblemKind;

/// The most objects one step and repeat may add.
const MAX_REPEATED_OBJECTS: usize = 1_000_000;

/// `FSLAX46Y46` as its format and whether coordinates are incremental.
fn parse_format(statement: &str) -> Option<(CoordFormat, bool)> {
    let rest = statement.strip_prefix("FS")?;
    let mut chars = rest.chars();
    let omitted = match chars.next()? {
        // `D` (no zeros left out) reads the same either way.
        'L' | 'D' => Omitted::Leading,
        'T' => Omitted::Trailing,
        _ => return None,
    };
    let incremental = match chars.next()? {
        'A' => false,
        'I' => true,
        _ => return None,
    };
    let digits = rest.get(rest.find('X')? + 1..)?;
    let mut digits = digits.chars().map_while(|c| c.to_digit(10));
    let (integer, decimal) = (digits.next()?, digits.next()?);
    Some((
        CoordFormat {
            integer,
            decimal,
            omitted,
        },
        incremental,
    ))
}

/// Whether every number in `rest` (split at its letters) is `identity`.
fn is_identity(rest: &str, identity: f64) -> bool {
    rest.split(|c: char| c.is_ascii_alphabetic())
        .filter(|s| !s.is_empty())
        .all(|n| n.parse::<f64>().is_ok_and(|v| v == identity))
}

/// The value after `letter` in `rest`, up to the next letter.
fn letter_value(rest: &str, letter: char) -> Option<&str> {
    let start = rest.find(letter)? + 1;
    let tail = &rest[start..];
    let end = tail
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(tail.len());
    Some(&tail[..end])
}

impl Reader<'_> {
    pub(super) fn extended(&mut self, statements: &[(String, u32)], line: u32, closed: bool) {
        if !closed {
            self.report_once("end", line, ProblemKind::MissingEndOfFile);
        }
        let Some((first, _)) = statements.first() else {
            return;
        };
        if first.starts_with("AM") {
            self.define_macro(statements, line);
            return;
        }
        for (statement, at) in statements {
            self.statement(statement, *at);
        }
    }

    fn statement(&mut self, statement: &str, line: u32) {
        let code = statement.get(..2).unwrap_or(statement);
        let rest = statement.get(2..).unwrap_or_default();
        let accepted = match code {
            "FS" => {
                self.coordinate_format(statement, line);
                true
            }
            "MO" => self.units_statement(rest),
            "AD" => {
                self.define_aperture(statement, line);
                true
            }
            "LP" => self.load_polarity(rest),
            "TF" | "TA" | "TO" | "TD" => {
                self.attribute(statement, line);
                true
            }
            "SR" => {
                self.step_repeat(rest, line);
                true
            }
            "AB" => {
                self.block_aperture(statement, line);
                true
            }
            "LM" => rest == "N",
            "LR" | "OF" | "MI" => is_identity(rest, 0.0),
            "LS" | "SF" => is_identity(rest, 1.0),
            "IP" => rest == "POS",
            "AS" => rest == "AXBY",
            "IR" => is_identity(rest, 0.0),
            // Image and layer names: labels only.
            "IN" | "LN" => true,
            _ => false,
        };
        if !accepted {
            self.unsupported(line, statement);
        }
    }

    fn coordinate_format(&mut self, statement: &str, line: u32) {
        match parse_format(statement) {
            Some((format, incremental)) => {
                self.format = Some(format);
                if incremental {
                    self.unsupported(line, statement);
                }
            }
            None => self.report_once("format", line, ProblemKind::MissingFormat),
        }
    }

    fn units_statement(&mut self, rest: &str) -> bool {
        self.image.units = Some(match rest {
            "MM" => Units::Millimetres,
            "IN" => Units::Inches,
            _ => return false,
        });
        true
    }

    fn load_polarity(&mut self, rest: &str) -> bool {
        self.polarity = match rest {
            "D" => Polarity::Dark,
            "C" => Polarity::Clear,
            _ => return false,
        };
        true
    }

    fn define_macro(&mut self, statements: &[(String, u32)], line: u32) {
        let Some(defined) = parse_macro(statements) else {
            let first = statements.first().map_or("AM", |(s, _)| s.as_str());
            self.unsupported(line, first);
            return;
        };
        if defined.bad {
            let name = defined.name.clone();
            self.report(line, ProblemKind::BadMacroParameters { name });
        }
        self.macros.insert(defined.name.clone(), defined);
    }

    /// `%SRX3Y2I5.0J4.0*%` opens a step and repeat, `%SR*%` (or the next
    /// `%SR`) closes it and places the copies.
    fn step_repeat(&mut self, rest: &str, line: u32) {
        self.close_step();
        if rest.is_empty() {
            return;
        }
        let units = self.units(line);
        let count = |letter| letter_value(rest, letter).and_then(|v| v.parse::<u32>().ok());
        let step = |letter| letter_value(rest, letter).map_or(Some(0), |v| decimal_to_nm(v, units));
        let (Some(nx), Some(ny), Some(dx), Some(dy)) =
            (count('X'), count('Y'), step('I'), step('J'))
        else {
            self.unsupported(line, &format!("SR{rest}"));
            return;
        };
        if nx == 0 || ny == 0 {
            self.unsupported(line, &format!("SR{rest}"));
            return;
        }
        if nx > 1 || ny > 1 {
            self.step = Some(StepRepeat {
                first_object: self.image.objects.len(),
                nx,
                ny,
                step: PointNm::new(dx, dy),
                line,
            });
        }
    }

    /// Place the copies of the open step and repeat's objects.
    pub(super) fn close_step(&mut self) {
        let Some(step) = self.step.take() else {
            return;
        };
        let first = step.first_object.min(self.image.objects.len());
        let base = self.image.objects.split_off(first);
        if base.is_empty() {
            return;
        }
        let copies = (step.nx as usize).saturating_mul(step.ny as usize);
        if copies.saturating_mul(base.len()) > MAX_REPEATED_OBJECTS {
            self.unsupported(step.line, "SR with more copies than can be drawn");
            self.image.objects.extend(base);
            return;
        }
        for iy in 0..i64::from(step.ny) {
            for ix in 0..i64::from(step.nx) {
                let by = PointNm::new(
                    ix.saturating_mul(step.step.x),
                    iy.saturating_mul(step.step.y),
                );
                self.image
                    .objects
                    .extend(base.iter().map(|o| o.translated(by)));
            }
        }
    }

    /// `%ABD12*%` opens block aperture D12; `%AB*%` closes the last open
    /// block and its objects become the aperture.
    fn block_aperture(&mut self, statement: &str, line: u32) {
        let rest = &statement[2..];
        if !rest.is_empty() {
            let d_code = rest
                .strip_prefix('D')
                .and_then(|d| d.parse::<u32>().ok())
                .filter(|d| *d >= 10);
            match d_code {
                Some(d_code) => self.blocks.push((d_code, line, self.image.objects.len())),
                None => self.unsupported(line, statement),
            }
            return;
        }
        let Some((d_code, opened, first)) = self.blocks.pop() else {
            self.unsupported(line, statement);
            return;
        };
        let objects = self
            .image
            .objects
            .split_off(first.min(self.image.objects.len()));
        if self.templates.contains_key(&d_code) {
            self.report(opened, ProblemKind::BadApertureDefinition { d_code });
            return;
        }
        self.templates.insert(d_code, Template::Block(objects));
        self.image.apertures.insert(
            d_code,
            ApertureDef {
                d_code,
                definition: format!("%ABD{d_code}*%"),
                function: None,
                at: self.source.at(opened),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_formats_read_their_digits() {
        let (format, incremental) = parse_format("FSLAX46Y46").expect("a format");
        assert_eq!(
            format,
            CoordFormat {
                integer: 4,
                decimal: 6,
                omitted: Omitted::Leading
            }
        );
        assert!(!incremental);
        let (format, incremental) = parse_format("FSTIX24Y24").expect("a format");
        assert_eq!(format.omitted, Omitted::Trailing);
        assert_eq!((format.integer, format.decimal), (2, 4));
        assert!(incremental);
        assert_eq!(parse_format("FSLA"), None);
        assert_eq!(parse_format("FSQAX46Y46"), None);
    }

    #[test]
    fn identity_values_are_recognised() {
        assert!(is_identity("A0B0", 0.0));
        assert!(is_identity("A1.0B1", 1.0));
        assert!(!is_identity("A0B1", 0.0));
        assert!(is_identity("0", 0.0));
        assert!(!is_identity("45", 0.0));
        assert_eq!(letter_value("X3Y2I5.0J4.0", 'I'), Some("5.0"));
        assert_eq!(letter_value("X3Y2", 'I'), None);
    }
}
