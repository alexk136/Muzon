// SPDX-License-Identifier: MIT OR Apache-2.0
//! TOML configuration loader for Muzon.
//!
//! The config struct is layered: defaults baked into
//! [`MuzonConfig::default`], then the TOML file at `MuzonPaths::config_file`
//! (or the path passed to [`MuzonConfig::load`]), then the
//! `MUZON_*` environment variables. The `MUZON_*` overrides win on
//! conflict. Sections mirror TZ.md §3.9, §4.3, §4.4.
//!
//! The `network` section is the contract anchor for decision 0001-N4
//! (no telemetry, ever): the default has every flag `false` and
//! the validator below fails the load if any `network.*` field is
//! set to `true` without a matching `network.explicit_opt_in` field.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Top-level Muzon configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MuzonConfig {
    pub audio: AudioConfig,
    pub library: LibraryConfig,
    pub ui: UiConfig,
    pub logging: LoggingConfig,
    /// See decision 0001-N4. Every flag must be `false` unless the
    /// user has flipped the corresponding opt-in switch.
    pub network: NetworkConfig,
}

/// Audio output configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AudioConfig {
    /// ALSA / PulseAudio / PipeWire sink name. Empty means "use the
    /// GStreamer default".
    pub output_device: String,
    /// Strip `audioconvert` and `audioresample` from the pipeline so
    /// the source format reaches the sink unchanged. Default `false`
    /// (resample to sink rate); the user opts in for bit-perfect.
    pub bit_perfect: bool,
    /// Enable gapless playback via the `about-to-finish` signal.
    /// v0.1.0 ignores this; v0.2.0+ honours it.
    pub gapless: bool,
    /// ReplayGain mode. `None` means no normalisation; `Track` and
    /// `Album` match the r128 / RG2 specs.
    pub replaygain: ReplayGainMode,
    /// Crossfade duration in seconds. `0` disables crossfade.
    pub crossfade_seconds: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_device: String::new(),
            bit_perfect: false,
            gapless: true,
            replaygain: ReplayGainMode::None,
            crossfade_seconds: 0,
        }
    }
}

/// ReplayGain mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplayGainMode {
    #[default]
    None,
    Track,
    Album,
}

/// Library scanner / watcher configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LibraryConfig {
    /// Root folders the scanner walks. Paths are absolute.
    pub scan_roots: Vec<String>,
    /// Watch for filesystem changes after the initial scan.
    pub watch: bool,
    /// Minimum file size in bytes; smaller files are skipped.
    pub min_file_size_bytes: u64,
    /// Honour `.nomedia` and `.muzonignore` markers in subdirs.
    pub honor_ignore_markers: bool,
    /// Follow symbolic links. Duplicates produce a `tracing::warn!`
    /// line; the resolution UI is v0.4.0.
    pub follow_symlinks: bool,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            scan_roots: Vec::new(),
            watch: true,
            min_file_size_bytes: 32 * 1024,
            honor_ignore_markers: true,
            follow_symlinks: true,
        }
    }
}

/// UI configuration (theme, accent, font). v0.1.0 stores these for
/// the v0.2.0 UI to read; they have no effect on the v0.1.0 CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    pub theme: Theme,
    pub accent_color: String,
    pub font: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Auto,
            accent_color: String::from("#3b82f6"),
            font: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

/// Logging configuration (file rotation, level).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LoggingConfig {
    /// Default level if `RUST_LOG` is unset. `info` per TZ §4.3.
    pub level: String,
    /// Per-file size cap in MB before rotation. Default 10 per
    /// TZ §4.3.
    pub max_size_mb: u32,
    /// Number of historical log files to keep. Default 3 per
    /// TZ §4.3.
    pub max_files: u32,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: String::from("info"),
            max_size_mb: 10,
            max_files: 3,
        }
    }
}

