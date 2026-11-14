//! Backend metadata -> service projection. `format_id` is backend vocabulary;
//! `MediaFormat::id` is the consumer-facing alias, not an extension schema rule.
//! Projection copies fields: the captured input and its format_id remain intact.
use morid_core::Error;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MediaMeta {
    pub title: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub formats: Vec<MediaFormat>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MediaFormat {
    pub id: Option<String>,
    /// Preserve all other backend fields, including format_id and unknown keys.
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

#[derive(Deserialize)]
struct BackendMeta {
    title: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    formats: Vec<Map<String, Value>>,
}

impl MediaMeta {
    pub fn from_yt_dlp(line: &str) -> Result<Self, Error> {
        let raw: BackendMeta = serde_json::from_str(line)
            .map_err(|e| Error::BackendFailure(format!("invalid metadata: {e}")))?;
        let formats = raw
            .formats
            .into_iter()
            .map(|mut fields| {
                let id = fields
                    .get("format_id")
                    .or_else(|| fields.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                // Avoid duplicate serialized id keys. Keep the original format_id;
                // id is owned by this projection, never rewritten in input bytes.
                fields.remove("id");
                MediaFormat { id, fields }
            })
            .collect();
        Ok(Self {
            title: raw.title,
            duration: raw.duration,
            thumbnail: raw.thumbnail,
            formats,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_format_projection_preserves_backend_fields_and_adds_id() {
        let input = include_str!("../../../fixtures/streams/yt-dlp/list.json");
        let source: Value = serde_json::from_str(input).expect("capture");
        let meta = MediaMeta::from_yt_dlp(input).expect("projection");
        assert_eq!(
            meta.formats.len(),
            source["formats"].as_array().expect("formats").len()
        );
        for (projected, original) in meta
            .formats
            .iter()
            .zip(source["formats"].as_array().expect("formats"))
        {
            assert_eq!(projected.id.as_deref(), original["format_id"].as_str());
            assert_eq!(Value::Object(projected.fields.clone()), *original);
        }
        assert!(source["formats"][0].get("id").is_none());
        assert_eq!(source["formats"][0]["format_id"], "mp4");
        assert!(MediaMeta::from_yt_dlp("[]").is_err());
        assert!(MediaMeta::from_yt_dlp(r#"{"formats":[3]}"#).is_err());
    }
}
