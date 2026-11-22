//! Shared data types of the core contract. Every ticket uses these; none renames them.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A Google Workspace service the person can grant to their agents.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::Service;
/// assert_eq!(Service::ALL.len(), 15);
/// assert_eq!(serde_json::to_string(&Service::Gmail).unwrap(), "\"gmail\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    Gmail,
    Calendar,
    Chat,
    Meet,
    Drive,
    Docs,
    Sheets,
    Slides,
    Forms,
    Tasks,
    People,
    #[serde(rename = "admin-reports")]
    AdminReports,
    #[serde(rename = "admin-directory")]
    AdminDirectory,
    #[serde(rename = "admin-groups")]
    AdminGroups,
    #[serde(rename = "cloud-identity")]
    CloudIdentity,
}

impl Service {
    /// Every service, in the order the UI shows them.
    pub const ALL: [Service; 15] = [
        Service::Gmail,
        Service::Calendar,
        Service::Chat,
        Service::Meet,
        Service::Drive,
        Service::Docs,
        Service::Sheets,
        Service::Slides,
        Service::Forms,
        Service::Tasks,
        Service::People,
        Service::AdminReports,
        Service::AdminDirectory,
        Service::AdminGroups,
        Service::CloudIdentity,
    ];

    /// The service's lowercase name, as used in gws skill folders (`gws-<slug>*`) and JSON.
    ///
    /// # Example
    ///
    /// ```
    /// use fraccional_googleworkspace_mcp_core::Service;
    /// assert_eq!(Service::Calendar.slug(), "calendar");
    /// ```
    pub fn slug(self) -> &'static str {
        match self {
            Service::Gmail => "gmail",
            Service::Calendar => "calendar",
            Service::Chat => "chat",
            Service::Meet => "meet",
            Service::Drive => "drive",
            Service::Docs => "docs",
            Service::Sheets => "sheets",
            Service::Slides => "slides",
            Service::Forms => "forms",
            Service::Tasks => "tasks",
            Service::People => "people",
            Service::AdminReports => "admin-reports",
            Service::AdminDirectory => "admin-directory",
            Service::AdminGroups => "admin-groups",
            Service::CloudIdentity => "cloud-identity",
        }
    }
}

/// Access level for one service. `Write` always implies `Read` (see `scopes::normalize`).
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::Level;
/// assert_eq!(serde_json::to_string(&Level::Write).unwrap(), "\"write\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Read,
    Write,
}

/// One checkbox of the UI: a service at a level.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{Level, Service, ServiceLevel};
/// let level = ServiceLevel { service: Service::Gmail, level: Level::Read };
/// assert_eq!(
///     serde_json::to_string(&level).unwrap(),
///     r#"{"service":"gmail","level":"read"}"#
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ServiceLevel {
    pub service: Service,
    pub level: Level,
}

/// A set of granted (or requested) service levels. Serialises as a JSON array of
/// [`ServiceLevel`]. It does not normalise itself; use `scopes::normalize`.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{Level, Selection, Service};
/// let mut selection = Selection::new();
/// selection.insert(Service::Gmail, Level::Read);
/// assert!(selection.contains(Service::Gmail, Level::Read));
/// assert!(!selection.contains(Service::Gmail, Level::Write));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Selection(BTreeSet<ServiceLevel>);

impl Selection {
    /// Creates an empty selection.
    ///
    /// # Example
    ///
    /// ```
    /// assert!(fraccional_googleworkspace_mcp_core::Selection::new().is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a service level; returns `true` if it was not there yet.
    ///
    /// # Example
    ///
    /// ```
    /// use fraccional_googleworkspace_mcp_core::{Level, Selection, Service};
    /// let mut selection = Selection::new();
    /// assert!(selection.insert(Service::Drive, Level::Write));
    /// assert!(!selection.insert(Service::Drive, Level::Write));
    /// ```
    pub fn insert(&mut self, service: Service, level: Level) -> bool {
        self.0.insert(ServiceLevel { service, level })
    }

    /// Removes a service level; returns `true` if it was there.
    ///
    /// # Example
    ///
    /// ```
    /// use fraccional_googleworkspace_mcp_core::{Level, Selection, Service};
    /// let mut selection = Selection::from_levels([(Service::Chat, Level::Read)]);
    /// assert!(selection.remove(Service::Chat, Level::Read));
    /// assert!(selection.is_empty());
    /// ```
    pub fn remove(&mut self, service: Service, level: Level) -> bool {
        self.0.remove(&ServiceLevel { service, level })
    }