/// Network / telemetry configuration. Locks decision 0001-N4:
/// every flag defaults to `false`; setting any of them to `true`
/// is a deliberate user opt-in and must be accompanied by
/// `explicit_opt_in = true` or the loader rejects the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkConfig {
    /// Master switch. Must be `true` for any other field to be
    /// honoured. Default `false`. See decision 0001-N4.
    pub explicit_opt_in: bool,
    /// Optional anonymous crash reports. v0.1.0+ rejects `true` here
    /// unless `explicit_opt_in` is also `true`; a v0.2.0+ RFC can
    /// promote this to a real opt-in flow.
    pub anonymous_crash_reports: bool,
    /// Optional anonymous usage analytics. Same rules as
    /// `anonymous_crash_reports`.
    pub anonymous_usage_analytics: bool,
    /// LLM provider calls (Ollama / OpenAI / Anthropic / OpenRouter).
    /// v0.6.0 work; v0.1.0 has no LLM call sites.
    pub llm_providers: bool,
    /// AcoustID lookup. v0.4.0 work; v0.1.0 has no AcoustID call sites.
    pub acoustid: bool,
    /// MusicBrainz enrichment. v0.4.0 work; v0.1.0 has no
    /// MusicBrainz call sites.
    pub musicbrainz: bool,
    /// Last.fm scrobbling. v0.6.0 work.
    pub lastfm: bool,
    /// ListenBrainz scrobbling. v0.6.0 work.
    pub listenbrainz: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            explicit_opt_in: false,
            anonymous_crash_reports: false,
            anonymous_usage_analytics: false,
            llm_providers: false,
            acoustid: false,
            musicbrainz: false,
            lastfm: false,
            listenbrainz: false,
        }
    }
}

/// Errors produced by [`MuzonConfig::load`].
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: std::path::PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error(
        "network.{0} is true but network.explicit_opt_in is false; \
         per decision 0001-N4 ('no telemetry, ever'), every network \
         call requires explicit_opt_in = true. Set both, or leave the \
         field at its default of false."
    )]
    NetworkOptInRequired(&'static str),
}

