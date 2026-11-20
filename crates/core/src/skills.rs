//! T08 — `Core::export_skills_zip()` and the skills an agent installs: the pinned gws skills,
//! chosen by the granted services.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use crate::gws_binary::sha256_hex;
use crate::ports::download;
use crate::{Core, CoreError, Selection, Service};

/// Commit of gws tag `v0.22.5` (a lightweight tag). Pinned by commit because tags can move.
///
/// # Example
///
/// ```
/// assert_eq!(fraccional_googleworkspace_mcp_core::skills::SKILLS_COMMIT.len(), 40);
/// ```
pub const SKILLS_COMMIT: &str = "705fb0ecac6f4249679958f6325b809b63fdde17";

/// Pinned tree of the gws repository, as `npx skills add` takes it (gws v0.22.5).
///
/// # Example
///
/// ```
/// assert!(fraccional_googleworkspace_mcp_core::skills::SKILLS_REPO_URL.ends_with("/tree/v0.22.5"));
/// ```
pub const SKILLS_REPO_URL: &str = "https://github.com/googleworkspace/cli/tree/v0.22.5";

/// Tarball of the gws repository at [`SKILLS_COMMIT`]; skills live under `<top>/skills/`.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::skills::{SKILLS_COMMIT, SKILLS_TARBALL_URL};
/// assert!(SKILLS_TARBALL_URL.ends_with(&format!("{SKILLS_COMMIT}.tar.gz")));
/// ```
pub const SKILLS_TARBALL_URL: &str = "https://github.com/googleworkspace/cli/archive/705fb0ecac6f4249679958f6325b809b63fdde17.tar.gz";

/// SHA-256 of [`SKILLS_TARBALL_URL`], computed when it was pinned. A download that does
/// not match is rejected before anything is extracted or written.
///
/// # Example
///
/// ```
/// assert_eq!(fraccional_googleworkspace_mcp_core::skills::SKILLS_TARBALL_SHA256.len(), 64);
/// ```
pub const SKILLS_TARBALL_SHA256: &str =
    "38a27bae67755c101da1278154ca4b9bf9f7fdccd7a5a1db916c2ff880bfa9fd";

/// The skills of the "Services" table in gws v0.22.5 `docs/skills.md`: `gws-shared` plus one
/// core skill per Google API. Helpers, personas and recipes are left out. Update it together
/// with the pin.
const SERVICE_SKILLS: [&str; 19] = [
    "gws-shared",
    "gws-drive",
    "gws-sheets",
    "gws-gmail",
    "gws-calendar",
    "gws-admin-reports",
    "gws-docs",
    "gws-slides",
    "gws-tasks",
    "gws-people",
    "gws-chat",
    "gws-classroom",
    "gws-forms",
    "gws-keep",
    "gws-meet",
    "gws-events",
    "gws-modelarmor",
    "gws-workflow",
    "gws-script",
];

/// The skill every per-service skill reads first; always installed.
const SHARED_SKILL: &str = "gws-shared";

/// Files of one skill folder: path relative to the folder -> content.
type SkillFiles = BTreeMap<PathBuf, Vec<u8>>;

impl Core {
    /// Builds a ZIP containing `gws-shared` and the skills of the currently granted services.
    pub fn export_skills_zip(&self) -> Result<Vec<u8>, CoreError> {
        let granted = self.status()?.granted;
        let mut skills = self.download_skills()?;
        let shared_skill_md = skills
            .get_mut(SHARED_SKILL)
            .and_then(|shared| shared.get_mut(Path::new("SKILL.md")));
        if let (Some(section), Some(content)) = (self.environment_section()?, shared_skill_md) {
            content.extend_from_slice(section.as_bytes());
        }

        let wanted = skills
            .iter()
            .filter(|(name, _)| *name == SHARED_SKILL || granted_skill(&granted, name));
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (name, files) in wanted {
            for (relative, content) in files {
                let path = Path::new(name).join(relative);
                archive
                    .start_file(path.to_string_lossy().replace('\\', "/"), options)
                    .map_err(|error| {
                        CoreError::Download(format!("could not create skills ZIP: {error}"))
                    })?;
                archive.write_all(content)?;
            }
        }
        archive
            .finish()
            .map(|cursor| cursor.into_inner())
            .map_err(|error| CoreError::Download(format!("could not finish skills ZIP: {error}")))
    }

    /// The "Environment" section for `gws-shared/SKILL.md` when agents need extra setup:
    /// the gws binary the app uses is not on PATH, the config folder is custom, or the key
    /// storage is not keyring. `None` when gws defaults work as-is.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // custom folder + downloaded gws:
    /// // export PATH='<app_data>/bin':"$PATH"
    /// // export GOOGLE_WORKSPACE_CLI_CONFIG_DIR='<dir>'
    /// // export GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND=file
    /// let section = self.environment_section()?;
    /// ```
    fn environment_section(&self) -> Result<Option<String>, CoreError> {
        let settings = self.settings()?;
        let backend = settings.resolved_key_storage().backend();
        let mut lines = Vec::new();
        // `find_gws` returns the bare name "gws" only when it is found on PATH.
        if let Some(gws) = self.find_gws()?
            && gws != Path::new("gws")
            && let Some(dir) = gws.parent()
        {
            lines.push(format!(
                "export PATH={}:\"$PATH\"",
                shell_quote(&dir.to_string_lossy())
            ));
        }
        if !settings.is_default_config_dir() {
            lines.push(format!(
                "export GOOGLE_WORKSPACE_CLI_CONFIG_DIR={}",
                shell_quote(&settings.active_config_dir(&self.paths).to_string_lossy())
            ));
        }
        if !settings.is_default_config_dir() || backend != "keyring" {
            lines.push(format!(
                "export GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND={backend}"
            ));
        }
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "\n## Environment\n\n\
             fraccional-googleworkspace-mcp set up gws for this machine. \
             Always run gws with these variables set:\n\n\
             ```bash\n{}\n```\n",
            lines.join("\n")
        )))
    }

    /// Downloads the pinned tarball, checks its SHA-256 (a mismatch is
    /// [`CoreError::ChecksumMismatch`] before anything is extracted) and returns its skills
    /// by folder name.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let skills = self.download_skills()?;
    /// assert!(skills.contains_key("gws-shared"));
    /// ```
    fn download_skills(&self) -> Result<BTreeMap<String, SkillFiles>, CoreError> {
        let archive = download(self.http.as_ref(), SKILLS_TARBALL_URL)?;
        let actual = sha256_hex(&archive);
        if actual != self.skills_sha256 {
            return Err(CoreError::ChecksumMismatch {
                expected: self.skills_sha256.clone(),
                actual,
            });
        }
        let mut skills = extract_skills(&archive)?;
        add_builtin_workspace_skills(&mut skills);
        Ok(skills)
    }
}

