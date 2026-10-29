//! Text substitution — resolves `${TITLE}`, `${DATE}`, `${REV}`, etc.
//!
//! See `OUTPUT_PLAN.md` §5. Resolved at render time only, never baked into
//! the Standard file — `.standard_sch` stores literal `${TITLE}` strings so the
//! round-trip stays lossless.
//!
//! **Rules:**
//! - Tokens are `${IDENT}` where IDENT matches `[A-Za-z_][A-Za-z0-9_]*`.
//! - Unknown tokens resolve to empty string (never literal `${FOO}`).
//! - Strings that look like tokens but aren't (whitespace, punctuation
//!   inside the braces) pass through verbatim — the scanner doesn't match.
//! - Works in any text object, not just title blocks.

use crate::ProjectMetadata;

/// Binds tokens to values for a single render pass — one sheet, one project
/// snapshot. Constructed per-export in the app layer.
#[derive(Debug, Clone)]
pub struct SubstitutionContext<'a> {
    pub metadata: &'a ProjectMetadata,
    pub filename: String,
    pub sheet_name: String,
    pub sheet_number: usize,
    pub sheet_count: usize,
    pub signex_version: &'static str,
    /// Active variant (or `None` for "no variant override"). Surfaces
    /// as `${VARIANT}` when `physical_structure` is on; resolves to
    /// empty string otherwise so legacy templates don't pick up
    /// stray variant text.
    pub variant: Option<String>,
    /// PDF Settings → Use Physical Structure. Gates `${VARIANT}` and
    /// the per-instance number/document fields.
    pub physical_structure: bool,
    /// Drop the sheet number / document number from the title block
    /// when the corresponding physical toggle is off.
    pub physical_sheet_number: bool,
    pub physical_document_number: bool,
}

impl<'a> SubstitutionContext<'a> {
    /// Look up a single token. Returns `None` for unknown tokens — the
    /// resolver renders that as an empty string.
    fn lookup(&self, token: &str) -> Option<String> {
        let m = self.metadata;
        match token {
            "TITLE" => Some(m.title.clone()),
            "REV" | "REVISION" => Some(m.revision.clone()),
            "DATE" => Some(m.date.clone()),
            "COMPANY" => Some(m.company.clone()),
            "COMMENT1" => Some(m.comments[0].clone()),
            "COMMENT2" => Some(m.comments[1].clone()),
            "COMMENT3" => Some(m.comments[2].clone()),
            "COMMENT4" => Some(m.comments[3].clone()),
            "FILENAME" => Some(self.filename.clone()),
            "SHEETNAME" => Some(self.sheet_name.clone()),
            "SHEETNUMBER" => Some(if self.physical_sheet_number {
                self.sheet_number.to_string()
            } else {
                String::new()
            }),
            "SHEETCOUNT" => Some(self.sheet_count.to_string()),
            "DOCUMENTNUMBER" => Some(if self.physical_document_number {
                m.custom_fields
                    .get("document_number")
                    .cloned()
                    .unwrap_or_default()
            } else {
                String::new()
            }),
            "VARIANT" => Some(if self.physical_structure {
                self.variant.clone().unwrap_or_default()
            } else {
                String::new()
            }),
            "VERSION" => Some(self.signex_version.to_string()),
            "PROJECT_NAME" => Some(
                m.custom_fields
                    .get("project_name")
                    .cloned()
                    .unwrap_or_default(),
            ),
            "DRAWNBY" => Some(m.custom_fields.get("drawn_by").cloned().unwrap_or_default()),
            "CHECKEDBY" => Some(
                m.custom_fields
                    .get("checked_by")
                    .cloned()
                    .unwrap_or_default(),
            ),
            "APPROVEDBY" => Some(
                m.custom_fields
                    .get("approved_by")
                    .cloned()
                    .unwrap_or_default(),
            ),
            other => m.custom_fields.get(other).cloned(),
        }
    }
}

/// Replace every `${IDENT}` in `input` with its resolved value. Unknown
/// tokens render as empty string; non-token `${...}` strings pass through.
pub fn resolve(input: &str, ctx: &SubstitutionContext<'_>) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'{'
            && let Some((token, end)) = scan_token(bytes, i + 2)
        {
            let value = ctx.lookup(token).unwrap_or_default();
            out.push_str(&value);
            i = end + 1;
            continue;
        }
        out.push(input[i..].chars().next().unwrap());
        i += input[i..].chars().next().unwrap().len_utf8();
    }
    out
}