impl MuzonConfig {
    /// Load the config from `path`. If the file does not exist, the
    /// defaults are returned. If it exists but is malformed, an
    /// error is returned.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let mut cfg: MuzonConfig = if path.exists() {
            let raw = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
                path: path.to_path_buf(),
                source,
            })?;
            toml::from_str(&raw).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?
        } else {
            MuzonConfig::default()
        };

        cfg.apply_env_overrides();
        cfg.validate_network_opt_in()?;
        Ok(cfg)
    }

    /// Override fields from `MUZON_*` environment variables. The
    /// naming convention is `MUZON_<SECTION>__<FIELD>` (double
    /// underscore separates the section from the field). Only the
    /// fields explicitly handled below are overridable; everything
    /// else stays at the TOML or default value.
    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("MUZON_AUDIO__BIT_PERFECT") {
            if let Ok(b) = v.parse::<bool>() {
                self.audio.bit_perfect = b;
            }
        }
        if let Ok(v) = std::env::var("MUZON_AUDIO__GAPLESS") {
            if let Ok(b) = v.parse::<bool>() {
                self.audio.gapless = b;
            }
        }
        if let Ok(v) = std::env::var("MUZON_AUDIO__REPLAYGAIN") {
            self.audio.replaygain = match v.to_lowercase().as_str() {
                "track" => ReplayGainMode::Track,
                "album" => ReplayGainMode::Album,
                _ => ReplayGainMode::None,
            };
        }
        if let Ok(v) = std::env::var("MUZON_AUDIO__CROSSFADE_SECONDS") {
            if let Ok(n) = v.parse::<u32>() {
                self.audio.crossfade_seconds = n;
            }
        }
        if let Ok(v) = std::env::var("MUZON_LOGGING__LEVEL") {
            self.logging.level = v;
        }
        if let Ok(v) = std::env::var("MUZON_LOGGING__MAX_SIZE_MB") {
            if let Ok(n) = v.parse::<u32>() {
                self.logging.max_size_mb = n;
            }
        }
        if let Ok(v) = std::env::var("MUZON_LOGGING__MAX_FILES") {
            if let Ok(n) = v.parse::<u32>() {
                self.logging.max_files = n;
            }
        }
        if let Ok(v) = std::env::var("MUZON_NETWORK__EXPLICIT_OPT_IN") {
            if let Ok(b) = v.parse::<bool>() {
                self.network.explicit_opt_in = b;
            }
        }
    }

    /// Enforce decision 0001-N4: every `network.*` field is `false`
    /// unless `network.explicit_opt_in` is `true`.
    fn validate_network_opt_in(&self) -> Result<(), ConfigError> {
        if self.network.explicit_opt_in {
            return Ok(());
        }
        if self.network.anonymous_crash_reports {
            return Err(ConfigError::NetworkOptInRequired("anonymous_crash_reports"));
        }
        if self.network.anonymous_usage_analytics {
            return Err(ConfigError::NetworkOptInRequired("anonymous_usage_analytics"));
        }
        if self.network.llm_providers {
            return Err(ConfigError::NetworkOptInRequired("llm_providers"));
        }
        if self.network.acoustid {
            return Err(ConfigError::NetworkOptInRequired("acoustid"));
        }
        if self.network.musicbrainz {
            return Err(ConfigError::NetworkOptInRequired("musicbrainz"));
        }
        if self.network.lastfm {
            return Err(ConfigError::NetworkOptInRequired("lastfm"));
        }
        if self.network.listenbrainz {
            return Err(ConfigError::NetworkOptInRequired("listenbrainz"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Tests that read or set MUZON_* env vars must serialise on this
    // mutex to avoid cargo's default parallel-test execution
    // clobbering the env between set and use.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_is_no_network() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        // Defensive: clear the env vars this test cares about, in
        // case a prior parallel test set them.
        for var in [
            "MUZON_AUDIO__BIT_PERFECT",
            "MUZON_AUDIO__GAPLESS",
            "MUZON_AUDIO__REPLAYGAIN",
            "MUZON_AUDIO__CROSSFADE_SECONDS",
            "MUZON_LOGGING__LEVEL",
            "MUZON_LOGGING__MAX_SIZE_MB",
            "MUZON_LOGGING__MAX_FILES",
            "MUZON_NETWORK__EXPLICIT_OPT_IN",
        ] {
            std::env::remove_var(var);
        }
        let cfg = MuzonConfig::default();
        assert!(!cfg.network.explicit_opt_in);
        assert!(!cfg.network.anonymous_crash_reports);
        assert!(!cfg.network.anonymous_usage_analytics);
        assert!(!cfg.network.llm_providers);
        assert!(!cfg.network.acoustid);
        assert!(!cfg.network.musicbrainz);
        assert!(!cfg.network.lastfm);
        assert!(!cfg.network.listenbrainz);
    }

    #[test]
    fn toml_round_trip() {
        let cfg = MuzonConfig::default();
        let raw = toml::to_string(&cfg).unwrap();
        let parsed: MuzonConfig = toml::from_str(&raw).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn toml_load_with_audio_section() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[audio]
output_device = "alsa:default"
bit_perfect = true
gapless = false
replaygain = "track"
crossfade_seconds = 3

[logging]
level = "debug"
max_size_mb = 20
max_files = 5
"#,
        )
        .unwrap();
        let cfg = MuzonConfig::load(&path).unwrap();
        assert_eq!(cfg.audio.output_device, "alsa:default");
        assert!(cfg.audio.bit_perfect);
        assert!(!cfg.audio.gapless);
        assert_eq!(cfg.audio.replaygain, ReplayGainMode::Track);
        assert_eq!(cfg.audio.crossfade_seconds, 3);
        assert_eq!(cfg.logging.level, "debug");
        assert_eq!(cfg.logging.max_size_mb, 20);
        assert_eq!(cfg.logging.max_files, 5);
    }

    #[test]
    fn missing_file_returns_default() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        for var in [
            "MUZON_AUDIO__BIT_PERFECT",
            "MUZON_AUDIO__GAPLESS",
            "MUZON_AUDIO__REPLAYGAIN",
            "MUZON_AUDIO__CROSSFADE_SECONDS",
            "MUZON_LOGGING__LEVEL",
            "MUZON_LOGGING__MAX_SIZE_MB",
            "MUZON_LOGGING__MAX_FILES",
            "MUZON_NETWORK__EXPLICIT_OPT_IN",
        ] {
            std::env::remove_var(var);
        }
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("does_not_exist.toml");
        let cfg = MuzonConfig::load(&path).unwrap();
        assert_eq!(cfg, MuzonConfig::default());
    }

    #[test]
    fn malformed_toml_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("bad.toml");
        std::fs::write(&path, "this is not valid toml [[[").unwrap();
        let err = MuzonConfig::load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn network_opt_in_required() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[network]
acoustid = true
"#,
        )
        .unwrap();
        let err = MuzonConfig::load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::NetworkOptInRequired("acoustid")));
    }

    #[test]
    fn network_opt_in_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[network]
explicit_opt_in = true
acoustid = true
"#,
        )
        .unwrap();
        let cfg = MuzonConfig::load(&path).unwrap();
        assert!(cfg.network.explicit_opt_in);
        assert!(cfg.network.acoustid);
    }

    #[test]
    fn env_overrides_toml() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[audio]
bit_perfect = false
"#,
        )
        .unwrap();
        std::env::set_var("MUZON_AUDIO__BIT_PERFECT", "true");
        let cfg = MuzonConfig::load(&path).unwrap();
        std::env::remove_var("MUZON_AUDIO__BIT_PERFECT");
        assert!(cfg.audio.bit_perfect);
    }

    #[test]
    fn unknown_field_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[audio]
output_device = "alsa"
definitely_not_a_field = true
"#,
        )
        .unwrap();
        let err = MuzonConfig::load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }
}
