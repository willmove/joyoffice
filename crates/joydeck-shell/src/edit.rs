use crate::document::{Deck, Result};
pub use joydeck_engine::edit::*;
use std::path::{Path, PathBuf};
pub fn save(path: &Path, deck: &Deck) -> Result<()> {
    save_impl(path, deck, true)
}
pub fn save_new(path: &Path, deck: &Deck) -> Result<()> {
    save_impl(path, deck, false)
}
fn save_impl(path: &Path, deck: &Deck, replace: bool) -> Result<()> {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("joydeck"))
    {
        return Err("只能保存为 .joydeck".into());
    }
    let bytes = joydeck_formats::native::encode(deck)?;
    joydeck_platform::files::publish(path, &bytes, replace)
}
pub fn open_editable(path: &Path) -> Result<Deck> {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let parts = crate::document::read_package(path, &cancel)?;
    joydeck_formats::native::validate_editable(&parts)?;
    crate::document::open(path, &cancel)
}
pub fn recovery_path() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")))
    };
    base.unwrap_or_else(std::env::temp_dir)
        .join("JoyDeck/recovery")
}
