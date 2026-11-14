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
}
