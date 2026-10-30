//! The published release of the Android driver app (`android/`), which
//! drivers download from this server instead of an app store. A release is
//! two files in one directory, written by `android/scripts/build-release.sh
//! --publish <dir>`:
//!
//! - the signed APK, e.g. `logistics-driver-0.2.0.apk`;
//! - `latest.json`, describing it (see [`Manifest`]).
//!
//! The directory is `DRIVER_APP_DIR`, or `{UPLOAD_DIR}/driver-app` by default,
//! so a deployment keeps releases on the uploads volume. See
//! `docs/android-driver-app.md` ("Distribution").

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use utoipa::ToSchema;

/// The file in the release directory that names the current release.
pub const MANIFEST_FILE: &str = "latest.json";

/// Where releases are kept. Passed to the routes as app data, so tests can
/// point each app at its own directory.
#[derive(Debug, Clone)]
pub struct DriverAppDir(pub PathBuf);

impl DriverAppDir {
    /// `DRIVER_APP_DIR`, else `driver-app` under `UPLOAD_DIR` (default `./uploads`).
    pub fn from_env() -> Self {
        let dir = std::env::var("DRIVER_APP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(
                    std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "./uploads".to_string()),
                )
                .join("driver-app")
            });
        DriverAppDir(dir)
    }
}

/// `latest.json` as the publish script writes it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    /// Android `versionCode`: the app compares this with its own to decide
    /// whether an update is available.
    pub version_code: i64,
    pub version_name: String,
    /// The APK's file name, in the same directory.
    pub file: String,
    /// Hex SHA-256 of the APK, so the office can check a download.
    pub sha256: String,
    /// Unix seconds.
    pub released_at: i64,
}

/// What `GET /api/driver-app/latest` returns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct DriverAppRelease {
    pub version_code: i64,
    pub version_name: String,
    pub sha256: String,
    pub size_bytes: u64,
    /// Unix seconds.
    pub released_at: i64,
    /// The APK's file name, as the download offers to save it.
    pub file_name: String,
    /// Path of the download on this server; the same for every release, so
    /// a link or QR code handed to drivers never goes stale.
    pub download_path: String,
}

pub const DOWNLOAD_PATH: &str = "/api/driver-app/download";

#[derive(Debug)]
pub enum ReleaseError {
    /// `latest.json` exists but can't be used; the message says why.
    BadManifest(String),
    Io(std::io::Error),
}

impl fmt::Display for ReleaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReleaseError::BadManifest(why) => {
                write!(f, "the published driver app release is invalid: {why}")
            }
            ReleaseError::Io(err) => write!(f, "couldn't read the driver app release: {err}"),
        }
    }
}

impl std::error::Error for ReleaseError {}

impl DriverAppDir {
    /// The current release, or `None` if nothing has been published.
    pub fn latest(&self) -> Result<Option<DriverAppRelease>, ReleaseError> {
        Ok(self.latest_apk()?.map(|(release, _)| release))
    }

    /// The current release and its APK's path, for the download.
    pub fn latest_apk(&self) -> Result<Option<(DriverAppRelease, PathBuf)>, ReleaseError> {
        Ok(self.load()?.map(|(manifest, size_bytes, path)| {
            let release = DriverAppRelease {
                version_code: manifest.version_code,
                version_name: manifest.version_name,
                sha256: manifest.sha256,
                size_bytes,
                released_at: manifest.released_at,
                file_name: manifest.file,
                download_path: DOWNLOAD_PATH.to_string(),
            };
            (release, path)
        }))
    }

    /// Read and check the manifest, and find its APK.
    fn load(&self) -> Result<Option<(Manifest, u64, PathBuf)>, ReleaseError> {
        let text = match std::fs::read_to_string(self.0.join(MANIFEST_FILE)) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(ReleaseError::Io(err)),
        };
        let manifest: Manifest = serde_json::from_str(&text)
            .map_err(|err| ReleaseError::BadManifest(format!("{MANIFEST_FILE}: {err}")))?;
        validate(&manifest)?;
        let path = self.0.join(&manifest.file);
        let size = match std::fs::metadata(&path) {
            Ok(meta) if meta.is_file() => meta.len(),
            Ok(_) => {
                return Err(ReleaseError::BadManifest(format!(
                    "{} is not a file",
                    manifest.file
                )));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(ReleaseError::BadManifest(format!(
                    "{} is missing",
                    manifest.file
                )));
            }
            Err(err) => return Err(ReleaseError::Io(err)),
        };
        Ok(Some((manifest, size, path)))
    }
}

