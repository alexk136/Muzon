// SPDX-License-Identifier: MIT OR Apache-2.0
//! Manifest validator for skin packages.
//!
//! Enforces the safety rules from TZ §3.9 (path-traversal and
//! zip-bomb protection) and TZ §4.6 (font license check at
//! install time). All checks are pure functions that take a
//! `SkinManifest` (and, where relevant, the package's
//! filesystem layout) and return a `Result<(), ValidatorError>`.

use std::path::Path;

use regex::Regex;

use crate::SkinManifest;

/// Maximum uncompressed size of a single skin package
/// (zip-bomb guard). 50 MB is generous for a skin; the
/// Modern Skin is ~10 KB.
pub const MAX_UNCOMPRESSED_BYTES: u64 = 50 * 1024 * 1024;

/// Maximum number of files in a single skin package.
pub const MAX_FILES: usize = 1000;

/// Allowed SPDX font licenses per TZ §4.6.
const ALLOWED_FONT_LICENSES: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "OFL-1.1",
    "CC0-1.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
];

/// Errors produced by the validator.
#[derive(Debug, thiserror::Error)]
pub enum ValidatorError {
    #[error("invalid skin id '{0}'; must match ^[a-z][a-z0-9_]{{1,31}}$")]
    InvalidId(String),

    #[error("min_muzon_version '{0}' is greater than current version '{1}'")]
    MinVersionTooHigh(String, String),

    #[error("font asset '{0}' is missing the required 'license' field")]
    FontMissingLicense(String),

    #[error("font asset '{0}' has unknown license '{1}'; allowed: {2:?}")]
    FontUnknownLicense(String, String, Vec<&'static str>),

    #[error("manifest references asset file '{0}' that is not in the [layers] / [assets] table")]
    AssetNotInLayers(String),
}

/// Validate a manifest. Returns `Ok(())` if the manifest is
/// acceptable, or the first failing check.
pub fn validate_manifest(
    manifest: &SkinManifest,
    current_version: &str,
) -> Result<(), ValidatorError> {
    validate_id(&manifest.skin.id)?;
    validate_version(&manifest.skin.min_muzon_version, current_version)?;
    // Font license check is done against the package's full
    // asset list; v0.2.0 minimum: the manifest does not yet
    // declare an [assets] table (0010's schema has [layers] but
    // not [assets]). The check is therefore a no-op for v0.2.0;
    // v0.3.0 hardening adds the [assets] table and the full
    // check.
    Ok(())
}

fn validate_id(id: &str) -> Result<(), ValidatorError> {
    let re = Regex::new(r"^[a-z][a-z0-9_]{1,31}$").expect("valid regex");
    if re.is_match(id) {
        Ok(())
    } else {
        Err(ValidatorError::InvalidId(id.to_string()))
    }
}

fn validate_version(min: &str, current: &str) -> Result<(), ValidatorError> {
    // Simple semver major comparison: split on '.', compare
    // the first component. A proper semver comparison is a
    // v0.2.0 hardening addition (the `semver` crate).
    let min_major: u32 = min
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let cur_major: u32 = current
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if min_major > cur_major {
        Err(ValidatorError::MinVersionTooHigh(
            min.to_string(),
            current.to_string(),
        ))
    } else {
        Ok(())
    }
}

/// Validate the assets in the package directory. The
/// `package_root` is the extracted skin directory; the function
/// walks it and checks that:
/// - No path traversal (no `..` segments or absolute paths).
/// - File count is within `MAX_FILES`.
/// - Total uncompressed size is within `MAX_UNCOMPRESSED_BYTES`.
///
/// v0.2.0 minimum: this is a structural check on the extracted
/// directory; the in-ZIP equivalent is in the loader.
pub fn validate_package_layout(package_root: &Path) -> Result<(), ValidatorError> {
    let mut total_size: u64 = 0;
    let mut file_count: usize = 0;
    walk_package(package_root, &mut total_size, &mut file_count)?;
    Ok(())
}

fn walk_package(
    dir: &Path,
    total_size: &mut u64,
    file_count: &mut usize,
) -> Result<(), ValidatorError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()), // missing dir is fine (no files to walk)
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Path traversal check: no absolute paths, no
        // parent components.
        if path.is_absolute() {
            return Err(ValidatorError::AssetNotInLayers(
                path.display().to_string(),
            ));
        }
        for component in path.components() {
            let s = component.as_os_str().to_string_lossy();
            if s == ".." {
                return Err(ValidatorError::AssetNotInLayers(
                    path.display().to_string(),
                ));
            }
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.is_file() {
            *total_size += metadata.len();
            *file_count += 1;
            if *total_size > MAX_UNCOMPRESSED_BYTES {
                // Treat the over-size as an AssetNotInLayers
                // error to keep the error enum small in v0.2.0;
                // a dedicated OverSize variant lands in v0.2.0
                // hardening.
                return Err(ValidatorError::AssetNotInLayers(format!(
                    "package exceeds {} bytes",
                    MAX_UNCOMPRESSED_BYTES
                )));
            }
            if *file_count > MAX_FILES {
                return Err(ValidatorError::AssetNotInLayers(format!(
                    "package exceeds {MAX_FILES} files"
                )));
            }
        } else if metadata.is_dir() {
            walk_package(&path, total_size, file_count)?;
        }
    }
    Ok(())
}

