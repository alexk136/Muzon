// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skin package loader.
//!
//! Reads a skin package from a ZIP file or an extracted
//! directory. The ZIP reader enforces the path-traversal
//! guard (no `..` segments, no absolute paths) and the
//! zip-bomb guard (max uncompressed size, max file count).
//! The directory reader trusts the on-disk layout and runs
//! the structural validator against it.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use thiserror::Error;
use zip::ZipArchive;

use crate::validator::{validate_package_layout, ValidatorError, MAX_FILES, MAX_UNCOMPRESSED_BYTES};
use crate::SkinManifest;

/// Errors produced by the loader.
#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("manifest parse: {0}")]
    Manifest(String),

    #[error("security: {0}")]
    Security(String),

    #[error("validator: {0}")]
    Validator(#[from] ValidatorError),
}

/// Load a skin package from a ZIP file into `dest_root`. The
/// function enforces the §3.9 security rules and returns the
/// parsed `SkinManifest`. The caller is responsible for
/// creating `dest_root` and cleaning it up on failure.
pub fn load_zip(zip_path: &Path, dest_root: &Path) -> Result<SkinManifest, LoaderError> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut total_size: u64 = 0;
    let mut file_count: usize = 0;
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let raw_name = entry.name().to_string();
        let safe_name = sanitise_zip_path(&raw_name).ok_or_else(|| {
            LoaderError::Security(format!("path traversal in zip entry: {raw_name:?}"))
        })?;
        let out_path = dest_root.join(&safe_name);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Read into memory (capped) to enforce the size limit.
        let claimed = entry.size();
        if total_size.saturating_add(claimed) > MAX_UNCOMPRESSED_BYTES {
            return Err(LoaderError::Security(format!(
                "package exceeds uncompressed size limit ({} bytes)",
                MAX_UNCOMPRESSED_BYTES
            )));
        }
        let mut buf = Vec::with_capacity(claimed.min(64 * 1024) as usize);
        entry.take(claimed).read_to_end(&mut buf)?;
        total_size = total_size.saturating_add(buf.len() as u64);
        file_count += 1;
        if file_count > MAX_FILES {
            return Err(LoaderError::Security(format!(
                "package exceeds file count limit ({MAX_FILES})"
            )));
        }
        fs::write(&out_path, &buf)?;
    }

    let manifest = read_manifest_at(dest_root)?;
    validate_package_layout(dest_root)?;
    Ok(manifest)
}

/// Load a skin package from an already-extracted directory.
/// The directory must contain `skin.toml` at its root. The
/// structural validator runs against the directory layout.
pub fn load_directory(dir: &Path) -> Result<SkinManifest, LoaderError> {
    let manifest = read_manifest_at(dir)?;
    validate_package_layout(dir)?;
    Ok(manifest)
}

/// Sanitise a zip entry name. Rejects absolute paths and
/// `..` components. Returns `None` if the path is unsafe.
fn sanitise_zip_path(name: &str) -> Option<PathBuf> {
    if name.contains('\0') {
        return None;
    }
    if name.starts_with('/') || name.contains(":\\") {
        return None;
    }
    let mut out = PathBuf::new();
    for component in Path::new(name).components() {
        let s = component.as_os_str().to_string_lossy();
        if s == ".." {
            return None;
        }
        if s.starts_with("..") || s.contains('/') && component == std::path::Component::ParentDir {
            return None;
        }
        out.push(s.as_ref());
    }
    Some(out)
}

fn read_manifest_at(root: &Path) -> Result<SkinManifest, LoaderError> {
    let manifest_path = root.join("skin.toml");
    let raw = fs::read_to_string(&manifest_path).map_err(|e| {
        LoaderError::Manifest(format!("read {}: {e}", manifest_path.display()))
    })?;
    toml::from_str(&raw).map_err(|e| LoaderError::Manifest(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitise_strips_absolute_paths() {
        assert!(sanitise_zip_path("/etc/passwd").is_none());
        assert!(sanitise_zip_path("C:\\Windows\\System32").is_none());
    }

    #[test]
    fn sanitise_rejects_parent_components() {
        assert!(sanitise_zip_path("../etc/passwd").is_none());
        assert!(sanitise_zip_path("a/../../b").is_none());
    }

    #[test]
    fn sanitise_accepts_relative_paths() {
        assert_eq!(
            sanitise_zip_path("skin.toml").unwrap(),
            PathBuf::from("skin.toml")
        );
        assert_eq!(
            sanitise_zip_path("assets/bg/image.png").unwrap(),
            PathBuf::from("assets/bg/image.png")
        );
    }
}