/// The `npx skills add` command for [`SERVICE_SKILLS`], from the pinned [`SKILLS_REPO_URL`]. It is
/// the same for every person: it does not depend on the granted services.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::skills::skills_add_command;
/// assert!(skills_add_command().starts_with(
///     "npx -y skills add https://github.com/googleworkspace/cli/tree/v0.22.5 -g -y -s gws-shared gws-drive "
/// ));
/// ```
pub fn skills_add_command() -> String {
    format!(
        "npx -y skills add {SKILLS_REPO_URL} -g -y -s {}",
        SERVICE_SKILLS.join(" ")
    )
}

/// Adds stable generic skills for Workspace APIs that may not exist in the pinned
/// gws skill tarball. The CLI still discovers their live methods with `help`/`schema`.
fn add_builtin_workspace_skills(skills: &mut BTreeMap<String, SkillFiles>) {
    for service in Service::ALL {
        let name = format!("gws-{}", service.slug());
        skills.entry(name.clone()).or_insert_with(|| {
            let content = format!(
                "---\nname: {name}\ndescription: Google Workspace {} API.\n---\n\n\
                 # {name}\n\
                 Read `../gws-shared/SKILL.md` first.\n\n\
                 Discover available resources and methods with:\n\
                 ```bash\n\
                 gws {} --help\n\
                 gws schema {}.<resource>.<method>\n\
                 ```\n\n\
                 Always request JSON output and inspect the schema before write operations.\n",
                service.slug(),
                service.slug(),
                service.slug(),
            );
            BTreeMap::from([(PathBuf::from("SKILL.md"), content.into_bytes())])
        });
    }
}

/// The service whose skills a folder name belongs to (`gws-gmail*`, `gws-meet`, ...).
fn skill_service(name: &str) -> Option<Service> {
    Service::ALL.into_iter().find(|service| {
        let base = format!("gws-{}", service.slug());
        match service {
            Service::Meet => name == base,
            _ => name == base || name.starts_with(&format!("{base}-")),
        }
    })
}

/// Whether `name` is the skill of a service granted at any level.
fn granted_skill(granted: &Selection, name: &str) -> bool {
    skill_service(name).is_some_and(|service| granted.iter().any(|level| level.service == service))
}

/// Quotes `text` as one POSIX shell word: single quotes, embedded `'` written as `'\''`.
///
/// # Example
///
/// ```ignore
/// assert_eq!(shell_quote("/Users/ana/it's $HOME"), r"'/Users/ana/it'\''s $HOME'");
/// ```
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Reads `<top>/skills/<name>/<file>` entries of a `.tar.gz`, whatever the top folder is
/// called (`cli-<commit>/` for a commit tarball): the first path component is stripped.
/// Rejects `..`, absolute paths and links under `skills/`; ignores everything else.
///
/// # Example
///
/// ```ignore
/// let skills = extract_skills(&archive)?; // "gws-shared" -> {"SKILL.md": b"..."}
/// ```
fn extract_skills(archive: &[u8]) -> Result<BTreeMap<String, SkillFiles>, CoreError> {
    let unsafe_entry = |path: &Path| {
        CoreError::Download(format!("unsafe path in skills archive: {}", path.display()))
    };
    let mut skills: BTreeMap<String, SkillFiles> = BTreeMap::new();
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(unsafe_entry(&path));
        }
        let parts: Vec<&std::ffi::OsStr> = path.iter().collect();
        if parts.len() < 4 || parts[1] != "skills" {
            continue;
        }
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            return Err(unsafe_entry(&path));
        }
        if !kind.is_file() {
            continue;
        }
        let name = parts[2].to_string_lossy().into_owned();
        let relative: PathBuf = parts[3..].iter().collect();
        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;
        skills.entry(name).or_default().insert(relative, content);
    }
    Ok(skills)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{SERVICE_SKILLS, add_builtin_workspace_skills};

    /// `npx skills add -s` takes exact names, so a repeated or misspelled one is a bug.
    #[test]
    fn service_skills_are_unique_gws_names() {
        for (index, name) in SERVICE_SKILLS.iter().enumerate() {
            assert!(name.starts_with("gws-"), "{name} is not a gws skill");
            assert!(
                !SERVICE_SKILLS[index + 1..].contains(name),
                "{name} is listed twice"
            );
        }
    }

    #[test]
    fn every_catalogue_service_has_an_exportable_skill() {
        let mut skills = BTreeMap::new();
        add_builtin_workspace_skills(&mut skills);

        for service in super::Service::ALL {
            assert!(skills.contains_key(&format!("gws-{}", service.slug())));
        }
    }
}