/// Reject a manifest that would serve anything but an APK in the release
/// directory, or that the app couldn't compare versions with.
fn validate(m: &Manifest) -> Result<(), ReleaseError> {
    let bad = |why: &str| Err(ReleaseError::BadManifest(why.to_string()));
    if m.version_code < 1 {
        return bad("version_code must be 1 or more");
    }
    if m.version_name.trim().is_empty() {
        return bad("version_name is empty");
    }
    // A bare file name: no directories, so it can't point outside the release dir.
    let plain = Path::new(&m.file)
        .file_name()
        .is_some_and(|name| name == m.file.as_str());
    if !plain || m.file.starts_with('.') || !m.file.ends_with(".apk") {
        return bad("file must be the name of an .apk in the release directory");
    }
    if m.sha256.len() != 64 || !m.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return bad("sha256 must be 64 hex characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::test_support::TempDir;

    fn manifest() -> Manifest {
        Manifest {
            version_code: 3,
            version_name: "0.3.0".into(),
            file: "logistics-driver-0.3.0.apk".into(),
            sha256: "a".repeat(64),
            released_at: 1_793_000_000,
        }
    }

    fn publish(dir: &Path, m: &Manifest, apk: &[u8]) {
        std::fs::write(dir.join(&m.file), apk).unwrap();
        std::fs::write(dir.join(MANIFEST_FILE), serde_json::to_string(m).unwrap()).unwrap();
    }

    #[test]
    fn test_nothing_published_is_none() {
        let tmp = TempDir::create();
        assert_eq!(DriverAppDir(tmp.0.clone()).latest().unwrap(), None);
        assert_eq!(
            DriverAppDir(tmp.0.join("does-not-exist")).latest().unwrap(),
            None
        );
    }

    #[test]
    fn test_latest_reads_the_manifest_and_the_apk_size() {
        let tmp = TempDir::create();
        publish(&tmp.0, &manifest(), b"0123456789");

        let release = DriverAppDir(tmp.0.clone()).latest().unwrap().unwrap();

        assert_eq!(release.version_code, 3);
        assert_eq!(release.version_name, "0.3.0");
        assert_eq!(release.size_bytes, 10);
        assert_eq!(release.file_name, "logistics-driver-0.3.0.apk");
        assert_eq!(release.download_path, "/api/driver-app/download");
        let (_, path) = DriverAppDir(tmp.0.clone()).latest_apk().unwrap().unwrap();
        assert_eq!(path, tmp.0.join("logistics-driver-0.3.0.apk"));
    }

    #[test]
    fn test_a_missing_apk_is_an_error_not_a_release() {
        let tmp = TempDir::create();
        let m = manifest();
        std::fs::write(
            tmp.0.join(MANIFEST_FILE),
            serde_json::to_string(&m).unwrap(),
        )
        .unwrap();

        let err = DriverAppDir(tmp.0.clone()).latest().unwrap_err();
        assert!(
            err.to_string()
                .contains("logistics-driver-0.3.0.apk is missing"),
            "{err}"
        );
    }

    #[test]
    fn test_unreadable_json_is_an_error() {
        let tmp = TempDir::create();
        std::fs::write(tmp.0.join(MANIFEST_FILE), "{not json").unwrap();
        assert!(matches!(
            DriverAppDir(tmp.0.clone()).latest(),
            Err(ReleaseError::BadManifest(_))
        ));
    }

    #[test]
    fn test_manifest_validation() {
        assert!(validate(&manifest()).is_ok());
        let cases: Vec<(&str, Manifest)> = vec![
            (
                "zero version",
                Manifest {
                    version_code: 0,
                    ..manifest()
                },
            ),
            (
                "blank name",
                Manifest {
                    version_name: " ".into(),
                    ..manifest()
                },
            ),
            (
                "parent dir",
                Manifest {
                    file: "../secret.apk".into(),
                    ..manifest()
                },
            ),
            (
                "absolute",
                Manifest {
                    file: "/etc/app.apk".into(),
                    ..manifest()
                },
            ),
            (
                "subdir",
                Manifest {
                    file: "old/app.apk".into(),
                    ..manifest()
                },
            ),
            (
                "hidden",
                Manifest {
                    file: ".apk".into(),
                    ..manifest()
                },
            ),
            (
                "not an apk",
                Manifest {
                    file: "latest.json".into(),
                    ..manifest()
                },
            ),
            (
                "short hash",
                Manifest {
                    sha256: "abc".into(),
                    ..manifest()
                },
            ),
            (
                "non-hex hash",
                Manifest {
                    sha256: "z".repeat(64),
                    ..manifest()
                },
            ),
        ];
        for (name, m) in cases {
            assert!(validate(&m).is_err(), "{name} should be rejected");
        }
    }

    #[test]
    fn test_from_env_defaults_under_the_upload_dir() {
        // Only checks the shape; the env vars themselves aren't touched, since
        // tests run in parallel.
        let DriverAppDir(dir) = DriverAppDir::from_env();
        assert!(dir.ends_with("driver-app") || std::env::var("DRIVER_APP_DIR").is_ok());
    }
}
