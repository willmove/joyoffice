//! EMF+ hatch tiles from the MS-EMFPLUS HatchStyle enumeration.

use crate::canvas::hatch_tile;

// Row-major coverage, bit zero at the top left. The additional 47 styles were measured
// from the Windows GDI+ HatchBrush API. The legacy six keep the shared GDI phase.
const ADDITIONAL: [u64; 47] = [
    0x0000001000000001, // 6
    0x0010000100100001, // 7
    0x0044001100440011, // 8
    0x4411441144114411, // 9
    0x8855225588552255, // 10
    0xa855aa558a55aa55, // 11
    0xaa55aa55aa55aa55, // 12
    0xaaddaa77aaddaa77, // 13
    0xbbeebbeebbeebbee, // 14
    0xffbbffeeffbbffee, // 15
    0xff7ffff7ff7ffff7, // 16
    0xfeffffffefffffff, // 17
    0x8844221188442211, // 18
    0x1122448811224488, // 19
    0x99cc663399cc6633, // 20
    0x993366cc993366cc, // 21
    0xc1e070381c0e0783, // 22
    0x83070e1c3870e0c1, // 23
    0x1111111111111111, // 24
    0x000000ff000000ff, // 25
    0xaaaaaaaaaaaaaaaa, // 26
    0x00ff00ff00ff00ff, // 27
    0x3333333333333333, // 28
    0x0000ffff0000ffff, // 29
    0x0000884422110000, // 30
    0x0000112244880000, // 31
    0x000000f00000000f, // 32
    0x1010101001010101, // 33
    0x2004800840021001, // 34
    0xb130031bd8c00c8d, // 35
    0x1824428118244281, // 36
    0x03a4180003a41800, // 37
    0x8142241810204080, // 38
    0x101010ff010101ff, // 39
    0x8a442811a2442a11, // 40
    0x0f0f0f0faa55aa55, // 41
    0x0180010008100800, // 42
    0x0001000100010055, // 43
    0x0044001000440001, // 44
    0x808040300c1221c0, // 45
    0x99ff66ff99ff66ff, // 46
    0x1f1f19eef1f191ee, // 47
    0x111111ff111111ff, // 48
    0x9966669999666699, // 49
    0xf0f0f0f00f0f0f0f, // 50
    0x8041221408142241, // 51
    0x00081c3e7f3e1c08, // 52
];

pub fn tile(style: u32, foreground: [u8; 4], background: [u8; 4]) -> Option<Vec<u8>> {
    if style < 6 {
        return hatch_tile(style, foreground, Some(background));
    }
    let mask = *ADDITIONAL.get((style - 6) as usize)?;
    let mut pixels = Vec::with_capacity(8 * 8 * 4);
    for bit in 0..64 {
        pixels.extend_from_slice(if mask & (1_u64 << bit) == 0 {
            &background
        } else {
            &foreground
        });
    }
    Some(pixels)
}