/// Try to scan an identifier followed by `}` starting at `start`. Returns
/// `(identifier, close_brace_index)` on success, `None` if what follows
/// isn't a valid identifier terminated by `}`.
fn scan_token(bytes: &[u8], start: usize) -> Option<(&str, usize)> {
    if start >= bytes.len() {
        return None;
    }
    // First char: [A-Za-z_]
    let first = bytes[start];
    if !is_ident_start(first) {
        return None;
    }
    let mut end = start + 1;
    while end < bytes.len() && is_ident_continue(bytes[end]) {
        end += 1;
    }
    if end >= bytes.len() || bytes[end] != b'}' {
        return None;
    }
    // Safety: ASCII range by construction.
    let token = std::str::from_utf8(&bytes[start..end]).ok()?;
    Some((token, end))
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> ProjectMetadata {
        let mut m = ProjectMetadata {
            title: "Power Supply".into(),
            revision: "B".into(),
            date: "2026-04-22".into(),
            company: "Alp Lab AB".into(),
            comments: [
                "comment 1".into(),
                "comment 2".into(),
                String::new(),
                String::new(),
            ],
            ..Default::default()
        };
        m.custom_fields
            .insert("PROJECT_CODE".into(), "HW-2026-042".into());
        m.custom_fields
            .insert("lower_case_field".into(), "ok".into());
        m
    }

    /// #1080 — the title block's sign-off and document-number fields resolve
    /// from the keys the sheet stores them under.
    #[test]
    fn sign_off_and_document_number_tokens_resolve() {
        let mut m = metadata();
        for (k, v) in [
            ("drawn_by", "A. Engineer"),
            ("checked_by", "B. Reviewer"),
            ("approved_by", "C. Lead"),
            ("document_number", "DOC-0042"),
        ] {
            m.custom_fields.insert(k.into(), v.into());
        }
        assert_eq!(
            resolve(
                "${DRAWNBY}/${CHECKEDBY}/${APPROVEDBY}/${DOCUMENTNUMBER}",
                &ctx(&m)
            ),
            "A. Engineer/B. Reviewer/C. Lead/DOC-0042"
        );
        // Unset fields are empty, never the literal token.
        assert_eq!(resolve("${DRAWNBY}", &ctx(&metadata())), "");
    }

    fn ctx<'a>(m: &'a ProjectMetadata) -> SubstitutionContext<'a> {
        SubstitutionContext {
            metadata: m,
            filename: "PowerSupply.standard_sch".into(),
            sheet_name: "Analog".into(),
            sheet_number: 2,
            sheet_count: 5,
            signex_version: "0.8.0",
            variant: None,
            physical_structure: true,
            physical_sheet_number: true,
            physical_document_number: true,
        }
    }

    #[test]
    fn resolves_builtin_tokens() {
        let m = metadata();
        let c = ctx(&m);
        assert_eq!(resolve("${TITLE}", &c), "Power Supply");
        assert_eq!(resolve("Rev: ${REV}", &c), "Rev: B");
        assert_eq!(resolve("${REVISION}", &c), "B");
        assert_eq!(resolve("${DATE}", &c), "2026-04-22");
        assert_eq!(resolve("${COMPANY}", &c), "Alp Lab AB");
        assert_eq!(resolve("${FILENAME}", &c), "PowerSupply.standard_sch");
        assert_eq!(resolve("${SHEETNAME}", &c), "Analog");
        assert_eq!(
            resolve("Sheet ${SHEETNUMBER} of ${SHEETCOUNT}", &c),
            "Sheet 2 of 5",
        );
        assert_eq!(resolve("${VERSION}", &c), "0.8.0");
        assert_eq!(resolve("${COMMENT1}", &c), "comment 1");
        assert_eq!(resolve("${COMMENT4}", &c), "");
    }

    #[test]
    fn resolves_custom_fields() {
        let m = metadata();
        let c = ctx(&m);
        assert_eq!(resolve("${PROJECT_CODE}", &c), "HW-2026-042");
        assert_eq!(resolve("${lower_case_field}", &c), "ok");
    }

    #[test]
    fn unknown_token_renders_empty() {
        let m = metadata();
        let c = ctx(&m);
        assert_eq!(resolve("[${UNDEFINED}]", &c), "[]");
        assert_eq!(resolve("x${NOPE}y", &c), "xy");
    }

    #[test]
    fn malformed_passes_through() {
        let m = metadata();
        let c = ctx(&m);
        // Space inside the braces: not an identifier, render verbatim.
        assert_eq!(resolve("${foo bar}", &c), "${foo bar}");
        // Hyphen isn't an identifier continuation.
        assert_eq!(resolve("${foo-bar}", &c), "${foo-bar}");
        // Leading digit isn't a valid identifier start.
        assert_eq!(resolve("${1TITLE}", &c), "${1TITLE}");
        // Unclosed brace.
        assert_eq!(resolve("${TITLE", &c), "${TITLE");
        // Empty braces.
        assert_eq!(resolve("${}", &c), "${}");
    }

    #[test]
    fn multiple_tokens_same_line() {
        let m = metadata();
        let c = ctx(&m);
        assert_eq!(
            resolve("${TITLE} Rev ${REV} — ${DATE}", &c),
            "Power Supply Rev B — 2026-04-22",
        );
    }

    #[test]
    fn multiline_text() {
        let m = metadata();
        let c = ctx(&m);
        assert_eq!(
            resolve("${TITLE}\n${REV}\n${DATE}", &c),
            "Power Supply\nB\n2026-04-22",
        );
    }

    #[test]
    fn plain_text_unchanged() {
        let m = metadata();
        let c = ctx(&m);
        assert_eq!(
            resolve("no substitutions here", &c),
            "no substitutions here",
        );
        assert_eq!(resolve("", &c), "");
    }

    #[test]
    fn preserves_utf8() {
        let m = metadata();
        let c = ctx(&m);
        // Turkish + Chinese + emoji round-trip unchanged.
        let text = "Başlık: ${TITLE} · 设计: ${COMPANY} · 🔧 Rev ${REV}";
        assert_eq!(
            resolve(text, &c),
            "Başlık: Power Supply · 设计: Alp Lab AB · 🔧 Rev B",
        );
    }

    #[test]
    fn dollar_without_brace_passes_through() {
        let m = metadata();
        let c = ctx(&m);
        assert_eq!(resolve("Cost: $42", &c), "Cost: $42");
        assert_eq!(resolve("$TITLE", &c), "$TITLE");
    }

    #[test]
    fn variant_token_only_emits_when_physical_structure_on() {
        let m = metadata();
        let mut c = ctx(&m);
        c.variant = Some("VarA".into());
        c.physical_structure = true;
        assert_eq!(resolve("[${VARIANT}]", &c), "[VarA]");
        c.physical_structure = false;
        assert_eq!(resolve("[${VARIANT}]", &c), "[]");
    }

    #[test]
    fn sheet_number_token_drops_when_toggle_off() {
        let m = metadata();
        let mut c = ctx(&m);
        c.physical_sheet_number = false;
        assert_eq!(resolve("Sheet ${SHEETNUMBER}", &c), "Sheet ");
    }

    #[test]
    fn document_number_falls_back_to_custom_field() {
        let mut m = metadata();
        m.custom_fields
            .insert("document_number".into(), "DOC-42".into());
        let mut c = ctx(&m);
        c.physical_document_number = true;
        assert_eq!(resolve("${DOCUMENTNUMBER}", &c), "DOC-42");
        c.physical_document_number = false;
        assert_eq!(resolve("${DOCUMENTNUMBER}", &c), "");
    }
}

