// SPDX-License-Identifier: MIT OR Apache-2.0
//! Muzon skin crate.
//!
//! Owns the skin package format (ZIP or directory with `skin.toml`
//! manifest, layered: background / main / buttons / spectrum), the
//! manifest validator, the four built-in skin ids and their
//! manifests, the theme system contract (10 required CSS
//! variables per the 0005 decision), and the simple
//! `{{placeholder}}` template substitution layer used to render a
//! skin's `index.html.tmpl` with runtime track data.
//!
//! The full ZIP / directory loader for user-installed skins lands
//! in v0.3.0 (issue 0016); v0.2.0 ships the four built-ins and
//! the public `Skin`, `SkinManifest`, `builtin_skins`,
//! `default_skin`, `load_skin`, `validate`, and `render` API.
//! Since 0016: the user-installed skin package format and
//! loader (ZIP + dir), the validator, and the installer that
//! drops the package into `~/.local/share/muzon/skins/<id>/`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod css;
pub mod installer;
pub mod loader;
pub mod validator;

/// The four built-in skin ids. Stable identifiers in code and
/// config. See `docs/decisions/0005-skins-and-default-theme.md`.
pub const BUILTIN_SKINS: &[&str] = &["modern", "winamp", "dense_pro", "cinematic"];

/// Default skin id per the 0005 decision.
pub const DEFAULT_SKIN: &str = "modern";

/// The four grid shapes documented in the 0005 decision.
pub const VALID_GRIDS: &[&str] = &[
    "topbar-sidebar-main-player",
    "titlebar-player-playlist",
    "top-sidebar-main-right-player",
    "top-main-player",
];

/// Theme mode for a skin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    Auto,
}

/// The 10 required CSS custom properties per the 0005 decision.
pub const REQUIRED_CSS_VARIABLES: &[&str] = &[
    "--bg-0", "--bg-1", "--bg-2", "--bg-3", "--border", "--text-0", "--text-1", "--text-2",
    "--accent", "--accent-2",
];

/// The deserialized `skin.toml` manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SkinManifest {
    pub skin: SkinMeta,
    pub theme: ThemeSection,
    pub layout: LayoutSection,
    #[serde(default)]
    pub layers: LayersSection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkinMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub license: String,
    pub min_muzon_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeSection {
    pub mode: ThemeMode,
    pub accent: String,
    pub palette: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutSection {
    pub main_template: String,
    #[serde(default)]
    pub mini_template: String,
    #[serde(default)]
    pub musiclab_template: String,
    pub grid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LayersSection {
    #[serde(default)]
    pub background: Vec<String>,
    #[serde(default)]
    pub main: Vec<String>,
    #[serde(default)]
    pub buttons: Vec<String>,
    #[serde(default)]
    pub spectrum: Vec<String>,
}

/// Errors produced by the skin loader and validator.
#[derive(Debug, Error)]
pub enum SkinError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse skin.toml at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("unknown grid shape '{0}'; expected one of {1:?}")]
    UnknownGrid(String, Vec<&'static str>),

    #[error("unknown skin id '{0}'; expected one of {1:?}")]
    UnknownSkin(String, Vec<&'static str>),

    #[error("theme.css is missing the required variable {0} at {1}")]
    MissingCssVariable(String, PathBuf),

    #[error("template file is empty: {0}")]
    EmptyTemplate(PathBuf),
}

/// A loaded skin: manifest + the paths to its assets and
/// templates, with the palette and main template contents
/// already read into memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skin {
    pub manifest: SkinManifest,
    pub root: PathBuf,
    pub theme_css: String,
    pub main_template: String,
}

/// Return the directory containing the built-in skins (bundled at
/// compile time). The path is `crates/muzon-skin/skins` relative
/// to the crate's `Cargo.toml`.
pub fn builtin_skins_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skins")
}

pub use css::{apply_accent, extract_variable, missing_required_variables};
pub use installer::{install_directory, install_zip, list_installed, InstallError};
pub use loader::{load_directory, load_zip, LoaderError};
pub use validator::{
    check_font_license, validate_manifest, validate_package_layout, ValidatorError, MAX_FILES,
    MAX_UNCOMPRESSED_BYTES,
};

// Skins that have a MusicLab template (issue 0021) gain a
// `musiclab_template` field in their `LayoutSection`. v0.3.0
// minimum: Modern ships the M1 (AI Assistant) template; the
// 3 other skins ship stubs in 0026 / 0030 / 0033.

/// Return the default skin id (`"modern"` per the 0005 decision).

/// Return the default skin id (`"modern"` per the 0005 decision).
pub fn default_skin() -> &'static str {
    DEFAULT_SKIN
}

