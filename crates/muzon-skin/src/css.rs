// SPDX-License-Identifier: MIT OR Apache-2.0
//! CSS variable re-tinter for the theme system.
//!
//! Reads the active skin's `theme.css`, finds the `:root` block,
//! extracts the 10 required variables (and any skin-specific
//! variables), and replaces the `--accent` and `--accent-2`
//! values with the user's chosen accent. The result is the
//! CSS string that the frontend injects into the
//! `<style id="muzon-skin">` element.
//!
//! v0.3.0 minimum: this re-tinter only overrides `--accent` and
//! `--accent-2`. The full Light/Dark palette swap is a v0.3.0
//! hardening addition (0018 step 3).

use muzon_core::AccentColor;

use crate::REQUIRED_CSS_VARIABLES;

/// Apply the user's accent to the skin's theme.css. Returns
/// the new CSS string. The skin's component rules are
/// unchanged; only the `--accent` and `--accent-2` values
/// inside the `:root` block are replaced.
pub fn apply_accent(theme_css: &str, accent: &AccentColor) -> String {
    let accent_value = accent.hex();
    let accent_2_value = accent.hex_accent_2();
    let mut out = String::with_capacity(theme_css.len() + 32);
    let mut in_root = false;
    for line in theme_css.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(":root") {
            in_root = true;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_root && trimmed == "}" {
            in_root = false;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_root && (trimmed.starts_with("--accent:") || trimmed.starts_with("--accent-2:")) {
            // Replace the value but keep the variable name and semicolon.
            let var = if trimmed.starts_with("--accent:") {
                "--accent"
            } else {
                "--accent-2"
            };
            let new_value = if var == "--accent" {
                &accent_value
            } else {
                &accent_2_value
            };
            out.push_str(&format!("    {var}: {new_value};\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Extract the value of a CSS variable from the skin's
/// `theme.css`. Returns `None` if the variable is not defined.
pub fn extract_variable(theme_css: &str, var: &str) -> Option<String> {
    for line in theme_css.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(var) {
            if let Some(value) = rest.strip_prefix(':') {
                if let Some(value) = value.strip_suffix(';') {
                    return Some(value.trim().to_string());
                }
            }
        }
    }
    None
}

/// Sanity check: assert that all 10 required CSS variables are
/// present in the skin's `theme.css`. Returns the list of
/// missing variables (empty on success).
pub fn missing_required_variables(theme_css: &str) -> Vec<&'static str> {
    REQUIRED_CSS_VARIABLES
        .iter()
        .copied()
        .filter(|var| extract_variable(theme_css, var).is_none())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CSS: &str = "\
:root {
    --bg-0: #0a0a0c;
    --bg-1: #131318;
    --text-0: #f5f5f7;
    --accent: #8b5cf6;
    --accent-2: #7c3aed;
}

body {
    color: var(--text-0);
}
";

    #[test]
    fn extract_variable_finds_value() {
        assert_eq!(extract_variable(SAMPLE_CSS, "--bg-0"), Some("#0a0a0c".into()));
        assert_eq!(extract_variable(SAMPLE_CSS, "--accent"), Some("#8b5cf6".into()));
    }

    #[test]
    fn extract_variable_returns_none_for_missing() {
        assert!(extract_variable(SAMPLE_CSS, "--missing").is_none());
    }

    #[test]
    fn missing_required_variables_finds_them() {
        let missing = missing_required_variables(SAMPLE_CSS);
        // SAMPLE_CSS has 5 of the 10 (--bg-0, --bg-1, --text-0,
        // --accent, --accent-2); 5 are missing.
        assert_eq!(missing.len(), 5);
    }

    #[test]
    fn apply_accent_replaces_accent_values() {
        let out = apply_accent(SAMPLE_CSS, &AccentColor::Pink);
        assert!(out.contains("--accent: #ec4899;"));
        assert!(out.contains("--accent-2: #db2777;"));
        // The other variables are unchanged.
        assert!(out.contains("--bg-0: #0a0a0c;"));
    }

    #[test]
    fn apply_accent_preserves_component_rules() {
        let out = apply_accent(SAMPLE_CSS, &AccentColor::Blue);
        // The body rule is preserved.
        assert!(out.contains("color: var(--text-0);"));
    }

    #[test]
    fn apply_accent_handles_custom_hex() {
        let out = apply_accent(SAMPLE_CSS, &AccentColor::Custom("#abcdef".into()));
        assert!(out.contains("--accent: #abcdef;"));
    }
}
