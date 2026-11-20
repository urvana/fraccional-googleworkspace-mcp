//! T08 — the pinned gws skills: the ZIP export and the names an agent installs.

use std::fs;
use std::io::{Cursor, Read};
use std::sync::Arc;

use haulmer_googleworkspace_mcp_core::skills::SKILLS_TARBALL_URL;
use haulmer_googleworkspace_mcp_core::testing::{FakeHttp, FakeReply, FakeRunner};
use haulmer_googleworkspace_mcp_core::{Core, CoreError, HttpResponse, Paths};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

/// Every skill folder of the fixture tarball (a subset of the real v0.22.5 one).
const FIXTURE_SKILLS: &[&str] = &[
    "gws-shared",
    "gws-gmail",
    "gws-gmail-send",
    "gws-calendar",
    "gws-calendar-agenda",
    "gws-chat",
    "gws-chat-send",
    "gws-meet",
    "gws-drive",
    "gws-drive-upload",
    "gws-people",
    "recipe-email-digest",
];

/// Builds a `.tar.gz` mimicking the GitHub tag tarball: `cli-705fb0ecac6f4249679958f6325b809b63fdde17/...`.
fn tarball(extra: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    append(
        &mut builder,
        "cli-705fb0ecac6f4249679958f6325b809b63fdde17/README.md",
        "# gws\n",
    );
    for name in FIXTURE_SKILLS {
        let content = format!("---\nname: {name}\n---\n\n# {name}\n");
        append(
            &mut builder,
            &format!("cli-705fb0ecac6f4249679958f6325b809b63fdde17/skills/{name}/SKILL.md"),
            &content,
        );
    }
    append(
        &mut builder,
        "cli-705fb0ecac6f4249679958f6325b809b63fdde17/skills/gws-gmail/references/flags.md",
        "flags\n",
    );
    extra(&mut builder);
    let tar = builder.into_inner().unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    std::io::Write::write_all(&mut gz, &tar).unwrap();
    gz.finish().unwrap()
}

/// Appends a regular file.
fn append(builder: &mut tar::Builder<Vec<u8>>, path: &str, content: &str) {
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    builder
        .append_data(&mut header, path, content.as_bytes())
        .unwrap();
}

/// Appends a regular file with a raw (unchecked) name, e.g. one containing `..`.
fn append_raw(builder: &mut tar::Builder<Vec<u8>>, raw_path: &str, content: &str) {
    let mut header = tar::Header::new_old();
    header.as_old_mut().name[..raw_path.len()].copy_from_slice(raw_path.as_bytes());
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    builder.append(&header, content.as_bytes()).unwrap();
}

/// Scopes `gws auth status` reports for Gmail read and Meet read.
const GMAIL_AND_MEET: &str = r#"{
  "user": "ana@haulmer.com",
  "scopes": [
    "https://www.googleapis.com/auth/gmail.readonly",
    "https://www.googleapis.com/auth/meetings.space.readonly"
  ],
  "token_valid": true
}"#;

/// A temp home with gws on PATH, a connected Google account, and a core whose HTTP serves a
/// skills archive.
struct Fixture {
    home: TempDir,
    _app_data: TempDir,
    core: Core,
}

impl Fixture {
    /// Serves `archive` and trusts its SHA-256 (the fixture stands in for the pinned tarball).
    fn new(archive: Vec<u8>) -> Self {
        let sha256: String = Sha256::digest(&archive)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Self::serving(HttpResponse::ok(archive)).trusting(&sha256)
    }

    /// Serves `response` and expects the real pinned checksum.
    fn serving(response: HttpResponse) -> Self {
        let home = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let config_dir = home.path().join(".config/gws");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("credentials.enc"), b"x").unwrap();
        let runner = Arc::new(FakeRunner::new());
        runner.on_program("gws", &["--version"], FakeReply::ok("gws 0.22.5\n"));
        runner.on(&["auth", "status"], FakeReply::ok(GMAIL_AND_MEET));
        let http = Arc::new(FakeHttp::new());
        http.on(SKILLS_TARBALL_URL, response);
        let paths = Paths {
            home: home.path().into(),
            app_data: app_data.path().into(),
        };
        let core = Core::new(paths, runner, http);
        Self {
            home,
            _app_data: app_data,
            core,
        }
    }

    fn trusting(mut self, sha256: &str) -> Self {
        self.core = self.core.with_skills_sha256(sha256);
        self
    }

    /// Exports the ZIP and opens it.
    fn export(&self) -> Result<zip::ZipArchive<Cursor<Vec<u8>>>, CoreError> {
        let bytes = self.core.export_skills_zip()?;
        Ok(zip::ZipArchive::new(Cursor::new(bytes)).unwrap())
    }
}

/// Sorted entry names of a ZIP.
fn entries(archive: &zip::ZipArchive<Cursor<Vec<u8>>>) -> Vec<String> {
    let mut names: Vec<String> = archive
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect();
    names.sort();
    names
}