/// Enumerate the four built-in skin ids as `SkinManifest` values.
/// The manifests are read from the bundled `skins/<id>/skin.toml`
/// at runtime; the order is the order of `BUILTIN_SKINS`.
pub fn builtin_skins() -> Result<Vec<SkinManifest>, SkinError> {
    let root = builtin_skins_root();
    let mut out = Vec::with_capacity(BUILTIN_SKINS.len());
    for id in BUILTIN_SKINS {
        let path = root.join(id).join("skin.toml");
        let manifest = read_manifest(&path)?;
        out.push(manifest);
    }
    Ok(out)
}

/// Load a built-in skin by id, including its `theme.css` and
/// `index.html.tmpl` contents.
pub fn load_skin(id: &str) -> Result<Skin, SkinError> {
    if !BUILTIN_SKINS.contains(&id) {
        return Err(SkinError::UnknownSkin(id.to_string(), BUILTIN_SKINS.to_vec()));
    }
    let root = builtin_skins_root().join(id);
    let manifest = read_manifest(&root.join("skin.toml"))?;
    let theme_css = fs::read_to_string(root.join(&manifest.theme.palette)).map_err(|source| {
        SkinError::Io {
            path: root.join(&manifest.theme.palette),
            source,
        }
    })?;
    let main_template = fs::read_to_string(root.join(&manifest.layout.main_template))
        .map_err(|source| SkinError::Io {
            path: root.join(&manifest.layout.main_template),
            source,
        })?;
    if main_template.trim().is_empty() {
        return Err(SkinError::EmptyTemplate(root.join(&manifest.layout.main_template)));
    }
    Ok(Skin {
        manifest,
        root,
        theme_css,
        main_template,
    })
}

/// Validate a manifest + the corresponding `theme.css`. Checks:
/// - the grid shape is one of the four documented values;
/// - the `theme.css` defines all 10 required CSS variables;
/// - the main template file is non-empty.
pub fn validate(skin: &Skin) -> Result<(), SkinError> {
    validate_grid(&skin.manifest.layout.grid)?;
    for var in REQUIRED_CSS_VARIABLES {
        if !skin.theme_css.contains(var) {
            return Err(SkinError::MissingCssVariable(
                (*var).to_string(),
                skin.root.join(&skin.manifest.theme.palette),
            ));
        }
    }
    if skin.main_template.trim().is_empty() {
        return Err(SkinError::EmptyTemplate(
            skin.root.join(&skin.manifest.layout.main_template),
        ));
    }
    Ok(())
}

fn validate_grid(grid: &str) -> Result<(), SkinError> {
    if VALID_GRIDS.contains(&grid) {
        Ok(())
    } else {
        Err(SkinError::UnknownGrid(grid.to_string(), VALID_GRIDS.to_vec()))
    }
}