    /// Whether the selection holds this service level.
    ///
    /// # Example
    ///
    /// ```
    /// use fraccional_googleworkspace_mcp_core::{Level, Selection, Service};
    /// let selection = Selection::from_levels([(Service::Meet, Level::Read)]);
    /// assert!(selection.contains(Service::Meet, Level::Read));
    /// ```
    pub fn contains(&self, service: Service, level: Level) -> bool {
        self.0.contains(&ServiceLevel { service, level })
    }

    /// Whether nothing is selected.
    ///
    /// # Example
    ///
    /// ```
    /// assert!(fraccional_googleworkspace_mcp_core::Selection::new().is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterates the service levels in a stable order (by service, then level).
    ///
    /// # Example
    ///
    /// ```
    /// use fraccional_googleworkspace_mcp_core::{Level, Selection, Service};
    /// let selection = Selection::from_levels([(Service::Drive, Level::Read), (Service::Gmail, Level::Read)]);
    /// let services: Vec<Service> = selection.iter().map(|level| level.service).collect();
    /// assert_eq!(services, [Service::Gmail, Service::Drive]);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = ServiceLevel> + '_ {
        self.0.iter().copied()
    }

    /// Builds a selection from `(service, level)` pairs.
    ///
    /// # Example
    ///
    /// ```
    /// use fraccional_googleworkspace_mcp_core::{Level, Selection, Service};
    /// let selection = Selection::from_levels([(Service::Gmail, Level::Read), (Service::Gmail, Level::Write)]);
    /// assert_eq!(selection.iter().count(), 2);
    /// ```
    pub fn from_levels(levels: impl IntoIterator<Item = (Service, Level)>) -> Self {
        Self(
            levels
                .into_iter()
                .map(|(service, level)| ServiceLevel { service, level })
                .collect(),
        )
    }
}

/// Where gws keeps its encryption key (`GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND`).
/// `Auto` resolves to `Keyring` for the default config folder and `File` for a custom one.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::KeyStorage;
/// assert_eq!(KeyStorage::default(), KeyStorage::Auto);
/// assert_eq!(serde_json::to_string(&KeyStorage::File).unwrap(), "\"file\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyStorage {
    #[default]
    Auto,
    Keyring,
    File,
}

impl KeyStorage {
    /// The value of `GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND` for a resolved setting:
    /// `"file"` for `File`, `"keyring"` otherwise. Resolve `Auto` first with
    /// `Settings::resolved_key_storage`.
    ///
    /// # Example
    ///
    /// ```
    /// use fraccional_googleworkspace_mcp_core::KeyStorage;
    /// assert_eq!(KeyStorage::File.backend(), "file");
    /// assert_eq!(KeyStorage::Keyring.backend(), "keyring");
    /// ```
    pub fn backend(self) -> &'static str {
        if self == KeyStorage::File {
            "file"
        } else {
            "keyring"
        }
    }
}

/// Folders injected into the core. Tests pass temp dirs; the app passes the real ones.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::Paths;
/// let paths = Paths { home: "/home/ana".into(), app_data: "/home/ana/.local/share/fraccional-googleworkspace-mcp".into() };
/// assert!(paths.app_data.starts_with(&paths.home));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paths {
    /// The person's home folder (gws default config is `<home>/.config/gws`; agent skills live here).
    pub home: PathBuf,
    /// The app's own data folder (settings, downloaded gws, run folder).
    pub app_data: PathBuf,
}

/// Persisted app settings (`<app_data>/settings.json`). Behaviour lives in the `settings` module.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{KeyStorage, Settings};
/// let settings = Settings::default();
/// assert_eq!(settings.config_dir, None);
/// assert_eq!(settings.key_storage, KeyStorage::Auto);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Custom gws config folder; `None` means the gws default `<home>/.config/gws`.
    pub config_dir: Option<PathBuf>,
    /// Key storage setting as chosen by the person (may be `Auto`).
    pub key_storage: KeyStorage,
}

/// What `Core::status()` reports to the UI.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{BusinessSelection, KeyStorage, Selection, Status};
/// let status = Status {
///     gws_available: true,
///     account: None,
///     granted: Selection::new(),
///     business_granted: BusinessSelection::new(),
///     token_valid: false,
///     config_dir: "/home/ana/.config/gws".into(),
///     config_dir_is_default: true,
///     key_storage: KeyStorage::Auto,
///     resolved_key_storage: KeyStorage::Keyring,
///     foreign_credentials: false,
/// };
/// assert!(status.account.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// The pinned gws binary is on the machine (if not, Save downloads it).
    pub gws_available: bool,
    /// Connected Google account email; `None` when not connected.
    pub account: Option<String>,
    /// Levels Google actually granted (from `gws auth status`).
    pub granted: Selection,
    /// Business connector levels Google actually granted.
    pub business_granted: BusinessSelection,
    /// gws could refresh the token.
    pub token_valid: bool,
    /// Active gws config folder.
    pub config_dir: PathBuf,
    /// `config_dir` is the gws default folder.
    pub config_dir_is_default: bool,
    /// Key storage setting (may be `Auto`).
    pub key_storage: KeyStorage,
    /// Backend actually passed to gws (`Keyring` or `File`, never `Auto`).
    pub resolved_key_storage: KeyStorage,
    /// Credentials exist in the active folder and were not created by fraccional-googleworkspace-mcp.
    pub foreign_credentials: bool,
}

