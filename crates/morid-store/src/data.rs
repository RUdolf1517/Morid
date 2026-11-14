//! Original data schemas. Unknown JSON members survive typed read/write cycles.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    #[serde(rename = "uiName")]
    pub ui_name: String,
    pub options: String,
    pub website: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrlRule {
    pub url: String,
    pub engine: String,
    #[serde(rename = "downloadOption")]
    pub download_option: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
impl UrlRule {
    pub fn matches(&self, url: &str) -> bool {
        match (self.url.strip_prefix('*'), self.url.strip_suffix('*')) {
            (Some(prefix), Some(_)) if self.url.len() > 1 => {
                url.contains(prefix.strip_suffix('*').unwrap_or(prefix))
            }
            (Some(suffix), _) => url.ends_with(suffix),
            (_, Some(prefix)) => url.starts_with(prefix),
            _ => url.contains(&self.url),
        }
    }
}
pub fn url_rule<'a>(rules: &'a [UrlRule], url: &str) -> Option<&'a UrlRule> {
    rules.iter().find(|rule| rule.matches(url))
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineDefault {
    #[serde(rename = "engineName")]
    pub engine_name: String,
    pub options: String,
    pub default: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
pub fn engine_default<'a>(rows: &'a [EngineDefault], engine: &str) -> Option<&'a EngineDefault> {
    rows.iter()
        .find(|row| row.engine_name == engine && row.default == "yes")
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subscription {
    #[serde(rename = "uiName")]
    pub ui_name: String,
    pub url: String,
    #[serde(rename = "getListOptions")]
    pub get_list_options: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedEntry {
    pub url: String,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

/// Bundled literal from configure.cpp::defaultData; constructor rewrites apply
/// only to generated defaults, never to imported user bytes.
pub fn bundled_presets() -> Result<Vec<Preset>, serde_json::Error> {
    let mut data = include_str!("bundled-presets.json").to_owned();
    for height in [144, 240, 360, 480, 720, 1080, 1440, 2160] {
        data = data.replace(
            &format!("bestvideo[height={height}]"),
            &format!("bestvideo[format_note*={height}p]"),
        );
    }
    data = data.replace("bestvideo[ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[ext=mp4]+bestaudio[ext=m4a]/bestvideo+bestaudio/best[ext=mp4]/best", "bestvideo[ext=mp4][vcodec^=av]+bestaudio[ext=m4a]/bestvideo[ext=mp4]+bestaudio[ext=m4a]/bestvideo+bestaudio/best[ext=mp4]/best");
    serde_json::from_str(&data)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Fragment {
    pub offset: usize,
    pub bytes: Vec<u8>,
}
/// Recover at each object opener, retaining every skipped byte in the report.
/// serde's stream parser handles nested objects and braces inside strings.
pub fn read_history(bytes: &[u8]) -> (Vec<Value>, Vec<Fragment>) {
    if let Ok(Value::Array(values)) = serde_json::from_slice::<Value>(bytes) {
        if values.iter().all(Value::is_object) {
            return (values, Vec::new());
        }
    }
    let mut values = Vec::new();
    let mut fragments = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        if bytes[offset].is_ascii_whitespace() {
            offset += 1;
            continue;
        }
        let mut stream =
            serde_json::Deserializer::from_slice(&bytes[offset..]).into_iter::<Value>();
        if let Some(Ok(value @ Value::Object(_))) = stream.next() {
            values.push(value);
            offset += stream.byte_offset();
        } else {
            let start = offset;
            offset += 1;
            while offset < bytes.len() && bytes[offset] != b'{' {
                offset += 1;
            }
            fragments.push(Fragment {
                offset: start,
                bytes: bytes[start..offset].to_vec(),
            });
        }
    }
    (values, fragments)
}

#[cfg(test)]
mod tests {
    use super::*;
    const HISTORY: &[u8] =
        include_bytes!("../../../fixtures/userdata/media-downloader/data/downloadHistory.json");
    #[test]
    fn bundled_presets_match_the_captured_file() {
        let expected: Vec<Preset> = serde_json::from_slice(include_bytes!(
            "../../../fixtures/userdata/media-downloader/data/presetOptions.json"
        ))
        .expect("capture");
        let actual = bundled_presets().expect("bundled");
        assert_eq!(actual.len(), 15);
        assert_eq!(actual, expected);
    }
    #[test]
    fn url_manager_takes_the_first_match() {
        for (pattern, yes, no) in [
            ("*end", "the-end", "end-no"),
            ("start*", "start-here", "not-start"),
            ("*mid*", "a-mid-b", "absent"),
            ("mid", "a-mid-b", "absent"),
        ] {
            let make = |engine: &str| UrlRule {
                url: pattern.into(),
                engine: engine.into(),
                download_option: String::new(),
                extra: Map::new(),
            };
            let rules = vec![make("first"), make("second")];
            assert_eq!(url_rule(&rules, yes).expect("match").engine, "first");
            assert!(url_rule(&rules, no).is_none());
        }
    }
    #[test]
    fn engine_defaults_take_the_first_enabled_row() {
        let mut rows: Vec<EngineDefault> = serde_json::from_slice(include_bytes!(
            "../../../fixtures/userdata/media-downloader/data/downloadEngineDefaultOptions.json"
        ))
        .expect("capture");
        assert_eq!(
            engine_default(&rows, "yt-dlp").expect("enabled").options,
            "--engine-default \"two words\""
        );
        let mut later = rows[1].clone();
        later.options = "must not win".into();
        rows.push(later);
        assert_eq!(
            engine_default(&rows, "yt-dlp")
                .expect("first enabled")
                .options,
            "--engine-default \"two words\""
        );
    }
    #[test]
    fn history_reads_20_objects_from_the_fixture() {
        let (values, fragments) = read_history(HISTORY);
        assert_eq!(values.len(), 20);
        assert!(fragments.is_empty());
    }
    #[test]
    fn history_keeps_the_rest_when_one_fragment_is_corrupt() {
        let mut bytes = HISTORY.to_vec();
        let offset = bytes
            .windows(19)
            .position(|w| w == b"example.com/history")
            .expect("url");
        bytes[offset] = 0xff;
        let (values, fragments) = read_history(&bytes);
        assert_eq!(values.len(), 19);
        assert!(!fragments.is_empty());
        assert!(fragments
            .iter()
            .any(|fragment| fragment.bytes.contains(&0xff)));
    }
}

macro_rules! json_file {
    ($read:ident, $write:ident, $ty:ty) => {
        pub fn $read(bytes: &[u8]) -> Result<Vec<$ty>, serde_json::Error> {
            serde_json::from_slice(bytes)
        }
        pub fn $write(rows: &[$ty]) -> Result<Vec<u8>, serde_json::Error> {
            serde_json::to_vec_pretty(rows)
        }
    };
}
json_file!(read_presets, write_presets, Preset);
json_file!(read_url_rules, write_url_rules, UrlRule);
json_file!(read_engine_defaults, write_engine_defaults, EngineDefault);
json_file!(read_subscriptions, write_subscriptions, Subscription);
json_file!(read_saved_list, write_saved_list, SavedEntry);

/// Themes remain opaque bytes after shape validation. Palette resolution and
/// formatting changes have no place in the data migration layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme(Vec<u8>);
impl Theme {
    pub fn read(bytes: &[u8]) -> Result<Self, String> {
        let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if !value.is_object() {
            return Err("theme must be a colour map".into());
        }
        Ok(Self(bytes.to_vec()))
    }
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}
#[cfg(test)]
mod file_tests {
    use super::*;
    #[test]
    fn themes_round_trip_byte_for_byte() {
        for bytes in [
            include_bytes!("../../../fixtures/userdata/media-downloader/data/themes/Dark.json")
                .as_slice(),
            include_bytes!(
                "../../../fixtures/userdata/media-downloader/data/themes/Pure Dark.json"
            )
            .as_slice(),
        ] {
            assert_eq!(Theme::read(bytes).expect("theme").bytes(), bytes);
        }
    }
    #[test]
    fn six_captured_data_files_use_real_fields_and_preserve_unknown_keys() {
        let base = std::path::Path::new(file!())
            .parent()
            .expect("source")
            .join("../../../fixtures/userdata/media-downloader/data");
        macro_rules! check {
            ($file:literal, $read:ident, $write:ident, $count:expr) => {{
                let bytes = std::fs::read(base.join($file)).expect("capture");
                let rows = $read(&bytes).expect("schema");
                assert_eq!(rows.len(), $count);
                let written = $write(&rows).expect("write");
                assert_eq!(
                    serde_json::from_slice::<Value>(&written).expect("output"),
                    serde_json::from_slice::<Value>(&bytes).expect("input")
                );
            }};
        }
        check!("presetOptions.json", read_presets, write_presets, 15);
        check!(
            "downloadDefaultOptions.json",
            read_url_rules,
            write_url_rules,
            1
        );
        check!(
            "downloadEngineDefaultOptions.json",
            read_engine_defaults,
            write_engine_defaults,
            2
        );
        check!(
            "subscriptions.json",
            read_subscriptions,
            write_subscriptions,
            1
        );
        check!("autoSavedList.json", read_saved_list, write_saved_list, 2);
        let (rows, fragments) =
            read_history(&std::fs::read(base.join("downloadHistory.json")).expect("history"));
        assert_eq!(rows.len(), 20);
        assert!(fragments.is_empty());
    }
}

/// Morid history writer uses a proper JSON array; export of imported originals
/// continues to copy the preserved original stream, never this representation.
pub fn write_history(rows: &[Value]) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec_pretty(rows)
}

/// Restored state is not deserialized from backend/UI metadata. Retaining any
/// unknown running fields in `fields` must never make a restored job active.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoredState {
    NotStarted,
}
impl SavedEntry {
    pub fn restored_state(&self) -> RestoredState {
        RestoredState::NotStarted
    }
}