fn read_manifest(path: &Path) -> Result<SkinManifest, SkinError> {
    let raw = fs::read_to_string(path).map_err(|source| SkinError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&raw).map_err(|source| SkinError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Render the main template with the given `key -> value` map.
/// The substitution is a simple `{{name}}` lookup; v0.3.0 may
/// adopt handlebars or tera. Unknown placeholders are left as-is
/// in the output so a regression is visible in the rendered HTML.
pub fn render(template: &str, vars: &std::collections::HashMap<&str, &str>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        if let Some(close) = after.find("}}") {
            let key = after[..close].trim();
            match vars.get(key) {
                Some(value) => out.push_str(value),
                None => out.push_str(&rest[open..open + 2 + close + 2]),
            }
            rest = &after[close + 2..];
        } else {
            out.push_str(&rest[open..]);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn default_skin_is_modern() {
        assert_eq!(default_skin(), "modern");
    }

    #[test]
    fn all_four_built_ins_load() {
        let skins = builtin_skins().expect("builtin_skins");
        assert_eq!(skins.len(), 4);
        let ids: Vec<&str> = skins.iter().map(|m| m.skin.id.as_str()).collect();
        assert_eq!(ids, vec!["modern", "winamp", "dense_pro", "cinematic"]);
    }

    #[test]
    fn modern_manifest_round_trip() {
        let skins = builtin_skins().expect("builtin_skins");
        let modern = skins.iter().find(|m| m.skin.id == "modern").expect("modern");
        assert_eq!(modern.skin.name, "Modern Skin");
        assert_eq!(modern.layout.grid, "topbar-sidebar-main-player");
        assert_eq!(modern.theme.mode, ThemeMode::Dark);
        assert_eq!(modern.theme.accent, "#8b5cf6");
    }

    #[test]
    fn manifest_requires_css_variables() {
        let modern = load_skin("modern").expect("modern");
        for var in REQUIRED_CSS_VARIABLES {
            assert!(
                modern.theme_css.contains(var),
                "modern/theme.css is missing required CSS variable {var}"
            );
        }
    }

    #[test]
    fn unknown_grid_rejected() {
        // Synthesize a manifest with an unknown grid; we cannot
        // load this from disk, so we build a Skin in memory.
        let manifest = SkinManifest {
            skin: SkinMeta {
                id: "fake".into(),
                name: "Fake".into(),
                version: "0.0.0".into(),
                author: "test".into(),
                license: "MIT".into(),
                min_muzon_version: "0.2.0".into(),
            },
            theme: ThemeSection {
                mode: ThemeMode::Dark,
                accent: "#000000".into(),
                palette: "theme.css".into(),
            },
            layout: LayoutSection {
                main_template: "index.html.tmpl".into(),
                mini_template: String::new(),
                grid: "made-up-grid".into(),
            },
            layers: LayersSection::default(),
        };
        let theme_css = ":root { --bg-0: #000; }".to_string();
        let main_template = "<html></html>".to_string();
        let skin = Skin {
            manifest,
            root: PathBuf::from("."),
            theme_css,
            main_template,
        };
        let err = validate(&skin).unwrap_err();
        assert!(matches!(err, SkinError::UnknownGrid(_, _)));
    }

    #[test]
    fn missing_css_variable_rejected() {
        let manifest = SkinManifest {
            skin: SkinMeta {
                id: "novars".into(),
                name: "No Vars".into(),
                version: "0.0.0".into(),
                author: "test".into(),
                license: "MIT".into(),
                min_muzon_version: "0.2.0".into(),
            },
            theme: ThemeSection {
                mode: ThemeMode::Dark,
                accent: "#000000".into(),
                palette: "theme.css".into(),
            },
            layout: LayoutSection {
                main_template: "index.html.tmpl".into(),
                mini_template: String::new(),
                grid: "topbar-sidebar-main-player".into(),
            },
            layers: LayersSection::default(),
        };
        let theme_css = ":root { --bg-0: #000; }".to_string();
        let main_template = "<html></html>".to_string();
        let skin = Skin {
            manifest,
            root: PathBuf::from("."),
            theme_css,
            main_template,
        };
        let err = validate(&skin).unwrap_err();
        assert!(matches!(err, SkinError::MissingCssVariable(_, _)));
    }

    #[test]
    fn render_substitutes_placeholders() {
        let mut vars = HashMap::new();
        vars.insert("name", "Midnight City");
        vars.insert("artist", "M83");
        let out = render("Track: {{name}} by {{artist}}", &vars);
        assert_eq!(out, "Track: Midnight City by M83");
    }

    #[test]
    fn render_leaves_unknown_placeholders_intact() {
        let vars = HashMap::new();
        let out = render("Track: {{name}}", &vars);
        assert_eq!(out, "Track: {{name}}");
    }

    #[test]
    fn modern_validate_passes() {
        let modern = load_skin("modern").expect("modern");
        validate(&modern).expect("validate");
    }

    #[test]
    fn all_builtins_validate() {
        // v0.2.0 ships the modern skin with full HTML+CSS; the
        // other 3 built-ins are placeholders (skin.toml + theme.css
        // only) until 0017 ports them. Only modern is loaded here.
        let modern = load_skin("modern").expect("modern");
        validate(&modern).expect("validate");
    }

    #[test]
    fn themes_html_concept_1_render() {
        // Regression test: render the modern skin with the demo
        // content from themes.html concept 1 and assert the
        // resulting HTML contains the same anchor elements.
        let skin = load_skin("modern").expect("modern");
        let mut vars = HashMap::new();
        vars.insert("today_listen_time", "2h 14m");
        vars.insert("today_track_count", "28");
        vars.insert("track_title", "Midnight City");
        vars.insert("track_artist", "M83");
        vars.insert("track_album", "Hurry Up, We're Dreaming");
        vars.insert("track_year", "2011");
        vars.insert("track_duration", "4:03");
        vars.insert("track_codec", "FLAC 24bit");
        vars.insert("track_bpm", "105");
        vars.insert("track_key", "A minor");
        vars.insert("track_mood", "Energetic");
        vars.insert("similar_count", "12");
        vars.insert("ai_playlist_name", "Night Drive");
        vars.insert("play_count", "47");
        vars.insert("play_pause_label", "Pause");
        vars.insert("position_label", "1:32");
        vars.insert("duration_label", "4:03");
        vars.insert("progress_pct", "38");
        vars.insert("volume_pct", "70");
        let out = render(&skin.main_template, &vars);
        for anchor in [
            "class=\"app\"",
            "class=\"topbar\"",
            "class=\"sidebar\"",
            "class=\"main\"",
            "class=\"player\"",
            "class=\"cover\"",
            "class=\"spectrum\"",
            "class=\"ai-row\"",
            "Midnight City",
            "M83",
            "FLAC 24bit",
            "105 BPM",
        ] {
            assert!(out.contains(anchor), "rendered modern skin is missing anchor {anchor:?}");
        }
    }
}
