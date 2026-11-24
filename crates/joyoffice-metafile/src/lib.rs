//! Bounded views of WMF and EMF record payloads. The byte order, POINTS,
//! POINTL and FLOAT encodings follow [MS-WMF] and [MS-EMF].

/// A borrowed WMF or EMF record; callers enforce the format-specific record budgets.
#[derive(Clone, Copy)]
pub struct Bytes<'a>(pub &'a [u8]);

impl<'a> Bytes<'a> {
    pub fn slice(&self, off: usize, len: usize) -> Option<&'a [u8]> {
        let (_, rest) = self.0.split_at_checked(off)?;
        let (field, _) = rest.split_at_checked(len)?;
        Some(field)
    }

    pub fn tail(&self, off: usize) -> Option<Bytes<'a>> {
        let (_, rest) = self.0.split_at_checked(off)?;
        Some(Bytes(rest))
    }

    fn field<const WIDTH: usize>(&self, off: usize) -> Option<[u8; WIDTH]> {
        self.tail(off)?.0.first_chunk().copied()
    }

    pub fn u16(&self, off: usize) -> Option<u16> {
        self.field(off).map(u16::from_le_bytes)
    }

    pub fn i16(&self, off: usize) -> Option<i16> {
        self.field(off).map(i16::from_le_bytes)
    }

    pub fn u32(&self, off: usize) -> Option<u32> {
        self.field(off).map(u32::from_le_bytes)
    }

    pub fn i32(&self, off: usize) -> Option<i32> {
        self.field(off).map(i32::from_le_bytes)
    }

    pub fn f32(&self, off: usize) -> Option<f32> {
        self.u32(off).map(f32::from_bits)
    }

    /// `n` consecutive (x, y) pairs of 16- or 32-bit signed integers starting at `off`.
    pub fn points(&self, off: usize, n: usize, wide: bool) -> Option<Vec<(f64, f64)>> {
        let width = if wide { 8 } else { 4 };
        let raw = self.slice(off, n.checked_mul(width)?)?;
        let points = if wide {
            let (points, _) = raw.as_chunks::<8>();
            points
                .iter()
                .map(|&[a, b, c, d, e, f, g, h]| {
                    (
                        f64::from(i32::from_le_bytes([a, b, c, d])),
                        f64::from(i32::from_le_bytes([e, f, g, h])),
                    )
                })
                .collect()
        } else {
            let (points, _) = raw.as_chunks::<4>();
            points
                .iter()
                .map(|&[a, b, c, d]| {
                    (
                        f64::from(i16::from_le_bytes([a, b])),
                        f64::from(i16::from_le_bytes([c, d])),
                    )
                })
                .collect()
        };
        Some(points)
    }
}

#[cfg(test)]
mod tests {
    use super::Bytes;

    #[test]
    fn slices_and_points_check_the_whole_span_before_allocating() {
        let data = Bytes(&[0xfd, 0xff, 5, 0, 7, 0, 0xf7, 0xff]);
        assert_eq!(
            data.points(0, 2, false),
            Some(vec![(-3.0, 5.0), (7.0, -9.0)])
        );
        assert_eq!(data.points(1, 2, false), None);
        assert_eq!(data.points(0, usize::MAX, true), None);
        assert_eq!(data.slice(usize::MAX, 1), None);
        assert_eq!(data.points(8, 0, true), Some(vec![]));
        assert!(data.tail(9).is_none());
    }

    #[test]
    fn unaligned_32_bit_coordinates_and_float_bits_are_little_endian() {
        let bytes = [0, 0xff, 0xff, 0xfe, 0xff, 0x04, 0x03, 0x02, 0x01];
        let data = Bytes(&bytes);
        assert_eq!(
            data.points(1, 1, true),
            Some(vec![(-65_537.0, 16_909_060.0)])
        );
        assert_eq!(data.f32(5).map(f32::to_bits), Some(0x01020304));
        assert_eq!(data.u32(6), None);
    }

    #[test]
    fn signed_extremes_float_payloads_and_boundary_offsets_are_preserved() {
        let payload = [0x00, 0x80, 0xff, 0x7f, 0x34, 0x12, 0xc0, 0xff];
        let record = Bytes(&payload);
        assert_eq!(
            record.points(0, 1, false),
            Some(vec![(-32_768.0, 32_767.0)])
        );
        assert_eq!(record.i16(2), Some(i16::MAX));
        assert_eq!(record.u16(0), Some(0x8000));
        assert_eq!(record.f32(4).map(f32::to_bits), Some(0xffc0_1234));
        assert_eq!(record.slice(8, 0), Some(&[][..]));
        assert!(record.points(9, 0, false).is_none());
        assert!(record.slice(1, usize::MAX).is_none());
        assert!(record.i32(usize::MAX).is_none());
    }
}
