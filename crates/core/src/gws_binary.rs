//! Find or download the pinned gws binary.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::ports::download;
use crate::{Core, CoreError};

/// The only gws version the app runs.
///
/// # Example
///
/// ```
/// assert_eq!(haulmer_googleworkspace_mcp_core::gws_binary::GWS_VERSION, "0.22.5");
/// ```
pub const GWS_VERSION: &str = "0.22.5";

/// Base URL of the pinned GitHub release; assets are `<RELEASE_URL>/<asset>`.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::gws_binary::RELEASE_URL;
/// assert!(RELEASE_URL.ends_with("/v0.22.5"));
/// ```
pub const RELEASE_URL: &str = "https://github.com/googleworkspace/cli/releases/download/v0.22.5";

/// File name of the gws binary on this platform (also its name at the archive root).
const BINARY_NAME: &str = if cfg!(windows) { "gws.exe" } else { "gws" };

/// Release asset for the platform this build targets; `None` where gws publishes none.
/// The checksum file is `<asset>.sha256`.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::gws_binary::release_asset;
/// if let Some(asset) = release_asset() {
///     assert!(asset.starts_with("google-workspace-cli-"));
/// }
/// ```
pub fn release_asset() -> Option<&'static str> {
    let asset = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "google-workspace-cli-aarch64-apple-darwin.tar.gz",
        ("macos", "x86_64") => "google-workspace-cli-x86_64-apple-darwin.tar.gz",
        ("windows", "x86_64") => "google-workspace-cli-x86_64-pc-windows-msvc.zip",
        ("linux", "aarch64") => "google-workspace-cli-aarch64-unknown-linux-gnu.tar.gz",
        ("linux", "x86_64") => "google-workspace-cli-x86_64-unknown-linux-gnu.tar.gz",
        _ => return None,
    };
    Some(asset)
}

impl Core {
    /// The pinned gws to run, without downloading. Checks, in order, the first that
    /// reports [`GWS_VERSION`]: `gws` on PATH; on Unix the usual install folders
    /// `/opt/homebrew/bin/gws`, `/usr/local/bin/gws`, `<home>/.cargo/bin/gws` and
    /// `<home>/.local/bin/gws` (only those that exist); then `<app_data>/bin/gws[.exe]`.
    /// `None` otherwise. Never makes HTTP calls.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{Core, Paths};
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: "/a".into() });
    /// let gws = core.find_gws().unwrap(); // Some("gws") or Some("<app_data>/bin/gws") or None
    /// ```
    pub fn find_gws(&self) -> Result<Option<PathBuf>, CoreError> {
        let on_path = PathBuf::from("gws");
        if self.is_pinned(&on_path)? {
            return Ok(Some(on_path));
        }
        let mut candidates = self.well_known_gws();
        candidates.push(self.local_gws());
        for candidate in candidates {
            if candidate.is_file() && self.is_pinned(&candidate)? {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }

    /// Like [`Core::find_gws`], but downloads and verifies the release asset when missing.
    /// A checksum mismatch returns [`CoreError::ChecksumMismatch`] and writes nothing executable.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{Core, Paths};
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: "/a".into() });
    /// let gws = core.ensure_gws().unwrap();
    /// ```
    pub fn ensure_gws(&self) -> Result<PathBuf, CoreError> {
        if let Some(gws) = self.find_gws()? {
            return Ok(gws);
        }
        let asset = release_asset().ok_or_else(|| {
            CoreError::Download(format!(
                "no gws release for {} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            ))
        })?;
        let expected = parse_checksum(&download(
            self.http.as_ref(),
            &format!("{RELEASE_URL}/{asset}.sha256"),
        )?)?;
        let archive = download(self.http.as_ref(), &format!("{RELEASE_URL}/{asset}"))?;
        let actual = sha256_hex(&archive);
        if actual != expected {
            return Err(CoreError::ChecksumMismatch { expected, actual });
        }
        let binary = if asset.ends_with(".zip") {
            binary_from_zip(&archive)?
        } else {
            binary_from_tar_gz(&archive)?
        };
        let target = self.local_gws();
        install(&binary, &target)?;
        Ok(target)
    }

    /// Where a downloaded gws lives: `<app_data>/bin/gws[.exe]`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// assert!(self.local_gws().starts_with(&self.paths.app_data));
    /// ```
    fn local_gws(&self) -> PathBuf {
        self.paths.app_data.join("bin").join(BINARY_NAME)
    }

    /// Usual install locations of gws outside PATH (Homebrew, cargo, pipx-style); none on Windows.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // with home = /Users/ana on macOS:
    /// // [/opt/homebrew/bin/gws, /usr/local/bin/gws, /Users/ana/.cargo/bin/gws, /Users/ana/.local/bin/gws]
    /// let candidates = self.well_known_gws();
    /// ```
    fn well_known_gws(&self) -> Vec<PathBuf> {
        if cfg!(unix) {
            vec![
                PathBuf::from("/opt/homebrew/bin/gws"),
                PathBuf::from("/usr/local/bin/gws"),
                self.paths.home.join(".cargo").join("bin").join("gws"),
                self.paths.home.join(".local").join("bin").join("gws"),
            ]
        } else {
            Vec::new()
        }
    }

    /// Whether `program --version` starts with `gws <GWS_VERSION>`, run in the same
    /// controlled environment as every gws call ([`Core::gws_command`]). A program that
    /// cannot start counts as not pinned; `Err` only if the environment cannot be built.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if self.is_pinned(Path::new("gws"))? {
    ///     println!("gws on PATH is the pinned version");
    /// }
    /// ```
    fn is_pinned(&self, program: &Path) -> Result<bool, CoreError> {
        let command = self.gws_command(program, &["--version"])?;
        match self.runner.run(&command) {
            Ok(output) => Ok(output.exit_code == 0
                && output.stdout.lines().next().map(str::trim)
                    == Some(format!("gws {GWS_VERSION}").as_str())),
            Err(_) => Ok(false),
        }
    }
}

/// Reads the hex digest from a `<hex>  <filename>` checksum file (CRLF tolerated).
fn parse_checksum(body: &[u8]) -> Result<String, CoreError> {
    let text = String::from_utf8_lossy(body);
    let hex = text.split_whitespace().next().unwrap_or_default();
    if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(hex.to_ascii_lowercase())
    } else {
        Err(CoreError::Download(format!(
            "unreadable checksum file: {:?}",
            text.trim()
        )))
    }
}

