//! T04 — persisted settings (`<app_data>/settings.json`) and their resolution.

use std::path::PathBuf;

use crate::{Core, CoreError, KeyStorage, Paths, Settings};

/// File name of the settings inside `Paths::app_data`.
///
/// # Example
///
/// ```
/// assert_eq!(haulmer_googleworkspace_mcp_core::settings::SETTINGS_FILE, "settings.json");
/// ```
pub const SETTINGS_FILE: &str = "settings.json";

impl Core {
    /// Reads the persisted settings; defaults when the file does not exist.
    ///
    /// # Example
    ///
    /// ```
    /// # use haulmer_googleworkspace_mcp_core::{Core, Paths, Settings};
    /// # let app_data = tempfile::tempdir().unwrap();
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: app_data.path().into() });
    /// assert_eq!(core.settings().unwrap(), Settings::default());
    /// ```
    pub fn settings(&self) -> Result<Settings, CoreError> {
        let path = self.paths.app_data.join(SETTINGS_FILE);
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| CoreError::Parse(format!("{}: {error}", path.display()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Sets (or with `None` resets to the gws default) the gws config folder and persists it.
    ///
    /// # Example
    ///
    /// ```
    /// # use haulmer_googleworkspace_mcp_core::{Core, Paths};
    /// # let app_data = tempfile::tempdir().unwrap();
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: app_data.path().into() });
    /// core.set_config_dir(Some("/h/.config/haulmer-gws".into())).unwrap();
    /// assert!(!core.settings().unwrap().is_default_config_dir());
    /// ```
    pub fn set_config_dir(&self, dir: Option<PathBuf>) -> Result<(), CoreError> {
        let mut settings = self.settings()?;
        settings.config_dir = dir;
        self.write_settings(&settings)
    }

    /// Sets the key storage setting and persists it.
    ///
    /// # Example
    ///
    /// ```
    /// # use haulmer_googleworkspace_mcp_core::{Core, KeyStorage, Paths};
    /// # let app_data = tempfile::tempdir().unwrap();
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: app_data.path().into() });
    /// core.set_key_storage(KeyStorage::File).unwrap();
    /// assert_eq!(core.settings().unwrap().key_storage, KeyStorage::File);
    /// ```
    pub fn set_key_storage(&self, storage: KeyStorage) -> Result<(), CoreError> {
        let mut settings = self.settings()?;
        settings.key_storage = storage;
        self.write_settings(&settings)
    }

    /// Writes `<app_data>/settings.json`, creating `app_data` if missing.
    fn write_settings(&self, settings: &Settings) -> Result<(), CoreError> {
        std::fs::create_dir_all(&self.paths.app_data)?;
        let json = serde_json::to_vec_pretty(settings)
            .map_err(|error| CoreError::Parse(error.to_string()))?;
        std::fs::write(self.paths.app_data.join(SETTINGS_FILE), json)?;
        Ok(())
    }
}

impl Settings {
    /// The active gws config folder: the custom one, or `<home>/.config/gws`.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::{Paths, Settings};
    /// let paths = Paths { home: "/h".into(), app_data: "/a".into() };
    /// assert_eq!(Settings::default().active_config_dir(&paths), std::path::PathBuf::from("/h/.config/gws"));
    /// ```
    pub fn active_config_dir(&self, paths: &Paths) -> PathBuf {
        match &self.config_dir {
            Some(dir) => dir.clone(),
            None => paths.home.join(".config").join("gws"),
        }
    }

    /// Whether the config folder is the gws default (no custom folder set).
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::Settings;
    /// assert!(Settings::default().is_default_config_dir());
    /// ```
    pub fn is_default_config_dir(&self) -> bool {
        self.config_dir.is_none()
    }

    /// The backend passed to gws: `Auto` -> `Keyring` (default folder) or `File` (custom folder).
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::{KeyStorage, Settings};
    /// assert_eq!(Settings::default().resolved_key_storage(), KeyStorage::Keyring);
    /// let custom = Settings { config_dir: Some("/x".into()), key_storage: KeyStorage::Auto };
    /// assert_eq!(custom.resolved_key_storage(), KeyStorage::File);
    /// ```
    pub fn resolved_key_storage(&self) -> KeyStorage {
        match self.key_storage {
            KeyStorage::Auto if self.is_default_config_dir() => KeyStorage::Keyring,
            KeyStorage::Auto => KeyStorage::File,
            explicit => explicit,
        }
    }
}
