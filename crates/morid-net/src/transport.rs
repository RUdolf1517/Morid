//! Policy before I/O. Transports must return redirects, never follow them.
use crate::endpoints::RELEASE_ENDPOINTS;
use morid_core::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Byte/status transport, independent of engines and JSON. Implementations must
/// disable automatic redirects: a Location header is not authority to open a socket.
pub trait Transport {
    fn get(&mut self, url: &str) -> Result<Response, Error>;
}

/// Explicit archive mapping supplied by the engine's installation policy.
pub struct ArchiveTarget<'a> {
    pub bin: &'a std::path::Path,
    pub name: &'a str,
    pub member: &'a std::path::Path,
}

/// No caller-provided allow-list: a user extension cannot authorize itself.
pub struct ReleaseClient<T> {
    transport: T,
}
impl<T: Transport> ReleaseClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn metadata(&mut self, endpoint: &str) -> Result<Vec<u8>, Error> {
        if !RELEASE_ENDPOINTS.iter().any(|row| row.url == endpoint) {
            // Include the complete rejected string, thus also naming its host.
            return Err(Error::Internal(format!(
                "release endpoint is not allowed (exact URL required): {endpoint}"
            )));
        }
        let response = self.transport.get(endpoint)?;
        if (300..400).contains(&response.status) {
            return Err(Error::Internal(format!(
                "release redirect refused: {endpoint} (HTTP {})",
                response.status
            )));
        }
        if response.status != 200 {
            return Err(Error::BackendFailure(format!(
                "release metadata: {endpoint}: HTTP {}",
                response.status
            )));
        }
        Ok(response.body)
    }
    pub fn install_binary(
        &mut self,
        endpoint: &str,
        matches: impl Fn(&str) -> bool,
        bin: &std::path::Path,
        name: &str,
    ) -> Result<crate::install::Published, Error> {
        let (asset, body) = self.selected_asset(endpoint, false, matches)?;
        if asset.name.ends_with(".zip") || asset.name.ends_with(".tar.gz") {
            return Err(Error::BackendFailure(
                "archive asset requires verified extraction, not binary publication".into(),
            ));
        }
        crate::install::publish_binary(bin, name, &body, &asset.integrity)
    }

    pub fn install_archive(
        &mut self,
        endpoint: &str,
        codeberg_list: bool,
        matches: impl Fn(&str) -> bool,
        target: ArchiveTarget<'_>,
    ) -> Result<crate::install::Published, Error> {
        let (asset, body) = self.selected_asset(endpoint, codeberg_list, matches)?;
        let format = if asset.name.ends_with(".tar.gz") {
            crate::archive::Format::TarGz
        } else if asset.name.ends_with(".zip") {
            crate::archive::Format::Zip
        } else {
            return Err(Error::BackendFailure(format!(
                "unsupported archive: {}",
                asset.name
            )));
        };
        crate::install::publish_archive(
            target.bin,
            target.name,
            &body,
            &asset.integrity,
            format,
            target.member,
        )
    }

    /// The asset authority is the exact selected URL returned by an approved
    /// metadata request, additionally bound to that release repository. This
    /// never adds an asset URL (or a host wildcard) to RELEASE_ENDPOINTS.
    pub fn selected_asset(
        &mut self,
        endpoint: &str,
        codeberg_list: bool,
        matches: impl Fn(&str) -> bool,
    ) -> Result<(crate::release::Asset, Vec<u8>), Error> {
        let metadata = self.metadata(endpoint)?;
        let asset = crate::release::select_matching(&metadata, codeberg_list, matches)?;
        let repository = if let Some(path) = endpoint.strip_prefix("https://api.github.com/repos/")
        {
            path.split_once("/releases")
                .map(|(repo, _)| format!("https://github.com/{repo}/releases/download/"))
        } else if let Some(path) = endpoint.strip_prefix("https://codeberg.org/api/v1/repos/") {
            path.split_once("/releases")
                .map(|(repo, _)| format!("https://codeberg.org/{repo}/releases/download/"))
        } else {
            None
        };
        let repository = repository.ok_or_else(|| {
            Error::Internal(format!("no asset authority rule for endpoint {endpoint}"))
        })?;
        let remainder = asset.url.strip_prefix(&repository);
        if remainder.is_none_or(|path| {
            path.is_empty()
                || path.contains(['%', '\\', '?', '#'])
                || path
                    .split('/')
                    .any(|part| part.is_empty() || matches!(part, "." | ".."))
        }) {
            return Err(Error::Internal(format!(
                "asset URL is outside its approved release repository: {}",
                asset.url
            )));
        }
        let response = self.transport.get(&asset.url)?;
        if (300..400).contains(&response.status) {
            return Err(Error::Internal(format!(
                "asset redirect refused: {} (HTTP {})",
                asset.url, response.status
            )));
        }
        if response.status != 200 {
            return Err(Error::BackendFailure(format!(
                "asset {}: HTTP {}",
                asset.url, response.status
            )));
        }
        Ok((asset, response.body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct CountingTransport<'a> {
        calls: &'a Cell<usize>,
        status: u16,
    }
    impl Transport for CountingTransport<'_> {
        fn get(&mut self, _: &str) -> Result<Response, Error> {
            self.calls.set(self.calls.get() + 1);
            Ok(Response {
                status: self.status,
                body: b"synthetic response, not a captured API response".to_vec(),
            })
        }
    }

    fn refused_before_transport(url: &str) {
        let calls = Cell::new(0);
        let mut client = ReleaseClient::new(CountingTransport {
            calls: &calls,
            status: 200,
        });
        let error = client.metadata(url).expect_err("foreign URL must fail");
        assert_eq!(error.exit_code(), 9);
        assert!(error.to_string().contains(url));
        assert_eq!(calls.get(), 0);
        println!(
            "ENDPOINT_REFUSED exit={} transport_calls={} error={error}",
            error.exit_code(),
            calls.get()
        );
    }

    #[test]
    fn different_host_is_refused_before_transport() {
        refused_before_transport("https://example.invalid/repos/yt-dlp/yt-dlp/releases/latest");
    }
    #[test]
    fn different_path_is_refused_before_transport() {
        refused_before_transport("https://api.github.com/repos/foreign/project/releases/latest");
    }
    #[test]
    fn different_scheme_is_refused_before_transport() {
        refused_before_transport("http://api.github.com/repos/yt-dlp/yt-dlp/releases/latest");
    }
    #[test]
    fn ports_queries_fragments_userinfo_and_encoded_paths_do_not_bypass_exact_match() {
        for url in [
            "https://api.github.com:443/repos/yt-dlp/yt-dlp/releases/latest",
            "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest?anything",
            "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest#anything",
            "https://user@api.github.com/repos/yt-dlp/yt-dlp/releases/latest",
            "https://api.github.com/repos/yt-dlp/yt-dlp/releases/%6catest",
        ] {
            refused_before_transport(url);
        }
    }
    #[test]
    fn synthetic_allowed_metadata_response_calls_transport_once() {
        for endpoint in RELEASE_ENDPOINTS {
            let calls = Cell::new(0);
            let mut client = ReleaseClient::new(CountingTransport {
                calls: &calls,
                status: 200,
            });
            assert_eq!(
                client.metadata(endpoint.url).expect("allowed"),
                b"synthetic response, not a captured API response"
            );
            assert_eq!(calls.get(), 1);
            println!(
                "EXACT_ENDPOINT_ALLOWED calls=1 url={} source={}",
                endpoint.url, endpoint.source
            );
        }
    }
    #[test]
    fn synthetic_redirect_is_not_a_second_request() {
        let calls = Cell::new(0);
        let mut client = ReleaseClient::new(CountingTransport {
            calls: &calls,
            status: 302,
        });
        let error = client
            .metadata(RELEASE_ENDPOINTS[0].url)
            .expect_err("redirect");
        assert_eq!(error.exit_code(), 9);
        assert_eq!(calls.get(), 1);
        println!("REDIRECT_REFUSED exit=9 transport_calls=1 second_request_calls=0 error={error}");
    }
    #[test]
    fn synthetic_http_failure_is_download_error_3() {
        let calls = Cell::new(0);
        let mut client = ReleaseClient::new(CountingTransport {
            calls: &calls,
            status: 503,
        });
        let error = client
            .metadata(RELEASE_ENDPOINTS[0].url)
            .expect_err("HTTP failure");
        assert_eq!(error.exit_code(), 3);
        assert_eq!(calls.get(), 1);
        println!("HTTP_FAILURE exit=3 transport_calls=1 error={error}");
    }
    struct SyntheticAssetTransport<'a> {
        calls: &'a Cell<usize>,
        asset_url: &'a str,
        asset_status: u16,
    }
    impl Transport for SyntheticAssetTransport<'_> {
        fn get(&mut self, url: &str) -> Result<Response, Error> {
            let call = self.calls.get();
            self.calls.set(call + 1);
            if call == 0 {
                assert_eq!(
                    url,
                    "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest"
                );
                Ok(Response { status:200, body: serde_json::to_vec(&serde_json::json!({"_test_provenance":"SYNTHETIC NOT CAPTURED", "assets":[{"name":"backend", "size":11, "digest":"sha256:42404f785c9107ddb29581517f9eb04fc57a168eb84cac0f6b7e2ac9c6ae5ccf", "browser_download_url":self.asset_url}]})).unwrap() })
            } else {
                assert_eq!(call, 1, "no redirects or additional requests");
                assert_eq!(url, self.asset_url);
                Ok(Response {
                    status: self.asset_status,
                    body: b"new-backend".to_vec(),
                })
            }
        }
    }
    #[test]
    fn synthetic_foreign_asset_is_refused_before_its_transport_call() {
        for url in [
            "https://evil.invalid/backend",
            "https://github.com/foreign/project/releases/download/v/backend",
            "https://github.com/yt-dlp/yt-dlp/releases/download/../backend",
            "https://github.com/yt-dlp/yt-dlp/releases/download/%2e%2e/backend",
        ] {
            let calls = Cell::new(0);
            let mut client = ReleaseClient::new(SyntheticAssetTransport {
                calls: &calls,
                asset_url: url,
                asset_status: 200,
            });
            let error = client
                .selected_asset(
                    "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest",
                    false,
                    |name| name == "backend",
                )
                .unwrap_err();
            assert_eq!(error.exit_code(), 9);
            assert_eq!(calls.get(), 1);
            println!("SYNTHETIC_ASSET_REFUSED exit=9 metadata_calls=1 asset_calls=0 {error}");
        }
    }
    #[test]
    fn synthetic_asset_redirect_cannot_open_a_third_request() {
        let calls = Cell::new(0);
        let mut client = ReleaseClient::new(SyntheticAssetTransport {
            calls: &calls,
            asset_url: "https://github.com/yt-dlp/yt-dlp/releases/download/v/backend",
            asset_status: 302,
        });
        let error = client
            .selected_asset(
                "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest",
                false,
                |name| name == "backend",
            )
            .unwrap_err();
        assert_eq!(error.exit_code(), 9);
        assert_eq!(calls.get(), 2);
        println!("SYNTHETIC_ASSET_REDIRECT_REFUSED metadata_calls=1 asset_calls=1 redirect_calls=0 {error}");
    }
}
