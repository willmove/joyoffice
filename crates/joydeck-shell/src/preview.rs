//! Service-owned diagnostics publication; the scene layer only reads snapshots.
use joydeck_model::document::Deck;
pub use joydeck_scene::preview::*;
pub fn diagnose_fonts(deck: &mut Deck) {
    deck.warnings.extend(font_diagnostics(deck));
}
