//! What each image is: its X2 file function, else the job file's entry for
//! it, else a guess from the file name. See `docs/GERBER_VIEWER.md`,
//! "Package".

use crate::{CopperSide, DrillKind, FileFunction, FileKind, Image, JobFile, Side};

use super::{NameSource, PackageLayer};

/// The layers of `images`, in the order given, unpaired.
pub(super) fn name_images(images: Vec<Image>, job: Option<&JobFile>) -> Vec<PackageLayer> {
    let mut layers: Vec<PackageLayer> = images
        .into_iter()
        .map(|image| {
            let (function, name_source) = match image.function.clone() {
                Some(function) => (Some(function), NameSource::X2),
                None => match job.and_then(|job| job_function(job, &image.file)) {
                    Some(function) => (Some(function), NameSource::JobFile),
                    None => (None, NameSource::FileName),
                },
            };
            PackageLayer {
                image,
                function,
                name_source,
                file_label: None,
                pair: None,
            }
        })
        .collect();
    let guesses: Vec<Guess> = layers
        .iter()
        .map(|layer| match layer.name_source {
            NameSource::FileName => guess(&layer.image),
            NameSource::X2 | NameSource::JobFile => Guess::Unknown,
        })
        .collect();
    let copper = copper_count(&layers, &guesses, job.and_then(|job| job.copper_layers));
    for (layer, guess) in layers.iter_mut().zip(guesses) {
        if layer.name_source == NameSource::FileName {
            layer.file_label = Some(label_from_file_name(&layer.image.file));
            layer.function = guess.resolve(copper);
        }
    }
    layers
}

/// The job file's function for `file`: the entry whose path names the same
/// file (folders and the case of the letters aside).
fn job_function(job: &JobFile, file: &str) -> Option<FileFunction> {
    let name = base_name(file);
    job.entries
        .iter()
        .find(|entry| base_name(&entry.path).eq_ignore_ascii_case(name))
        .map(|entry| entry.function.clone())
}

/// The file name without its folders.
fn base_name(file: &str) -> &str {
    file.rsplit(['/', '\\']).next().unwrap_or(file)
}

/// A name for a layer from its file name: the name without folders and
/// extension, `-`, `_` and `.` read as spaces, the first letter a
/// capital. "fab-notes.gbr" is "Fab notes". This is the user's own file
/// name, shown as it is, not interface text.
pub fn label_from_file_name(file: &str) -> String {
    let name = base_name(file);
    let stem = match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    };
    let words: Vec<&str> = stem
        .split(|c: char| c == '-' || c == '_' || c == '.' || c.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect();
    let joined = words.join(" ");
    let mut chars = joined.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => name.to_string(),
    }
}

/// What a file name says a layer is, before the copper count is known.
#[derive(Debug, Clone, PartialEq)]
enum Guess {
    Known(FileFunction),
    /// Copper layer `n` by position: L1 is Top, the last is Bottom.
    Copper(u32),
    BottomCopper,
    Drill {
        plated: bool,
    },
    Unknown,
}

impl Guess {
    fn resolve(self, copper: u32) -> Option<FileFunction> {
        let copper = copper.max(2);
        match self {
            Guess::Known(function) => Some(function),
            Guess::Copper(number) => Some(copper_function(number, copper)),
            Guess::BottomCopper => Some(copper_function(copper, copper)),
            Guess::Drill { plated } => Some(FileFunction::Drill {
                plated,
                from: 1,
                to: copper,
                kind: if plated {
                    DrillKind::Pth
                } else {
                    DrillKind::Npth
                },
            }),
            Guess::Unknown => None,
        }
    }
}

/// Copper layer `number` of `count`, its side by position.
fn copper_function(number: u32, count: u32) -> FileFunction {
    let side = if number == 1 {
        CopperSide::Top
    } else if number >= count {
        CopperSide::Bottom
    } else {
        CopperSide::Inner
    };
    FileFunction::Copper { number, side }
}

/// The package's copper count: the job file's layer number, else the
/// highest copper number any layer has or a file name gives, one more for
/// a bottom copper named without one.
fn copper_count(layers: &[PackageLayer], guesses: &[Guess], job: Option<u32>) -> u32 {
    if let Some(count) = job.filter(|count| *count >= 1) {
        return count;
    }
    let known = layers
        .iter()
        .filter_map(|layer| layer.function.as_ref())
        .filter_map(copper_number);
    let guessed = guesses.iter().filter_map(|guess| match guess {
        Guess::Copper(number) => Some(*number),
        _ => None,
    });
    let highest = known.chain(guessed).max().unwrap_or(1);
    let bottom_unnumbered = guesses.contains(&Guess::BottomCopper);
    if bottom_unnumbered && !has_bottom_copper(layers) {
        highest.saturating_add(1)
    } else {
        highest
    }
}

fn has_bottom_copper(layers: &[PackageLayer]) -> bool {
    layers.iter().any(|layer| {
        matches!(
            layer.function,
            Some(FileFunction::Copper {
                side: CopperSide::Bottom,
                ..
            })
        )
    })
}

/// The copper number of a copper function.
pub(super) fn copper_number(function: &FileFunction) -> Option<u32> {
    match function {
        FileFunction::Copper { number, .. } => Some(*number),
        _ => None,
    }
}

