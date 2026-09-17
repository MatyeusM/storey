//! GUI design tokens: resolved [`GuiTheme`] plus [`GuiOverrides`].
//!
//! Defaults are monochrome by design: deep background/surface pair,
//! one bone tone for text, gray secondary copy, one danger color.
//! A `[gui]` table in `ui.toml` overrides individual tokens — see
//! [`GuiTheme::resolve`]:
//!
//! ```toml
//! [gui]
//! rem_px = 18
//! sans_family = "Inter"
//! ```

use serde::{Deserialize, Serialize};

/// Default base font size; `Base.rem()` multiplies off this.
pub const REM_PX: f64 = 16.0;
/// Default UI copy family (bundled; override with a system family).
pub const SANS_FAMILY: &str = "PT Sans";
/// Window background.
pub const BACKGROUND: &str = "#050d0d";
/// Panels, inputs.
pub const SURFACE: &str = "#25302f";
/// Body copy.
pub const TEXT: &str = "#d3e1e0";
/// Disabled, secondary copy.
pub const MUTED: &str = "#859291";
/// Destructive actions, errors.
pub const DANGER: &str = "#9c113f";

/// Concrete GUI theme handed to QML (all values filled).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiTheme {
    pub rem_px: f64,
    pub sans_family: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub muted: String,
    pub danger: String,
}

impl Default for GuiTheme {
    fn default() -> Self {
        Self {
            rem_px: REM_PX,
            sans_family: SANS_FAMILY.to_string(),
            background: BACKGROUND.to_string(),
            surface: SURFACE.to_string(),
            text: TEXT.to_string(),
            muted: MUTED.to_string(),
            danger: DANGER.to_string(),
        }
    }
}

/// Sparse `[gui]` overrides from `ui.toml`; `None` keeps the default.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuiOverrides {
    #[serde(default)]
    pub rem_px: Option<f64>,
    #[serde(default)]
    pub sans_family: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub surface: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub muted: Option<String>,
    #[serde(default)]
    pub danger: Option<String>,
}

impl GuiTheme {
    /// Defaults with every valid override applied; invalid values
    /// (bad hex, non-positive rem) keep the current token.
    pub fn resolve(overrides: &GuiOverrides) -> Self {
        let mut theme = Self::default();
        theme.apply(overrides);
        theme
    }

    /// Explicit `ui.toml` entries as JSON, valid values only.
    ///
    /// The QML theme singletons own the defaults; the GUI applies
    /// this object over them at startup, so invalid or absent keys
    /// simply keep the QML-side value.
    pub fn overrides_json(overrides: &GuiOverrides) -> serde_json::Value {
        let mut out = serde_json::Map::new();
        if let Some(rem) = overrides.rem_px.and_then(clean_rem) {
            out.insert("rem_px".into(), rem.into());
        }
        if let Some(raw) = overrides.sans_family.as_deref() {
            let name = clean_name(raw, "");
            if !name.is_empty() {
                out.insert("sans_family".into(), name.into());
            }
        }
        for (key, raw) in [
            ("background", &overrides.background),
            ("surface", &overrides.surface),
            ("text", &overrides.text),
            ("muted", &overrides.muted),
            ("danger", &overrides.danger),
        ] {
            if let Some(raw) = raw.as_deref() {
                let color = clean_hex(raw, "");
                if !color.is_empty() {
                    out.insert(key.into(), color.into());
                }
            }
        }
        serde_json::Value::Object(out)
    }
    /// Apply overrides onto an existing theme (same validity rules).
    pub fn apply(&mut self, overrides: &GuiOverrides) {
        if let Some(rem) = overrides.rem_px.and_then(clean_rem) {
            self.rem_px = rem;
        }
        if let Some(raw) = overrides.sans_family.as_deref() {
            self.sans_family = clean_name(raw, &self.sans_family);
        }
        if let Some(raw) = overrides.background.as_deref() {
            self.background = clean_hex(raw, &self.background);
        }
        if let Some(raw) = overrides.surface.as_deref() {
            self.surface = clean_hex(raw, &self.surface);
        }
        if let Some(raw) = overrides.text.as_deref() {
            self.text = clean_hex(raw, &self.text);
        }
        if let Some(raw) = overrides.muted.as_deref() {
            self.muted = clean_hex(raw, &self.muted);
        }
        if let Some(raw) = overrides.danger.as_deref() {
            self.danger = clean_hex(raw, &self.danger);
        }
    }
}

/// Valid rem: finite and positive; anything else is ignored.
fn clean_rem(raw: f64) -> Option<f64> {
    if raw.is_finite() && raw > 0.0 {
        Some(raw)
    } else {
        None
    }
}

/// Valid family name: non-empty after trimming, else the fallback.
fn clean_name(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Valid hex (`#rgb`, `#rrggbb`, `#rrggbbaa`); else the fallback.
fn clean_hex(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or("");
    let ok = matches!(digits.len(), 3 | 6 | 8) && digits.chars().all(|c| c.is_ascii_hexdigit());
    if trimmed.starts_with('#') && ok {
        trimmed.to_string()
    } else {
        fallback.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_palette() {
        let theme = GuiTheme::default();
        assert_eq!(theme.rem_px, 16.0);
        assert_eq!(theme.background, "#050d0d");
        assert_eq!(theme.text, "#d3e1e0");
        assert_eq!(theme.danger, "#9c113f");
    }

    #[test]
    fn resolve_applies_valid_overrides() {
        let overrides = GuiOverrides {
            rem_px: Some(18.0),
            danger: Some("#ff00ff".to_string()),
            sans_family: Some("Inter".to_string()),
            ..GuiOverrides::default()
        };
        let theme = GuiTheme::resolve(&overrides);
        assert_eq!(theme.rem_px, 18.0);
        assert_eq!(theme.danger, "#ff00ff");
        assert_eq!(theme.sans_family, "Inter");
        assert_eq!(theme.background, BACKGROUND);
    }

    #[test]
    fn overrides_json_keeps_valid_explicit_keys() {
        let overrides = GuiOverrides {
            rem_px: Some(18.0),
            sans_family: Some("Inter".to_string()),
            background: Some("#112233".to_string()),
            ..GuiOverrides::default()
        };
        let json = GuiTheme::overrides_json(&overrides);
        assert_eq!(json["rem_px"], 18.0);
        assert_eq!(json["sans_family"], "Inter");
        assert_eq!(json["background"], "#112233");
        assert!(json.get("surface").is_none());
        assert!(json.get("danger").is_none());
    }

    #[test]
    fn overrides_json_drops_invalid_values() {
        let overrides = GuiOverrides {
            rem_px: Some(0.0),
            danger: Some("not-a-color".to_string()),
            sans_family: Some("   ".to_string()),
            ..GuiOverrides::default()
        };
        assert_eq!(GuiTheme::overrides_json(&overrides), serde_json::json!({}));
        assert_eq!(
            GuiTheme::overrides_json(&GuiOverrides::default()),
            serde_json::json!({})
        );
    }
    #[test]
    fn resolve_ignores_invalid_values() {
        let overrides = GuiOverrides {
            rem_px: Some(0.0),
            danger: Some("not-a-color".to_string()),
            text: Some("#12".to_string()),
            sans_family: Some("   ".to_string()),
            ..GuiOverrides::default()
        };
        let theme = GuiTheme::resolve(&overrides);
        assert_eq!(theme.rem_px, REM_PX);
        assert_eq!(theme.danger, DANGER);
        assert_eq!(theme.text, TEXT);
        assert_eq!(theme.sans_family, SANS_FAMILY);
    }
}
