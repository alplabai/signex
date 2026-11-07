//! The Language preference (#2055): a BCP 47 tag under `language`, absent
//! for "System default".

use super::*;
use signex_i18n::LanguagePref;

const KEY: &str = "language";

/// Read the saved language; a missing or non-string value is System default.
pub fn read_language_pref_from(json: Option<&serde_json::Value>) -> LanguagePref {
    let tag = json.and_then(|json| json[KEY].as_str());
    LanguagePref::from_saved(tag)
}

pub fn write_language_pref(pref: &LanguagePref) {
    write_language_pref_at(&prefs_path(), pref)
}

/// System default removes the key, so the file reads the same as one that
/// never had the setting.
pub fn write_language_pref_at(path: &Path, pref: &LanguagePref) {
    update_prefs_json(path, KEY, |prefs| match pref.saved_tag() {
        Some(tag) => {
            prefs.insert(KEY.to_string(), serde_json::json!(tag));
        }
        None => {
            prefs.remove(KEY);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_prefs(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, contents).unwrap();
        (dir, path)
    }

    fn read(path: &Path) -> LanguagePref {
        read_language_pref_from(read_prefs_json(path).as_ref())
    }

    #[test]
    fn an_absent_or_non_string_key_reads_as_system_default() {
        assert_eq!(read_language_pref_from(None), LanguagePref::System);
        let (_dir, path) = temp_prefs(r#"{"theme": "nord"}"#);
        assert_eq!(read(&path), LanguagePref::System);
        let (_dir, path) = temp_prefs(r#"{"language": 7}"#);
        assert_eq!(read(&path), LanguagePref::System);
    }

    #[test]
    fn a_language_round_trips_and_keeps_other_keys() {
        let (_dir, path) = temp_prefs(r#"{"theme": "nord"}"#);
        write_language_pref_at(&path, &LanguagePref::Locale("en-XA".into()));
        assert_eq!(read(&path), LanguagePref::Locale("en-XA".into()));
        let json = read_prefs_json(&path).unwrap();
        assert_eq!(json["theme"], "nord");
    }

    #[test]
    fn system_default_removes_the_key() {
        let (_dir, path) = temp_prefs(r#"{"language": "en-XA", "theme": "nord"}"#);
        write_language_pref_at(&path, &LanguagePref::System);
        let json = read_prefs_json(&path).unwrap();
        assert!(json.get("language").is_none(), "{json}");
        assert_eq!(json["theme"], "nord");
        assert_eq!(read(&path), LanguagePref::System);
    }

    #[test]
    fn an_unavailable_saved_language_survives_a_save() {
        let (_dir, path) = temp_prefs(r#"{"language": "tr-TR", "theme": "nord"}"#);
        let i18n = signex_i18n::I18n::new(&read(&path));
        // What Save writes: the pref of the instance in force.
        write_language_pref_at(&path, i18n.pref());
        let json = read_prefs_json(&path).unwrap();
        assert_eq!(json["language"], "tr-TR");
        assert_eq!(json["theme"], "nord");
    }
}
