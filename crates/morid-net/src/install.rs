//! Single executable publication. No engine selection or network access here.
use morid_core::Error;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

/// Expected values must come from release metadata, never from downloaded bytes.
#[derive(Debug, Clone)]
pub struct Integrity {
    pub size: u64,
    sha256: [u8; 32],
}
impl Integrity {
    pub fn from_metadata(size: u64, digest: &str) -> Result<Self, Error> {
        let hex = digest.strip_prefix("sha256:").unwrap_or(digest);
        if hex.len() != 64 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(failure("metadata: missing or invalid SHA-256"));
        }
        let mut sha256 = [0; 32];
        for (index, byte) in sha256.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
                .map_err(|_| failure("metadata: invalid SHA-256"))?;
        }
        Ok(Self { size, sha256 })
    }
}

fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn failure(message: impl Into<String>) -> Error {
    Error::BackendFailure(message.into())
}
fn io_error(error: std::io::Error) -> Error {
    failure(format!("install: {error}"))
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 15) as usize] as char);
    }
    result
}

/// Size is checked before hashing. The caller cannot publish a failed result.
pub fn verify(path: &Path, expected: &Integrity) -> Result<(), Error> {
    let mut file = fs::File::open(path).map_err(io_error)?;
    let size = file.metadata().map_err(io_error)?.len();
    if size != expected.size {
        return Err(failure(format!(
            "size check failed: expected {}, got {size}",
            expected.size
        )));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let actual: [u8; 32] = hash.finalize().into();
    if actual != expected.sha256 {
        return Err(failure(format!(
            "SHA-256 check failed: expected {}, got {}",
            hex(&expected.sha256),
            hex(&actual)
        )));
    }
    Ok(())
}

struct Staged(PathBuf);
impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Published {
    Installed,
    Unchanged,
}

/// Stage inside `bin`, verify size then SHA-256, then rename on the same
/// filesystem. Never move/remove the old binary as a preparation step.
/// Archive callers must separately verify the archive and safely unpack it;
/// this entry point is for an asset that is itself the executable.
pub fn publish_binary(
    bin: &Path,
    name: &str,
    body: &[u8],
    expected: &Integrity,
) -> Result<Published, Error> {
    let mut parts = Path::new(name).components();
    if name.is_empty()
        || name.contains(['/', '\\', ':'])
        || !matches!(parts.next(), Some(Component::Normal(_)))
        || parts.next().is_some()
    {
        return Err(failure(format!("invalid executable filename: {name:?}")));
    }
    fs::create_dir_all(bin).map_err(io_error)?;
    let (stage, mut file) = loop {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = bin.join(format!(".morid-stage-{}-{serial}", stamp()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => break (Staged(path), file),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_error(e)),
        }
    };
    file.write_all(body).map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    drop(file);
    verify(&stage.0, expected)?;
    let destination = bin.join(name);
    // A verified download is still checked even when a binary already exists.
    if let Ok(metadata) = fs::symlink_metadata(&destination) {
        if metadata.file_type().is_file() && verify(&destination, expected).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o111 != 0 {
                    return Ok(Published::Unchanged);
                }
            }
            #[cfg(not(unix))]
            return Ok(Published::Unchanged);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stage.0, fs::Permissions::from_mode(0o755)).map_err(io_error)?;
    }
    // fs::rename replaces an existing file; on failure the old path is retained.
    fs::rename(&stage.0, &destination).map_err(io_error)?;
    Ok(Published::Installed)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SYNTHETIC_DIGEST: &str =
        "sha256:42404f785c9107ddb29581517f9eb04fc57a168eb84cac0f6b7e2ac9c6ae5ccf";
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "morid-integrity-{}-{}",
                stamp(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("private directory");
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn synthetic_verified_binary_is_published_then_idempotent() {
        let root = Directory::new();
        let expected = Integrity::from_metadata(11, SYNTHETIC_DIGEST).unwrap();
        assert_eq!(
            publish_binary(&root.0, "backend", b"new-backend", &expected).unwrap(),
            Published::Installed
        );
        assert_eq!(
            publish_binary(&root.0, "backend", b"new-backend", &expected).unwrap(),
            Published::Unchanged
        );
        assert_eq!(fs::read(root.0.join("backend")).unwrap(), b"new-backend");
        println!(
            "SYNTHETIC_PUBLICATION first=Installed second=Unchanged bytes=equal stage_residue=0"
        );
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
    }

    #[test]
    fn synthetic_verified_update_replaces_existing_bytes() {
        let root = Directory::new();
        fs::write(root.0.join("backend"), b"old-backend").unwrap();
        let expected = Integrity::from_metadata(11, SYNTHETIC_DIGEST).unwrap();
        assert_eq!(
            publish_binary(&root.0, "backend", b"new-backend", &expected).unwrap(),
            Published::Installed
        );
        assert_eq!(fs::read(root.0.join("backend")).unwrap(), b"new-backend");
        println!("SYNTHETIC_VERIFIED_UPDATE old-backend -> new-backend; no backup move before verification");
    }

    #[test]
    fn synthetic_missing_digest_fails_closed() {
        for digest in ["", "sha256:", "md5:abc", "not-a-hash"] {
            assert!(Integrity::from_metadata(11, digest).is_err());
        }
    }
}