#[cfg(test)]
mod special_string_tests {
    use super::*;
    use std::collections::{BTreeMap, HashMap};

    fn project() -> ProjectMetadata {
        let params = BTreeMap::from([
            ("title".to_string(), "Project title".to_string()),
            ("company".to_string(), "Acme".to_string()),
            ("drawn_by".to_string(), "A. Engineer".to_string()),
            ("Voltage".to_string(), "24 V".to_string()),
        ]);
        let mut m = ProjectMetadata::from_values(&params);
        m.project_parameters = Some(params);
        m.custom_fields
            .insert("project_name".to_string(), "Oscar".to_string());
        m
    }

    fn resolve_on(m: &ProjectMetadata, text: &str) -> String {
        resolve(
            text,
            &SubstitutionContext {
                metadata: m,
                filename: "power.snxsch".into(),
                sheet_name: "power".into(),
                sheet_number: 2,
                sheet_count: 5,
                signex_version: "1.0.0",
                variant: None,
                physical_structure: false,
                physical_sheet_number: true,
                physical_document_number: true,
            },
        )
    }

    #[test]
    fn a_sheet_inherits_the_project_values() {
        let sheet = project().for_sheet(&HashMap::new());
        assert_eq!(
            resolve_on(
                &sheet,
                "${TITLE}|${COMPANY}|${DRAWNBY}|${Voltage}|${PROJECT_NAME}"
            ),
            "Project title|Acme|A. Engineer|24 V|Oscar"
        );
    }

    #[test]
    fn a_sheet_value_overrides_and_an_empty_one_falls_back() {
        let over = HashMap::from([
            ("title".to_string(), "Power".to_string()),
            ("company".to_string(), String::new()),
        ]);
        let sheet = project().for_sheet(&over);
        assert_eq!(resolve_on(&sheet, "${TITLE}|${COMPANY}"), "Power|Acme");
        // Another sheet does not see this one's override.
        let other = project().for_sheet(&HashMap::new());
        assert_eq!(resolve_on(&other, "${TITLE}"), "Project title");
    }

    #[test]
    fn every_token_resolves_from_the_one_resolver() {
        let sheet = project().for_sheet(&HashMap::from([
            ("rev".to_string(), "C".to_string()),
            ("date".to_string(), "2026-10-28".to_string()),
            ("document_number".to_string(), "DOC-7".to_string()),
        ]));
        assert_eq!(
            resolve_on(
                &sheet,
                "${PROJECT_NAME} ${TITLE} ${REVISION} ${REV} ${DATE} ${SHEETNUMBER}/${SHEETCOUNT} ${DOCUMENTNUMBER}"
            ),
            "Oscar Project title C C 2026-10-28 2/5 DOC-7"
        );
    }

    #[test]
    fn metadata_built_by_hand_is_its_own_base() {
        let m = ProjectMetadata {
            title: "Hand".into(),
            ..ProjectMetadata::default()
        };
        assert_eq!(
            resolve_on(&m.for_sheet(&HashMap::new()), "${TITLE}"),
            "Hand"
        );
    }
}
