//! Composition attached to vector paint operations.

use crate::raster_transfer::ternary;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VectorMix {
    #[default]
    SourceOver,
    SourceCopy,
    Raster(Rop2),
}

/// The four-bit GDI truth table for (pen, destination), with destination as the low bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rop2(u8);

impl Rop2 {
    pub(crate) const PEN: Self = Self(0b1100);
    pub(crate) const DESTINATION: Self = Self(0b1010);

    pub(crate) fn read(mode: u32) -> Option<Self> {
        (1..=16).contains(&mode).then(|| Self((mode - 1) as u8))
    }

    pub fn reads_destination(self) -> bool {
        self.0 & 0b0101 != (self.0 >> 1) & 0b0101
    }

    pub fn paint(self, pen: [u8; 4], destination: [u8; 4]) -> Option<[u8; 4]> {
        let code = self.0 | (self.0 << 4);
        let evaluate = |channel, d| ternary(code, 0, channel, d);
        if pen[..3]
            .iter()
            .all(|&channel| evaluate(channel, 0) == 0 && evaluate(channel, 255) == 255)
        {
            return None;
        }
        let backdrop = if destination[3] == 0 {
            [255; 4]
        } else {
            destination
        };
        Some([
            evaluate(pen[0], backdrop[0]),
            evaluate(pen[1], backdrop[1]),
            evaluate(pen[2], backdrop[2]),
            pen[3],
        ])
    }
}
