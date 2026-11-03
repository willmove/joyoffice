use std::path::Path;
pub fn save_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::platform::publish(path, bytes, false)
}
pub fn save(path: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::platform::publish(path, bytes, path.exists())
}
pub fn open_editable(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = crate::platform::read_bounded(path, crate::document::MAX_INPUT as usize)?;
    crate::native::load(&bytes, &crate::Cancellation::default())?;
    Ok(bytes)
}
