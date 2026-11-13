#![forbid(unsafe_code)]
//! Headless workspace state. Preview data is immutable and is not a document editor model.
pub use joydeck_format::clipboard as clipboard_import;
use serde::{Deserialize, Serialize};
pub mod automation;
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Integration assertions"
)]
mod automation_tests;
pub mod comparison;
pub mod desktop_control;
pub mod document;
pub mod editor;
pub mod exchange;
pub mod helpers;
pub mod legacy_commands;
mod legacy_semantics;
pub mod native;
pub mod output;
pub mod platform;
pub mod playback;
#[cfg(test)]
mod r9_tests;
mod recovery_io;
pub mod show;
pub mod show_media;
pub mod show_render;
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Integration assertions"
)]
mod show_tests;
pub mod storage;
pub use document::{Cancellation, Document};

pub const GPUI_KIT_VERSION: &str = "0.8.1";
pub const GPUI_VERSION: &str = "0.3.10";
pub const RUST_TOOLCHAIN: &str = "1.98.1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewDeck {
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub slides: Vec<PreviewSlide>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewSlide {
    pub title: String,
    pub subtitle: String,
    pub points: Vec<String>,
    pub accent: u32,
}

impl PreviewDeck {
    pub fn bundled() -> Result<Self, String> {
        Self::from_json(include_str!("../../../tests/fixtures/joydeck/welcome.json"))
    }

    pub fn from_json(input: &str) -> Result<Self, String> {
        if input.len() > 1024 * 1024 {
            return Err("preview exceeds 1 MiB".into());
        }
        let deck: Self =
            serde_json::from_str(input).map_err(|e| format!("invalid preview: {e}"))?;
        if !deck.width.is_finite()
            || !deck.height.is_finite()
            || !(1.0..=4096.0).contains(&deck.width)
            || !(1.0..=4096.0).contains(&deck.height)
            || deck.slides.is_empty()
            || deck.slides.len() > 500
        {
            return Err("invalid preview dimensions or slide count".into());
        }
        if deck.slides.iter().any(|s| {
            s.title.len() > 4096
                || s.subtitle.len() > 8192
                || s.points.len() > 16
                || s.points.iter().any(|p| p.len() > 4096)
                || s.accent > 0xffffff
        }) {
            return Err("invalid preview slide".into());
        }
        Ok(deck)
    }
}

#[derive(Debug)]
pub struct WorkspaceState {
    page: usize,
    pub dark: bool,
    pub inspector: bool,
    pub notes: bool,
    pub zoom: f32,
}

impl WorkspaceState {
    pub fn new() -> Self {
        Self {
            page: 0,
            dark: false,
            inspector: true,
            notes: false,
            zoom: 1.0,
        }
    }
    pub fn page(&self) -> usize {
        self.page
    }
    pub fn select(&mut self, page: usize, count: usize) -> bool {
        if page >= count {
            return false;
        }
        self.page = page;
        true
    }
    pub fn step(&mut self, delta: isize, count: usize) {
        if count == 0 {
            self.page = 0;
            return;
        }
        self.page = self.page.saturating_add_signed(delta).min(count - 1);
    }
    pub fn zoom_by(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.zoom = (self.zoom * factor).clamp(0.25, 4.0);
        }
    }
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn diagnostics() -> serde_json::Value {
    serde_json::json!({
        "product": "JoyDeck", "version": env!("CARGO_PKG_VERSION"), "stage": "R9-joyoffice-integration",
        "rust_toolchain": RUST_TOOLCHAIN, "gpui_kit": GPUI_KIT_VERSION,
        "gpui_pre": GPUI_VERSION, "gpui_fast": false,
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "cli_uses_gui": false, "preview": "new blank document by default; original bundled sample available via CLI",
        "capabilities": ["native-workspace", "page-navigation", "zoom", "theme", "pptx-read", "deckcraft-read", "cpu-render", "thumbnails", "inspect", "render-png", "basic-edit", "undo-redo", "pptx-preserving-save", "joydeck-v2", "recovery", "inline-text", "native-ime", "table-edit", "text-clipboard", "table-tsv", "notes-edit", "gpui-scene", "incremental-object-cache", "scene-bench", "desktop-render-benchmark", "master-theme-edit", "picture-insert-crop-adjust", "category-chart-edit", "chart-workbook", "context-ribbon", "content-inspector", "legacy-migration-subset", "pptx-exchange", "pdf-searchable", "notes-handouts", "png-jpeg-gif", "windows-print-dialog", "slideshow", "presenter", "windows-media", "static-mp4", "title-vtt", "headless-control", "mcp-2025-11-25", "export-jobs", "native-grouped-ribbon", "native-save-as", "basic-object-clipboard", "system-image-clipboard", "inline-notes", "slide-sorter-view", "selection-handles"],
        "pending": ["rotated-in-place-text", "advanced-object-clipboard", "reference-full-ui-parity", "macos-linux-runtime-qualification", "advanced-editing", "legacy-media-review-migration", "multi-display-qualification", "full-animation-editing", "media-device-qualification", "pdf-a-tagged"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_preview_is_rejected_instead_of_entering_layout() {
        for json in [
            r#"{"title":"x","width":0,"height":540,"slides":[]}"#,
            r#"{"title":"x","width":1e100,"height":540,"slides":[]}"#,
            r#"{"title":"x","width":960,"height":540,"slides":[],"future":true}"#,
        ] {
            assert!(PreviewDeck::from_json(json).is_err());
        }
        assert!(PreviewDeck::bundled().is_ok());
    }

    #[test]
    fn navigation_and_zoom_remain_bounded_for_untrusted_events() {
        let mut state = WorkspaceState::new();
        assert!(!state.select(usize::MAX, 3));
        state.step(isize::MAX, 3);
        assert_eq!(state.page(), 2);
        state.step(isize::MIN, 3);
        assert_eq!(state.page(), 0);
        state.zoom_by(f32::NAN);
        state.zoom_by(f32::INFINITY);
        assert_eq!(state.zoom, 1.0);
        state.zoom_by(100.0);
        assert_eq!(state.zoom, 4.0);
    }
}
