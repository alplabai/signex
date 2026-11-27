//! `<converter> describe --json` (contract §2.1): who the converter is,
//! which contract it speaks, and what it reads. The file-picker filters are
//! built from its answer, so no foreign extension is compiled into Signex.

use std::path::Path;

use serde::Deserialize;

/// The contract major this build implements.
pub const CONTRACT_MAJOR: u32 = 1;

/// A contract version, `<major>.<minor>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractVersion {
    pub major: u32,
    pub minor: u32,
}

impl ContractVersion {
    fn parse(text: &str) -> Option<Self> {
        let (major, minor) = text.trim().split_once('.')?;
        Some(Self {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
        })
    }
}

impl std::fmt::Display for ContractVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// What a converter's input is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKindClass {
    /// A symbol, footprint or combined library file or folder.
    Library,
    /// A file describing a database of parts that points at libraries.
    Database,
    /// A kind a newer minor added; shown, treated as a library.
    Other,
}

/// One kind of input a converter reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputKind {
    pub label: String,
    /// Lower case, no leading dot. An entry may be a whole fixed file name.
    pub extensions: Vec<String>,
    pub kind: InputKindClass,
}

/// A parsed `describe` answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConverterDescription {
    pub contract: ContractVersion,
    pub converter: String,
    pub version: String,
    pub inputs: Vec<InputKind>,
}

/// One file-picker filter: a label and the extensions it lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerFilter {
    pub label: String,
    pub extensions: Vec<String>,
}

/// Why a `describe` answer was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescribeError {
    /// Not the documented JSON object; what was wrong.
    Malformed(String),
    /// A contract major this build does not implement.
    UnsupportedContract(String),
}

#[derive(Deserialize)]
struct RawDescribe {
    contract: String,
    converter: String,
    version: String,
    #[serde(default)]
    inputs: Vec<RawInput>,
}

#[derive(Deserialize)]
struct RawInput {
    label: String,
    extensions: Vec<String>,
    kind: String,
}

/// Parse the JSON `describe` printed. A newer or unknown contract major is
/// refused; any minor of major [`CONTRACT_MAJOR`] is accepted.
pub fn parse_describe(stdout: &str) -> Result<ConverterDescription, DescribeError> {
    let raw: RawDescribe =
        serde_json::from_str(stdout.trim()).map_err(|e| DescribeError::Malformed(e.to_string()))?;
    let contract = ContractVersion::parse(&raw.contract)
        .ok_or_else(|| DescribeError::Malformed(format!("contract {:?}", raw.contract)))?;
    if contract.major != CONTRACT_MAJOR {
        return Err(DescribeError::UnsupportedContract(raw.contract));
    }
    if raw.converter.trim().is_empty() {
        return Err(DescribeError::Malformed("empty converter name".into()));
    }
    let inputs = raw
        .inputs
        .into_iter()
        .map(|input| InputKind {
            label: input.label,
            extensions: input
                .extensions
                .iter()
                .map(|e| e.trim().trim_start_matches('.').to_lowercase())
                .filter(|e| !e.is_empty())
                .collect(),
            kind: match input.kind.as_str() {
                "library" => InputKindClass::Library,
                "database" => InputKindClass::Database,
                _ => InputKindClass::Other,
            },
        })
        .collect();
    Ok(ConverterDescription {
        contract,
        converter: raw.converter,
        version: raw.version,
        inputs,
    })
}

impl ConverterDescription {
    /// One filter per input kind, in the converter's order. A whole-name
    /// entry is listed too; [`Self::input_for`] is what decides a pick.
    pub fn picker_filters(&self) -> Vec<PickerFilter> {
        self.inputs
            .iter()
            .filter(|input| !input.extensions.is_empty())
            .map(|input| PickerFilter {
                label: input.label.clone(),
                extensions: input.extensions.clone(),
            })
            .collect()
    }

    /// The input kind a picked file belongs to: an entry matching its
    /// extension first, then one matching its whole name (contract §2.1).
    pub fn input_for(&self, path: &Path) -> Option<&InputKind> {
        let lower = |s: &std::ffi::OsStr| s.to_string_lossy().to_lowercase();
        let lists = |wanted: Option<String>| {
            let wanted = wanted?;
            self.inputs
                .iter()
                .find(|input| input.extensions.contains(&wanted))
        };
        lists(path.extension().map(lower)).or_else(|| lists(path.file_name().map(lower)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../../docs/fixtures/library-converter/describe.json");

    #[test]
    fn the_fixture_parses_into_filters() {
        let d = parse_describe(FIXTURE).unwrap();
        assert_eq!(d.contract, ContractVersion { major: 1, minor: 0 });
        assert_eq!(d.converter, "example-import");
        assert_eq!(d.version, "0.16.0");
        let filters = d.picker_filters();
        assert_eq!(filters.len(), 3);
        assert_eq!(filters[0].label, "Integrated library");
        assert_eq!(filters[0].extensions, ["lib1"]);
        assert_eq!(d.inputs[2].kind, InputKindClass::Database);
    }

    #[test]
    fn a_newer_minor_is_accepted_a_newer_major_refused() {
        let newer_minor = FIXTURE.replace("\"1.0\"", "\"1.7\"");
        assert_eq!(parse_describe(&newer_minor).unwrap().contract.minor, 7);
        let newer_major = FIXTURE.replace("\"1.0\"", "\"2.0\"");
        assert_eq!(
            parse_describe(&newer_major),
            Err(DescribeError::UnsupportedContract("2.0".into()))
        );
        let zero = FIXTURE.replace("\"1.0\"", "\"0.9\"");
        assert!(matches!(
            parse_describe(&zero),
            Err(DescribeError::UnsupportedContract(_))
        ));
    }

    #[test]
    fn garbage_is_malformed_not_a_panic() {
        for text in [
            "",
            "{",
            "[]",
            r#"{"contract":"one","converter":"x","version":"1"}"#,
        ] {
            assert!(
                matches!(parse_describe(text), Err(DescribeError::Malformed(_))),
                "{text}"
            );
        }
    }

    #[test]
    fn a_pick_matches_extension_then_whole_name() {
        let d = parse_describe(
            r#"{"contract":"1.0","converter":"c","version":"1","inputs":[
                {"label":"Table","extensions":["lib-table"],"kind":"database"},
                {"label":"Lib","extensions":[".LIB1"],"kind":"library"}]}"#,
        )
        .unwrap();
        assert_eq!(d.input_for(Path::new("/x/A.Lib1")).unwrap().label, "Lib");
        assert_eq!(
            d.input_for(Path::new("/x/lib-table")).unwrap().label,
            "Table"
        );
        assert_eq!(
            d.input_for(Path::new("/x/LIB-TABLE")).unwrap().label,
            "Table"
        );
        assert!(d.input_for(Path::new("/x/a.txt")).is_none());
    }
}
