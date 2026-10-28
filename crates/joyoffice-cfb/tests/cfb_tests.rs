use joyoffice_cfb::{CfbBudget, CfbError, CfbWriter, CompoundFile};

#[test]
fn test_roundtrip_small_and_large_streams() {
    let mut writer = CfbWriter::new();
    let small_data = b"Hello, JoyOffice CFB small stream!";
    let large_data = vec![0x42u8; 10000]; // > 4096 bytes cutoff
    let empty_data = Vec::new();

    writer.add_stream("WordDocument", small_data.to_vec());
    writer.add_stream("1Table", large_data.clone());
    writer.add_stream("EmptyStream", empty_data.clone());

    let cfb_bytes = writer.build().expect("build cfb bytes");
    assert!(cfb_bytes.len() >= 512);

    let doc = CompoundFile::parse(&cfb_bytes, CfbBudget::default()).expect("parse cfb");
    let streams = doc.list_streams();
    assert_eq!(streams.len(), 3);

    let read_small = doc.read_stream("WordDocument").expect("read small");
    assert_eq!(read_small, small_data);

    let read_large = doc.read_stream("1Table").expect("read large");
    assert_eq!(read_large, large_data);

    let read_empty = doc.read_stream("EmptyStream").expect("read empty");
    assert_eq!(read_empty, empty_data);
}

#[test]
fn test_malformed_cfb_rejections() {
    let budget = CfbBudget::default();

    // 1. Truncated
    assert_eq!(
        CompoundFile::parse(&[0u8; 100], budget).unwrap_err(),
        CfbError::TruncatedHeader
    );

    // 2. Bad signature
    let bad_magic = vec![0u8; 512];
    assert_eq!(
        CompoundFile::parse(&bad_magic, budget).unwrap_err(),
        CfbError::InvalidSignature
    );

    // 3. Budget limits
    let tiny_budget = CfbBudget {
        max_file_size: 1000,
        ..CfbBudget::default()
    };
    let mut writer = CfbWriter::new();
    writer.add_stream("Test", vec![0x99; 2000]);
    let cfb_bytes = writer.build().unwrap();
    assert_eq!(
        CompoundFile::parse(&cfb_bytes, tiny_budget).unwrap_err(),
        CfbError::BudgetExceeded("file size exceeds budget")
    );
}

#[test]
fn test_fuzz_smoke_no_panic() {
    let budget = CfbBudget {
        max_file_size: 64 * 1024,
        max_sectors: 128,
        max_entries: 64,
        max_stream_size: 32 * 1024,
        max_tree_depth: 8,
    };

    let mut state: u64 = 0x123456789ABCDEF0;
    let mut rng = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    for _ in 0..100 {
        let len = (rng() % 4096) as usize;
        let mut bytes = vec![0u8; len];
        for b in &mut bytes {
            *b = (rng() & 0xFF) as u8;
        }
        let _ = CompoundFile::parse(&bytes, budget);
    }
}
