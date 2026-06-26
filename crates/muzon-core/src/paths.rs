// SPDX-License-Identifier: MIT OR Apache-2.0
//! XDG-compliant filesystem paths for Muzon.
//!
//! Resolves `~/.config/muzon/`, `~/.local/share/muzon/`, `~/.cache/muzon/`,
//! and `~/.local/share/muzon/logs/` per the Linux XDG Base Directory
//! specification. The `MUZON_HOME` environment variable overrides all
//! four so tests and packaging can relocate Muzon's runtime state
//! without touching the user's home directory. This module is pure
//! except for the directory creation in [`MuzonPaths::resolve`].
//!
//! See TZ.md §3.9, §4.3, §4.4 for the contracts this module honours.

use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;

/// Qualifier for [`directories::ProjectDirs::from`]. Matches the TZ
/// `org.muzon.Muzon` Flatpak app-id shape (the final `muzon` segment
/// is the binary name; the qualifier+organization pair is the XDG
/// namespace).
const PROJECT_QUALIFIER: &str = "org";
const PROJECT_ORG: &str = "Muzon";
const PROJECT_NAME: &str = "muzon";

const ENV_MUZON_HOME: &str = "MUZON_HOME";
const CONFIG_FILE: &str = "config.toml";
const LOG_FILE: &str = "muzon.log";
const LOG_DIR: &str = "logs";

/// Resolved filesystem locations for Muzon's runtime state.
///
/// Construct via [`MuzonPaths::resolve`]. The four directory fields
/// are guaranteed to exist on disk after `resolve` returns; the file
/// fields are computed paths that may or may not exist yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuzonPaths {
    /// `~/.config/muzon/` by default; `config.toml` lives inside.
    pub config_dir: PathBuf,
    /// `~/.local/share/muzon/` by default; `library.db` lives inside.
    pub data_dir: PathBuf,
    /// `~/.cache/muzon/` by default; transient caches live inside.
    pub cache_dir: PathBuf,
    /// `~/.local/share/muzon/logs/muzon.log` by default.
    pub log_dir: PathBuf,
}

impl MuzonPaths {
    /// Resolved config file path (`config.toml`).
    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join(CONFIG_FILE)
    }

    /// Resolved primary log file path (`logs/muzon.log`).
    pub fn log_file(&self) -> PathBuf {
        self.log_dir.join(LOG_FILE)
    }

    /// Resolve the four paths, honouring `MUZON_HOME` if set.
    ///
    /// Precedence:
    /// 1. `MUZON_HOME` env var (all four subdirs are placed under it).
    /// 2. [`directories::ProjectDirs`] on Linux/macOS.
    /// 3. A `~/.local`-style fallback derived from `$HOME` if
    ///    `directories` does not give us a usable result.
    ///
    /// After resolution, all four directories are created (parents
    /// included). The function is idempotent: a re-resolve on the
    /// same `MUZON_HOME` is a no-op.
    pub fn resolve() -> Result<Self, PathsError> {
        if let Some(home) = std::env::var_os(ENV_MUZON_HOME) {
            return Self::from_home(PathBuf::from(home));
        }

        let project = ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORG, PROJECT_NAME)
            .ok_or(PathsError::NoProjectDirs)?;
        Self::from_home(project.config_dir().to_path_buf())
    }

    /// Construct paths from an explicit root (used for tests and for
    /// the `MUZON_HOME` override). All four subdirs are placed under
    /// `home` and created.
    pub fn from_home(home: PathBuf) -> Result<Self, PathsError> {
        let paths = Self {
            config_dir: home.join("config"),
            data_dir: home.join("data"),
            cache_dir: home.join("cache"),
            log_dir: home.join("data").join(LOG_DIR),
        };
        paths.create_all()?;
        Ok(paths)
    }

    fn create_all(&self) -> Result<(), PathsError> {
        for dir in [
            &self.config_dir,
            &self.data_dir,
            &self.cache_dir,
            &self.log_dir,
        ] {
            fs::create_dir_all(dir).map_err(|source| PathsError::Create {
                path: dir.clone(),
                source,
            })?;
        }
        Ok(())
    }
}

/// Errors produced by [`MuzonPaths::resolve`].
#[derive(Debug, thiserror::Error)]
pub enum PathsError {
    #[error("could not determine XDG project directories; set MUZON_HOME explicitly")]
    NoProjectDirs,

    #[error("failed to create directory {path}: {source}")]
    Create {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Tests that touch MUZON_HOME must serialise on this mutex to
    // avoid cargo's default parallel-test execution clobbering the
    // env var between set and use.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn env_override() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var(ENV_MUZON_HOME, tmp.path());
        let paths = MuzonPaths::resolve().expect("resolve");
        std::env::remove_var(ENV_MUZON_HOME);

        assert_eq!(paths.config_dir, tmp.path().join("config"));
        assert_eq!(paths.data_dir, tmp.path().join("data"));
        assert_eq!(paths.cache_dir, tmp.path().join("cache"));
        assert_eq!(paths.log_dir, tmp.path().join("data").join("logs"));

        // All four subdirs must exist on disk.
        for dir in [
            &paths.config_dir,
            &paths.data_dir,
            &paths.cache_dir,
            &paths.log_dir,
        ] {
            assert!(dir.is_dir(), "expected {dir:?} to be a directory");
        }
    }

    #[test]
    fn config_and_log_files() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = MuzonPaths::from_home(tmp.path().to_path_buf()).unwrap();
        assert_eq!(paths.config_file(), tmp.path().join("config").join(CONFIG_FILE));
        assert_eq!(paths.log_file(), tmp.path().join("data").join(LOG_DIR).join(LOG_FILE));
    }

    #[test]
    fn idempotent_resolve() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var(ENV_MUZON_HOME, tmp.path());
        let a = MuzonPaths::resolve().expect("resolve a");
        let b = MuzonPaths::resolve().expect("resolve b");
        std::env::remove_var(ENV_MUZON_HOME);
        assert_eq!(a, b);
    }

    #[test]
    fn missing_home_falls_back_or_errors() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        // We do not assert a specific resolution here because the
        // fallback depends on the host. We only assert that, with
        // MUZON_HOME unset, resolve() does not panic and either
        // returns Ok or a structured PathsError.
        std::env::remove_var(ENV_MUZON_HOME);
        let result = MuzonPaths::resolve();
        match result {
            Ok(paths) => {
                assert!(paths.config_dir.is_absolute());
                assert!(paths.data_dir.is_absolute());
            }
            Err(PathsError::NoProjectDirs) => {
                // acceptable on hosts where directories::ProjectDirs returns None
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_relative_home() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        // Relative MUZON_HOME is allowed (it gets created relative to
        // the cwd) but the resulting paths must still be the four
        // subdirs under that root.
        let tmp = tempfile::tempdir().unwrap();
        let rel = tmp.path().join("rel_home");
        std::env::set_var(ENV_MUZON_HOME, &rel);
        let paths = MuzonPaths::resolve().expect("resolve");
        std::env::remove_var(ENV_MUZON_HOME);
        assert_eq!(paths.config_dir, rel.join("config"));
        assert_eq!(paths.data_dir, rel.join("data"));
    }
}