/// How many scopes a save of a selection requests, and the most that one Google sign-in may
/// request (`scopes::MAX_LOGIN_SCOPES`). The window shows it next to Save.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::ScopeCount;
/// let count = ScopeCount { requested: 52, max: 47 };
/// assert_eq!(serde_json::to_string(&count).unwrap(), r#"{"requested":52,"max":47}"#);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeCount {
    pub requested: usize,
    pub max: usize,
}

/// Result of `Core::save`.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::SaveOutcome;
/// assert_eq!(
///     serde_json::to_string(&SaveOutcome::NeedsReplaceConfirmation).unwrap(),
///     r#"{"kind":"needs_replace_confirmation"}"#
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "status", rename_all = "snake_case")]
pub enum SaveOutcome {
    /// Login finished; the fresh status.
    Connected(Status),
    /// Foreign gws credentials exist; ask "Replace?" and call `save` again with `confirm_replace = true`.
    NeedsReplaceConfirmation,
}

/// Result of `Core::revoke_all`.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::RevokeOutcome;
/// assert_eq!(
///     serde_json::to_string(&RevokeOutcome::NeedsForeignConfirmation).unwrap(),
///     r#"{"kind":"needs_foreign_confirmation"}"#
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RevokeOutcome {
    /// Access was revoked and local credentials cleared.
    Revoked,
    /// The credentials were created by another app; nothing ran. Ask "Revoke them anyway?"
    /// and call `revoke_all` again with `confirm_foreign = true`.
    NeedsForeignConfirmation,
}

/// Plain-language labels for one service row of the UI (built by `scopes::catalogue`).
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{Service, ServiceCategory, ServiceInfo};
/// let info = ServiceInfo {
///     service: Service::Gmail,
///     name: "Gmail",
///     read: "Read: read email",
///     write: "Write: send and modify email",
///     category: ServiceCategory::Workspace,
///     enabled: true,
///     availability_reason: None,
///     write_enabled: true,
/// };
/// assert_eq!(info.name, "Gmail");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceInfo {
    pub service: Service,
    /// Display name, e.g. "Gmail".
    pub name: &'static str,
    /// Description of the Read level.
    pub read: &'static str,
    /// Description of the Write level.
    pub write: &'static str,
    /// UI category for this capability.
    pub category: ServiceCategory,
    /// Whether this capability can be selected with the current account.
    pub enabled: bool,
    /// Whether the Write level is supported by this capability.
    pub write_enabled: bool,
    /// Explanation shown when the capability is unavailable.
    pub availability_reason: Option<&'static str>,
}

/// One requested access level for a non-Workspace Google connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BusinessAccess {
    pub connector: BusinessConnector,
    pub level: Level,
}

/// Granted or requested levels for business connectors.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BusinessSelection(BTreeSet<BusinessAccess>);

impl BusinessSelection {
    /// Creates an empty business selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one connector level.
    pub fn insert(&mut self, connector: BusinessConnector, level: Level) -> bool {
        self.0.insert(BusinessAccess { connector, level })
    }

    /// Builds a selection from connector and level pairs.
    pub fn from_levels(levels: impl IntoIterator<Item = (BusinessConnector, Level)>) -> Self {
        Self(
            levels
                .into_iter()
                .map(|(connector, level)| BusinessAccess { connector, level })
                .collect(),
        )
    }

    /// Whether a connector level is selected.
    pub fn contains(&self, connector: BusinessConnector, level: Level) -> bool {
        self.0.contains(&BusinessAccess { connector, level })
    }

    /// Whether no business connector is selected.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterates connector levels in stable order.
    pub fn iter(&self) -> impl Iterator<Item = BusinessAccess> + '_ {
        self.0.iter().copied()
    }
}

/// UI category for a Google Workspace capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceCategory {
    Workspace,
    Admin,
}

