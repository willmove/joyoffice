pub use joydeck_model::document::*;
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};
pub fn read_package(path: &Path, cancel: &AtomicBool) -> Result<BTreeMap<String, Vec<u8>>> {
    check_cancel(cancel)?;
    let bytes = joydeck_platform::files::read_bounded(path, MAX_FILE as usize)?;
    joydeck_formats::document::read_package_bytes(&bytes, cancel)
}
pub use joydeck_formats::document::{part, read_nested_package};
pub fn open(path: &Path, cancel: &AtomicBool) -> Result<Deck> {
    let parts = read_package(path, cancel)?;
    let is_native = parts.contains_key("mimetype");
    let mut deck = joydeck_formats::document::decode_parts(&parts, cancel)?;
    check_cancel(cancel)?;
    deck.validate()?;
    crate::preview::diagnose_fonts(&mut deck);
    if !path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case(if is_native { "joydeck" } else { "pptx" }))
    {
        deck.warn("extension", None, "", "扩展名与内容不一致；按容器身份读取");
    }
    Ok(deck)
}
