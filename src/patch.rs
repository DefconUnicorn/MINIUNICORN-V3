//! Versioned patch file format for MiniUnicorn.
//!
//! A patch bundles the complete instrument state — DSP parameters (which
//! include the jack routing masks) and the visual layout — into one
//! self-describing JSON document:
//!
//! ```json
//! {
//!   "schema": "miniunicorn.patch",
//!   "version": 1,
//!   "app_version": "0.3.0",
//!   "name": "Fat Bass",
//!   "params": { ... },
//!   "layout": { ... }
//! }
//! ```
//!
//! Older files that stored a bare `Params` object (no envelope) are still
//! accepted and silently upgraded to the current schema on load.

use crate::layout::CanvasLayout;
use crate::params::Params;

/// Magic string identifying a MiniUnicorn patch envelope.
pub const SCHEMA: &str = "miniunicorn.patch";
/// Current schema revision. Bump when the on-disk shape changes.
pub const VERSION: u32 = 1;

/// A complete, self-describing instrument state saved to disk.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PatchFile {
    #[serde(default = "default_schema")]
    pub schema: String,
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub app_version: String,
    #[serde(default)]
    pub name: Option<String>,
    pub params: Params,
    #[serde(default)]
    pub layout: Option<CanvasLayout>,
}

fn default_schema() -> String {
    SCHEMA.to_string()
}

fn default_version() -> u32 {
    VERSION
}

impl PatchFile {
    /// Wrap a live instrument state into a patch document.
    pub fn capture(name: Option<String>, params: Params, layout: Option<CanvasLayout>) -> Self {
        Self {
            schema: SCHEMA.to_string(),
            version: VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            name,
            params,
            layout,
        }
    }

    /// Serialize to pretty JSON.
    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string_pretty(self).ok()
    }
}

/// Parse a patch from JSON text, migrating legacy bare-`Params` files.
///
/// Returns `None` only if the text is neither a patch envelope nor a bare
/// parameter set.
pub fn from_json(data: &str) -> Option<PatchFile> {
    // Preferred: full envelope.
    if let Ok(patch) = serde_json::from_str::<PatchFile>(data) {
        return Some(migrate(patch));
    }
    // Legacy: a bare Params document (pre-0.3.0 last.json / presets).
    if let Ok(params) = serde_json::from_str::<Params>(data) {
        return Some(PatchFile::capture(None, params, None));
    }
    None
}

/// Bring an older patch up to the current schema revision.
///
/// No shape changes exist yet, so this only stamps the current values;
/// future revisions add their step here.
fn migrate(mut patch: PatchFile) -> PatchFile {
    if patch.version < VERSION {
        // v0 (bare params) had no layout key; nothing to rewrite yet.
        patch.version = VERSION;
    }
    if patch.schema.is_empty() {
        patch.schema = SCHEMA.to_string();
    }
    patch
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_patch() {
        let params = Params::default();
        let patch = PatchFile::capture(Some("Test".into()), params.clone(), None);
        let json = patch.to_json().unwrap();
        let back = from_json(&json).unwrap();
        assert_eq!(back.schema, SCHEMA);
        assert_eq!(back.version, VERSION);
        assert_eq!(back.name.as_deref(), Some("Test"));
        assert_eq!(back.params.cutoff_0_10, params.cutoff_0_10);
    }

    #[test]
    fn migrates_bare_params() {
        let bare = serde_json::to_string(&Params::default()).unwrap();
        let patch = from_json(&bare).expect("legacy params should load");
        assert_eq!(patch.schema, SCHEMA);
        assert_eq!(patch.version, VERSION);
        assert!(patch.layout.is_none());
    }

    #[test]
    fn rejects_garbage() {
        assert!(from_json("not json at all").is_none());
    }
}