/// A Google API outside the gws Workspace service catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BusinessConnector {
    Youtube,
    GoogleAds,
    Analytics,
    SearchConsole,
    BusinessProfile,
    MerchantCenter,
}

/// Metadata for a business connector. Connectors use the shared OAuth account but
/// require product-specific API calls and configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BusinessConnectorInfo {
    pub connector: BusinessConnector,
    pub name: &'static str,
    pub read: &'static str,
    pub write: &'static str,
    pub enabled: bool,
    pub availability_reason: Option<&'static str>,
}

/// Every error the core returns. Serialises as its display message for the UI.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::CoreError;
/// let error = CoreError::Gws { exit_code: 2, message: "No credentials".into() };
/// assert_eq!(error.to_string(), "gws failed (exit 2): No credentials");
/// assert_eq!(serde_json::to_string(&CoreError::Cancelled).unwrap(), "\"Google sign-in was cancelled\"");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    /// A stub that a later ticket implements.
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
    /// A local file operation failed.
    #[error("file error: {0}")]
    Io(String),
    /// A program could not be started or awaited (e.g. not found).
    #[error("could not run a program: {0}")]
    Process(String),
    /// Network failure (no HTTP response at all).
    #[error("network error: {0}")]
    Http(String),
    /// The gws download failed (bad HTTP status, bad archive, missing binary).
    #[error("gws download failed: {0}")]
    Download(String),
    /// A download (gws or the skills tarball) did not match its expected SHA-256.
    #[error("download checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    /// gws exited non-zero; `message` is `error.message` from its JSON when present.
    #[error("gws failed (exit {exit_code}): {message}")]
    Gws { exit_code: i32, message: String },
    /// Unexpected output (e.g. JSON that does not parse).
    #[error("unexpected output: {0}")]
    Parse(String),
    /// The person cancelled the Google sign-in.
    #[error("Google sign-in was cancelled")]
    Cancelled,
    /// Another save or revocation is already changing access on this Core.
    #[error("another access change is already in progress")]
    AccessInProgress,
    /// Local credentials were cleared but Google did not confirm the revocation.
    #[error("local credentials were cleared but Google revocation failed: {0}")]
    RevokeFailed(String),
    /// The app runs from a path that will not last (a translocated macOS app), so the Install
    /// prompt would point at nothing later. The message is Spanish, for the person.
    #[error("{0}")]
    UnstableExe(String),
    /// No app opened the MCP extension file. `src-tauri` returns it when opening the `.mcpb`
    /// fails. The message is Spanish, for the person.
    #[error("No encontré una app que abra extensiones MCP. Usa el prompt de abajo.")]
    NoMcpExtensionApp,
    /// `save` revoked the old access (a level was removed) but the new sign-in was
    /// cancelled or failed: nothing is connected now.
    #[error(
        "Access was removed, but the new Google sign-in did not finish. Save again to reconnect."
    )]
    AccessRemoved,
    /// The selection needs more scopes than Google accepts in one sign-in
    /// ([`crate::scopes::MAX_LOGIN_SCOPES`]). `save` returns it before it changes anything.
    /// The message is Spanish, for the person.
    #[error(
        "Google acepta hasta {max} permisos por inicio de sesión y esta selección pide {requested}. Quita algunos servicios o cambia alguno a solo lectura."
    )]
    TooManyScopes { requested: usize, max: usize },
}

impl From<std::io::Error> for CoreError {
    /// Maps any I/O error to [`CoreError::Io`].
    ///
    /// # Example
    ///
    /// ```
    /// use fraccional_googleworkspace_mcp_core::CoreError;
    /// let error: CoreError = std::io::Error::other("disk full").into();
    /// assert_eq!(error, CoreError::Io("disk full".into()));
    /// ```
    fn from(error: std::io::Error) -> Self {
        CoreError::Io(error.to_string())
    }
}

impl From<serde_json::Error> for CoreError {
    /// Maps a JSON error to [`CoreError::Parse`].
    ///
    /// # Example
    ///
    /// ```
    /// use fraccional_googleworkspace_mcp_core::CoreError;
    /// let error: CoreError = serde_json::from_str::<u8>("x").unwrap_err().into();
    /// assert!(matches!(error, CoreError::Parse(_)));
    /// ```
    fn from(error: serde_json::Error) -> Self {
        CoreError::Parse(error.to_string())
    }
}

impl Serialize for CoreError {
    /// Serialises the error as its display message (what the UI shows).
    ///
    /// # Example
    ///
    /// ```
    /// use fraccional_googleworkspace_mcp_core::CoreError;
    /// assert_eq!(serde_json::to_string(&CoreError::Io("x".into())).unwrap(), "\"file error: x\"");
    /// ```
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
