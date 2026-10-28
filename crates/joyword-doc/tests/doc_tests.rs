use joyoffice_cfb::CfbWriter;
use joyword_doc::{DocBudget, DocError, DocumentFormat, import_doc};
use joyword_model::Block;

fn build_test_doc_cfb(
    text: &str,
    is_wps: bool,
    is_encrypted: bool,
    extra_stream: Option<(&str, &[u8])>,
) -> Vec<u8> {
    let mut word_doc = vec![0u8; 1024];

    // 1. FIB Header
    // wIdent = 0xA5EC
    word_doc[0..2].copy_from_slice(&0xA5ECu16.to_le_bytes());
    // nFib = 0x00D9 (Word 2000)
    word_doc[2..4].copy_from_slice(&0x00D9u16.to_le_bytes());

    // flags: fWhichTblStm = 1 (1Table), fEncrypted
    let mut flags = 0x0200u16;
    if is_encrypted {
        flags |= 0x0100;
    }
    word_doc[10..12].copy_from_slice(&flags.to_le_bytes());

    // csw = 14
    let csw = 14u16;
    word_doc[32..34].copy_from_slice(&csw.to_le_bytes());

    // cslw offset = 34 + 14 * 2 = 62
    let cslw_offset = 62;
    let cslw = 22u16;
    word_doc[cslw_offset..cslw_offset + 2].copy_from_slice(&cslw.to_le_bytes());

    // ccpText is at cslw_offset + 2 + 3 * 4 = 76
    let ccp_text = text.len() as u32;
    word_doc[76..80].copy_from_slice(&ccp_text.to_le_bytes());

    // cbRgFcLcb offset = cslw_offset + 2 + 22 * 4 = 152
    let fc_lcb_offset = 152;
    let cb_rg_fc_lcb = 93u16;
    word_doc[fc_lcb_offset..fc_lcb_offset + 2].copy_from_slice(&cb_rg_fc_lcb.to_le_bytes());

    // Index 66 in FibRgFcLcb is fcClx, lcbClx:
    // offset = 154 + 66 * 8 = 682
    let clx_index_offset = 154 + 66 * 8;
    let fc_clx = 0u32;
    // Clx length: 1 byte clxt (0x02) + 4 bytes lcb (16) + 16 bytes PlcPcd = 21 bytes
    let lcb_clx = 21u32;
    word_doc[clx_index_offset..clx_index_offset + 4].copy_from_slice(&fc_clx.to_le_bytes());
    word_doc[clx_index_offset + 4..clx_index_offset + 8].copy_from_slice(&lcb_clx.to_le_bytes());

    // Put text in WordDocument stream at offset 512
    let text_bytes_offset = 512usize;
    if word_doc.len() < text_bytes_offset + text.len() {
        word_doc.resize(text_bytes_offset + text.len(), 0);
    }
    word_doc[text_bytes_offset..text_bytes_offset + text.len()].copy_from_slice(text.as_bytes());

    // 2. 1Table stream containing Clx
    let mut table_stream = Vec::new();
    // clxt = 0x02 (Pcdt)
    table_stream.push(0x02);
    // lcb = 16 (PlcPcd length for 1 piece: 2 CPs (8 bytes) + 1 PCD (8 bytes))
    let lcb_pcd = 16u32;
    table_stream.extend_from_slice(&lcb_pcd.to_le_bytes());
    // PlcPcd:
    // CP[0] = 0
    table_stream.extend_from_slice(&0u32.to_le_bytes());
    // CP[1] = text.len()
    table_stream.extend_from_slice(&ccp_text.to_le_bytes());
    // PCD:
    // flags = 0
    table_stream.extend_from_slice(&0u16.to_le_bytes());
    // fc: compressed flag (0x4000_0000) | (text_bytes_offset * 2)
    let fc_compressed = 0x4000_0000u32 | ((text_bytes_offset as u32) * 2);
    table_stream.extend_from_slice(&fc_compressed.to_le_bytes());
    // prm = 0
    table_stream.extend_from_slice(&0u16.to_le_bytes());

    // 3. CFB package
    let mut cfb = CfbWriter::new();
    cfb.add_stream("WordDocument", word_doc);
    cfb.add_stream("1Table", table_stream);

    if is_wps {
        cfb.add_stream("KSOTree", b"WPS Private Tree Structure".to_vec());
    }

    if let Some((name, data)) = extra_stream {
        cfb.add_stream(name, data.to_vec());
    }

    cfb.build().expect("build cfb")
}

#[test]
fn test_doc_basic_import() {
    let raw = "Hello MS-DOC World!\rThis is paragraph 2.\r";
    let cfb_bytes = build_test_doc_cfb(raw, false, false, None);

    let (doc, report, meta) = import_doc(&cfb_bytes, DocBudget::default()).expect("import doc");
    assert_eq!(meta.format, DocumentFormat::Doc);
    assert!(!report.has_loss());

    let body = doc.body();
    let blocks: Vec<_> = body.blocks.iter().collect();
    assert_eq!(blocks.len(), 2);

    if let Block::Paragraph(p) = blocks[0] {
        assert_eq!(p.raw_text(), "Hello MS-DOC World!");
    } else {
        panic!("expected paragraph");
    }

    if let Block::Paragraph(p) = blocks[1] {
        assert_eq!(p.raw_text(), "This is paragraph 2.");
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn test_wps_detection_and_preservation() {
    let raw = "WPS Native File Content\r";
    let cfb_bytes = build_test_doc_cfb(raw, true, false, None);

    let (doc, report, meta) = import_doc(&cfb_bytes, DocBudget::default()).expect("import wps");
    assert_eq!(meta.format, DocumentFormat::Wps);
    assert!(!report.has_loss());

    assert!(meta.preserved_streams.contains_key("KSOTree"));
    let preserved = meta.preserved_streams.get("KSOTree").unwrap();
    assert_eq!(preserved.data, b"WPS Private Tree Structure");

    let body = doc.body();
    let blocks: Vec<_> = body.blocks.iter().collect();
    if let Block::Paragraph(p) = blocks[0] {
        assert_eq!(p.raw_text(), "WPS Native File Content");
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn test_table_parsing_and_loss_report() {
    let raw = "Cell 1\x07Cell 2\x07\x08Special\r";
    let cfb_bytes = build_test_doc_cfb(raw, false, false, None);

    let (doc, report, _) = import_doc(&cfb_bytes, DocBudget::default()).expect("import table");
    assert!(report.has_loss()); // OLE anchor 0x08 recorded as loss

    let body = doc.body();
    let has_table = body.blocks.iter().any(|b| matches!(b, Block::Table(_)));
    assert!(has_table, "expected table in body blocks");
}

#[test]
fn test_encrypted_rejection() {
    let raw = "Secret Document\r";
    let cfb_bytes = build_test_doc_cfb(raw, false, true, None);

    let err = import_doc(&cfb_bytes, DocBudget::default()).unwrap_err();
    assert_eq!(err, DocError::EncryptedDocument);
}

#[test]
fn test_malformed_and_budget() {
    // 1. Not a CFB
    let err = import_doc(b"not a cfb container", DocBudget::default()).unwrap_err();
    assert!(matches!(err, DocError::CfbError(_)));

    // 2. Budget exceeded
    let raw = "Text\r";
    let cfb_bytes = build_test_doc_cfb(raw, false, false, None);
    let budget = DocBudget {
        max_file_bytes: 10,
        ..DocBudget::default()
    };
    let err = import_doc(&cfb_bytes, budget).unwrap_err();
    assert_eq!(err, DocError::BudgetExceeded("file bytes exceed budget"));
}
