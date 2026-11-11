//! Ribbon-only optical weight. Document drawing assets retain their original strokes.
use gpui_kit::{AssetSource, component::Icon};
use joyoffice_ui::IconName;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

pub(crate) fn icon(name: IconName) -> Icon {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<[u8]>>>> = OnceLock::new();
    let path = name.path();
    let Ok(mut cache) = CACHE.get_or_init(Default::default).lock() else {
        return Icon::default().path(path);
    };
    if let Some(bytes) = cache.get(path.as_ref()) {
        return Icon::default().data(bytes);
    }
    match gpui_kit::assets::AllAssets.load(path.as_ref()) {
        Ok(Some(bytes)) => {
            let thin = String::from_utf8_lossy(&bytes).replace("stroke-width=\"2\"", "stroke-width=\"1.25\"");
            let bytes: Arc<[u8]> = Arc::from(thin.into_bytes());
            let icon = Icon::default().data(&bytes);
            cache.insert(path.to_string(), bytes);
            icon
        }
        _ => Icon::default().path(path),
    }
}
