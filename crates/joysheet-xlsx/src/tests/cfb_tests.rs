use super::*;

/// A minimal valid compound file (512-byte sectors: FAT at sector 0, directory at sector 1)
/// whose directory holds a root entry plus the given stream names.
pub(crate) fn build(streams: &[&str]) -> Vec<u8> {
    let mut b = vec![0u8; 512 * 3];
    b[..8].copy_from_slice(&SIGNATURE);
    b[24..26].copy_from_slice(&0x3Eu16.to_le_bytes()); // minor version
    b[26..28].copy_from_slice(&3u16.to_le_bytes()); // major version
    b[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes()); // byte order
    b[30..32].copy_from_slice(&9u16.to_le_bytes()); // sector shift
    b[32..34].copy_from_slice(&6u16.to_le_bytes()); // mini sector shift
    b[44..48].copy_from_slice(&1u32.to_le_bytes()); // FAT sectors
    b[48..52].copy_from_slice(&1u32.to_le_bytes()); // first directory sector
    b[60..64].copy_from_slice(&END_OF_CHAIN.to_le_bytes()); // no mini FAT
    b[68..72].copy_from_slice(&END_OF_CHAIN.to_le_bytes()); // no DIFAT sectors
    for i in 0..109 {
        b[76 + i * 4..80 + i * 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    }
    b[76..80].copy_from_slice(&0u32.to_le_bytes()); // FAT lives in sector 0
    // FAT (sector 0, at byte 512): sector 0 is a FAT sector, sector 1 ends its chain.
    for i in 0..128 {
        b[512 + i * 4..516 + i * 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    }
    b[512..516].copy_from_slice(&0xFFFF_FFFDu32.to_le_bytes());
    b[516..520].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
    // Directory (sector 1, at byte 1024).
    for (k, name) in std::iter::once("Root Entry").chain(streams.iter().copied()).take(4).enumerate() {
        let e = 1024 + k * 128;
        let units: Vec<u16> = name.encode_utf16().take(31).collect();
        for (i, u) in units.iter().enumerate() {
            b[e + i * 2..e + i * 2 + 2].copy_from_slice(&u.to_le_bytes());
        }
        b[e + 64..e + 66].copy_from_slice(&((units.len() as u16 + 1) * 2).to_le_bytes());
        b[e + 66] = if k == 0 { 5 } else { 2 };
    }
    b
}

#[test]
fn reads_directory_names() {
    let b = build(&["EncryptionInfo", "EncryptedPackage"]);
    assert_eq!(directory_names(&b), Some(vec!["Root Entry".into(), "EncryptionInfo".into(), "EncryptedPackage".into()]));
    assert!(is_encrypted(&b));
    assert!(!is_encrypted(&build(&["Workbook", "\u{5}SummaryInformation"])));
}

#[test]
fn hostile_headers_never_panic() {
    let mut b = build(&["Workbook"]);
    for at in [30usize, 48, 68, 76, 512, 516, 1024 + 64] {
        for v in [0u32, 1, 7, 0xFFFF_FFFA, 0xFFFF_FFFF, 0x7FFF_FFFF] {
            let mut c = b.clone();
            c[at..at + 4].copy_from_slice(&v.to_le_bytes());
            let _ = is_encrypted(&c);
            for cut in [0usize, 8, 100, 600, 1100] {
                let _ = is_encrypted(&c[..cut.min(c.len())]);
            }
        }
    }
    // A directory chain that loops back on itself stops at the bound.
    b[516..520].copy_from_slice(&1u32.to_le_bytes());
    assert!(!is_encrypted(&b));
}

/// `compound_file` with one regular-sector stream of `len` bytes (FAT in sector 0, directory
/// in sector 1, the stream from sector 2), cut right after the stream's last byte, the way
/// some writers leave the final sector unpadded.
fn unpadded(len: usize) -> (Vec<u8>, Vec<u8>) {
    let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
    let mut file = crate::testing::compound_file(&[("Workbook", &data)]);
    file.truncate(512 * 3 + len);
    (file, data)
}

#[test]
fn an_unpadded_last_sector_still_reads() {
    let (file, data) = unpadded(5000);
    assert_ne!(file.len() % 512, 0);
    let cf = Compound::open(&file).unwrap();
    assert_eq!(cf.root_stream("Workbook").unwrap().unwrap(), data);
    // Bytes the stream needs that the file doesn't hold are still damage.
    let cf = Compound::open(&file[..file.len() - 1]).unwrap();
    assert!(cf.root_stream("Workbook").is_err());
}

#[test]
fn an_unpadded_last_mini_sector_still_reads() {
    let data: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
    let mut file = crate::testing::compound_file(&[("Workbook", &data)]);
    // Header, FAT, directory, mini FAT, then the mini stream holding the stream's bytes.
    file.truncate(512 * 4 + data.len());
    let cf = Compound::open(&file).unwrap();
    assert_eq!(cf.root_stream("Workbook").unwrap().unwrap(), data);
    let cf = Compound::open(&file[..file.len() - 1]).unwrap();
    assert!(cf.root_stream("Workbook").is_err());
}

#[test]
fn a_cut_sector_inside_a_chain_is_damage() {
    let (mut file, _) = unpadded(5000);
    // The cut sector (11) points on to the directory sector and the stream claims those
    // bytes too: the partial sector would splice 392 bytes into the middle of the stream.
    file[512 + 11 * 4..512 + 12 * 4].copy_from_slice(&1u32.to_le_bytes());
    file[1024 + 128 + 120..1024 + 128 + 124].copy_from_slice(&5512u32.to_le_bytes());
    let cf = Compound::open(&file).unwrap();
    assert!(cf.root_stream("Workbook").is_err());
}

#[test]
fn a_cut_mini_sector_inside_a_chain_is_damage() {
    let small = [7u8; 100];
    let data: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
    let mut file = crate::testing::compound_file(&[("Small", &small), ("Workbook", &data)]);
    // "Small" holds mini sectors 0-1, "Workbook" 2-17; the mini stream starts at sector 3.
    file.truncate(512 * 4 + 128 + data.len());
    assert_eq!(Compound::open(&file).unwrap().root_stream("Workbook").unwrap().unwrap(), data);
    // The cut mini sector (17) points on to sector 0 and the stream claims those bytes too.
    file[1536 + 17 * 4..1536 + 18 * 4].copy_from_slice(&0u32.to_le_bytes());
    file[1024 + 2 * 128 + 120..1024 + 2 * 128 + 124].copy_from_slice(&1064u32.to_le_bytes());
    let cf = Compound::open(&file).unwrap();
    assert!(cf.root_stream("Workbook").is_err());
}
