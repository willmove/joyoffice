use joyoffice_ui::{Brand, product_window_options};
use std::sync::Arc;

#[test]
fn native_icons_are_decodable_distinct_and_cached_for_all_products() {
    let products = [
        (Brand::Word, "joyword"),
        (Brand::Deck, "joydeck"),
        (Brand::Sheet, "joysheet"),
    ];
    let mut pixels = Vec::new();
    for (brand, id) in products {
        let options = product_window_options(brand);
        assert_eq!(options.app_id.as_deref(), Some(id));
        let icon = options
            .icon
            .expect("checked-in native product icon must decode");
        assert_eq!(icon.dimensions(), (256, 256));
        assert!(icon.pixels().any(|pixel| pixel[3] == 0));
        assert!(icon.pixels().any(|pixel| pixel[3] == 255));
        let again = product_window_options(brand).icon.expect("cached icon");
        assert!(Arc::ptr_eq(&icon, &again));
        pixels.push(icon.as_raw().clone());
    }
    assert_ne!(pixels[0], pixels[1]);
    assert_ne!(pixels[0], pixels[2]);
    assert_ne!(pixels[1], pixels[2]);
}