/// Lowercase hex SHA-256 of `bytes`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Whether an archive entry path is the binary at the archive root (`gws` or `./gws`).
fn is_root_binary(path: &Path) -> bool {
    let mut parts = path
        .components()
        .filter(|component| *component != Component::CurDir);
    matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(name)), None) if name == BINARY_NAME
    )
}

/// The binary's bytes from a `.tar.gz` release archive.
fn binary_from_tar_gz(archive: &[u8]) -> Result<Vec<u8>, CoreError> {
    let bad = |error: std::io::Error| CoreError::Download(format!("bad archive: {error}"));
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries().map_err(bad)? {
        let mut entry = entry.map_err(bad)?;
        if is_root_binary(&entry.path().map_err(bad)?) {
            let mut binary = Vec::new();
            entry.read_to_end(&mut binary).map_err(bad)?;
            return Ok(binary);
        }
    }
    Err(missing_binary())
}

/// The binary's bytes from a `.zip` release archive.
fn binary_from_zip(archive: &[u8]) -> Result<Vec<u8>, CoreError> {
    let bad = |error: zip::result::ZipError| CoreError::Download(format!("bad archive: {error}"));
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).map_err(bad)?;
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).map_err(bad)?;
        if is_root_binary(Path::new(file.name().map_err(bad)?.as_ref())) {
            let mut binary = Vec::new();
            file.read_to_end(&mut binary)
                .map_err(|error| CoreError::Download(format!("bad archive: {error}")))?;
            return Ok(binary);
        }
    }
    Err(missing_binary())
}

/// Error for an archive without the binary at its root.
fn missing_binary() -> CoreError {
    CoreError::Download(format!("{BINARY_NAME} not found at the archive root"))
}

/// Writes `binary` to `target` atomically: a temp file in the same folder, the executable
/// bit on Unix, then a rename. On failure the temp file is removed.
fn install(binary: &[u8], target: &Path) -> Result<(), CoreError> {
    let folder = target.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(folder)?;
    let partial = folder.join(format!("{BINARY_NAME}.partial"));
    let result = write_executable(&partial, binary).and_then(|()| fs::rename(&partial, target));
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    Ok(result?)
}

/// Writes `bytes` to `path` and makes it executable on Unix.
fn write_executable(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    /// The zip path only runs on Windows builds, so check it here on every platform.
    #[test]
    fn zip_archive_yields_the_binary_at_its_root() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (name, data) in [
            ("LICENSE", "Apache"),
            ("sub/gws", "nested"),
            (BINARY_NAME, "root"),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(data.as_bytes()).unwrap();
        }
        let archive = zip.finish().unwrap().into_inner();

        assert_eq!(binary_from_zip(&archive).unwrap(), b"root");
        assert!(matches!(
            binary_from_zip(b"not a zip"),
            Err(CoreError::Download(_))
        ));
    }
}
