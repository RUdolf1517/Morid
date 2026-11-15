//! Synthetic integrity acceptance: the saved native test executable stands in
//! for the previously installed backend. No real backend or socket is used.
use morid_core::Error;
use morid_net::transport::{ReleaseClient, Response, Transport};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::{fs, path::PathBuf};
const SYNTHETIC_DIGEST: &str =
    "sha256:42404f785c9107ddb29581517f9eb04fc57a168eb84cac0f6b7e2ac9c6ae5ccf";
const ENDPOINT: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const ASSET: &str = "https://github.com/yt-dlp/yt-dlp/releases/download/synthetic/backend";
struct SyntheticTransport<'a> {
    body: &'a [u8],
    calls: &'a Cell<usize>,
}
impl Transport for SyntheticTransport<'_> {
    fn get(&mut self, url: &str) -> Result<Response, Error> {
        self.calls.set(self.calls.get() + 1);
        let body = if url == ENDPOINT {
            serde_json::to_vec(&serde_json::json!({"_test_provenance":"SYNTHETIC NOT CAPTURED", "tag_name":"synthetic", "assets":[{"name":"backend", "size":11, "digest":SYNTHETIC_DIGEST, "browser_download_url":ASSET}]})).unwrap()
        } else {
            assert_eq!(url, ASSET);
            self.body.to_vec()
        };
        Ok(Response { status: 200, body })
    }
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("morid-i9-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut text, byte| {
        write!(&mut text, "{byte:02x}").unwrap();
        text
    })
}
#[test]
fn synthetic_bad_size_and_hash_preserve_old_executable_and_it_still_runs() {
    let root = Directory::new();
    let old = root.0.join(if cfg!(windows) {
        "backend.exe"
    } else {
        "backend"
    });
    fs::copy(std::env::current_exe().expect("test binary"), &old).expect("old backend");
    let before = fs::read(&old).expect("before");
    let before_hash = hex(&Sha256::digest(&before));
    println!("SYNTHETIC_EXPECTED_ORIGIN fixed test metadata size=11 digest={SYNTHETIC_DIGEST}; not derived from downloaded bytes");
    for (body, failed_check) in [
        (b"short".as_slice(), "size check failed: expected 11, got 5"),
        (b"bad-backend".as_slice(), "SHA-256 check failed: expected"),
    ] {
        let calls = Cell::new(0);
        let mut client = ReleaseClient::new(SyntheticTransport {
            body,
            calls: &calls,
        });
        let error = client
            .install_binary(
                ENDPOINT,
                |name| name == "backend",
                &root.0,
                old.file_name().unwrap().to_str().unwrap(),
            )
            .expect_err("corruption");
        assert_eq!(calls.get(), 2);
        println!(
            "SYNTHETIC_PIPELINE metadata_requests=1 exact_selected_asset_requests=1 real_sockets=0"
        );
        assert_eq!(error.exit_code(), 3);
        assert!(error.to_string().contains(failed_check), "{error}");
        let after = fs::read(&old).expect("after");
        assert_eq!(before, after);
        assert_eq!(hex(&Sha256::digest(&after)), before_hash);
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1, "staging residue");
        println!("INTEGRITY_REFUSED exit=3 {error}\nOLD_SHA256_BEFORE={before_hash}\nOLD_SHA256_AFTER={}\nOLD_BYTE_COMPARISON=equal", hex(&Sha256::digest(&after)));
        // Run the saved native executable, not a parser or a mock result.
        let output = std::process::Command::new(&old)
            .arg("--list")
            .output()
            .expect("execute old binary");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout)
            .contains("synthetic_bad_size_and_hash_preserve_old_executable_and_it_still_runs"));
        println!(
            "OLD_EXECUTABLE_RUN argv=[--list] exit={} stdout={} ",
            output.status,
            String::from_utf8_lossy(&output.stdout)
        );
        if let Some(directory) = std::env::var_os("MORID_INTEGRITY_EVIDENCE") {
            let directory = PathBuf::from(directory);
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("before.bin"), &before).unwrap();
            fs::copy(&old, directory.join("after.bin")).unwrap();
        }
    }
}