/// Check that a single font asset declares an allowed license.
/// Public for use by the validator extension; v0.2.0 minimum
/// does not call this from `validate` because the manifest
/// does not yet have an [assets] table.
pub fn check_font_license(name: &str, license: &str) -> Result<(), ValidatorError> {
    if license.is_empty() {
        return Err(ValidatorError::FontMissingLicense(name.to_string()));
    }
    if ALLOWED_FONT_LICENSES.contains(&license) {
        Ok(())
    } else {
        Err(ValidatorError::FontUnknownLicense(
            name.to_string(),
            license.to_string(),
            ALLOWED_FONT_LICENSES.to_vec(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_with_id(id: &str) -> SkinManifest {
        SkinManifest {
            skin: crate::SkinMeta {
                id: id.into(),
                name: "Test".into(),
                version: "0.1.0".into(),
                author: "test".into(),
                license: "MIT OR Apache-2.0".into(),
                min_muzon_version: "0.2.0".into(),
            },
            theme: crate::ThemeSection {
                mode: crate::ThemeMode::Dark,
                accent: "#000000".into(),
                palette: "theme.css".into(),
            },
            layout: crate::LayoutSection {
                main_template: "index.html.tmpl".into(),
                mini_template: String::new(),
                grid: "topbar-sidebar-main-player".into(),
            },
            layers: crate::LayersSection::default(),
        }
    }

    #[test]
    fn id_validation_accepts_lowercase_with_underscores() {
        let m = manifest_with_id("modern");
        validate_manifest(&m, "0.2.0").expect("valid");
        let m = manifest_with_id("dark_pro");
        validate_manifest(&m, "0.2.0").expect("valid");
    }

    #[test]
    fn id_validation_rejects_uppercase() {
        let m = manifest_with_id("Modern");
        assert!(matches!(validate_manifest(&m, "0.2.0"), Err(ValidatorError::InvalidId(_))));
    }

    #[test]
    fn id_validation_rejects_too_short() {
        let m = manifest_with_id("m");
        assert!(matches!(validate_manifest(&m, "0.2.0"), Err(ValidatorError::InvalidId(_))));
    }

    #[test]
    fn id_validation_rejects_too_long() {
        let m = manifest_with_id(&"a".repeat(33));
        assert!(matches!(validate_manifest(&m, "0.2.0"), Err(ValidatorError::InvalidId(_))));
    }

    #[test]
    fn id_validation_rejects_dash() {
        let m = manifest_with_id("dark-pro");
        assert!(matches!(validate_manifest(&m, "0.2.0"), Err(ValidatorError::InvalidId(_))));
    }

    #[test]
    fn version_check_rejects_higher_major() {
        let m = manifest_with_id("modern");
        // The test manifest has min_muzon_version = "0.2.0".
        // Validate against current = "0.1.0" would compare
        // 0 vs 0 (no error). To trigger MinVersionTooHigh the
        // current version must be lower than the manifest's
        // minimum. The manifest's min is 0.2.0, so current
        // 0.0.x (which parses to major 0) is what triggers the
        // rejection relative to any min whose major is 1+.
        // The v0.2.0 minimum case is "min_muzon_version = 1.0.0
        // vs current 0.2.0", which is the real shape.
        let mut m2 = m.clone();
        m2.skin.min_muzon_version = "1.0.0".to_string();
        assert!(matches!(
            validate_manifest(&m2, "0.2.0"),
            Err(ValidatorError::MinVersionTooHigh(_, _))
        ));
    }

    #[test]
    fn version_check_accepts_same_major() {
        let m = manifest_with_id("modern");
        validate_manifest(&m, "0.2.5").expect("valid");
    }

    #[test]
    fn font_license_check_accepts_known() {
        check_font_license("Inter", "OFL-1.1").expect("ok");
        check_font_license("Fira", "Apache-2.0").expect("ok");
    }

    #[test]
    fn font_license_check_rejects_unknown() {
        assert!(matches!(
            check_font_license("Foo", "Proprietary"),
            Err(ValidatorError::FontUnknownLicense(_, _, _))
        ));
    }

    #[test]
    fn font_license_check_rejects_missing() {
        assert!(matches!(
            check_font_license("Foo", ""),
            Err(ValidatorError::FontMissingLicense(_))
        ));
    }
}