/// What the file name of `image` says it is. A drill file is a drill
/// layer whatever its name; the words of the name are read in lower case.
fn guess(image: &Image) -> Guess {
    let name = base_name(&image.file).to_ascii_lowercase();
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) => (stem.to_string(), extension.to_string()),
        None => (name.clone(), String::new()),
    };
    let words: Vec<&str> = stem
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    if matches!(image.kind, FileKind::Excellon) {
        let plated = !words
            .iter()
            .any(|w| matches!(*w, "npth" | "nonplated" | "unplated"));
        return Guess::Drill { plated };
    }
    if let Some(guess) = by_extension(&extension) {
        return guess;
    }
    by_words(&words)
}

/// The usual three-letter Gerber extensions.
fn by_extension(extension: &str) -> Option<Guess> {
    use Side::{Bottom, Top};
    Some(match extension {
        "gtl" => Guess::Copper(1),
        "gbl" => Guess::BottomCopper,
        "gto" => Guess::Known(FileFunction::Legend(Top)),
        "gbo" => Guess::Known(FileFunction::Legend(Bottom)),
        "gts" => Guess::Known(FileFunction::Soldermask(Top)),
        "gbs" => Guess::Known(FileFunction::Soldermask(Bottom)),
        "gtp" => Guess::Known(FileFunction::Paste(Top)),
        "gbp" => Guess::Known(FileFunction::Paste(Bottom)),
        "gko" | "gm1" => Guess::Known(FileFunction::Profile { plated: false }),
        _ => return None,
    })
}

/// The words of a file name: a layer word and, for a side layer, a side.
fn by_words(words: &[&str]) -> Guess {
    let has = |list: &[&str]| words.iter().any(|w| list.contains(w));
    let side = if has(&["top", "front"]) {
        Some(Side::Top)
    } else if has(&["bottom", "bot", "back"]) {
        Some(Side::Bottom)
    } else {
        None
    };
    if has(&["profile", "outline", "edge", "edgecuts", "boardoutline"]) {
        return Guess::Known(FileFunction::Profile { plated: false });
    }
    let side_layer = |make: fn(Side) -> FileFunction| match side {
        Some(side) => Guess::Known(make(side)),
        None => Guess::Unknown,
    };
    if has(&["silk", "silkscreen", "legend", "overlay"]) {
        return side_layer(FileFunction::Legend);
    }
    if has(&["mask", "soldermask"]) {
        return side_layer(FileFunction::Soldermask);
    }
    if has(&["paste", "stencil"]) {
        return side_layer(FileFunction::Paste);
    }
    if let Some(number) = words.iter().find_map(|w| layer_number(w)) {
        return Guess::Copper(number);
    }
    if has(&["copper", "cu", "signal"]) {
        return match side {
            Some(Side::Top) => Guess::Copper(1),
            Some(Side::Bottom) => Guess::BottomCopper,
            None => Guess::Unknown,
        };
    }
    Guess::Unknown
}

/// The number of a word `l3`: copper layer 3.
fn layer_number(word: &str) -> Option<u32> {
    let digits = word.strip_prefix('l')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|n| *n >= 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_name_reads_as_a_sentence_case_label() {
        assert_eq!(label_from_file_name("fab-notes.gbr"), "Fab notes");
        assert_eq!(label_from_file_name("out/fab_notes_v2.GBR"), "Fab notes v2");
        assert_eq!(
            label_from_file_name("C:\\pkg\\assembly.top.gbr"),
            "Assembly top"
        );
        assert_eq!(label_from_file_name("README"), "README");
        assert_eq!(label_from_file_name(".gbr"), "Gbr");
        assert_eq!(label_from_file_name("--"), "--");
    }

    #[test]
    fn the_words_of_a_name_give_the_layer() {
        let words = |name: &str| -> Guess {
            let lower = name.to_ascii_lowercase();
            let parts: Vec<&str> = lower
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|w| !w.is_empty())
                .collect();
            by_words(&parts)
        };
        assert_eq!(
            words("dcdc-buck-silk-bottom"),
            Guess::Known(FileFunction::Legend(Side::Bottom))
        );
        assert_eq!(
            words("board-soldermask-top"),
            Guess::Known(FileFunction::Soldermask(Side::Top))
        );
        assert_eq!(words("dcdc-buck-L3"), Guess::Copper(3));
        assert_eq!(words("copper-bottom"), Guess::BottomCopper);
        assert_eq!(words("fab-notes"), Guess::Unknown);
        assert_eq!(words("paste"), Guess::Unknown, "a side layer needs a side");
        assert_eq!(
            by_extension("gbo"),
            Some(Guess::Known(FileFunction::Legend(Side::Bottom)))
        );
        assert_eq!(by_extension("gbr"), None);
    }

    #[test]
    fn copper_sides_follow_the_position() {
        assert_eq!(
            Guess::Copper(1).resolve(4),
            Some(FileFunction::Copper {
                number: 1,
                side: CopperSide::Top
            })
        );
        assert_eq!(
            Guess::Copper(3).resolve(4),
            Some(FileFunction::Copper {
                number: 3,
                side: CopperSide::Inner
            })
        );
        assert_eq!(
            Guess::BottomCopper.resolve(4),
            Some(FileFunction::Copper {
                number: 4,
                side: CopperSide::Bottom
            })
        );
    }
}
