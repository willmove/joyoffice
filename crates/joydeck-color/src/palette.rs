use crate::{ColorScheme, Rgba, SchemeSlot};

/// DrawingML maps twelve document roles to the twelve swatches in a theme.
/// The role and swatch lists use different names for the four text and background slots.
const ROLES: [SchemeSlot; 12] = [
    SchemeSlot::Tx1,
    SchemeSlot::Bg1,
    SchemeSlot::Tx2,
    SchemeSlot::Bg2,
    SchemeSlot::Accent1,
    SchemeSlot::Accent2,
    SchemeSlot::Accent3,
    SchemeSlot::Accent4,
    SchemeSlot::Accent5,
    SchemeSlot::Accent6,
    SchemeSlot::Hlink,
    SchemeSlot::FolHlink,
];

impl ColorScheme {
    pub fn get(&self, slot: SchemeSlot) -> Rgba {
        self.colors[slot.palette_index()]
    }

    pub fn set(&mut self, slot: SchemeSlot, color: Rgba) {
        if slot.is_swatch() {
            self.colors[slot.palette_index()] = color;
        }
    }
}

impl SchemeSlot {
    fn palette_index(self) -> usize {
        use SchemeSlot::*;

        match self {
            Dk1 | Tx1 => 0,
            Lt1 | Bg1 => 1,
            Dk2 | Tx2 => 2,
            Lt2 | Bg2 => 3,
            Accent1 | PhClr => 4,
            Accent2 => 5,
            Accent3 => 6,
            Accent4 => 7,
            Accent5 => 8,
            Accent6 => 9,
            Hlink => 10,
            FolHlink => 11,
        }
    }

    fn is_swatch(self) -> bool {
        !matches!(
            self,
            Self::Tx1 | Self::Bg1 | Self::Tx2 | Self::Bg2 | Self::PhClr
        )
    }
}

const DEFAULT_MAPPING: ColorMapping = ColorMapping(SchemeSlot::THEME);

/// A lookup from document color roles to theme swatches. Mappings are applied once.
#[derive(Clone, Copy, Debug)]
pub struct ColorMapping([SchemeSlot; 12]);

impl ColorMapping {
    pub fn from_entries(entries: &[(String, String)]) -> Self {
        let mut mapping = DEFAULT_MAPPING;
        for (target, role) in mapping.0.iter_mut().zip(ROLES) {
            if let Some(slot) = entries
                .iter()
                .find(|(key, _)| key == role.xml_name())
                .and_then(|(_, value)| SchemeSlot::from_xml(value))
                .filter(|slot| slot.is_swatch())
            {
                *target = slot;
            }
        }
        mapping
    }

    pub fn slot(self, role: SchemeSlot) -> SchemeSlot {
        match role {
            SchemeSlot::Dk1
            | SchemeSlot::Lt1
            | SchemeSlot::Dk2
            | SchemeSlot::Lt2
            | SchemeSlot::PhClr => role,
            _ => self.0[role.palette_index()],
        }
    }

    /// Project document roles onto a palette for swatches and other consumers.
    pub fn project(self, theme: &ColorScheme) -> ColorScheme {
        ColorScheme {
            name: theme.name.clone(),
            colors: self.0.map(|slot| theme.get(slot)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_map_once_from_the_original_swatches() {
        use SchemeSlot::*;

        let entries = [
            ("bg1", "dk1"),
            ("tx1", "lt1"),
            ("accent1", "accent2"),
            ("accent2", "accent1"),
            ("hlink", "folHlink"),
            ("folHlink", "hlink"),
            ("accent3", "phClr"), // Only a physical swatch can be a mapping target.
        ]
        .map(|(key, slot)| (key.into(), slot.into()));
        let mapping = ColorMapping::from_entries(&entries);
        let theme = ColorScheme {
            name: "Test".into(),
            colors: std::array::from_fn(|i| Rgba::rgb(i as u8, 0, 0)),
        };
        let projected = mapping.project(&theme);
        for (role, swatch) in [
            (Bg1, Dk1),
            (Tx1, Lt1),
            (Accent1, Accent2),
            (Accent2, Accent1),
            (Hlink, FolHlink),
            (FolHlink, Hlink),
            (Accent3, Accent3),
        ] {
            assert_eq!(mapping.slot(role), swatch);
            assert_eq!(projected.get(role), theme.get(swatch));
        }
        assert_eq!(mapping.slot(Dk1), Dk1);
        assert_eq!(mapping.slot(PhClr), PhClr);
    }
}
