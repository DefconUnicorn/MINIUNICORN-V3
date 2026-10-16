//! Last-state + named preset persistence (JSON files, no extra deps).
//!
//! Layout:
//! - `<dir>/last.json` — auto-saved working state, reloaded at startup.
//! - `<dir>/presets/<name>.json` — named user presets.
//!
//! Both files use the versioned [`crate::patch::PatchFile`] envelope, which
//! bundles the DSP parameters (including jack routing) with the visual
//! layout. Legacy bare-`Params` files still load and are upgraded on read.
//!
//! Dir: `%APPDATA%/miniunicorn` on Windows, else
//! `$HOME/.miniunicorn` (fallback: OS temp dir).

use crate::layout::CanvasLayout;
use crate::params::Params;
use crate::patch::PatchFile;
use std::path::PathBuf;

fn state_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p = PathBuf::from(appdata).join("miniunicorn");
            let _ = std::fs::create_dir_all(&p);
            return p;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".miniunicorn");
        let _ = std::fs::create_dir_all(&p);
        return p;
    }
    let p = std::env::temp_dir().join("miniunicorn");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn presets_dir() -> PathBuf {
    let p = state_dir().join("presets");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn last_path() -> PathBuf {
    state_dir().join("last.json")
}

/// Keep names filesystem-safe: letters, digits, space, - _ . (max 48).
pub fn sanitize_name(raw: &str) -> String {
    let mut s: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_' || *c == '.')
        .collect::<String>()
        .trim()
        .to_string();
    while s.contains("  ") {
        s = s.replace("  ", " ");
    }
    if s.len() > 48 {
        s.truncate(48);
        s = s.trim().to_string();
    }
    s
}

fn preset_path(name: &str) -> Option<PathBuf> {
    let clean = sanitize_name(name);
    if clean.is_empty() || clean == "." || clean == ".." {
        return None;
    }
    Some(presets_dir().join(format!("{clean}.json")))
}

// --- last state ---

/// Full last-state patch (params + layout), or None if absent/unreadable.
pub fn load_last_patch() -> Option<PatchFile> {
    let data = std::fs::read_to_string(last_path()).ok()?;
    crate::patch::from_json(&data)
}

/// Convenience accessor for the DSP parameters only (startup path).
pub fn load_last() -> Option<Params> {
    load_last_patch().map(|p| p.params)
}

pub fn save_last(p: &Params, layout: Option<&CanvasLayout>) {
    let patch = PatchFile::capture(None, *p, layout.cloned());
    if let Some(data) = patch.to_json() {
        let _ = std::fs::write(last_path(), data);
    }
}

// --- named presets ---

pub fn list_presets() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(presets_dir()) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|x| x.to_str()) {
                    out.push(stem.to_string());
                }
            }
        }
    }
    out.sort_by_key(|s| s.to_lowercase());
    out
}

/// Load a named preset as a full patch (params + optional layout).
pub fn load_preset_patch(name: &str) -> Option<PatchFile> {
    let data = std::fs::read_to_string(preset_path(name)?).ok()?;
    crate::patch::from_json(&data)
}

/// Load a named preset's parameters and (if present) its layout.
pub fn load_preset(name: &str) -> Option<(Params, Option<CanvasLayout>)> {
    let patch = load_preset_patch(name)?;
    Some((patch.params, patch.layout))
}

/// Returns the sanitized name actually used, or None on empty/invalid.
pub fn save_preset(name: &str, p: &Params, layout: Option<&CanvasLayout>) -> Option<String> {
    let path = preset_path(name)?;
    let clean = sanitize_name(name);
    let patch = PatchFile::capture(Some(clean.clone()), *p, layout.cloned());
    if let Some(data) = patch.to_json() {
        if std::fs::write(&path, data).is_ok() {
            return Some(clean);
        }
    }
    None
}

pub fn delete_preset(name: &str) -> bool {
    if let Some(path) = preset_path(name) {
        return std::fs::remove_file(path).is_ok();
    }
    false
}