/// Text of one ZIP entry.
fn read_entry(archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>, name: &str) -> String {
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

#[test]
fn export_holds_shared_and_granted_service_skills() {
    let fixture = Fixture::new(tarball(|_| {}));

    let mut archive = fixture.export().unwrap();

    assert_eq!(
        entries(&archive),
        strings(&[
            "gws-gmail-send/SKILL.md",
            "gws-gmail/SKILL.md",
            "gws-gmail/references/flags.md",
            "gws-meet/SKILL.md",
            "gws-shared/SKILL.md",
        ])
    );
    assert_eq!(
        read_entry(&mut archive, "gws-meet/SKILL.md"),
        "---\nname: gws-meet\n---\n\n# gws-meet\n"
    );
    assert_eq!(
        read_entry(&mut archive, "gws-gmail/references/flags.md"),
        "flags\n"
    );
}

#[test]
fn a_custom_config_folder_adds_an_environment_section_to_the_shared_skill() {
    let fixture = Fixture::new(tarball(|_| {}));
    let custom = fixture.home.path().join("haulmer-gws");
    fixture.core.set_config_dir(Some(custom.clone())).unwrap();
    // The credentials of the connected account now live in the custom folder.
    fs::create_dir_all(&custom).unwrap();
    fs::write(custom.join("credentials.enc"), b"x").unwrap();

    let mut archive = fixture.export().unwrap();

    let shared = read_entry(&mut archive, "gws-shared/SKILL.md");
    assert!(shared.starts_with("---\nname: gws-shared\n---\n\n# gws-shared\n"));
    assert!(shared.contains("## Environment"), "{shared}");
    assert!(
        shared.contains(&format!(
            "export GOOGLE_WORKSPACE_CLI_CONFIG_DIR='{}'\nexport GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND=file\n",
            custom.display()
        )),
        "{shared}"
    );
}

#[test]
fn the_config_folder_is_single_quoted_for_posix_shells() {
    let fixture = Fixture::new(tarball(|_| {}));
    let custom = fixture.home.path().join("it's $HOME");
    fixture.core.set_config_dir(Some(custom.clone())).unwrap();
    fs::create_dir_all(&custom).unwrap();
    fs::write(custom.join("credentials.enc"), b"x").unwrap();

    let mut archive = fixture.export().unwrap();

    let shared = read_entry(&mut archive, "gws-shared/SKILL.md");
    let expected = format!(
        "export GOOGLE_WORKSPACE_CLI_CONFIG_DIR='{}/it'\\''s $HOME'\n",
        fixture.home.path().display()
    );
    assert!(shared.contains(&expected), "{shared}");
}

#[test]
fn the_default_config_folder_leaves_the_shared_skill_untouched() {
    let fixture = Fixture::new(tarball(|_| {}));

    let mut archive = fixture.export().unwrap();

    assert_eq!(
        read_entry(&mut archive, "gws-shared/SKILL.md"),
        "---\nname: gws-shared\n---\n\n# gws-shared\n"
    );
}

#[test]
fn a_failed_download_is_an_error() {
    let fixture = Fixture::serving(HttpResponse::new(404, "Not Found"));

    let error = fixture.export().err().unwrap();

    assert!(matches!(error, CoreError::Download(_)), "{error:?}");
}

#[test]
fn a_tarball_not_matching_the_pinned_checksum_is_rejected() {
    let fixture = Fixture::serving(HttpResponse::ok(tarball(|_| {})));

    let error = fixture.export().err().unwrap();

    assert!(
        matches!(error, CoreError::ChecksumMismatch { .. }),
        "{error:?}"
    );
}

/// Exporting from `archive` fails with a download error.
fn assert_rejected(archive: Vec<u8>) {
    let fixture = Fixture::new(archive);

    let error = fixture.export().err().unwrap();

    assert!(matches!(error, CoreError::Download(_)), "{error:?}");
    assert!(!fixture.home.path().join("evil").exists());
}

#[test]
fn archive_entries_climbing_out_with_dot_dot_are_rejected() {
    assert_rejected(tarball(|builder| {
        append_raw(
            builder,
            "cli-705fb0ecac6f4249679958f6325b809b63fdde17/skills/../../evil/SKILL.md",
            "x",
        );
    }));
}

#[test]
fn archive_entries_with_absolute_paths_are_rejected() {
    assert_rejected(tarball(|builder| {
        append_raw(builder, "/tmp/evil/SKILL.md", "x");
    }));
}

#[test]
fn symlinks_inside_skills_are_rejected() {
    assert_rejected(tarball(|builder| {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        builder
            .append_link(
                &mut header,
                "cli-705fb0ecac6f4249679958f6325b809b63fdde17/skills/gws-meet/evil",
                "/etc/passwd",
            )
            .unwrap();
    }));
}
