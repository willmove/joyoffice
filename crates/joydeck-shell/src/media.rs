use joydeck_model::document::{MAX_PART, Result};
pub use joydeck_model::media::*;
use std::{io::Read, path::Path};
pub fn read(path: &Path) -> Result<Resource> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_PART as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Resource::import(&bytes)
}
