use super::*;

fn links() -> Links {
    Links {
        sheets: vec!["Sheet1".into(), "My Data".into()],
        names: vec!["Rate".into()],
        skipped: vec![false],
        books: vec![
            SupBook { internal: true, addin: false, names: vec![] },
            SupBook { internal: false, addin: true, names: vec!["_xlfn.IFERROR".into()] },
            SupBook::default(),
        ],
        xti: vec![
            Xti { book: 0, first: 0, last: 1 },
            Xti { book: 1, first: -2, last: -2 },
            Xti { book: 2, first: 0, last: 0 },
            Xti { book: 0, first: -1, last: -1 },
        ],
    }
}

fn text(rgce: &[u8], rgcb: &[u8]) -> Result<String, Unsupported> {
    decode(rgce, rgcb, CellRef::new(0, 0), Mode::Cell, &links()).map(|e| joysheet_formula::print(&e))
}

#[test]
fn decodes_common_tokens() {
    // {1,"a";TRUE,#N/A} via PtgArray + PtgExtraArray.
    let extra =
        [&[1u8, 1, 0][..], &[1], &1f64.to_le_bytes(), &[2, 1, 0, 0, b'a'], &[4, 1, 0, 0, 0, 0, 0, 0, 0], &[0x10, 0x2A, 0, 0, 0, 0, 0, 0, 0]].concat();
    assert_eq!(text(&[0x60, 0, 0, 0, 0, 0, 0, 0], &extra).unwrap(), "{1,\"a\";TRUE,#N/A}");
    // SUM via PtgAttrSum over a 3-D span, whole column.
    let area3d = [&[0x3Bu8][..], &0u16.to_le_bytes(), &[0, 0, 0xFF, 0xFF, 1, 0, 1, 0]].concat();
    assert_eq!(text(&[area3d.as_slice(), &[0x19, 0x10, 0, 0]].concat(), &[]).unwrap(), "SUM('Sheet1:My Data'!$B:$B)");
    // IFERROR (a newer function) called by name through PtgNameX, with a missing argument.
    let namex = [&[0x39u8][..], &1u16.to_le_bytes(), &1u32.to_le_bytes()].concat();
    let call = [namex.as_slice(), &[0x1E, 5, 0, 0x16, 0x22, 3, 0xFF, 0]].concat();
    assert_eq!(text(&call, &[]).unwrap(), "IFERROR(5,)");
    // A deleted sheet becomes #REF!; the CHOOSE jump table is skipped.
    let ref_deleted = [&[0x3Au8][..], &3u16.to_le_bytes(), &[0, 0, 0, 0]].concat();
    let choose = [&[0x1Eu8, 1, 0][..], &[0x19, 0x04, 1, 0, 0, 0, 0, 0], ref_deleted.as_slice(), &[0x1E, 2, 0, 0x22, 3, 100, 0]].concat();
    assert_eq!(text(&choose, &[]).unwrap(), "CHOOSE(1,#REF!,2)");
    // External links, data tables and unbalanced stacks are refused, never guessed.
    let external = [&[0x3Au8][..], &2u16.to_le_bytes(), &[0, 0, 0, 0]].concat();
    assert!(text(&external, &[]).is_err());
    assert!(text(&[0x02, 0, 0, 0, 0], &[]).is_err());
    assert!(text(&[0x1E, 1, 0, 0x1E, 2, 0], &[]).is_err());
    assert!(text(&[0x03], &[]).is_err());
    assert!(text(&[0x21, 0xFF, 0x7F], &[]).is_err());
    assert!(text(&[0x60, 0, 0, 0, 0, 0, 0, 0], &[0xFF, 0xFF, 0xFF]).is_err());
}

#[test]
fn deep_nesting_is_refused_not_a_stack_overflow() {
    // 1 followed by thousands of PtgParen / PtgUminus tokens: a hostile file used to build a
    // 16K-deep tree that overflowed the stack when printed or dropped.
    for op in [0x15u8, 0x13] {
        let mut rgce = vec![0x1E, 1, 0];
        rgce.resize(MAX_RGCE, op);
        assert_eq!(text(&rgce, &[]), Err(Unsupported("formula nested too deeply")));
    }
    // Binary chains nest too: 1+1+1+... left-associated.
    let mut chain = vec![0x1E, 1, 0];
    while chain.len() + 4 <= MAX_RGCE {
        chain.extend([0x1E, 1, 0, 0x03]);
    }
    assert!(text(&chain, &[]).is_err());
    // Nesting up to the limit still decodes.
    let mut ok = vec![0x1E, 1, 0];
    ok.resize(3 + MAX_DEPTH - 1, 0x15);
    assert!(text(&ok, &[]).is_ok());
}
