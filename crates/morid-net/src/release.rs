//! Release API parsing; tests are synthetic, not evidence of a live response.
//! Original order: gallery-dl uses the first release in an array, then the
//! original's utility::parseJsonDataFromGitHub uses the first matching asset.
use crate::install::Integrity;
use morid_core::Error;
use serde_json::Value;

#[derive(Debug)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub tag: String,
    pub integrity: Integrity,
}
fn invalid(message: &str) -> Error {
    Error::BackendFailure(format!("release metadata: {message}"))
}

/// Exact names come from the application's per-engine/platform selection rule;
/// net neither guesses an engine nor silently chooses an unrelated first asset.
/// A malformed chosen asset fails closed; it is not skipped in favor of a later
/// asset, which would change the original's first-match behavior.
pub fn select(bytes: &[u8], codeberg_list: bool, names: &[&str]) -> Result<Asset, Error> {
    select_matching(bytes, codeberg_list, |name| names.contains(&name))
}

pub fn select_matching(
    bytes: &[u8],
    codeberg_list: bool,
    matches: impl Fn(&str) -> bool,
) -> Result<Asset, Error> {
    let json: Value = serde_json::from_slice(bytes).map_err(|e| invalid(&e.to_string()))?;
    let release = if codeberg_list {
        json.as_array()
            .and_then(|rows| rows.first())
            .ok_or_else(|| invalid("expected a nonempty release array"))?
    } else {
        &json
    };
    let rows = release
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing assets array"))?;
    for row in rows {
        let Some(name) = row.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !matches(name) {
            continue;
        }
        let size = row
            .get("size")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("selected asset missing unsigned size"))?;
        let digest = row
            .get("digest")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("selected asset missing SHA-256 digest"))?;
        let integrity = Integrity::from_metadata(size, digest)?;
        let url = row
            .get("browser_download_url")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("selected asset missing browser_download_url"))?;
        if !url.starts_with("https://") || url.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(invalid(
                "selected asset URL must be HTTPS without whitespace",
            ));
        }
        return Ok(Asset {
            name: name.into(),
            url: url.into(),
            tag: release
                .get("tag_name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            integrity,
        });
    }
    Err(invalid("no matching platform asset"))
}

#[cfg(test)]
mod tests {
    use super::*;
    const SYNTHETIC_GITHUB: &[u8] = br#"{
      "_test_provenance":"SYNTHETIC, NOT A CAPTURED API RESPONSE",
      "tag_name":"test-v1", "assets":[
        {"name":"other-platform", "size":99},
        {"name":"backend", "size":11, "digest":"sha256:42404f785c9107ddb29581517f9eb04fc57a168eb84cac0f6b7e2ac9c6ae5ccf", "browser_download_url":"https://github.com/example/project/releases/download/test-v1/backend"},
        {"name":"backend", "size":100}
      ]}"#;
    #[test]
    fn synthetic_github_selects_first_matching_asset_and_validates_integrity_metadata() {
        let asset = select(SYNTHETIC_GITHUB, false, &["backend"]).unwrap();
        assert_eq!(asset.name, "backend");
        assert_eq!(asset.integrity.size, 11);
        assert_eq!(asset.tag, "test-v1");
        println!("SYNTHETIC_GITHUB selected={} tag={} expected_size={} digest_origin=metadata first_matching_asset=true", asset.name, asset.tag, asset.integrity.size);
        assert!(select(SYNTHETIC_GITHUB, false, &["absent"]).is_err());
    }
    #[test]
    fn synthetic_codeberg_uses_first_release_not_sorted_by_tag_or_date() {
        let object: Value = serde_json::from_slice(SYNTHETIC_GITHUB).unwrap();
        let bytes = serde_json::to_vec(
            &serde_json::json!([object, {"tag_name":"newer-but-later", "assets":[]}]),
        )
        .unwrap();
        let asset = select(&bytes, true, &["backend"]).unwrap();
        assert_eq!(asset.tag, "test-v1");
        println!(
            "SYNTHETIC_CODEBERG release_index=0 selected={} tag={} (not a live format assertion)",
            asset.name, asset.tag
        );
    }
    #[test]
    fn synthetic_selected_missing_digest_is_refused_without_fallback() {
        let mut json: Value = serde_json::from_slice(SYNTHETIC_GITHUB).unwrap();
        json["assets"][1].as_object_mut().unwrap().remove("digest");
        let error = select(&serde_json::to_vec(&json).unwrap(), false, &["backend"]).unwrap_err();
        assert!(error.to_string().contains("missing SHA-256"));
        println!(
            "SYNTHETIC_RELEASE_REFUSED exit={} {error}",
            error.exit_code()
        );
    }
}
