use super::data_cols_exceed_sheet;

#[test]
fn column_budget_counts_value_fields_and_grand_total() {
    assert!(!data_cols_exceed_sheet(64, true, 128));
    assert!(data_cols_exceed_sheet(64, true, 256));
    assert!(!data_cols_exceed_sheet(64, false, 256));
    assert!(!data_cols_exceed_sheet(16_384, false, 0));
    assert!(data_cols_exceed_sheet(16_385, false, 0));
}
