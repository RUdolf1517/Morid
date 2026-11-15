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

fn valid_name(name: &str) -> Result<(), Error> {
    let mut parts = Path::new(name).components();
    if name.is_empty()
        || name.contains(['/', '\\', ':'])
        || !matches!(parts.next(), Some(Component::Normal(_)))
        || parts.next().is_some()
    {
        return Err(failure(format!("invalid executable filename: {name:?}")));
    }
    Ok(())
}

struct StagedDirectory(PathBuf);
impl Drop for StagedDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Verify the downloaded archive beside the destination before inspecting it.
/// Extract only into a fresh private directory; publish exactly the explicitly
/// selected regular file, never a guessed basename or an archive-provided link.
/// The archive digest authenticates the member. No release digest is invented
/// for that member: its internal digest below only rechecks the local copy.
pub fn publish_archive(
    bin: &Path,
    name: &str,
    body: &[u8],
    expected: &Integrity,
    format: crate::archive::Format,
    member: &Path,
) -> Result<Published, Error> {
    valid_name(name)?;
    if member.as_os_str().is_empty()
        || member.to_string_lossy().contains(['\\', ':'])
        || member
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(failure(format!(
            "invalid archive executable member: {member:?}"
        )));
    }
    fs::create_dir_all(bin).map_err(io_error)?;
    let (download, mut file) = loop {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = bin.join(format!(".morid-download-{}-{serial}", stamp()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => break (Staged(path), file),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_error(e)),
        }
    };
    file.write_all(body).map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    drop(file);
    verify(&download.0, expected)?;
    let staging = loop {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = bin.join(format!(".morid-unpack-{}-{serial}", stamp()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => break StagedDirectory(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_error(e)),
        }
    };
    let verified_body = fs::read(&download.0).map_err(io_error)?;
    crate::archive::extract(&verified_body, format, &staging.0)?;
    let executable = staging.0.join(member);
    if !fs::symlink_metadata(&executable)
        .map_err(io_error)?
        .file_type()
        .is_file()
    {
        return Err(failure(format!(
            "archive executable is not a regular file: {member:?}"
        )));
    }
    let bytes = fs::read(executable).map_err(io_error)?;
    let local_copy = Integrity {
        size: bytes.len() as u64,
        sha256: Sha256::digest(&bytes).into(),
    };
    publish_binary(bin, name, &bytes, &local_copy)
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
    valid_name(name)?;
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

    fn synthetic_tar(member: &str) -> Vec<u8> {
        let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(gzip);
        let mut header = tar::Header::new_gnu();
        header.set_size(11);
        header.set_mode(0o7777); // Archive privileges must not survive publication.
        header.set_cksum();
        archive
            .append_data(&mut header, member, &b"new-backend"[..])
            .unwrap();
        archive.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn synthetic_verified_archive_publishes_only_explicit_member_and_cleans_staging() {
        let root = Directory::new();
        let body = synthetic_tar("package/backend");
        // Synthetic release metadata for this dynamically generated archive.
        let digest = format!("sha256:{}", hex(&Sha256::digest(&body)));
        let expected = Integrity::from_metadata(body.len() as u64, &digest).unwrap();
        for outcome in [Published::Installed, Published::Unchanged] {
            assert_eq!(
                publish_archive(
                    &root.0,
                    "backend",
                    &body,
                    &expected,
                    crate::archive::Format::TarGz,
                    Path::new("package/backend")
                )
                .unwrap(),
                outcome
            );
            assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
        }
        assert_eq!(fs::read(root.0.join("backend")).unwrap(), b"new-backend");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(root.0.join("backend"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o7777,
                0o755
            );
        }
        println!("SYNTHETIC_ARCHIVE_PUBLICATION verified_archive=true explicit_member=package/backend first=Installed repeat=Unchanged residue=0 privileged_bits=not_preserved");
    }

    #[test]
    fn synthetic_archive_bad_hash_and_missing_member_preserve_old_binary() {
        let root = Directory::new();
        fs::write(root.0.join("backend"), b"old-backend").unwrap();
        let body = synthetic_tar("package/backend");
        let digest = format!("sha256:{}", hex(&Sha256::digest(&body)));
        let expected = Integrity::from_metadata(body.len() as u64, &digest).unwrap();
        let wrong = Integrity::from_metadata(body.len() as u64, SYNTHETIC_DIGEST).unwrap();
        for (metadata, member) in [
            (&wrong, "package/backend"),
            (&expected, "missing"),
            (&expected, "../backend"),
        ] {
            let error = publish_archive(
                &root.0,
                "backend",
                &body,
                metadata,
                crate::archive::Format::TarGz,
                Path::new(member),
            )
            .unwrap_err();
            assert_eq!(error.exit_code(), 3);
            assert_eq!(fs::read(root.0.join("backend")).unwrap(), b"old-backend");
            assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
            println!(
                "SYNTHETIC_ARCHIVE_FAILURE old_bytes=equal stage_residue=0 exit=3 error={error}"
            );
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
