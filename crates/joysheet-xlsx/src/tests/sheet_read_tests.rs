use super::*;

#[test]
fn iso_dates() {
    assert_eq!(parse_iso_datetime("1900-01-01", DateSystem::D1900), Some(1.0));
    assert_eq!(parse_iso_datetime("2024-01-01T12:00:00Z", DateSystem::D1900), Some(45292.5));
    assert_eq!(parse_iso_datetime("1904-01-02", DateSystem::D1904), Some(1.0));
    assert_eq!(parse_iso_datetime("06:00:00", DateSystem::D1900), Some(0.25));
    assert_eq!(parse_iso_datetime("garbage", DateSystem::D1900), None);
    assert_eq!(parse_iso_datetime("2024-13-01", DateSystem::D1900), None);
}

#[test]
fn sqref_keeps_more_than_ten_thousand_ranges() {
    let sq = (1..=12_000).map(|r| format!("A{r}")).collect::<Vec<_>>().join(" ");
    let ranges = parse_sqref(Some(&sq));
    assert_eq!(ranges.len(), 12_000);
    assert_eq!(ranges.last().copied(), RangeRef::parse("A12000"));
}
